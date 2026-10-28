use super::controls::Control;
use crate::{
    gfx::{Point, Rect, lerp},
    page::Frame,
};

// Layout converts content DIP to device pixels; the host adds padding and the header.
/// Must match the fixed CONTENT_HEIGHT in host/chrome.rs.
pub const PLAIN_HEIGHT: f32 = 86.0;
pub const TEXT_MAX_WIDTH: f32 = 180.0;
pub const TEXT_MIN_WIDTH: f32 = 140.0;
const ARTWORK_SIZE: f32 = 86.0;
const ARTWORK_RADIUS: f32 = 14.0;
const TEXT_GAP: f32 = 19.0;
pub fn plain_width(text_width: f32) -> f32 {
    ARTWORK_SIZE + TEXT_GAP + text_width
}
pub const LYRICS_WIDTH: f32 = 582.0;
const COLUMN_WIDTH_WITH_LYRICS: f32 = 160.0;
pub const TITLE_MAX_LINES: usize = 2;
const LINE_HEIGHT: f32 = 16.0;
const DETAIL_GAP: f32 = 2.0;
const CONTROLS_GAP: f32 = 6.0;
const BUTTON_SIZE: f32 = 30.0;
const BUTTON_GAP: f32 = 2.0;
const BUTTON_OUTSET: f32 = 6.0;
const LYRIC_GAP: f32 = 8.0;
pub const LYRIC_MAX_LINES: usize = 2;
const BADGE_SIZE: f32 = 19.0;
const BADGE_RADIUS: f32 = 5.0;
const BADGE_OVERHANG: f32 = 4.0;
const BADGE_RING: f32 = 3.0;

pub fn lyric_panel_width() -> f32 {
    LYRICS_WIDTH - (ARTWORK_SIZE + TEXT_GAP + COLUMN_WIDTH_WITH_LYRICS + LYRIC_GAP)
}

pub fn title_width(has_lyrics: bool) -> f32 {
    if has_lyrics { COLUMN_WIDTH_WITH_LYRICS } else { TEXT_MAX_WIDTH }
}

pub struct Layout {
    pub scale: f32,
    pub bounds: Rect,
    blend: f32,
    title_lines: usize,
}

impl Layout {
    pub fn new(frame: &Frame, title_lines: usize, text_width: f32) -> Self {
        let plain = plain_width(text_width);
        let blend = ((frame.expanded.width - plain) / (LYRICS_WIDTH - plain)).clamp(0.0, 1.0);
        Self { scale: frame.scale, bounds: frame.bounds, blend, title_lines: title_lines.clamp(1, TITLE_MAX_LINES) }
    }

    pub fn lyric_blend(&self) -> f32 {
        self.blend
    }

    fn stack_top(&self) -> f32 {
        let height = self.title_lines as f32 * LINE_HEIGHT + DETAIL_GAP + LINE_HEIGHT + CONTROLS_GAP + BUTTON_SIZE;
        (PLAIN_HEIGHT - height) / 2.0
    }

    pub fn artwork(&self) -> Rect {
        Rect::new(
            self.bounds.left,
            self.bounds.top,
            self.bounds.left + ARTWORK_SIZE * self.scale,
            self.bounds.top + ARTWORK_SIZE * self.scale,
        )
    }

    pub fn artwork_radius(&self) -> f32 {
        ARTWORK_RADIUS * self.scale
    }

    pub fn app_badge(&self) -> Rect {
        let artwork = self.artwork();
        let right = artwork.right + BADGE_OVERHANG * self.scale;
        let bottom = artwork.bottom + BADGE_OVERHANG * self.scale;
        Rect::new(right - BADGE_SIZE * self.scale, bottom - BADGE_SIZE * self.scale, right, bottom)
    }

    pub fn app_badge_radius(&self) -> f32 {
        BADGE_RADIUS * self.scale
    }

    pub fn app_badge_ring(&self) -> (Rect, f32) {
        let badge = self.app_badge();
        let ring = BADGE_RING * self.scale;
        let rect = Rect::new(badge.left - ring, badge.top - ring, badge.right + ring, badge.bottom + ring);
        (rect, self.app_badge_radius() + ring)
    }

    fn text_left(&self) -> f32 {
        self.bounds.left + (ARTWORK_SIZE + TEXT_GAP) * self.scale
    }

    fn text_right(&self) -> f32 {
        let with_lyrics = self.text_left() + COLUMN_WIDTH_WITH_LYRICS * self.scale;
        lerp(self.bounds.right, with_lyrics, self.blend).min(self.bounds.right)
    }

    fn row(&self, top: f32) -> Rect {
        let top = self.bounds.top + (self.stack_top() + top) * self.scale;
        Rect::new(self.text_left(), top, self.text_right(), top + LINE_HEIGHT * self.scale)
    }

    pub fn title_line(&self, index: usize) -> Rect {
        self.row(index as f32 * LINE_HEIGHT)
    }

    pub fn detail(&self) -> Rect {
        self.row(self.title_lines as f32 * LINE_HEIGHT + DETAIL_GAP)
    }

    pub fn control_center(&self, control: Control) -> Point {
        let top = self.stack_top()
            + self.title_lines as f32 * LINE_HEIGHT
            + DETAIL_GAP
            + LINE_HEIGHT
            + CONTROLS_GAP
            + BUTTON_SIZE / 2.0;
        let x = self.text_left()
            + (-BUTTON_OUTSET + BUTTON_SIZE / 2.0 + control.slot() * (BUTTON_SIZE + BUTTON_GAP)) * self.scale;
        Point::new(x, self.bounds.top + top * self.scale)
    }

