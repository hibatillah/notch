use super::controls::Control;
use crate::{
    gfx::{Point, Rect, lerp},
    page::Frame,
};

// Sizes are in device-independent pixels; `Layout` converts to device pixels. They are the size of the page's content,
// which the notch surrounds with its padding and header.
/// Without lyrics: the artwork and a column of text and controls.
pub const PLAIN_WIDTH: f32 = 308.0;
/// With lyrics: the player on the left, three lyric lines on the right.
pub const LYRICS_WIDTH: f32 = 568.0;
/// Three lines of text and the controls under them set the height without lyrics.
pub const PLAIN_HEIGHT: f32 = 82.0;
/// With lyrics the text is two lines, so the artwork sets the height.
pub const LYRICS_HEIGHT: f32 = 72.0;

const ARTWORK_SIZE: f32 = 72.0;
const ARTWORK_RADIUS: f32 = 12.0;
const TEXT_GAP_PLAIN: f32 = 12.0;
const TEXT_GAP_LYRICS: f32 = 16.0;
/// The text column beside the lyrics.
const COLUMN_WIDTH_WITH_LYRICS: f32 = 160.0;
const LINE_HEIGHT: f32 = 16.0;
const BUTTON_SIZE: f32 = 30.0;
const BUTTON_GAP: f32 = 2.0;
/// The first button starts left of the text, so its icon lines up with the text above it.
const BUTTON_OUTSET: f32 = 6.0;
const LYRIC_GAP: f32 = 16.0;

/// Where everything in the expanded view goes, in device pixels, for the current animation frame.
pub struct Layout {
    pub scale: f32,
    pub bounds: Rect,
    /// 0 without lyrics, 1 with them; in between while the pill changes width.
    blend: f32,
}

impl Layout {
    pub fn new(frame: &Frame) -> Self {
        let blend = ((frame.expanded.width - PLAIN_WIDTH) / (LYRICS_WIDTH - PLAIN_WIDTH)).clamp(0.0, 1.0);
        Self { scale: frame.scale, bounds: frame.bounds, blend }
    }

    /// The part of the lyrics layout that is in effect, from 0 to 1.
    pub fn lyric_blend(&self) -> f32 {
        self.blend
    }

    fn content_height(&self) -> f32 {
        lerp(PLAIN_HEIGHT, LYRICS_HEIGHT, self.blend)
    }

    /// Top of the text column, from the top of the content.
    fn column_top(&self) -> f32 {
        lerp(0.0, 2.0, self.blend)
    }

    pub fn artwork(&self) -> Rect {
        let top = self.bounds.top + (self.content_height() - ARTWORK_SIZE) / 2.0 * self.scale;
        Rect::new(self.bounds.left, top, self.bounds.left + ARTWORK_SIZE * self.scale, top + ARTWORK_SIZE * self.scale)
    }

    pub fn artwork_radius(&self) -> f32 {
        ARTWORK_RADIUS * self.scale
    }

    fn text_left(&self) -> f32 {
        self.bounds.left + (ARTWORK_SIZE + lerp(TEXT_GAP_PLAIN, TEXT_GAP_LYRICS, self.blend)) * self.scale
    }

    fn text_right(&self) -> f32 {
        let plain = self.bounds.right;
        let with_lyrics = self.text_left() + COLUMN_WIDTH_WITH_LYRICS * self.scale;
        lerp(plain, with_lyrics.min(plain), self.blend)
    }

    fn row(&self, top: f32) -> Rect {
        let top = self.bounds.top + (self.column_top() + top) * self.scale;
        Rect::new(self.text_left(), top, self.text_right(), top + LINE_HEIGHT * self.scale)
    }

    pub fn title(&self) -> Rect {
        self.row(0.0)
    }

    /// The album, shown only without lyrics.
    pub fn album(&self) -> Rect {
        self.row(LINE_HEIGHT)
    }

    pub fn artist(&self) -> Rect {
        self.row(lerp(2.0 * LINE_HEIGHT, LINE_HEIGHT, self.blend))
    }

