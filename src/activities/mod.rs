//! The only activity registry; optional activities persist their tray choice as activity.<id>=off.

mod battery;
mod bluetooth;
mod brightness;
mod camera;
mod caps_lock;
mod clipboard;
mod microphone;
mod notifications;
mod privacy;
mod volume;

use crate::activity::Activity;

#[cfg(test)]
pub fn sample_notification(fonts: &crate::gfx::Fonts) -> Box<dyn Activity> {
    Box::new(notifications::NotificationsActivity::sample(fonts))
}

pub fn all() -> Vec<Box<dyn Activity>> {
    vec![
        Box::new(volume::VolumeActivity::new()),
        Box::new(brightness::BrightnessActivity::new()),
        Box::new(battery::BatteryActivity::new()),
        Box::new(bluetooth::BluetoothActivity::new()),
        Box::new(microphone::MicrophoneActivity::new()),
        Box::new(camera::CameraActivity::new()),
        Box::new(clipboard::ClipboardActivity::new()),
        Box::new(caps_lock::CapsLockActivity::new()),
        Box::new(notifications::NotificationsActivity::new()),
    ]
}
