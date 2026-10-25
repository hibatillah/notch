//! What the open notch has around every page: the padding, and a header with the page dots on the left and the gear on
//! the right. Geometry and drawing only; `notch.rs` decides when to use them. Sizes are in device-independent pixels.

use crate::{
    gfx::{Canvas, Color, Point, Rect},
    ui::Icon,
};

pub const PADDING_TOP: f32 = 14.0;
pub const PADDING_SIDE: f32 = 16.0;
pub const PADDING_BOTTOM: f32 = 14.0;
pub const HEADER_HEIGHT: f32 = 16.0;
pub const HEADER_GAP: f32 = 12.0;
/// What the chrome adds to the width and the height of a page's own size.
pub const WIDTH_EXTRA: f32 = 2.0 * PADDING_SIDE;
pub const HEIGHT_EXTRA: f32 = PADDING_TOP + HEADER_HEIGHT + HEADER_GAP + PADDING_BOTTOM;

const DOT_SIZE: f32 = 6.0;
const ACTIVE_DOT_WIDTH: f32 = 16.0;
const DOT_GAP: f32 = 4.0;
/// How far from a dot a click still counts as on it.
const DOT_HIT_SLACK: f32 = 5.0;
const GEAR_BUTTON: f32 = 20.0;
/// The gear button sits this far past the right padding, so its glyph lines up with the page's right edge.
const GEAR_OVERHANG: f32 = 3.0;
const GEAR_SIZE: f32 = 14.0;
const DOT_INACTIVE: Color = Color::rgb(0.227, 0.227, 0.267);
const DOT_ACTIVE: Color = Color::rgb(0.9, 0.9, 0.92);
const GEAR: Color = Color::rgb(0.43, 0.43, 0.48);
const GEAR_HOVERED: Color = Color::rgb(0.8, 0.8, 0.84);

/// Where a page draws: the open notch minus the padding and the header. `pill` is the whole notch in device pixels.
pub fn content_rect(pill: Rect, scale: f32) -> Rect {
    let left = pill.left + PADDING_SIDE * scale;
    let top = pill.top + (PADDING_TOP + HEADER_HEIGHT + HEADER_GAP) * scale;
    Rect::new(left, top, (pill.right - PADDING_SIDE * scale).max(left), (pill.bottom - PADDING_BOTTOM * scale).max(top))
}

/// The header row across the top of the open notch.
pub fn header_rect(pill: Rect, scale: f32) -> Rect {
    Rect::new(
        pill.left + PADDING_SIDE * scale,
        pill.top + PADDING_TOP * scale,
        (pill.right - PADDING_SIDE * scale).max(pill.left + PADDING_SIDE * scale),
        pill.top + (PADDING_TOP + HEADER_HEIGHT) * scale,
    )
}

fn header_center_y(pill: Rect, scale: f32) -> f32 {
    pill.top + (PADDING_TOP + HEADER_HEIGHT / 2.0) * scale
}

/// How long dot `position` of `count` is when the pages are at `track` (a page number that can fall between two): 1 on
/// the page, fading to 0 a page away, so two dots both stretch in the middle of a swipe. The row of pages is a loop, so
/// the last dot is next to the first.
fn stretch(position: usize, count: usize, track: f32) -> f32 {
    let count = count as f32;
    let distance = (position as f32 - track).rem_euclid(count);
    (1.0 - distance.min(count - distance)).clamp(0.0, 1.0)
}

/// The dots for `count` pages, left to right; the ones near `track` are long.
pub fn dot_rects(count: usize, track: f32, pill: Rect, scale: f32) -> Vec<Rect> {
    let center_y = header_center_y(pill, scale);
    let mut left = pill.left + PADDING_SIDE * scale;
    (0..count)
        .map(|position| {
            let width = (DOT_SIZE + (ACTIVE_DOT_WIDTH - DOT_SIZE) * stretch(position, count, track)) * scale;
            let rect =
                Rect::new(left, center_y - DOT_SIZE * scale / 2.0, left + width, center_y + DOT_SIZE * scale / 2.0);
            left += width + DOT_GAP * scale;
            rect
        })
        .collect()
}

pub fn draw_dots(canvas: &Canvas, dots: &[Rect], track: f32, opacity: f32) {
    for (position, rect) in dots.iter().enumerate() {
        let amount = stretch(position, dots.len(), track);
        let color = Color::rgb(
            DOT_INACTIVE.r + (DOT_ACTIVE.r - DOT_INACTIVE.r) * amount,
            DOT_INACTIVE.g + (DOT_ACTIVE.g - DOT_INACTIVE.g) * amount,
            DOT_INACTIVE.b + (DOT_ACTIVE.b - DOT_INACTIVE.b) * amount,
        );
        canvas.fill_rounded_rect(*rect, rect.height() / 2.0, color.faded(opacity));
    }
}

