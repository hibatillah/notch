//! The microphone being in use, found through the audio sessions of the default recording device: a session turns
//! active while an app records. Core Audio calls back on a system thread when a session appears or changes state,
//! so nothing polls.

use windows::{
    Win32::{
        Media::Audio::{
            AudioSessionDisconnectReason, AudioSessionState, AudioSessionStateActive, IAudioSessionControl,
            IAudioSessionControl2, IAudioSessionEvents, IAudioSessionEvents_Impl, IAudioSessionManager2,
            IAudioSessionNotification, IAudioSessionNotification_Impl, IMMDeviceEnumerator, MMDeviceEnumerator,
            eCapture, eConsole,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree},
    },
    core::{BOOL, GUID, Interface, PCWSTR, Ref, Result, implement},
};

use super::privacy::{Device, Privacy};
use crate::{
    activity::{Activity, Presence},
    gfx::{Canvas, Color, Rect},
    log,
    page::{Context, Notifier, TimerId},
    ui::Icon,
};

const ORANGE: Color = Color::rgb(1.0, 0.62, 0.04);

#[implement(IAudioSessionNotification)]
struct SessionCreated {
    notifier: Notifier,
}

impl IAudioSessionNotification_Impl for SessionCreated_Impl {
    fn OnSessionCreated(&self, _session: Ref<'_, IAudioSessionControl>) -> Result<()> {
        self.notifier.notify();
        Ok(())
    }
}

#[implement(IAudioSessionEvents)]
struct SessionEvents {
    notifier: Notifier,
}

impl IAudioSessionEvents_Impl for SessionEvents_Impl {
    fn OnDisplayNameChanged(&self, _name: &PCWSTR, _context: *const GUID) -> Result<()> {
        Ok(())
    }

    fn OnIconPathChanged(&self, _path: &PCWSTR, _context: *const GUID) -> Result<()> {
        Ok(())
    }

    fn OnSimpleVolumeChanged(&self, _volume: f32, _is_muted: BOOL, _context: *const GUID) -> Result<()> {
        Ok(())
    }

    fn OnChannelVolumeChanged(
        &self,
        _count: u32,
        _volumes: *const f32,
        _changed: u32,
        _context: *const GUID,
    ) -> Result<()> {
        Ok(())
    }

    fn OnGroupingParamChanged(&self, _grouping: *const GUID, _context: *const GUID) -> Result<()> {
        Ok(())
    }

    fn OnStateChanged(&self, _state: AudioSessionState) -> Result<()> {
        self.notifier.notify();
        Ok(())
    }

    fn OnSessionDisconnected(&self, _reason: AudioSessionDisconnectReason) -> Result<()> {
        self.notifier.notify();
        Ok(())
    }
}

struct Listener {
    manager: IAudioSessionManager2,
    created: IAudioSessionNotification,
    events: IAudioSessionEvents,
    /// Sessions whose state changes are already watched, by their instance identifier.
    watched: Vec<(String, IAudioSessionControl)>,
}

impl Listener {
    fn new(notifier: Notifier) -> Result<Self> {
        let enumerator: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
        let device = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)? };
        let manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None)? };
        let created: IAudioSessionNotification = SessionCreated { notifier }.into();
        unsafe { manager.RegisterSessionNotification(&created)? };
        let events: IAudioSessionEvents = SessionEvents { notifier }.into();
        Ok(Self { manager, created, events, watched: Vec::new() })
    }

    /// Watches any new session and tells whether some session is recording right now.
    fn refresh(&mut self) -> bool {
        let Ok(sessions) = (unsafe { self.manager.GetSessionEnumerator() }) else { return false };
        let count = unsafe { sessions.GetCount() }.unwrap_or(0);
        let mut present = Vec::new();
        let mut is_recording = false;
        for index in 0..count {
            let Ok(control) = (unsafe { sessions.GetSession(index) }) else { continue };
            is_recording |= unsafe { control.GetState() }.is_ok_and(|state| state == AudioSessionStateActive);
            let Some(id) = instance_identifier(&control) else { continue };
            if !self.watched.iter().any(|(watched_id, _)| *watched_id == id) {
                let _ = unsafe { control.RegisterAudioSessionNotification(&self.events) };
                self.watched.push((id.clone(), control));
            }
            present.push(id);
        }
        let events = &self.events;
        self.watched.retain(|(id, control)| {
            let is_present = present.contains(id);
            if !is_present {
                let _ = unsafe { control.UnregisterAudioSessionNotification(events) };
            }
            is_present
        });
        is_recording
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        for (_, control) in &self.watched {
            let _ = unsafe { control.UnregisterAudioSessionNotification(&self.events) };
        }
        let _ = unsafe { self.manager.UnregisterSessionNotification(&self.created) };
    }
}

fn instance_identifier(control: &IAudioSessionControl) -> Option<String> {
    let identifier = unsafe { control.cast::<IAudioSessionControl2>().ok()?.GetSessionInstanceIdentifier().ok()? };
    let text = unsafe { identifier.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(identifier.0.cast())) };
    text
}

pub struct MicrophoneActivity {
    privacy: Privacy,
    listener: Option<Listener>,
}

impl MicrophoneActivity {
    pub fn new() -> Self {
        Self {
            privacy: Privacy::new(Device { name: "Microphone", icon: Icon::Microphone, color: ORANGE }),
            listener: None,
        }
    }
}

impl Activity for MicrophoneActivity {
    fn id(&self) -> &'static str {
        "microphone"
    }

    fn label(&self) -> &'static str {
        "Microphone in use"
    }

    fn presence(&self) -> Option<Presence> {
        self.privacy.presence()
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        self.privacy.draw(canvas, area, opacity);
    }

    fn on_start(&mut self, context: &mut Context) {
        match Listener::new(context.notifier()) {
            Ok(mut listener) => {
                let is_recording = listener.refresh();
                self.privacy.initialize(is_recording);
                self.listener = Some(listener);
            }
            Err(error) => log::debug(&format!("microphone: no recording device to watch: {error}")),
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.privacy.on_timer(timer, context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        if let Some(listener) = self.listener.as_mut() {
            let is_recording = listener.refresh();
            self.privacy.update(is_recording, context);
        }
    }
}
