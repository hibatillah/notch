//! The notch itself: the window, the animation, hover and input handling, the tray icon and page selection.
//! Nothing in here knows what any particular page shows.

mod activity_layout;
mod chrome;
mod display;
mod navigation;
mod notch;
mod page_editor;
mod registry;
mod settings;
mod tray;
mod window;

pub use settings::Settings;
pub use window::run;
