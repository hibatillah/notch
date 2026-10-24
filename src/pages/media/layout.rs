use super::controls::Control;
use crate::{
    gfx::{Point, Rect, lerp},
    page::Frame,
    ui::ProgressBar,
};

// Sizes are in device-independent pixels; `Layout` converts to device pixels.
pub const PLAIN_WIDTH: f32 = 420.0;
/// While the lyric panel is open the controls column shrinks, which leaves the panel the larger share.
const COLUMN_WIDTH_WITH_LYRICS: f32 = 300.0;
const LYRIC_PANEL_WIDTH: f32 = 400.0;
pub const LYRICS_WIDTH: f32 = COLUMN_WIDTH_WITH_LYRICS + LYRIC_PANEL_WIDTH;
pub const EXPANDED_HEIGHT: f32 = 136.0;

pub const CONTENT_PADDING: f32 = 20.0;
const ARTWORK_SIZE: f32 = 52.0;
const ARTWORK_TOP: f32 = 14.0;
const ARTWORK_RADIUS: f32 = 10.0;
const TEXT_GAP: f32 = 12.0;
const CORNER_ICON_SIZE: f32 = 18.0;
const BAR_CENTER_Y: f32 = 84.0;
/// Room on the right of the bar for the remaining-time label.
const BAR_RIGHT_INSET: f32 = 64.0;
const BUTTON_CENTER_Y: f32 = 111.0;
const BUTTON_STEP_PLAIN: f32 = 46.0;
const BUTTON_STEP_WITH_LYRICS: f32 = 35.0;
const LYRIC_PANEL_PADDING: f32 = 24.0;

/// Where everything in the expanded view goes, in device pixels, for the current animation frame.
pub struct Layout {
    pub scale: f32,
    pub bounds: Rect,
    column_width: f32,
    button_step: f32,
}

impl Layout {
    pub fn new(frame: &Frame) -> Self {
        let lyric_blend = ((frame.expanded.width - PLAIN_WIDTH) / (LYRICS_WIDTH - PLAIN_WIDTH)).clamp(0.0, 1.0);
        Self {
            scale: frame.scale,
            bounds: frame.bounds,
            column_width: lerp(PLAIN_WIDTH, COLUMN_WIDTH_WITH_LYRICS, lyric_blend) * frame.scale,
            button_step: lerp(BUTTON_STEP_PLAIN, BUTTON_STEP_WITH_LYRICS, lyric_blend) * frame.scale,
        }
    }

    /// Right edge of the controls column, which is the pill's right edge unless the lyric panel is open.
    pub fn column_right(&self) -> f32 {
        self.bounds.left + self.column_width
    }

    pub fn artwork(&self) -> Rect {
        let left = self.bounds.left + CONTENT_PADDING * self.scale;
        let top = self.bounds.top + ARTWORK_TOP * self.scale;
        Rect::new(left, top, left + ARTWORK_SIZE * self.scale, top + ARTWORK_SIZE * self.scale)
    }

    pub fn artwork_radius(&self) -> f32 {
        ARTWORK_RADIUS * self.scale
    }

    pub fn title(&self) -> Rect {
        self.text_row(17.0, 39.0)
    }

    pub fn artist(&self) -> Rect {
        self.text_row(39.0, 59.0)
    }

    /// The app icon sits above the remaining-time label, right-aligned with it.
    pub fn corner_icon(&self) -> Rect {
        let size = CORNER_ICON_SIZE * self.scale;
        let right = self.column_right() - CONTENT_PADDING * self.scale;
        let top = self.bounds.top + ARTWORK_TOP * self.scale;
        Rect::new(right - size, top, right, top + size)
    }

    pub fn progress_bar(&self) -> ProgressBar {
        ProgressBar {
            left: self.bounds.left + CONTENT_PADDING * self.scale,
            right: self.column_right() - BAR_RIGHT_INSET * self.scale,
            center_y: self.bounds.top + BAR_CENTER_Y * self.scale,
            scale: self.scale,
        }
    }

    pub fn remaining_label(&self) -> Rect {
        let bar = self.progress_bar();
        let half_height = 10.0 * self.scale;
        Rect::new(
            bar.right + 6.0 * self.scale,
            bar.center_y - half_height,
            self.column_right() - CONTENT_PADDING * self.scale,
            bar.center_y + half_height,
        )
    }

    pub fn control_center(&self, control: Control) -> Point {
        let center_x = self.bounds.left + self.column_width / 2.0;
        Point::new(center_x + control.slot() * self.button_step, self.bounds.top + BUTTON_CENTER_Y * self.scale)
    }

    pub fn lyric_panel(&self) -> Rect {
        Rect::new(
            self.column_right() + 8.0 * self.scale,
            self.bounds.top + 4.0 * self.scale,
            self.bounds.right - LYRIC_PANEL_PADDING * self.scale,
            self.bounds.bottom - 4.0 * self.scale,
        )
    }

    /// A row of text next to the artwork, between two heights measured from the pill's top.
    fn text_row(&self, top: f32, bottom: f32) -> Rect {
        let left = self.bounds.left + (CONTENT_PADDING + ARTWORK_SIZE + TEXT_GAP) * self.scale;
        // Keep text clear of the app icon above the remaining-time label.
        let right = self.column_right() - (CONTENT_PADDING + CORNER_ICON_SIZE + 8.0) * self.scale;
        Rect::new(left, self.bounds.top + top * self.scale, right, self.bounds.top + bottom * self.scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::Size;

    fn frame(expanded_width: f32) -> Frame {
        Frame {
            scale: 2.0,
            bounds: Rect::new(100.0, 0.0, 100.0 + expanded_width * 2.0, EXPANDED_HEIGHT * 2.0),
            expansion: 1.0,
            expanded: Size::new(expanded_width, EXPANDED_HEIGHT),
        }
    }

    #[test]
    fn without_lyrics_the_controls_use_the_full_width() {
        let layout = Layout::new(&frame(PLAIN_WIDTH));
        assert_eq!(layout.column_right(), 100.0 + PLAIN_WIDTH * 2.0);
    }

    #[test]
    fn with_lyrics_the_controls_column_narrows() {
        let layout = Layout::new(&frame(LYRICS_WIDTH));
        assert_eq!(layout.column_right(), 100.0 + COLUMN_WIDTH_WITH_LYRICS * 2.0);
    }

    #[test]
    fn the_column_narrows_gradually_while_the_pill_widens() {
        let halfway = (PLAIN_WIDTH + LYRICS_WIDTH) / 2.0;
        let plain = Layout::new(&frame(PLAIN_WIDTH)).column_right();
        let middle = Layout::new(&frame(halfway)).column_right();
        let wide = Layout::new(&frame(LYRICS_WIDTH)).column_right();
        assert!(plain > middle && middle > wide);
    }

    #[test]
    fn controls_follow_each_other_left_to_right() {
        let layout = Layout::new(&frame(PLAIN_WIDTH));
        let xs: Vec<f32> = Control::ALL.iter().map(|&control| layout.control_center(control).x).collect();
        assert!(xs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn the_progress_bar_stays_inside_the_controls_column() {
        let layout = Layout::new(&frame(LYRICS_WIDTH));
        let bar = layout.progress_bar();
        assert!(bar.left < bar.right);
        assert!(bar.right < layout.column_right());
    }

    #[test]
    fn the_lyric_panel_starts_after_the_controls_column() {
        let layout = Layout::new(&frame(LYRICS_WIDTH));
        assert!(layout.lyric_panel().left > layout.column_right());
        assert!(layout.lyric_panel().right <= layout.bounds.right);
    }
}
