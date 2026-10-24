use crate::gfx::{Align, FontStyle, SEGOE_UI, Weight};

pub const RESTING_LABEL: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Center);
pub const RESTING_TITLE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
pub const RESTING_LYRIC: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
pub const TITLE: FontStyle = FontStyle::new(SEGOE_UI, 16.0, Weight::SemiBold, Align::Leading);
pub const ARTIST: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::Regular, Align::Leading);
pub const TIME_REMAINING: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::Regular, Align::Trailing);
/// Lyric rows in the expanded panel; long lines wrap instead of being cut off.
pub const PANEL_LINE: FontStyle = FontStyle::new(SEGOE_UI, 17.0, Weight::SemiBold, Align::Leading).wrapped();
