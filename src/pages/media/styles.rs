use crate::gfx::{Align, FontStyle, SEGOE_UI, Weight};

pub const RESTING_LABEL: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Center);
/// The title and artist in the collapsed pill when a track starts.
pub const COMPACT_TITLE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
pub const COMPACT_ARTIST: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
pub const RESTING_LYRIC: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
pub const TITLE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
/// The album and artist in the open player.
pub const DETAIL: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
/// Lyric rows in the open player; a long line is cut with an ellipsis.
pub const LYRIC_LINE: FontStyle = FontStyle::new(SEGOE_UI, 14.0, Weight::SemiBold, Align::Leading);
