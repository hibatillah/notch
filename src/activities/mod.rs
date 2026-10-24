//! Every activity the notch can show. This list is the only place activities are registered.
//!
//! To add an activity: create a file or folder here with a type that implements `Activity`, declare the module below
//! and add the type to `all()`. To remove one: delete it, its `mod` line and its entry in `all()`.
//! Each activity can be switched off from the tray menu, which stores `activity.<id>=off` in the settings file.

mod battery;
mod brightness;
mod camera;
mod clipboard;
mod microphone;
mod privacy;
mod volume;

use crate::activity::Activity;

pub fn all() -> Vec<Box<dyn Activity>> {
    vec![
        Box::new(volume::VolumeActivity::new()),
        Box::new(brightness::BrightnessActivity::new()),
        Box::new(battery::BatteryActivity::new()),
        Box::new(microphone::MicrophoneActivity::new()),
        Box::new(camera::CameraActivity::new()),
        Box::new(clipboard::ClipboardActivity::new()),
    ]
}
