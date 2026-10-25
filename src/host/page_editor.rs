//! The "Edit pages" view, opened with the settings button: the pages that are in the notch as chips to drag into a new order or
//! double-click to remove, and the pages that are available to click and add. Layout and the drag rules are pure
//! functions and a small state machine, so they are tested without a window; `notch.rs` draws and applies them.
//! Sizes are in device-independent pixels.

use crate::{
    gfx::{Align, Canvas, Color, FontStyle, Point, Rect, UI_FONT, Weight},
    ui::Icon,
};

const LABEL_HEIGHT: f32 = 12.0;
const CHIP_HEIGHT: f32 = 28.0;
const CHIP_GAP: f32 = 8.0;
const ROW_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 10.0;
const LABEL_GAP: f32 = 6.0;
/// Space inside a chip around its icon and name.
const CHIP_PADDING: f32 = 10.0;
const CHIP_ICON: f32 = 14.0;
const CHIP_ICON_GAP: f32 = 6.0;
const DONE_WIDTH: f32 = 54.0;
const DONE_HEIGHT: f32 = 24.0;
/// Two presses on a chip within this time, without dragging, remove it.
const DOUBLE_CLICK_MILLISECONDS: u64 = 450;
/// A press that moves less than this is a click, not a drag.
const DRAG_THRESHOLD: f32 = 4.0;

pub const TITLE: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Medium, Align::Leading);
const LABEL: FontStyle = FontStyle::new(UI_FONT, 10.0, Weight::Medium, Align::Leading);
pub const CHIP: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Regular, Align::Leading);
const DONE: FontStyle = FontStyle::new(UI_FONT, 11.0, Weight::Medium, Align::Center);

const CHIP_FILL: Color = Color::rgb(0.125, 0.125, 0.153);
const CHIP_FILL_DRAGGED: Color = Color::rgb(0.184, 0.227, 0.282);
const CHIP_TEXT: Color = Color::rgb(0.839, 0.839, 0.871);
const OFF_BORDER: Color = Color::rgb(0.227, 0.227, 0.267);
const OFF_TEXT: Color = Color::rgb(0.541, 0.541, 0.588);
const LABEL_COLOR: Color = Color::rgb(0.431, 0.431, 0.478);

pub const IN_NOTCH_LABEL: &str = "IN THE ISLAND \u{00b7} DRAG TO REORDER \u{00b7} DOUBLE-CLICK TO REMOVE";
pub const AVAILABLE_LABEL: &str = "AVAILABLE \u{00b7} CLICK TO ADD";

/// One page as the editor shows it.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub page: usize,
    pub is_on: bool,
    /// Width of the chip in device-independent pixels: its padding, icon and name.
    pub chip_width: f32,
}

/// The width of a chip for a name that is `text_width` wide.
pub fn chip_width(text_width: f32) -> f32 {
    2.0 * CHIP_PADDING + CHIP_ICON + CHIP_ICON_GAP + text_width.ceil()
}

/// Where everything is, in device pixels.
pub struct Layout {
    pub in_label: Rect,
    pub available_label: Rect,
    pub on_chips: Vec<(usize, Rect)>,
    pub off_chips: Vec<(usize, Rect)>,
    pub title: Rect,
    pub done: Rect,
    /// The height the content needs, in device-independent pixels.
    pub content_height: f32,
}

/// Lays chips left to right and wraps them into rows no wider than `width`; positions are relative to the top-left.
fn flow(widths: &[f32], width: f32, top: f32) -> (Vec<Rect>, f32) {
    let (mut x, mut y) = (0.0, top);
    let mut rects = Vec::with_capacity(widths.len());
    for &chip in widths {
        if x > 0.0 && x + chip > width {
            x = 0.0;
            y += CHIP_HEIGHT + ROW_GAP;
        }
        rects.push(Rect::new(x, y, x + chip, y + CHIP_HEIGHT));
        x += chip + CHIP_GAP;
    }
    (rects, if widths.is_empty() { top } else { y + CHIP_HEIGHT })
}

