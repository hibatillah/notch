//! A Bluetooth device connecting or disconnecting, with its battery level when Windows has one. Windows tells a
//! program when a paired device's connection changes, so nothing polls; the subscriptions live on a thread of their
//! own, which wakes the UI thread through the activity's `Notifier`.
//!
//! The battery level is read after a connection, from the Windows device node of the device that reports it. Not every
//! device reports one, and Windows may take a few seconds to learn it, so it is shown when it is there.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use windows::{
    Devices::{
        Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice},
        Enumeration::DeviceInformation,
    },
    Foundation::{IReference, TypedEventHandler},
    core::{HSTRING, IInspectable, Interface, Ref, Result},
};

use crate::{
    activity::{Activity, Flash, Presence},
    gfx::{Canvas, Color, Rect},
    log,
    page::{Context, Notifier, TimerId},
    ui::{self, Icon},
};

const SHOW_MILLISECONDS: u32 = 3000;
/// Windows reports a device that is already connected when the subscriptions start; changes in this time are that.
const SETTLE_TIME: Duration = Duration::from_secs(4);
/// How long after a connection Windows is given to learn the battery level.
const BATTERY_DELAY: Duration = Duration::from_secs(2);
const MAX_NAME_CHARACTERS: usize = 22;
const CONNECTED_BLUE: Color = Color::rgb(0.302, 0.671, 0.969);
/// The Windows property that holds the battery percentage of a device node.
const BATTERY_PROPERTY: &str = "{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2";
const BATTERY_NODES_FILTER: &str = "System.Devices.DeviceInstanceId:~<\"BTHENUM\" OR System.Devices.DeviceInstanceId:~<\"BTHHFENUM\" OR System.Devices.DeviceInstanceId:~<\"BTHLE\"";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    name: String,
    is_connected: bool,
    battery_percent: Option<u8>,
}

/// A name short enough for the pill: long ones are cut and end in an ellipsis.
fn short_name(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_CHARACTERS {
        return name.to_owned();
    }
    let cut: String = name.chars().take(MAX_NAME_CHARACTERS - 1).collect();
    format!("{}\u{2026}", cut.trim_end())
}

fn message(change: &Change) -> String {
    format!("{} {}", short_name(&change.name), if change.is_connected { "connected" } else { "disconnected" })
}

fn battery_text(change: &Change) -> Option<String> {
    change.battery_percent.filter(|_| change.is_connected).map(|percent| format!("{percent}%"))
}

/// The battery percentage Windows knows for a paired device named `name`, found among the device nodes that begin
/// with the device's name.
fn read_battery(name: &str) -> Option<u8> {
    let properties = windows_collections::IIterable::<HSTRING>::from(vec![HSTRING::from(BATTERY_PROPERTY)]);
    let nodes = DeviceInformation::FindAllAsyncAqsFilterAndAdditionalProperties(
        &HSTRING::from(BATTERY_NODES_FILTER),
        &properties,
    )
    .ok()?
    .join()
    .ok()?;
    nodes
        .into_iter()
        .filter(|node| node.Name().is_ok_and(|node_name| node_name.to_string().starts_with(name)))
        .find_map(|node| {
            let value = node.Properties().ok()?.Lookup(&HSTRING::from(BATTERY_PROPERTY)).ok()?;
            value.cast::<IReference<u8>>().ok()?.Value().ok()
        })
}

/// What the subscriptions share with the activity.
struct Shared {
    changes: Mutex<VecDeque<Change>>,
    notifier: Notifier,
    started_at: Instant,
}

impl Shared {
    fn report(self: &Arc<Self>, name: String, is_connected: bool) {
        // What Windows reports while the subscriptions are starting is the state before they started, not a change.
        if self.started_at.elapsed() < SETTLE_TIME {
            return;
        }
        let shared = self.clone();
        thread::spawn(move || {
            let battery_percent = if is_connected {
                thread::sleep(BATTERY_DELAY);
                read_battery(&name)
            } else {
                None
            };
            if let Ok(mut changes) = shared.changes.lock() {
                changes.push_back(Change { name, is_connected, battery_percent });
            }
            shared.notifier.notify();
        });
    }
}

