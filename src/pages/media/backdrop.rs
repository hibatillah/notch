//! Album-art glow fades upward over the open player.

use crate::gfx::{Canvas, Color, Rect};

const BOTTOM_ALPHA: f32 = 0.55;
const FALLOFF: f32 = 1.6;
const STRIP: f32 = 2.0;
/// Keeps the glow off the antialiased edge of the notch.
const EDGE_INSET: f32 = 0.25;

fn alpha_at(from_top: f32) -> f32 {
    BOTTOM_ALPHA * from_top.clamp(0.0, 1.0).powf(FALLOFF)
}

fn corner_inset(y: f32, bottom: f32, radius: f32) -> f32 {
    let into_corner = y - (bottom - radius);
    if into_corner <= 0.0 || radius <= 0.0 {
        return 0.0;
    }
    radius - (radius * radius - into_corner * into_corner).max(0.0).sqrt()
}

/// Split a row into (from, to, weight) spans with weighted end pixels; zero weight skips drawing.
fn row_pieces(left: f32, right: f32) -> [(f32, f32, f32); 3] {
    let (inner_left, inner_right) = (left.ceil(), right.floor());
    if inner_left > inner_right {
        return [(left, right, 1.0), (0.0, 0.0, 0.0), (0.0, 0.0, 0.0)];
    }
    [
        (inner_left, inner_right, 1.0),
        (left.floor(), inner_left, inner_left - left),
        (inner_right, right.ceil(), right - inner_right),
    ]
}

pub fn draw(canvas: &Canvas, pill: Rect, radius: f32, scale: f32, color: Color, amount: f32) {
    if amount <= 0.01 || pill.height() <= 0.0 {
        return;
    }
    // Whole device pixels, so no strip has a blurred edge that would show as a line between two of them.
    let strip = (STRIP * scale).round().max(1.0);
    let corner_top = (pill.bottom - radius).round();
    let mut top = pill.top.round();
    while top < pill.bottom {
        let height = if top >= corner_top { 1.0 } else { strip.min(corner_top - top) };
        let bottom = (top + height).min(pill.bottom);
        let center = (top + bottom) / 2.0;
        let alpha = alpha_at((center - pill.top) / pill.height()) * amount;
        if alpha > 0.004 {
            let inset = corner_inset(center, pill.bottom, radius) + EDGE_INSET * scale;
            for (from, to, weight) in row_pieces(pill.left + inset, pill.right - inset) {
                if weight > 0.0 && to > from {
                    canvas.fill_rounded_rect(Rect::new(from, top, to, bottom), 0.0, color.with_alpha(alpha * weight));
                }
            }
        }
        top = bottom;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glow_is_strongest_at_the_bottom_and_gone_at_the_top() {
        assert_eq!(alpha_at(0.0), 0.0);
        assert_eq!(alpha_at(1.0), BOTTOM_ALPHA);
        assert!(alpha_at(0.5) < alpha_at(0.75));
        assert!(alpha_at(0.75) < alpha_at(1.0));
    }

    #[test]
    fn the_glow_stays_bottom_heavy() {
        assert!(alpha_at(0.5) < BOTTOM_ALPHA / 2.0);
    }

    #[test]
    fn the_strips_are_pulled_in_only_inside_the_rounded_corners() {
        assert_eq!(corner_inset(10.0, 100.0, 20.0), 0.0);
        assert_eq!(corner_inset(80.0, 100.0, 20.0), 0.0);
        assert!(corner_inset(90.0, 100.0, 20.0) > 0.0);
        assert!(corner_inset(99.0, 100.0, 20.0) > corner_inset(90.0, 100.0, 20.0));
        assert!(corner_inset(100.0, 100.0, 20.0) <= 20.0);
    }

    #[test]
    fn a_row_covers_each_end_pixel_by_the_part_it_reaches() {
        let pieces = row_pieces(10.25, 20.5);
        assert_eq!(pieces[0], (11.0, 20.0, 1.0));
        assert_eq!(pieces[1], (10.0, 11.0, 0.75));
        assert_eq!(pieces[2], (20.0, 21.0, 0.5));
    }

    #[test]
    fn a_row_ending_on_whole_pixels_has_no_partial_ends() {
        let pieces = row_pieces(10.0, 20.0);
        assert_eq!(pieces[0], (10.0, 20.0, 1.0));
        assert!(pieces[1].2 == 0.0 && pieces[2].2 == 0.0);
    }

    #[test]
    fn a_square_corner_needs_no_inset() {
        assert_eq!(corner_inset(99.0, 100.0, 0.0), 0.0);
    }
}
