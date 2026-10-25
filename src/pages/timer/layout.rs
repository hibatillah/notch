//! Where everything in the open timer page goes. Sizes are in device-independent pixels from the page's top-left.

use crate::gfx::{Point, Rect, Size};

/// The page is as high as the open notch's fixed content area.
pub const EXPANDED_SIZE: Size = Size::new(290.0, 86.0);
// The window is 260 px high and the notch's padding and header take 50 px of it.
const _: () = assert!(EXPANDED_SIZE.height <= 190.0);
/// The top row holds the time and, beside it, the words, all sitting on the same bottom line.
const TOP_ROW_HEIGHT: f32 = 52.0;
/// Where the words end, a little above the bottom of the row so they sit on the baseline of the big time.
const WORDS_BOTTOM: f32 = 44.0;
const TITLE_HEIGHT: f32 = 18.0;
const DETAIL_HEIGHT: f32 = 16.0;
/// Room for the largest time ("25:00" in 40 px Consolas) and the gap before the words.
const TIME_WIDTH: f32 = 116.0;
const WORDS_GAP: f32 = 12.0;
const BUTTON_TOP: f32 = 56.0;
const BUTTON_HEIGHT: f32 = 30.0;
const BUTTON_GAP: f32 = 8.0;

pub struct Layout {
    pub scale: f32,
    /// The page in device pixels.
    pub bounds: Rect,
}

impl Layout {
    fn rect(&self, left: f32, top: f32, right: f32, bottom: f32) -> Rect {
        let (origin_x, origin_y, scale) = (self.bounds.left, self.bounds.top, self.scale);
        Rect::new(origin_x + left * scale, origin_y + top * scale, origin_x + right * scale, origin_y + bottom * scale)
    }

    fn width(&self) -> f32 {
        self.bounds.width() / self.scale
    }

    /// Where the words start: right of the time when there is one, else at the left edge.
    fn words_left(&self, has_time: bool) -> f32 {
        if has_time { TIME_WIDTH + WORDS_GAP } else { 0.0 }
    }

    /// The title, the upper line of the words.
    pub fn title(&self, has_time: bool, has_detail: bool) -> Rect {
        let bottom = if has_detail { WORDS_BOTTOM - DETAIL_HEIGHT } else { WORDS_BOTTOM };
        self.rect(self.words_left(has_time), bottom - TITLE_HEIGHT, self.width(), bottom)
    }

    /// The detail, under the title and on the bottom line of the row.
    pub fn detail(&self, has_time: bool) -> Rect {
        self.rect(self.words_left(has_time), WORDS_BOTTOM - DETAIL_HEIGHT, self.width(), WORDS_BOTTOM)
    }

    pub fn time(&self) -> Rect {
        self.rect(0.0, 0.0, TIME_WIDTH, TOP_ROW_HEIGHT)
    }

    /// `count` buttons of equal width side by side across the page.
    pub fn buttons(&self, count: usize) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        let available = self.width() - BUTTON_GAP * (count - 1) as f32;
        let button_width = available / count as f32;
        (0..count)
            .map(|index| {
                let left = index as f32 * (button_width + BUTTON_GAP);
                self.rect(left, BUTTON_TOP, left + button_width, BUTTON_TOP + BUTTON_HEIGHT)
            })
            .collect()
    }

    /// The button under `point`, by its position in the row.
    pub fn button_at(&self, count: usize, point: Point) -> Option<usize> {
        self.buttons(count).iter().position(|rect| rect.contains(point))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(scale: f32) -> Layout {
        Layout {
            scale,
            bounds: Rect::new(100.0, 0.0, 100.0 + EXPANDED_SIZE.width * scale, EXPANDED_SIZE.height * scale),
        }
    }

    #[test]
    fn the_buttons_share_the_width_evenly_and_fit_the_page() {
        let layout = layout(1.0);
        let rects = layout.buttons(3);
        assert_eq!(rects.len(), 3);
        assert!((rects[0].width() - rects[2].width()).abs() < 0.01);
        assert!(rects[0].left >= layout.bounds.left && rects[2].right <= layout.bounds.right + 0.01);
        assert!(rects[1].left > rects[0].right);
        assert!(rects.iter().all(|rect| rect.bottom <= layout.bounds.bottom));
    }

    #[test]
    fn two_buttons_are_wider_than_three() {
        let layout = layout(1.0);
        assert!(layout.buttons(2)[0].width() > layout.buttons(3)[0].width());
        assert!(layout.buttons(0).is_empty());
    }

    #[test]
    fn a_press_finds_the_button_under_it() {
        let layout = layout(1.0);
        let rects = layout.buttons(3);
        let inside = |rect: &Rect| Point::new(rect.center_x(), rect.center_y());
        assert_eq!(layout.button_at(3, inside(&rects[1])), Some(1));
        assert_eq!(layout.button_at(3, Point::new(0.0, 0.0)), None);
    }

    #[test]
    fn the_words_sit_right_of_the_time_only_when_there_is_one() {
        let layout = layout(1.0);
        assert!(layout.title(true, true).left >= layout.time().right);
        assert!(layout.detail(true).left >= layout.time().right);
        assert_eq!(layout.title(false, false).left, layout.bounds.left);
    }

    #[test]
    fn the_words_end_on_one_bottom_line_above_the_buttons() {
        let layout = layout(1.0);
        assert_eq!(layout.title(true, false).bottom, layout.detail(true).bottom);
        assert!(layout.title(true, true).bottom <= layout.detail(true).top);
        assert!(layout.detail(true).bottom <= layout.buttons(3)[0].top);
        assert!(layout.title(true, true).top >= layout.bounds.top);
    }

    #[test]
    fn the_buttons_end_at_the_bottom_of_the_page() {
        let layout = layout(1.0);
        assert_eq!(layout.buttons(3)[0].bottom, layout.bounds.bottom);
    }

    #[test]
    fn everything_scales_with_the_display() {
        assert!((layout(2.0).buttons(3)[0].height() - 2.0 * layout(1.0).buttons(3)[0].height()).abs() < 0.01);
    }
}