/// Subscribes to every paired device, classic and low energy, and then keeps them alive for as long as the notch runs.
fn watch(shared: Arc<Shared>) -> Result<()> {
    let mut classic = Vec::new();
    let mut low_energy = Vec::new();
    let selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    for information in &DeviceInformation::FindAllAsyncAqsFilter(&selector)?.join()? {
        let device = BluetoothDevice::FromIdAsync(&information.Id()?)?.join()?;
        let was_connected =
            Arc::new(AtomicBool::new(device.ConnectionStatus()? == BluetoothConnectionStatus::Connected));
        let shared = shared.clone();
        let token = device.ConnectionStatusChanged(&TypedEventHandler::new(
            move |sender: Ref<BluetoothDevice>, _: Ref<IInspectable>| {
                if let Some(device) = sender.as_ref() {
                    let is_connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    if was_connected.swap(is_connected, Ordering::SeqCst) != is_connected {
                        shared.report(device.Name()?.to_string(), is_connected);
                    }
                }
                Ok(())
            },
        ))?;
        classic.push((device, token));
    }
    let selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;
    for information in &DeviceInformation::FindAllAsyncAqsFilter(&selector)?.join()? {
        let device = BluetoothLEDevice::FromIdAsync(&information.Id()?)?.join()?;
        let was_connected =
            Arc::new(AtomicBool::new(device.ConnectionStatus()? == BluetoothConnectionStatus::Connected));
        let shared = shared.clone();
        let token = device.ConnectionStatusChanged(&TypedEventHandler::new(
            move |sender: Ref<BluetoothLEDevice>, _: Ref<IInspectable>| {
                if let Some(device) = sender.as_ref() {
                    let is_connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    if was_connected.swap(is_connected, Ordering::SeqCst) != is_connected {
                        shared.report(device.Name()?.to_string(), is_connected);
                    }
                }
                Ok(())
            },
        ))?;
        low_energy.push((device, token));
    }
    log::debug(&format!("bluetooth: watching {} classic and {} low-energy devices", classic.len(), low_energy.len()));
    // The devices must stay alive for their events to arrive.
    loop {
        thread::park();
        let _ = (&classic, &low_energy);
    }
}

pub struct BluetoothActivity {
    shared: Option<Arc<Shared>>,
    current: Option<Change>,
    width: f32,
    flash: Flash,
}

impl BluetoothActivity {
    pub fn new() -> Self {
        Self { shared: None, current: None, width: 0.0, flash: Flash::default() }
    }
}

impl Activity for BluetoothActivity {
    fn id(&self) -> &'static str {
        "bluetooth"
    }

    fn label(&self) -> &'static str {
        "Bluetooth devices"
    }

    fn presence(&self) -> Option<Presence> {
        self.flash.is_visible().then_some(Presence::Transient { width: self.width })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        let Some(change) = &self.current else { return };
        let tint = if change.is_connected { CONNECTED_BLUE } else { Color::gray(0.7) };
        ui::draw_message(
            canvas,
            area,
            opacity,
            Icon::Bluetooth,
            tint,
            &message(change),
            battery_text(change).as_deref(),
        );
    }

    fn on_start(&mut self, context: &mut Context) {
        let shared = Arc::new(Shared {
            changes: Mutex::new(VecDeque::new()),
            notifier: context.notifier(),
            started_at: Instant::now(),
        });
        self.shared = Some(shared.clone());
        thread::spawn(move || {
            if let Err(error) = watch(shared) {
                log::debug(&format!("bluetooth: cannot watch devices: {error}"));
            }
        });
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        let Some(shared) = &self.shared else { return };
        let latest = shared.changes.lock().ok().and_then(|mut changes| {
            let latest = changes.pop_back();
            changes.clear();
            latest
        });
        if let Some(change) = latest {
            self.width = ui::message_width(context.fonts(), &message(&change), battery_text(&change).as_deref());
            self.current = Some(change);
            self.flash.show(context, SHOW_MILLISECONDS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(name: &str, is_connected: bool, battery: Option<u8>) -> Change {
        Change { name: name.to_owned(), is_connected, battery_percent: battery }
    }

    #[test]
    fn short_names_are_kept_and_long_ones_are_cut_with_an_ellipsis() {
        assert_eq!(short_name("DAXA SL1"), "DAXA SL1");
        let long = short_name("Wireless Headphones Pro Max Edition");
        assert_eq!(long.chars().count(), MAX_NAME_CHARACTERS);
        assert!(long.ends_with('\u{2026}'));
    }

    #[test]
    fn the_message_says_what_happened_to_the_device() {
        assert_eq!(message(&change("DAXA SL1", true, None)), "DAXA SL1 connected");
        assert_eq!(message(&change("DAXA SL1", false, None)), "DAXA SL1 disconnected");
    }

    #[test]
    fn the_battery_shows_only_for_a_connected_device_that_reports_one() {
        assert_eq!(battery_text(&change("A", true, Some(80))), Some("80%".to_owned()));
        assert_eq!(battery_text(&change("A", true, None)), None);
        assert_eq!(battery_text(&change("A", false, Some(80))), None);
    }

    #[test]
    fn nothing_shows_before_anything_happens() {
        assert_eq!(BluetoothActivity::new().presence(), None);
    }
}
