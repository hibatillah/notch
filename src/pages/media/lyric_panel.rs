use super::{MediaPage, layout::Layout, styles};
use crate::gfx::{Canvas, Color, Point, Rect, lerp};

const LINE_HEIGHT: f32 = 22.0;
const NOTE: &str = "\u{266A}";
const ROW_GAP: f32 = 6.0;
const NEIGHBOR_SCALE: f32 = 0.8;
const NEIGHBOR_GRAY: f32 = 0.36;

fn center_of(row: i64, anchor: i64, height: &impl Fn(i64, i64) -> f32) -> f32 {
    if row == anchor {
        return 0.0;
    }
    let step = if row > anchor { 1 } else { -1 };
    let mut distance = height(anchor, anchor) / 2.0;
    let mut between = anchor + step;
    while between != row {
        distance += ROW_GAP + height(between, anchor);
        between += step;
    }
    step as f32 * (distance + ROW_GAP + height(row, anchor) / 2.0)
}

pub fn draw(page: &MediaPage, canvas: &Canvas, layout: &Layout, opacity: f32) {
    let Some(lines) = page.visible_lyric_lines() else { return };
    let scale = layout.scale;
    let panel = layout.lyric_panel();
    let transition = page.follower.transition().clamp(0.0, 1.0);
    // Before the first line starts, the "current" position sits just above line zero.
    let current = page.follower.index().map_or(-1, |index| index as i64);

    let text_lines = |row: i64| {
        usize::try_from(current + row)
            .ok()
            .and_then(|index| page.lyric_wraps.get(index))
            .map_or(1, |wrapped| wrapped.len().max(1))
    };
    let row_height = |row: i64, anchor: i64| {
        let size = if row == anchor { 1.0 } else { NEIGHBOR_SCALE };
        text_lines(row) as f32 * LINE_HEIGHT * size
    };

    canvas.clipped(panel, || {
        for relative in -2..=2_i64 {
            let Some(line) = usize::try_from(current + relative).ok().and_then(|index| lines.get(index)) else {
                continue;
            };
            let rows_from_center = relative as f32 + (1.0 - transition);
            let distance = rows_from_center.abs();
            let offset = lerp(center_of(relative, -1, &row_height), center_of(relative, 0, &row_height), transition);
            let center = Point::new(panel.center_x(), panel.center_y() + offset * scale);
            let gray = lerp(NEIGHBOR_GRAY, 1.0, (1.0 - distance).clamp(0.0, 1.0));
            let alpha = (1.0 - 0.5 * distance).clamp(0.0, 1.0);

            // Widen scaled neighbour rows so their text still fills the panel.
            let text_scale = lerp(1.0, NEIGHBOR_SCALE, distance.min(1.0));
            let wrapped: &[String] = page.lyric_wraps.get((current + relative) as usize).map_or(&[], Vec::as_slice);
            let fallback = [if line.text.is_empty() { NOTE.to_owned() } else { line.text.clone() }];
            let shown = if wrapped.is_empty() { &fallback[..] } else { wrapped };
            let half_width = panel.width() / text_scale / 2.0;
            canvas.scaled_about(center, text_scale, || {
                for (index, text) in shown.iter().enumerate() {
                    let line_center =
                        center.y + (index as f32 - (shown.len() as f32 - 1.0) / 2.0) * LINE_HEIGHT * scale;
                    let row = Rect::new(
                        center.x - half_width,
                        line_center - LINE_HEIGHT * scale / 2.0,
                        center.x + half_width,
                        line_center + LINE_HEIGHT * scale / 2.0,
                    );
                    canvas.text(&styles::LYRIC_LINE, text, row, Color::gray(gray).with_alpha(opacity * alpha));
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tall(rows: f32) -> impl Fn(i64, i64) -> f32 {
        move |_, _| rows * 10.0
    }

    #[test]
    fn the_row_that_is_the_anchor_sits_in_the_middle() {
        assert_eq!(center_of(3, 3, &tall(1.0)), 0.0);
    }

    #[test]
    fn rows_below_stack_downward_with_a_gap() {
        assert_eq!(center_of(1, 0, &tall(1.0)), 10.0 + ROW_GAP);
        assert_eq!(center_of(2, 0, &tall(1.0)), 2.0 * (10.0 + ROW_GAP));
    }

    #[test]
    fn rows_above_mirror_the_rows_below() {
        assert_eq!(center_of(-2, 0, &tall(1.0)), -center_of(2, 0, &tall(1.0)));
    }

    #[test]
    fn a_taller_row_pushes_the_next_one_further_away() {
        let height = |row: i64, _: i64| if row == 0 { 20.0 } else { 10.0 };
        assert_eq!(center_of(1, 0, &height), 10.0 + ROW_GAP + 5.0);
    }
}
