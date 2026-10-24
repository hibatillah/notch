use super::{
    MediaPage,
    layout::Layout,
    session::{MediaInfo, format_seconds},
    styles,
};
use crate::gfx::{Canvas, Color, Point};

fn is_seekable(info: &MediaInfo) -> bool {
    info.available && info.can_seek && info.duration_ticks > 0
}

/// Whether a press at `point` should start scrubbing.
pub fn can_grab(layout: &Layout, info: &MediaInfo, point: Point) -> bool {
    is_seekable(info) && layout.progress_bar().is_hit(point)
}

/// The position shown: where the user is dragging to, otherwise the live playback position.
fn shown_position_ticks(page: &MediaPage) -> i64 {
    match page.seek_drag {
        Some(fraction) => (fraction * page.info.duration_ticks as f32) as i64,
        None => page.info.position_ticks_now(),
    }
}

/// Draws the progress bar and the time left. The elapsed time is left out to give the bar the room.
pub fn draw(page: &MediaPage, canvas: &Canvas, layout: &Layout, opacity: f32) {
    let duration = page.info.duration_ticks;
    if duration <= 0 {
        return;
    }
    let position = shown_position_ticks(page);
    layout.progress_bar().draw(canvas, position as f32 / duration as f32, page.seek_drag.is_some(), opacity);
    let remaining = format!("-{}", format_seconds(duration - position));
    canvas.text(&styles::TIME_REMAINING, &remaining, layout.remaining_label(), Color::gray(0.65).faded(opacity));
}
