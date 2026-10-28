//! Frame Server events report new camera streams; existing streams and direct access are not detected.
//! Consent-store registry keys are not used because this Windows build no longer updates them.

use std::{
    collections::HashSet,
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use windows::{
    Win32::System::EventLog::{
        EVT_HANDLE, EVT_SUBSCRIBE_NOTIFY_ACTION, EvtClose, EvtRender, EvtRenderEventXml, EvtSubscribe,
        EvtSubscribeActionDeliver, EvtSubscribeToFutureEvents,
    },
    core::{Result, w},
};

use super::privacy::{Device, Privacy};
use crate::{
    activity::{Activity, Presence},
    gfx::{Canvas, Color, Rect},
    log,
    page::{Context, Notifier, TimerId},
    ui::Icon,
};

const GREEN: Color = Color::rgb(0.19, 0.82, 0.35);
const CHANNEL: windows::core::PCWSTR = w!("Microsoft-Windows-MF-FrameServer/Camera_FrameServer");
/// Only the watchdog events about streams: event 11 is logged when a stream operation begins.
const QUERY: windows::core::PCWSTR = w!("*[System[EventID=11] and EventData[Data[@Name='TimerScope']='2']]");
const OPERATION_START: u32 = 2;
const OPERATION_STOP: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StreamEvent {
    stream_id: u64,
    is_start: bool,
}

fn data_value<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("Name='{name}'>");
    let start = xml.find(&marker)? + marker.len();
    let length = xml[start..].find('<')?;
    Some(&xml[start..start + length])
}

fn parse_stream_event(xml: &str) -> Option<StreamEvent> {
    let stream_id = data_value(xml, "TimerScopeId")?.parse().ok()?;
    match data_value(xml, "WatchdogOperation")?.parse().ok()? {
        OPERATION_START => Some(StreamEvent { stream_id, is_start: true }),
        OPERATION_STOP => Some(StreamEvent { stream_id, is_start: false }),
        _ => None,
    }
}

#[derive(Default)]
struct Streams {
    open: HashSet<u64>,
}

impl Streams {
    fn apply(&mut self, event: StreamEvent) -> bool {
        if event.is_start {
            self.open.insert(event.stream_id);
        } else {
            self.open.remove(&event.stream_id);
        }
        !self.open.is_empty()
    }
}

struct Subscriber {
    streams: Mutex<Streams>,
    is_in_use: Arc<AtomicBool>,
    notifier: Notifier,
}

fn render_xml(event: EVT_HANDLE) -> Option<String> {
    let (mut used, mut properties) = (0u32, 0u32);
    let _ = unsafe { EvtRender(None, event, EvtRenderEventXml.0, 0, None, &mut used, &mut properties) };
    if used == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (used as usize).div_ceil(2)];
    let result = unsafe {
        EvtRender(None, event, EvtRenderEventXml.0, used, Some(buffer.as_mut_ptr().cast()), &mut used, &mut properties)
    };
    result.ok()?;
    let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}

// Windows invokes this on a thread-pool thread.
unsafe extern "system" fn on_event(
    action: EVT_SUBSCRIBE_NOTIFY_ACTION,
    context: *const c_void,
    event: EVT_HANDLE,
) -> u32 {
    if action != EvtSubscribeActionDeliver {
        return 0;
    }
    // SAFETY: the context is the leaked `Subscriber` passed when subscribing, which is never freed.
    let Some(subscriber) = (unsafe { (context as *const Subscriber).as_ref() }) else { return 0 };
    let Some(stream_event) = render_xml(event).as_deref().and_then(parse_stream_event) else { return 0 };
    let Ok(mut streams) = subscriber.streams.lock() else { return 0 };
    let is_in_use = streams.apply(stream_event);
    if subscriber.is_in_use.swap(is_in_use, Ordering::SeqCst) != is_in_use {
        subscriber.notifier.notify();
    }
    0
}

