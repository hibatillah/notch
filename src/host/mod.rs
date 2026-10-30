//! Window orchestration stays independent of individual page implementations.

mod activity_layout;
mod chrome;
mod clock;
mod display;
mod drop_target;
mod icon;
mod navigation;
mod notch;
mod page_editor;
mod registry;
mod screen_mode;
mod settings;
mod settings_launcher;
mod startup;
mod tray;
mod visibility;
mod window;

pub use settings::Settings;
pub use window::run;