    pub fn control_center(&self, control: Control) -> Point {
        let top = self.column_top() + lerp(3.0 * LINE_HEIGHT + 4.0, 2.0 * LINE_HEIGHT + 6.0, self.blend);
        let x = self.text_left()
            + (-BUTTON_OUTSET + BUTTON_SIZE / 2.0 + control.slot() * (BUTTON_SIZE + BUTTON_GAP)) * self.scale;
        Point::new(x, self.bounds.top + (top + BUTTON_SIZE / 2.0) * self.scale)
    }

    /// The thin line between the player and the lyrics.
    pub fn divider(&self) -> Rect {
        let left = self.text_left() + (COLUMN_WIDTH_WITH_LYRICS + LYRIC_GAP) * self.scale;
        Rect::new(left, self.bounds.top, left + self.scale, self.bounds.top + self.content_height() * self.scale)
    }

    pub fn lyric_panel(&self) -> Rect {
        let divider = self.divider();
        Rect::new(divider.right + LYRIC_GAP * self.scale, divider.top, self.bounds.right, divider.bottom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Size;

    fn frame(expanded_width: f32) -> Frame {
        Frame {
            scale: 2.0,
            bounds: Rect::new(100.0, 0.0, 100.0 + PLAIN_WIDTH * 2.0, PLAIN_HEIGHT * 2.0),
            expansion: 1.0,
            expanded: Size::new(expanded_width, PLAIN_HEIGHT),
        }
    }

    #[test]
    fn the_artwork_sits_at_the_left_edge_and_is_square() {
        let layout = Layout::new(&frame(PLAIN_WIDTH));
        let artwork = layout.artwork();
        assert_eq!(artwork.left, 100.0);
        assert_eq!(artwork.width(), artwork.height());
        assert_eq!(artwork.width(), 144.0);
    }

    #[test]
    fn the_text_column_fills_the_width_without_lyrics_and_narrows_with_them() {
        let plain = Layout::new(&frame(PLAIN_WIDTH));
        let lyrics = Layout::new(&frame(LYRICS_WIDTH));
        assert_eq!(plain.title().right, plain.bounds.right);
        assert!(lyrics.title().right < lyrics.divider().left);
        assert_eq!(lyrics.title().width(), COLUMN_WIDTH_WITH_LYRICS * 2.0);
    }

    #[test]
    fn the_title_album_and_artist_stack_without_lyrics() {
        let layout = Layout::new(&frame(PLAIN_WIDTH));
        assert!(layout.title().bottom <= layout.album().top);
        assert!(layout.album().bottom <= layout.artist().top);
    }

    #[test]
    fn with_lyrics_the_artist_moves_up_into_the_albums_place() {
        let layout = Layout::new(&frame(LYRICS_WIDTH));
        assert_eq!(layout.artist().top, layout.album().top);
    }

    #[test]
    fn the_controls_sit_under_the_text_and_follow_each_other_left_to_right() {
        let layout = Layout::new(&frame(PLAIN_WIDTH));
        let centers: Vec<Point> = Control::ALL.iter().map(|&control| layout.control_center(control)).collect();
        assert!(centers.windows(2).all(|pair| pair[0].x < pair[1].x));
        assert!(centers.iter().all(|center| center.y > layout.artist().bottom));
        assert!(centers.iter().all(|center| center.y < layout.bounds.bottom));
    }

    #[test]
    fn the_lyric_panel_starts_after_the_divider_and_ends_at_the_page_edge() {
        let layout = Layout::new(&frame(LYRICS_WIDTH));
        assert!(layout.lyric_panel().left > layout.divider().right);
        assert_eq!(layout.lyric_panel().right, layout.bounds.right);
    }

    #[test]
    fn the_layout_changes_gradually_while_the_pill_widens() {
        let halfway = (PLAIN_WIDTH + LYRICS_WIDTH) / 2.0;
        let (plain, middle, wide) = (
            Layout::new(&frame(PLAIN_WIDTH)).title().right,
            Layout::new(&frame(halfway)).title().right,
            Layout::new(&frame(LYRICS_WIDTH)).title().right,
        );
        assert!(plain > middle && middle > wide);
    }
}
