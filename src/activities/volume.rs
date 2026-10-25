//! Volume and mute of the default speakers. Core Audio calls back on a system thread when either changes, so nothing
//! polls; the callback stores the change and wakes the UI thread.

use std::sync::{Arc, Mutex};

use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole,
            Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl},
            IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, eMultimedia,
            eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance},
    },
    core::{PCWSTR, Result, implement},
};

use crate::{
    activity::{Activity, Flash, Presence},
    gfx::{Canvas, Color, Point, Rect},
    log,
    page::{Context, Notifier, TimerId},
    ui::{self, Icon},
};

const SHOW_MILLISECONDS: u32 = 1500;

#[derive(Default)]
struct Shared {
    /// The latest volume (0 to 1) and mute state reported, until the UI thread takes it.
    level: Option<(f32, bool)>,
    default_device_changed: bool,
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    shared: Arc<Mutex<Shared>>,
    notifier: Notifier,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, notification: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        // SAFETY: Core Audio passes a valid pointer for the duration of the call.
        if let Some(data) = unsafe { notification.as_ref() } {
            if let Ok(mut shared) = self.shared.lock() {
                shared.level = Some((data.fMasterVolume, data.bMuted.as_bool()));
            }
            self.notifier.notify();
        }
        Ok(())
    }
}

#[implement(IMMNotificationClient)]
struct DeviceWatcher {
    shared: Arc<Mutex<Shared>>,
    notifier: Notifier,
}

impl IMMNotificationClient_Impl for DeviceWatcher_Impl {
    fn OnDeviceStateChanged(&self, _device: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _device: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _device: &PCWSTR) -> Result<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _device: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eMultimedia {
            if let Ok(mut shared) = self.shared.lock() {
                shared.default_device_changed = true;
            }
            self.notifier.notify();
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _device: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

/// Everything held while listening, so it can be released in the right order.
struct Listener {
    enumerator: IMMDeviceEnumerator,
    device_watcher: IMMNotificationClient,
    endpoint: Option<(IAudioEndpointVolume, IAudioEndpointVolumeCallback)>,
}

pub struct VolumeActivity {
    shared: Arc<Mutex<Shared>>,
    listener: Option<Listener>,
    notifier: Option<Notifier>,
    volume: f32,
    is_muted: bool,
    /// Width of the "Muted" message, measured when the speakers are muted.
    muted_width: f32,
    flash: Flash,
}

impl VolumeActivity {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared::default())),
            listener: None,
            notifier: None,
            volume: 0.0,
            is_muted: false,
            muted_width: 0.0,
            flash: Flash::default(),
        }
    }

    /// Listens to the current default speakers, replacing the previous ones.
    fn attach(&mut self) -> Result<()> {
        let (Some(listener), Some(notifier)) = (self.listener.as_mut(), self.notifier) else { return Ok(()) };
        if let Some((endpoint, callback)) = listener.endpoint.take() {
            let _ = unsafe { endpoint.UnregisterControlChangeNotify(&callback) };
        }
        let device = unsafe { listener.enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)? };
        let endpoint: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
        // Changes are reported, but the state at the start is not.
        self.is_muted = unsafe { endpoint.GetMute() }.is_ok_and(|muted| muted.as_bool());
        let callback: IAudioEndpointVolumeCallback = VolumeCallback { shared: self.shared.clone(), notifier }.into();
        unsafe { endpoint.RegisterControlChangeNotify(&callback)? };
        listener.endpoint = Some((endpoint, callback));
        Ok(())
    }

    fn listen(&mut self, notifier: Notifier) -> Result<()> {
        let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let device_watcher: IMMNotificationClient = DeviceWatcher { shared: self.shared.clone(), notifier }.into();
        unsafe { enumerator.RegisterEndpointNotificationCallback(&device_watcher)? };
        self.notifier = Some(notifier);
        self.listener = Some(Listener { enumerator, device_watcher, endpoint: None });
        self.attach()
    }
}

impl Drop for VolumeActivity {
    fn drop(&mut self) {
        if let Some(listener) = self.listener.take() {
            if let Some((endpoint, callback)) = listener.endpoint {
                let _ = unsafe { endpoint.UnregisterControlChangeNotify(&callback) };
            }
            let _ = unsafe { listener.enumerator.UnregisterEndpointNotificationCallback(&listener.device_watcher) };
        }
    }
}

/// "42" for a volume of 0.42.
fn value_text(volume: f32) -> String {
    format!("{}", (volume * 100.0).round() as u32)
}

impl Activity for VolumeActivity {
    fn id(&self) -> &'static str {
        "volume"
    }

    fn label(&self) -> &'static str {
        "Volume"
    }

    fn presence(&self) -> Option<Presence> {
        let width = if self.is_muted { self.muted_width } else { ui::LEVEL_WIDTH };
        self.flash.is_visible().then_some(Presence::Transient { width })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        if self.is_muted {
            ui::draw_message(canvas, area, opacity, Icon::SpeakerMuted, Color::WHITE, "Muted", None);
        } else {
            ui::draw_level(canvas, area, opacity, Icon::Speaker, self.volume, &value_text(self.volume));
        }
    }

    fn header_status_width(&self) -> Option<f32> {
        self.is_muted.then_some(Icon::SpeakerMuted.size())
    }

    fn draw_header_status(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        let center = Point::new(area.center_x(), area.center_y());
        Icon::SpeakerMuted.draw(canvas, center, Icon::SpeakerMuted.size() * scale, Color::WHITE, opacity);
    }

    fn on_start(&mut self, context: &mut Context) {
        if let Err(error) = self.listen(context.notifier()) {
            log::debug(&format!("volume: no audio output to listen to: {error}"));
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        let (level, device_changed) = match self.shared.lock() {
            Ok(mut shared) => (shared.level.take(), std::mem::take(&mut shared.default_device_changed)),
            Err(_) => return,
        };
        if device_changed && let Err(error) = self.attach() {
            log::debug(&format!("volume: new default output not usable: {error}"));
        }
        if let Some((volume, is_muted)) = level {
            self.volume = volume;
            self.is_muted = is_muted;
            self.muted_width = ui::message_width(context.fonts(), "Muted", None);
            self.flash.show(context, SHOW_MILLISECONDS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_is_shown_as_a_percentage() {
        assert_eq!(value_text(0.42), "42");
        assert_eq!(value_text(1.0), "100");
        assert_eq!(value_text(0.0), "0");
    }
}