/// Lays the editor out in `content` (the notch below its header) and `header` (the row at the top).
pub fn layout(entries: &[Entry], content: Rect, header: Rect, scale: f32) -> Layout {
    let width = content.width() / scale;
    let on: Vec<&Entry> = entries.iter().filter(|entry| entry.is_on).collect();
    let off: Vec<&Entry> = entries.iter().filter(|entry| !entry.is_on).collect();
    let on_widths: Vec<f32> = on.iter().map(|entry| entry.chip_width).collect();
    let off_widths: Vec<f32> = off.iter().map(|entry| entry.chip_width).collect();

    let (on_rects, on_bottom) = flow(&on_widths, width, LABEL_HEIGHT + LABEL_GAP);
    let available_top = on_bottom + SECTION_GAP;
    let (off_rects, off_bottom) = flow(&off_widths, width, available_top + LABEL_HEIGHT + LABEL_GAP);
    let content_height = if off.is_empty() { on_bottom } else { off_bottom };

    let place = |rect: Rect| {
        Rect::new(
            content.left + rect.left * scale,
            content.top + rect.top * scale,
            content.left + rect.right * scale,
            content.top + rect.bottom * scale,
        )
    };
    let label = |top: f32| place(Rect::new(0.0, top, width, top + LABEL_HEIGHT));
    Layout {
        in_label: label(0.0),
        available_label: label(available_top),
        on_chips: on.iter().zip(on_rects).map(|(entry, rect)| (entry.page, place(rect))).collect(),
        off_chips: off.iter().zip(off_rects).map(|(entry, rect)| (entry.page, place(rect))).collect(),
        title: Rect::new(header.left, header.top, header.right - (DONE_WIDTH + 12.0) * scale, header.bottom),
        done: Rect::new(
            header.right - DONE_WIDTH * scale,
            header.center_y() - DONE_HEIGHT * scale / 2.0,
            header.right,
            header.center_y() + DONE_HEIGHT * scale / 2.0,
        ),
        content_height,
    }
}

/// Where a dragged chip lands: the place in the order of the chip nearest to `point`.
pub fn drop_position(chips: &[(usize, Rect)], point: Point) -> usize {
    let distance = |rect: &Rect| {
        let (dx, dy) = (rect.center_x() - point.x, (rect.center_y() - point.y) * 3.0);
        dx * dx + dy * dy
    };
    chips
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| distance(&a.1).total_cmp(&distance(&b.1)))
        .map_or(0, |(position, _)| position)
}

/// What a press, move or release asks the notch to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Nothing,
    /// Leave the editor.
    Done,
    /// Put the page at this position among the pages that are on.
    Move {
        page: usize,
        position: usize,
    },
    /// Take the page out of the notch.
    Remove(usize),
    /// Put the page into the notch.
    Add(usize),
}

struct Drag {
    page: usize,
    start: Point,
    pointer: Point,
    is_moving: bool,
}

#[derive(Default)]
pub struct PageEditor {
    drag: Option<Drag>,
    last_click: Option<(usize, u64)>,
}

impl PageEditor {
    pub fn press(&mut self, layout: &Layout, point: Point, now: u64) -> Outcome {
        if layout.done.contains(point) {
            return Outcome::Done;
        }
        if let Some((page, _)) = layout.off_chips.iter().find(|(_, rect)| rect.contains(point)) {
            return Outcome::Add(*page);
        }
        if let Some((page, _)) = layout.on_chips.iter().find(|(_, rect)| rect.contains(point)) {
            let is_double_click = self
                .last_click
                .is_some_and(|(last, at)| last == *page && now.saturating_sub(at) <= DOUBLE_CLICK_MILLISECONDS);
            if is_double_click {
                self.last_click = None;
                return Outcome::Remove(*page);
            }
            self.last_click = Some((*page, now));
            self.drag = Some(Drag { page: *page, start: point, pointer: point, is_moving: false });
        }
        Outcome::Nothing
    }

    /// Follows the pointer while a chip is held; returns true when something should be redrawn.
    pub fn moved(&mut self, point: Point) -> bool {
        let Some(drag) = self.drag.as_mut() else { return false };
        drag.pointer = point;
        let travelled = (point.x - drag.start.x).hypot(point.y - drag.start.y);
        drag.is_moving |= travelled > DRAG_THRESHOLD;
        drag.is_moving
    }

    pub fn release(&mut self, layout: &Layout, point: Point) -> Outcome {
        let Some(drag) = self.drag.take() else { return Outcome::Nothing };
        if !drag.is_moving {
            return Outcome::Nothing;
        }
        // A drag cancels a double-click that could have followed.
        self.last_click = None;
        Outcome::Move { page: drag.page, position: drop_position(&layout.on_chips, point) }
    }

