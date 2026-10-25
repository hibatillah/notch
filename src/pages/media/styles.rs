use crate::gfx::{Align, FontStyle, UI_FONT, Weight};

pub const RESTING_LABEL: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Center);
/// The title and artist in the collapsed pill when a track starts.
pub const COMPACT_TITLE: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Leading);
pub const COMPACT_ARTIST: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Regular, Align::Leading);
pub const RESTING_LYRIC: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Center);
pub const TITLE: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Leading);
/// The artist in the open player.
pub const DETAIL: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Regular, Align::Leading);
/// The lyric line being sung in the open player, centered; the lines around it are drawn smaller from this style.
pub const LYRIC_LINE: FontStyle = FontStyle::new(UI_FONT, 17.0, Weight::Medium, Align::Center);