pub struct CameraActivity {
    privacy: Privacy,
    is_in_use: Arc<AtomicBool>,
    subscription: Option<EVT_HANDLE>,
}

impl CameraActivity {
    pub fn new() -> Self {
        Self {
            privacy: Privacy::new(Device { name: "Camera", icon: Icon::Camera, color: GREEN }),
            is_in_use: Arc::new(AtomicBool::new(false)),
            subscription: None,
        }
    }

    fn subscribe(&mut self, notifier: Notifier) -> Result<()> {
        let subscriber = Box::new(Subscriber {
            streams: Mutex::new(Streams::default()),
            is_in_use: self.is_in_use.clone(),
            notifier,
        });
        let context = Box::into_raw(subscriber).cast::<c_void>();
        let subscription = unsafe {
            EvtSubscribe(None, None, CHANNEL, QUERY, None, Some(context), Some(on_event), EvtSubscribeToFutureEvents.0)?
        };
        self.subscription = Some(subscription);
        Ok(())
    }
}

impl Drop for CameraActivity {
    fn drop(&mut self) {
        if let Some(subscription) = self.subscription.take() {
            let _ = unsafe { EvtClose(subscription) };
        }
    }
}

impl Activity for CameraActivity {
    fn id(&self) -> &'static str {
        "camera"
    }

    fn label(&self) -> &'static str {
        "Camera in use"
    }

    fn presence(&self) -> Option<Presence> {
        self.privacy.presence()
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        self.privacy.draw(canvas, area, opacity);
    }

    fn on_start(&mut self, context: &mut Context) {
        if let Err(error) = self.subscribe(context.notifier()) {
            log::debug(&format!("camera: cannot follow the frame server log: {error}"));
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.privacy.on_timer(timer, context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        self.privacy.update(self.is_in_use.load(Ordering::SeqCst), context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(scope: u32, id: u64, operation: u32) -> String {
        format!(
            "<Event><System><EventID>11</EventID></System><EventData><Data Name='TimerScope'>{scope}</Data>\
             <Data Name='TimerScopeId'>{id}</Data><Data Name='WatchdogOperation'>{operation}</Data>\
             <Data Name='DurationToTriggerInHns'>200000000</Data></EventData></Event>"
        )
    }

    #[test]
    fn a_stream_start_event_is_read_from_the_event_xml() {
        assert_eq!(
            parse_stream_event(&xml(2, 1678722579792, 2)),
            Some(StreamEvent { stream_id: 1678722579792, is_start: true })
        );
    }

    #[test]
    fn a_stream_stop_event_is_read_from_the_event_xml() {
        assert_eq!(parse_stream_event(&xml(2, 7, 3)), Some(StreamEvent { stream_id: 7, is_start: false }));
    }

    #[test]
    fn other_watchdog_operations_are_ignored() {
        assert_eq!(parse_stream_event(&xml(2, 7, 1)), None);
        assert_eq!(parse_stream_event(&xml(2, 7, 4)), None);
    }

    #[test]
    fn xml_without_the_expected_data_is_ignored() {
        assert_eq!(parse_stream_event("<Event></Event>"), None);
        assert_eq!(parse_stream_event(&xml(2, 7, 2).replace("TimerScopeId", "Other")), None);
    }

    #[test]
    fn the_camera_is_in_use_while_any_stream_is_open() {
        let mut streams = Streams::default();
        assert!(streams.apply(StreamEvent { stream_id: 1, is_start: true }));
        assert!(streams.apply(StreamEvent { stream_id: 2, is_start: true }));
        assert!(streams.apply(StreamEvent { stream_id: 1, is_start: false }));
        assert!(!streams.apply(StreamEvent { stream_id: 2, is_start: false }));
    }

    #[test]
    fn stopping_a_stream_that_was_never_seen_leaves_the_camera_unused() {
        assert!(!Streams::default().apply(StreamEvent { stream_id: 9, is_start: false }));
    }
}
