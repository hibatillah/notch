use super::controls::Control;
use crate::{
    gfx::{Point, Rect, lerp},
    page::Frame,
};

// Sizes are in device-independent pixels; `Layout` converts to device pixels. They are the size of the page's content,
// which the notch surrounds with its padding and header.
/// The height of the open player, with and without lyrics. The notch gives every open page this height, so it must
/// equal `CONTENT_HEIGHT` in `host/chrome.rs`.
pub const PLAIN_HEIGHT: f32 = 86.0;
/// The most room the title and the artist get; the title wraps to fit it.
pub const TEXT_MAX_WIDTH: f32 = 180.0;
/// The least room the text column gets: enough for the buttons.
pub const TEXT_MIN_WIDTH: f32 = 140.0;
/// The artwork is as high as the page, which has room for two title lines, the detail line and the buttons.
const ARTWORK_SIZE: f32 = 86.0;
const ARTWORK_RADIUS: f32 = 14.0;
const TEXT_GAP: f32 = 19.0;
/// Without lyrics the page is the artwork and a column of text and buttons, as wide as its widest text.
pub fn plain_width(text_width: f32) -> f32 {
    ARTWORK_SIZE + TEXT_GAP + text_width
}
/// With lyrics: the player on the left, three lyric lines on the right.
pub const LYRICS_WIDTH: f32 = 582.0;
/// The text column beside the lyrics.
const COLUMN_WIDTH_WITH_LYRICS: f32 = 160.0;
/// The title can use this many lines; what does not fit ends with an ellipsis.
pub const TITLE_MAX_LINES: usize = 2;
const LINE_HEIGHT: f32 = 16.0;
const DETAIL_GAP: f32 = 2.0;
const CONTROLS_GAP: f32 = 6.0;
const BUTTON_SIZE: f32 = 30.0;
const BUTTON_GAP: f32 = 2.0;
/// The first button starts left of the text, so its icon lines up with the text above it.
const BUTTON_OUTSET: f32 = 6.0;
/// The empty space between the player and the lyrics.
const LYRIC_GAP: f32 = 8.0;
/// A lyric line can take this many lines of text; what does not fit ends with an ellipsis.
pub const LYRIC_MAX_LINES: usize = 2;
/// The icon of the app that plays the track, at the artwork's bottom right corner and partly outside it.
const BADGE_SIZE: f32 = 19.0;
const BADGE_RADIUS: f32 = 5.0;
/// How far the badge sticks out past the artwork's right and bottom edges.
const BADGE_OVERHANG: f32 = 4.0;
/// The black rim around the badge that separates it from the artwork.
const BADGE_RING: f32 = 3.0;

/// The width of the lyric panel, in device-independent pixels.
pub fn lyric_panel_width() -> f32 {
    LYRICS_WIDTH - (ARTWORK_SIZE + TEXT_GAP + COLUMN_WIDTH_WITH_LYRICS + LYRIC_GAP)
}

/// How wide a line of the title can be, in device-independent pixels.
pub fn title_width(has_lyrics: bool) -> f32 {
    if has_lyrics { COLUMN_WIDTH_WITH_LYRICS } else { TEXT_MAX_WIDTH }
}

/// Where everything in the expanded view goes, in device pixels, for the current animation frame.
pub struct Layout {
    pub scale: f32,
    pub bounds: Rect,
    /// 0 without lyrics, 1 with them; in between while the pill changes width.
    blend: f32,
    /// How many lines the title takes, one or two; the text beside the artwork is centered on whatever it adds up to.
    title_lines: usize,
}

impl Layout {
    /// `text_width` is the width of the text column without lyrics, which sets how wide the page is then.
    pub fn new(frame: &Frame, title_lines: usize, text_width: f32) -> Self {
        let plain = plain_width(text_width);
        let blend = ((frame.expanded.width - plain) / (LYRICS_WIDTH - plain)).clamp(0.0, 1.0);
        Self { scale: frame.scale, bounds: frame.bounds, blend, title_lines: title_lines.clamp(1, TITLE_MAX_LINES) }
    }

    /// The part of the lyrics layout that is in effect, from 0 to 1.
    pub fn lyric_blend(&self) -> f32 {
        self.blend
    }

    /// Top of the title, from the top of the content: the title, the detail line and the buttons are centered together.
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

    /// The app icon square, overlapping the artwork's bottom right corner.
    pub fn app_badge(&self) -> Rect {
        let artwork = self.artwork();
        let right = artwork.right + BADGE_OVERHANG * self.scale;
        let bottom = artwork.bottom + BADGE_OVERHANG * self.scale;
        Rect::new(right - BADGE_SIZE * self.scale, bottom - BADGE_SIZE * self.scale, right, bottom)
    }

    pub fn app_badge_radius(&self) -> f32 {
        BADGE_RADIUS * self.scale
    }

    /// The black rim around the badge, and its corner radius.
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

    /// One line of the title, counting from 0.
    pub fn title_line(&self, index: usize) -> Rect {
        self.row(index as f32 * LINE_HEIGHT)
    }

    /// The artist, under the title.
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
