use super::{MediaPage, layout::Layout, styles};
use crate::gfx::{Canvas, Color, Point, Rect, lerp};

/// Distance between lyric line centers.
const ROW_SPACING: f32 = 24.0;
const ROW_HEIGHT: f32 = 22.0;
/// Size of the previous and next lines relative to the current one.
const NEIGHBOR_SCALE: f32 = 0.93;
/// The gray of the lines before and after the current one.
const NEIGHBOR_GRAY: f32 = 0.36;

/// Three lyric lines beside the player: the line being sung is white and centered, the ones before and after are
/// smaller and gray. When the line changes, all of them slide up by one place.
pub fn draw(page: &MediaPage, canvas: &Canvas, layout: &Layout, opacity: f32) {
    let Some(lines) = page.visible_lyric_lines() else { return };
    let scale = layout.scale;
    let panel = layout.lyric_panel();
    let center_y = panel.center_y();
    let transition = page.follower.transition().clamp(0.0, 1.0);
    // Before the first line starts, the "current" position sits just above line zero.
    let current = page.follower.index().map_or(-1, |index| index as i64);

    canvas.clipped(panel, || {
        for relative in -2..=2_i64 {
            let Some(line) = usize::try_from(current + relative).ok().and_then(|index| lines.get(index)) else {
                continue;
            };
            if line.text.is_empty() {
                continue;
            }
            // Every row slides up by one place while the transition runs, then rests.
            let rows_from_center = relative as f32 + (1.0 - transition);
            let distance = rows_from_center.abs();
            let center = center_y + rows_from_center * ROW_SPACING * scale;
            let gray = lerp(NEIGHBOR_GRAY, 1.0, (1.0 - distance).clamp(0.0, 1.0));
            let alpha = (1.0 - 0.5 * distance).clamp(0.0, 1.0);

            // Neighbours are scaled about their left edge and row center; the row is widened by the same factor so
            // the text still fills the panel.
            let text_scale = lerp(1.0, NEIGHBOR_SCALE, distance.min(1.0));
            let half_height = ROW_HEIGHT * scale / 2.0;
            let row = Rect::new(
                panel.left,
                center - half_height,
                panel.left + panel.width() / text_scale,
                center + half_height,
            );
            canvas.scaled_about(Point::new(panel.left, center), text_scale, || {
                canvas.text(&styles::LYRIC_LINE, &line.text, row, Color::gray(gray).with_alpha(opacity * alpha));
            });
        }
    });
}
