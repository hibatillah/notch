//! The notch itself: the window, the animation, hover and input handling, the tray icon and page selection.
//! Nothing in here knows what any particular page shows.

mod display;
mod notch;
mod registry;
mod settings;
mod tray;
mod window;

pub use settings::Settings;
pub use window::run;
