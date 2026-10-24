use super::{
    MediaPage,
    layout::Layout,
    session::{MediaInfo, RepeatMode},
};
use crate::{
    gfx::{Canvas, Color, Point},
    ui::{Icon, IconButton},
};

const ACCENT: Color = Color::rgb(0.12, 0.84, 0.38);
const INACTIVE: Color = Color::gray(0.5);
const INACTIVE_HOVERED: Color = Color::gray(0.85);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Lyrics,
    Shuffle,
    Previous,
    PlayPause,
    Next,
    Repeat,
}

impl Control {
    pub const ALL: [Control; 6] =
        [Control::Lyrics, Control::Shuffle, Control::Previous, Control::PlayPause, Control::Next, Control::Repeat];

    /// Position in the row, in button steps from the center of the controls column.
    pub fn slot(self) -> f32 {
        match self {
            Control::Lyrics => -2.5,
            Control::Shuffle => -1.5,
            Control::Previous => -0.5,
            Control::PlayPause => 0.5,
            Control::Next => 1.5,
            Control::Repeat => 2.5,
        }
    }
}

/// Shuffle and repeat only show when the player supports them.
fn is_available(control: Control, info: &MediaInfo) -> bool {
    match control {
        Control::Shuffle => info.shuffle_enabled,
        Control::Repeat => info.repeat_enabled,
        _ => true,
    }
}

pub fn hit_test(layout: &Layout, info: &MediaInfo, point: Point) -> Option<Control> {
    Control::ALL
        .into_iter()
        .filter(|&control| is_available(control, info))
        .find(|&control| IconButton::is_hit(layout.control_center(control), layout.scale, point))
}

pub fn draw(page: &MediaPage, canvas: &Canvas, layout: &Layout, opacity: f32) {
    let info = &page.info;
    for control in Control::ALL {
        if !is_available(control, info) {
            continue;
        }
        // Toggles are green when on and gray when off; plain buttons are white.
        let (icon, toggle) = match control {
            Control::Lyrics => (Icon::Lyrics, Some(page.is_lyric_panel_visible())),
            Control::Shuffle => (Icon::Shuffle, Some(info.shuffle_active)),
            Control::Previous => (Icon::Previous, None),
            Control::PlayPause => (if info.is_playing { Icon::Pause } else { Icon::Play }, None),
            Control::Next => (Icon::Next, None),
            Control::Repeat => (
                if info.repeat_mode == RepeatMode::Track { Icon::RepeatOne } else { Icon::Repeat },
                Some(info.repeat_mode != RepeatMode::Off),
            ),
        };
        let is_hovered = page.hovered_control == Some(control);
        let color = match toggle {
            Some(true) => ACCENT,
            Some(false) if is_hovered => INACTIVE_HOVERED,
            Some(false) => INACTIVE,
            None => Color::WHITE,
        };
        IconButton { icon, center: layout.control_center(control), color, is_hovered }.draw(canvas, opacity);
    }
}
