use super::{MediaPage, layout::Layout};
use crate::{
    gfx::{Canvas, Color, Point},
    ui::{Icon, IconButton},
};

/// The color of the lyrics button while lyrics are on.
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

    /// Position in the row, counting from the first button.
    pub fn slot(self) -> f32 {
        match self {
            Control::Previous => 0.0,
            Control::PlayPause => 1.0,
            Control::Next => 2.0,
            Control::Lyrics => 3.0,
        }
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
        // The lyrics button is green when lyrics are on and gray when off; the others are white.
        let color = match is_on {
            Some(true) => ACCENT,
            Some(false) if is_hovered => INACTIVE_HOVERED,
            Some(false) => INACTIVE,
            None => Color::WHITE,
        };
        IconButton { icon, center: layout.control_center(control), color, is_hovered }.draw(canvas, opacity);
    }
}
