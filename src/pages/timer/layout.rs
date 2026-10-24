//! Where everything in the open timer page goes. Sizes are in device-independent pixels from the page's top-left.

use crate::gfx::{Point, Rect, Size};

pub const EXPANDED_SIZE: Size = Size::new(380.0, 124.0);
// The window is 175 px high and the page dots take 16 px of it.
const _: () = assert!(EXPANDED_SIZE.height <= 159.0);
const PADDING: f32 = 16.0;
const TITLE_TOP: f32 = 14.0;
const TITLE_HEIGHT: f32 = 22.0;
const DETAIL_TOP: f32 = 38.0;
const DETAIL_HEIGHT: f32 = 20.0;
const TIME_TOP: f32 = 8.0;
const TIME_HEIGHT: f32 = 52.0;
/// Share of the width the title may use when a time sits beside it.
const TITLE_SHARE_WITH_TIME: f32 = 0.5;
const BUTTON_TOP: f32 = 74.0;
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

    pub fn title(&self, has_time: bool) -> Rect {
        let right = if has_time { self.width() * TITLE_SHARE_WITH_TIME } else { self.width() - PADDING };
        self.rect(PADDING, TITLE_TOP, right, TITLE_TOP + TITLE_HEIGHT)
    }

    pub fn detail(&self) -> Rect {
        self.rect(PADDING, DETAIL_TOP, self.width() - PADDING, DETAIL_TOP + DETAIL_HEIGHT)
    }

    pub fn time(&self) -> Rect {
        self.rect(self.width() * TITLE_SHARE_WITH_TIME, TIME_TOP, self.width() - PADDING, TIME_TOP + TIME_HEIGHT)
    }

    /// `count` buttons of equal width side by side across the page.
    pub fn buttons(&self, count: usize) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        let available = self.width() - 2.0 * PADDING - BUTTON_GAP * (count - 1) as f32;
        let button_width = available / count as f32;
        (0..count)
            .map(|index| {
                let left = PADDING + index as f32 * (button_width + BUTTON_GAP);
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
        assert!(rects[0].left > layout.bounds.left && rects[2].right < layout.bounds.right);
        assert!(rects[1].left > rects[0].right);
        assert!(rects.iter().all(|rect| rect.bottom < layout.bounds.bottom));
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
    fn the_title_leaves_room_for_the_time_only_when_there_is_one() {
        let layout = layout(1.0);
        assert!(layout.title(true).right <= layout.time().left);
        assert!(layout.title(false).right > layout.time().left);
    }

    #[test]
    fn everything_scales_with_the_display() {
        assert!((layout(2.0).buttons(3)[0].height() - 2.0 * layout(1.0).buttons(3)[0].height()).abs() < 0.01);
    }
}