    pub fn lyric_panel(&self) -> Rect {
        let left = self.text_left() + (COLUMN_WIDTH_WITH_LYRICS + LYRIC_GAP) * self.scale;
        Rect::new(left, self.bounds.top, self.bounds.right, self.bounds.top + PLAIN_HEIGHT * self.scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Size;

    const TEXT_WIDTH: f32 = 180.0;
    const PLAIN_WIDTH: f32 = ARTWORK_SIZE + TEXT_GAP + TEXT_WIDTH;

    fn frame(expanded_width: f32) -> Frame {
        Frame {
            scale: 2.0,
            bounds: Rect::new(100.0, 0.0, 100.0 + PLAIN_WIDTH * 2.0, PLAIN_HEIGHT * 2.0),
            expansion: 1.0,
            expanded: Size::new(expanded_width, PLAIN_HEIGHT),
        }
    }

    fn plain(title_lines: usize) -> Layout {
        Layout::new(&frame(PLAIN_WIDTH), title_lines, TEXT_WIDTH)
    }

    #[test]
    fn the_artwork_is_a_square_as_high_as_the_page_at_its_left_edge() {
        let artwork = plain(2).artwork();
        assert_eq!(artwork.left, 100.0);
        assert_eq!(artwork.width(), artwork.height());
        assert_eq!(artwork.height(), PLAIN_HEIGHT * 2.0);
    }

    #[test]
    fn the_app_icon_overlaps_the_artwork_corner_and_sticks_out_of_it() {
        let layout = plain(2);
        let (artwork, badge) = (layout.artwork(), layout.app_badge());
        assert!(badge.right > artwork.right && badge.bottom > artwork.bottom);
        assert!(badge.left < artwork.right && badge.top < artwork.bottom);
        let (ring, _) = layout.app_badge_ring();
        assert!(ring.left < badge.left && ring.bottom > badge.bottom);
    }

    #[test]
    fn the_badge_and_its_rim_stay_clear_of_the_text() {
        let layout = plain(2);
        let (ring, _) = layout.app_badge_ring();
        assert!(ring.right < layout.title_line(0).left);
    }

    #[test]
    fn two_title_lines_fill_the_page_and_stack_above_the_detail_and_the_buttons() {
        let layout = plain(2);
        assert_eq!(layout.title_line(0).top, layout.bounds.top);
        assert!(layout.title_line(0).bottom <= layout.title_line(1).top);
        assert!(layout.title_line(1).bottom <= layout.detail().top);
        let button = layout.control_center(Control::PlayPause);
        assert!(button.y > layout.detail().bottom);
        assert!(button.y < layout.bounds.bottom);
    }

    #[test]
    fn a_one_line_title_leaves_equal_space_above_and_below_the_text() {
        let layout = plain(1);
        let above = layout.title_line(0).top - layout.bounds.top;
        let buttons_bottom = layout.control_center(Control::PlayPause).y + BUTTON_SIZE * layout.scale / 2.0;
        let below = layout.bounds.bottom - buttons_bottom;
        assert!((above - below).abs() < 0.01);
        assert!(above > 0.0);
    }

    #[test]
    fn a_title_of_any_line_count_stays_inside_the_page() {
        for lines in [0, 1, 2, 5] {
            let layout = plain(lines);
            assert!(layout.title_line(0).top >= layout.bounds.top);
            assert!(layout.control_center(Control::Lyrics).y < layout.bounds.bottom);
        }
    }

    #[test]
    fn the_text_column_is_at_most_the_text_width_without_lyrics_and_narrows_with_them() {
        let without = plain(2);
        let with = Layout::new(&frame(LYRICS_WIDTH), 2, TEXT_WIDTH);
        assert_eq!(without.title_line(0).width(), TEXT_WIDTH * 2.0);
        assert!(with.title_line(0).right < with.lyric_panel().left);
        assert_eq!(with.title_line(0).width(), COLUMN_WIDTH_WITH_LYRICS * 2.0);
    }

    #[test]
    fn the_page_is_exactly_as_wide_as_the_artwork_the_gap_and_the_text() {
        let layout = plain(2);
        assert_eq!(layout.title_line(0).right, layout.bounds.right);
    }

    #[test]
    fn the_controls_follow_each_other_left_to_right_under_the_text() {
        let layout = plain(2);
        let centers: Vec<Point> = Control::ALL.iter().map(|&control| layout.control_center(control)).collect();
        assert!(centers.windows(2).all(|pair| pair[0].x < pair[1].x));
        assert!(centers.iter().all(|center| center.y > layout.detail().bottom));
    }

    #[test]
    fn the_lyric_panel_starts_a_gap_after_the_text_and_ends_at_the_page_edge() {
        let wide = Frame {
            bounds: Rect::new(100.0, 0.0, 100.0 + LYRICS_WIDTH * 2.0, PLAIN_HEIGHT * 2.0),
            ..frame(LYRICS_WIDTH)
        };
        let layout = Layout::new(&wide, 2, TEXT_WIDTH);
        assert_eq!(layout.lyric_panel().left - layout.title_line(0).right, LYRIC_GAP * 2.0);
        assert_eq!(layout.lyric_panel().right, layout.bounds.right);
        assert_eq!(layout.lyric_panel().width() / 2.0, lyric_panel_width());
    }

    #[test]
    fn the_layout_changes_gradually_while_the_pill_widens() {
        let halfway = (PLAIN_WIDTH + LYRICS_WIDTH) / 2.0;
        let (narrow, middle, wide) = (
            Layout::new(&frame(PLAIN_WIDTH), 2, TEXT_WIDTH).title_line(0).right,
            Layout::new(&frame(halfway), 2, TEXT_WIDTH).title_line(0).right,
            Layout::new(&frame(LYRICS_WIDTH), 2, TEXT_WIDTH).title_line(0).right,
        );
        assert!(narrow > middle && middle > wide);
    }
}
