//! Every activity the notch can show. This list is the only place activities are registered.
//!
//! To add an activity: create a file or folder here with a type that implements `Activity`, declare the module below
//! and add the type to `all()`. To remove one: delete it, its `mod` line and its entry in `all()`.
//! Each activity can be switched off from the tray menu, which stores `activity.<id>=off` in the settings file.

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

/// The notifications activity showing an example banner, for rendering the notch off-screen.
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