    /// True while a chip is held down, before it is known whether it will be dragged.
    pub fn is_holding(&self) -> bool {
        self.drag.is_some()
    }

    #[cfg(test)]
    pub fn is_dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|drag| drag.is_moving)
    }

    /// The page being dragged and where its chip is drawn, as an offset from where it sits.
    pub fn dragged(&self) -> Option<(usize, Point)> {
        let drag = self.drag.as_ref().filter(|drag| drag.is_moving)?;
        Some((drag.page, Point::new(drag.pointer.x - drag.start.x, drag.pointer.y - drag.start.y)))
    }
}

/// What the notch draws a chip with.
pub struct ChipView<'a> {
    pub name: &'a str,
    pub icon: Icon,
}

pub fn draw_labels(canvas: &Canvas, layout: &Layout, opacity: f32) {
    let faded = LABEL_COLOR.faded(opacity);
    canvas.text(&LABEL, IN_NOTCH_LABEL, layout.in_label, faded);
    if !layout.off_chips.is_empty() {
        canvas.text(&LABEL, AVAILABLE_LABEL, layout.available_label, faded);
    }
}

pub fn draw_chip(canvas: &Canvas, rect: Rect, view: &ChipView, is_on: bool, is_dragged: bool, opacity: f32) {
    let scale = canvas.scale();
    let radius = rect.height() / 2.0;
    let (text_color, icon_color) = if is_on { (CHIP_TEXT, CHIP_TEXT) } else { (OFF_TEXT, OFF_TEXT) };
    if is_on {
        let fill = if is_dragged { CHIP_FILL_DRAGGED } else { CHIP_FILL };
        canvas.fill_rounded_rect(rect, radius, fill.faded(opacity));
    } else {
        crate::ui::draw_dashed_outline(
            canvas,
            rect,
            radius,
            scale,
            4.0 * scale,
            3.0 * scale,
            OFF_BORDER.faded(opacity),
        );
    }
    let icon_center = Point::new(rect.left + (CHIP_PADDING + CHIP_ICON / 2.0) * scale, rect.center_y());
    view.icon.draw(canvas, icon_center, CHIP_ICON * scale, icon_color, opacity);
    let text_left = rect.left + (CHIP_PADDING + CHIP_ICON + CHIP_ICON_GAP) * scale;
    canvas.text(&CHIP, view.name, Rect::new(text_left, rect.top, rect.right, rect.bottom), text_color.faded(opacity));
}

