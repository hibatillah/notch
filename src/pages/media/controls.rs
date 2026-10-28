use super::{MediaPage, layout::Layout};
use crate::{
    gfx::{Canvas, Color, Point},
    ui::{Icon, IconButton},
};

const ACCENT: Color = Color::rgb(0.118, 0.843, 0.376);
const INACTIVE: Color = Color::rgb(0.431, 0.431, 0.478);
const INACTIVE_HOVERED: Color = Color::gray(0.85);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    Previous,
    PlayPause,
    Next,
    Lyrics,
}

impl Control {
    pub const ALL: [Control; 4] = [Control::Previous, Control::PlayPause, Control::Next, Control::Lyrics];

    pub fn slot(self) -> f32 {
        match self {
            Control::Previous => 0.0,
            Control::PlayPause => 1.0,
            Control::Next => 2.0,
            Control::Lyrics => 3.0,
        }
    }
}

fn lyrics_color(is_on: bool, has_lyrics: bool, is_hovered: bool) -> Color {
    match (is_on, has_lyrics) {
        (true, _) => ACCENT,
        (false, true) => Color::WHITE,
        (false, false) if is_hovered => INACTIVE_HOVERED,
        (false, false) => INACTIVE,
    }
}

pub fn hit_test(layout: &Layout, point: Point) -> Option<Control> {
    Control::ALL.into_iter().find(|&control| IconButton::is_hit(layout.control_center(control), layout.scale, point))
}

pub fn draw(page: &MediaPage, canvas: &Canvas, layout: &Layout, opacity: f32) {
    for control in Control::ALL {
        let (icon, is_on) = match control {
            Control::Previous => (Icon::Previous, None),
            Control::PlayPause => (if page.info.is_playing { Icon::Pause } else { Icon::Play }, None),
            Control::Next => (Icon::Next, None),
            Control::Lyrics => (Icon::Lyrics, Some(page.is_lyric_panel_visible())),
        };
        let is_hovered = page.hovered_control == Some(control);
        let color = match is_on {
            _ if !page.info.available => INACTIVE,
            Some(is_on) => lyrics_color(is_on, page.has_lyrics(), is_hovered),
            None => Color::WHITE,
        };
        IconButton { icon, center: layout.control_center(control), color, is_hovered }.draw(canvas, opacity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lyrics_button_is_green_when_on() {
        assert_eq!(lyrics_color(true, true, false), ACCENT);
    }

    #[test]
    fn the_lyrics_button_is_white_when_off_but_lyrics_exist() {
        assert_eq!(lyrics_color(false, true, false), Color::WHITE);
        assert_eq!(lyrics_color(false, true, true), Color::WHITE);
    }

    #[test]
    fn the_lyrics_button_is_gray_when_the_song_has_no_lyrics() {
        assert_eq!(lyrics_color(false, false, false), INACTIVE);
        assert_eq!(lyrics_color(false, false, true), INACTIVE_HOVERED);
    }
}