/// The dot a press at `point` lands on, with a little slack around each.
pub fn dot_at(dots: &[Rect], point: Point, scale: f32) -> Option<usize> {
    let slack = DOT_HIT_SLACK * scale;
    dots.iter().position(|rect| {
        point.x >= rect.left - slack
            && point.x <= rect.right + slack
            && (point.y - rect.center_y()).abs() <= 8.0 * scale
    })
}

/// The gear button's clickable square.
pub fn gear_button(pill: Rect, scale: f32) -> Rect {
    let right = pill.right - (PADDING_SIDE - GEAR_OVERHANG) * scale;
    let center_y = header_center_y(pill, scale);
    Rect::new(
        right - GEAR_BUTTON * scale,
        center_y - GEAR_BUTTON * scale / 2.0,
        right,
        center_y + GEAR_BUTTON * scale / 2.0,
    )
}

pub fn draw_gear(canvas: &Canvas, pill: Rect, scale: f32, is_hovered: bool, opacity: f32) {
    let button = gear_button(pill, scale);
    let color = if is_hovered { GEAR_HOVERED } else { GEAR };
    Icon::Gear.draw(canvas, Point::new(button.center_x(), button.center_y()), GEAR_SIZE * scale, color, opacity);
}

#[cfg(test)]
mod tests {
    use super::*;

    const PILL: Rect = Rect::new(100.0, 0.0, 440.0, 160.0);

    #[test]
    fn the_content_sits_inside_the_padding_and_below_the_header() {
        let content = content_rect(PILL, 1.0);
        assert_eq!(content.left, 116.0);
        assert_eq!(content.right, 424.0);
        assert_eq!(content.top, 14.0 + 16.0 + 12.0);
        assert_eq!(content.bottom, 160.0 - 14.0);
    }

    #[test]
    fn a_pill_sized_for_a_page_gives_the_page_exactly_its_size() {
        let (width, height) = (300.0, 80.0);
        let pill = Rect::new(0.0, 0.0, width + WIDTH_EXTRA, height + HEIGHT_EXTRA);
        let content = content_rect(pill, 1.0);
        assert_eq!((content.width(), content.height()), (width, height));
    }

    #[test]
    fn the_content_never_has_a_negative_size_while_the_pill_is_small() {
        let content = content_rect(Rect::new(0.0, 0.0, 10.0, 10.0), 1.0);
        assert!(content.width() >= 0.0 && content.height() >= 0.0);
    }

    #[test]
    fn the_active_dot_is_long_and_the_dots_are_spaced_evenly() {
        let dots = dot_rects(3, 1.0, PILL, 1.0);
        assert_eq!(dots[0].width(), 6.0);
        assert_eq!(dots[1].width(), 16.0);
        assert_eq!(dots[1].left - dots[0].right, 4.0);
        assert_eq!(dots[2].left - dots[1].right, 4.0);
        assert_eq!(dots[0].left, 116.0);
    }

    #[test]
    fn the_dots_are_centered_in_the_header_row() {
        let dots = dot_rects(2, 0.0, PILL, 1.0);
        assert_eq!(dots[0].center_y(), 14.0 + 8.0);
    }

    #[test]
    fn a_press_near_a_dot_picks_it_and_one_elsewhere_picks_none() {
        let dots = dot_rects(3, 0.0, PILL, 1.0);
        assert_eq!(dot_at(&dots, Point::new(dots[2].center_x() + 6.0, dots[2].center_y()), 1.0), Some(2));
        assert_eq!(dot_at(&dots, Point::new(300.0, 22.0), 1.0), None);
        assert_eq!(dot_at(&dots, Point::new(dots[0].center_x(), 60.0), 1.0), None);
    }

    #[test]
    fn the_gear_sits_at_the_right_end_of_the_header() {
        let gear = gear_button(PILL, 1.0);
        assert!(gear.right > PILL.right - PADDING_SIDE && gear.right <= PILL.right);
        assert_eq!(gear.center_y(), 22.0);
        assert!(gear.contains(Point::new(gear.center_x(), gear.center_y())));
    }

    #[test]
    fn everything_scales_with_the_display() {
        let single = dot_rects(2, 0.0, PILL, 1.0);
        let double = dot_rects(2, 0.0, Rect::new(100.0, 0.0, 440.0, 160.0), 2.0);
        assert!((double[0].width() - 2.0 * single[0].width()).abs() < 0.01);
    }

    #[test]
    fn the_dots_loop_so_the_last_one_stretches_with_the_first() {
        // Between the last of three pages and the first (position 2.5), both of those dots are half long.
        let dots = dot_rects(3, 2.5, PILL, 1.0);
        assert_eq!(dots[0].width(), dots[2].width());
        assert!(dots[0].width() > dots[1].width());
    }
}