pub fn draw_done(canvas: &Canvas, layout: &Layout, opacity: f32) {
    canvas.fill_rounded_rect(layout.done, layout.done.height() / 2.0, Color::WHITE.faded(opacity));
    canvas.text(&DONE, "Done", layout.done, Color::BLACK.faded(opacity));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(on: &[(usize, f32)], off: &[(usize, f32)]) -> Vec<Entry> {
        on.iter()
            .map(|&(page, chip_width)| Entry { page, is_on: true, chip_width })
            .chain(off.iter().map(|&(page, chip_width)| Entry { page, is_on: false, chip_width }))
            .collect()
    }

    fn laid_out(entries: &[Entry]) -> Layout {
        layout(entries, Rect::new(0.0, 40.0, 428.0, 200.0), Rect::new(0.0, 14.0, 428.0, 30.0), 1.0)
    }

    #[test]
    fn chips_sit_side_by_side_while_they_fit() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0), (2, 80.0)], &[]));
        let rects: Vec<Rect> = layout.on_chips.iter().map(|(_, rect)| *rect).collect();
        assert_eq!(rects[0].top, rects[2].top);
        assert_eq!(rects[1].left - rects[0].right, CHIP_GAP);
    }

    #[test]
    fn chips_wrap_to_a_new_row_when_they_do_not_fit() {
        let layout = laid_out(&entries(&[(0, 200.0), (1, 200.0), (2, 200.0)], &[]));
        let rects: Vec<Rect> = layout.on_chips.iter().map(|(_, rect)| *rect).collect();
        assert_eq!(rects[0].top, rects[1].top);
        assert_eq!(rects[2].left, 0.0);
        assert_eq!(rects[2].top - rects[0].top, CHIP_HEIGHT + ROW_GAP);
    }

    #[test]
    fn the_available_section_appears_below_the_chips_only_when_there_are_pages_to_add() {
        let none = laid_out(&entries(&[(0, 80.0)], &[]));
        let some = laid_out(&entries(&[(0, 80.0)], &[(1, 80.0)]));
        assert!(some.content_height > none.content_height);
        assert!(some.off_chips[0].1.top > some.on_chips[0].1.bottom);
        assert!(some.available_label.bottom <= some.off_chips[0].1.top);
    }

    #[test]
    fn the_done_button_is_at_the_right_end_of_the_header() {
        let layout = laid_out(&entries(&[(0, 80.0)], &[]));
        assert_eq!(layout.done.right, 428.0);
        assert!(layout.title.right < layout.done.left);
    }

    #[test]
    fn the_nearest_chip_decides_where_a_dragged_chip_lands() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0), (2, 80.0)], &[]));
        let near_last = Point::new(layout.on_chips[2].1.center_x() + 10.0, layout.on_chips[2].1.center_y());
        assert_eq!(drop_position(&layout.on_chips, near_last), 2);
        assert_eq!(drop_position(&layout.on_chips, Point::new(-50.0, 60.0)), 0);
        assert_eq!(drop_position(&[], Point::new(0.0, 0.0)), 0);
    }

    fn middle(rect: Rect) -> Point {
        Point::new(rect.center_x(), rect.center_y())
    }

    #[test]
    fn done_leaves_and_an_available_chip_is_added() {
        let layout = laid_out(&entries(&[(0, 80.0)], &[(1, 80.0)]));
        let mut editor = PageEditor::default();
        assert_eq!(editor.press(&layout, middle(layout.done), 0), Outcome::Done);
        assert_eq!(editor.press(&layout, middle(layout.off_chips[0].1), 0), Outcome::Add(1));
    }

    #[test]
    fn dragging_a_chip_to_another_place_moves_it() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0), (2, 80.0)], &[]));
        let mut editor = PageEditor::default();
        assert_eq!(editor.press(&layout, middle(layout.on_chips[0].1), 0), Outcome::Nothing);
        let target = middle(layout.on_chips[2].1);
        assert!(editor.moved(target));
        assert!(editor.is_dragging());
        assert_eq!(editor.release(&layout, target), Outcome::Move { page: 0, position: 2 });
        assert!(!editor.is_dragging());
    }

    #[test]
    fn a_press_that_barely_moves_is_a_click_not_a_drag() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0)], &[]));
        let mut editor = PageEditor::default();
        let start = middle(layout.on_chips[0].1);
        editor.press(&layout, start, 0);
        assert!(!editor.moved(Point::new(start.x + 2.0, start.y)));
        assert_eq!(editor.release(&layout, start), Outcome::Nothing);
    }

    #[test]
    fn a_quick_second_press_on_a_chip_removes_it() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0)], &[]));
        let mut editor = PageEditor::default();
        let chip = middle(layout.on_chips[1].1);
        editor.press(&layout, chip, 1000);
        editor.release(&layout, chip);
        assert_eq!(editor.press(&layout, chip, 1300), Outcome::Remove(1));
    }

    #[test]
    fn a_slow_second_press_or_one_on_another_chip_does_not_remove() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0)], &[]));
        let mut editor = PageEditor::default();
        let first = middle(layout.on_chips[0].1);
        editor.press(&layout, first, 1000);
        editor.release(&layout, first);
        assert_eq!(editor.press(&layout, first, 2000), Outcome::Nothing);
        editor.release(&layout, first);
        assert_eq!(editor.press(&layout, middle(layout.on_chips[1].1), 2100), Outcome::Nothing);
    }

    #[test]
    fn the_dragged_chip_follows_the_pointer() {
        let layout = laid_out(&entries(&[(0, 80.0), (1, 80.0)], &[]));
        let mut editor = PageEditor::default();
        let start = middle(layout.on_chips[0].1);
        editor.press(&layout, start, 0);
        editor.moved(Point::new(start.x + 30.0, start.y + 5.0));
        let (page, offset) = editor.dragged().unwrap();
        assert_eq!((page, offset.x, offset.y), (0, 30.0, 5.0));
    }

    #[test]
    fn a_chip_is_as_wide_as_its_padding_icon_and_name() {
        assert_eq!(chip_width(40.2), 2.0 * CHIP_PADDING + CHIP_ICON + CHIP_ICON_GAP + 41.0);
    }
}
