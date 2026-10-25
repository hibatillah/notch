//! The notch itself: the window, the animation, hover and input handling, the tray icon and page selection.
//! Nothing in here knows what any particular page shows.

mod activity_layout;
mod chrome;
mod clock;
mod display;
mod drop_target;
mod navigation;
mod notch;
mod page_editor;
mod registry;
mod screen_mode;
mod settings;
mod startup;
mod tray;
mod window;

pub use settings::Settings;
pub use window::run;
