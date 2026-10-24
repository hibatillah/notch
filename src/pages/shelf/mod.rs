//! The file shelf: a place to keep files for later. Drop files on the notch to put them on the shelf; open the page
//! to open them again, or right-click one for more. The files stay where they are; the shelf keeps their paths. When
//! files are added or removed, the collapsed pill says so for a moment.

mod items;

use std::path::PathBuf;

use self::items::{Shelf, added_text, category, count_text, file_name, removed_text, tile_color, tile_label};
use crate::{
    activity::{FLASH_TIMER, Flash, Presence},
    gfx::{Align, Canvas, Color, FontStyle, Point, Rect, SEGOE_UI, Size, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    platform,
    ui::{self, Icon},
};

const EXPANDED_SIZE: Size = Size::new(508.0, 124.0);
const RESTING_WIDTH: f32 = 150.0;
const ZONE_RADIUS: f32 = 14.0;
const BORDER_WIDTH: f32 = 1.5;
const ZONE_PADDING: f32 = 12.0;
const FILE_WIDTH: f32 = 76.0;
const FILE_GAP: f32 = 12.0;
const TILE_WIDTH: f32 = 64.0;
const TILE_HEIGHT: f32 = 44.0;
const TILE_RADIUS: f32 = 8.0;
const NAME_TOP: f32 = 6.0;
const NAME_HEIGHT: f32 = 28.0;
/// Files shown at once; with more, the last place says how many more there are.
const MAX_SLOTS: usize = 4;
const CLEAR_WIDTH: f32 = 40.0;
const CLEAR_HEIGHT: f32 = 20.0;
const ALERT_MILLISECONDS: u32 = 2000;

const TILE_LABEL: FontStyle = FontStyle::new("Consolas", 10.0, Weight::SemiBold, Align::Center);
const NAME: FontStyle = FontStyle::new(SEGOE_UI, 10.0, Weight::Regular, Align::Center).wrapped();
const CLEAR: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::SemiBold, Align::Center);
const HINT: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Center);

const BORDER: Color = Color::rgb(0.227, 0.227, 0.267);
const NAME_COLOR: Color = Color::rgb(0.769, 0.769, 0.8);
const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const WASH: Color = Color::rgb(1.0, 1.0, 1.0);

/// What a place on the shelf shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    File(usize),
    /// A place that stands for this many files that do not fit.
    More(usize),
}

/// The places to draw for `count` files: all of them while they fit, else the first ones and a "more" place.
fn slots(count: usize) -> Vec<Slot> {
    if count <= MAX_SLOTS {
        (0..count).map(Slot::File).collect()
    } else {
        (0..MAX_SLOTS - 1).map(Slot::File).chain(std::iter::once(Slot::More(count - (MAX_SLOTS - 1)))).collect()
    }
}

/// The column of one place: its tile and its name below.
fn slot_rect(position: usize, bounds: Rect, scale: f32) -> Rect {
    let left = bounds.left + (ZONE_PADDING + position as f32 * (FILE_WIDTH + FILE_GAP)) * scale;
    let top = bounds.top + ZONE_PADDING * scale;
    Rect::new(left, top, left + FILE_WIDTH * scale, top + (TILE_HEIGHT + NAME_TOP + NAME_HEIGHT) * scale)
}

fn tile_rect(column: Rect, scale: f32) -> Rect {
    let left = column.left + (FILE_WIDTH - TILE_WIDTH) / 2.0 * scale;
    Rect::new(left, column.top, left + TILE_WIDTH * scale, column.top + TILE_HEIGHT * scale)
}

fn clear_rect(bounds: Rect, scale: f32) -> Rect {
    Rect::new(
        bounds.right - (ZONE_PADDING + CLEAR_WIDTH) * scale,
        bounds.top + (ZONE_PADDING - 2.0) * scale,
        bounds.right - ZONE_PADDING * scale,
        bounds.top + (ZONE_PADDING - 2.0 + CLEAR_HEIGHT) * scale,
    )
}

/// What is under `point`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Slot(usize),
    Clear,
}

fn target_at(point: Point, bounds: Rect, scale: f32, count: usize) -> Option<Target> {
    if count > 0 && clear_rect(bounds, scale).contains(point) {
        return Some(Target::Clear);
    }
    (0..slots(count).len()).find(|&position| slot_rect(position, bounds, scale).contains(point)).map(Target::Slot)
}

pub struct ShelfPage {
    shelf: Shelf,
    storage: Option<PathBuf>,
    alert: Flash,
    alert_title: String,
    alert_value: String,
    alert_width: f32,
    hovered: Option<Target>,
}

impl ShelfPage {
    pub fn new() -> Self {
        let storage =
            std::env::var_os("APPDATA").map(|directory| PathBuf::from(directory).join("notch").join("shelf.txt"));
        Self {
            shelf: Shelf::default(),
            storage,
            alert: Flash::default(),
            alert_title: String::new(),
            alert_value: String::new(),
            alert_width: 0.0,
            hovered: None,
        }
    }

    fn save(&self) {
        if let Some(storage) = &self.storage {
            self.shelf.save(storage);
        }
    }

    fn announce(&mut self, title: String, context: &mut Context) {
        self.alert_value = count_text(self.shelf.len());
        self.alert_width = ui::message_width(context.fonts(), &title, Some(&self.alert_value));
        self.alert_title = title;
        self.alert.show(context, ALERT_MILLISECONDS);
    }

    fn remove(&mut self, index: usize, context: &mut Context) {
        if let Some(path) = self.shelf.remove(index) {
            self.save();
            self.announce(removed_text(&file_name(&path)), context);
        }
    }

    fn draw_tile(&self, canvas: &Canvas, column: Rect, slot: Slot, is_hovered: bool, opacity: f32) {
        let scale = canvas.scale();
        if is_hovered {
            canvas.fill_rounded_rect(column, 8.0 * scale, WASH.with_alpha(0.07 * opacity));
        }
        let tile = tile_rect(column, scale);
        let name_area = Rect::new(column.left, tile.bottom + NAME_TOP * scale, column.right, column.bottom);
        match slot {
            Slot::File(index) => {
                let path = &self.shelf.items()[index];
                let is_folder = path.is_dir();
                let color = tile_color(category(path, is_folder));
                canvas.fill_rounded_rect(tile, TILE_RADIUS * scale, color.faded(opacity));
                if is_folder {
                    Icon::Folder.draw(
                        canvas,
                        Point::new(tile.center_x(), tile.center_y()),
                        22.0 * scale,
                        Color::WHITE,
                        opacity,
                    );
                } else {
                    canvas.text(&TILE_LABEL, &tile_label(path, false), tile, Color::WHITE.faded(opacity));
                }
                canvas.text(&NAME, &file_name(path), name_area, NAME_COLOR.faded(opacity));
            }
            Slot::More(count) => {
                canvas.fill_rounded_rect(tile, TILE_RADIUS * scale, Color::gray(0.16).faded(opacity));
                canvas.text(&TILE_LABEL, &format!("+{count}"), tile, Color::WHITE.faded(opacity));
                canvas.text(&NAME, "more", name_area, NAME_COLOR.faded(opacity));
            }
        }
    }
}

impl Page for ShelfPage {
    fn id(&self) -> &'static str {
        "shelf"
    }

    fn name(&self) -> &'static str {
        "File shelf"
    }

    fn icon(&self) -> Icon {
        Icon::Folder
    }

    fn priority(&self, mode: Mode) -> Option<Priority> {
        (mode == Mode::Expanded).then_some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let (bounds, scale) = (frame.bounds, frame.scale);
        // The zone is an outline: the border color with the notch's black inside it.
        canvas.fill_rounded_rect(bounds, ZONE_RADIUS * scale, BORDER.faded(opacity));
        let inner = Rect::new(
            bounds.left + BORDER_WIDTH * scale,
            bounds.top + BORDER_WIDTH * scale,
            bounds.right - BORDER_WIDTH * scale,
            bounds.bottom - BORDER_WIDTH * scale,
        );
        canvas.fill_rounded_rect(inner, (ZONE_RADIUS - BORDER_WIDTH) * scale, Color::BLACK.faded(opacity));

        if self.shelf.len() == 0 {
            let center = Point::new(bounds.center_x(), bounds.center_y() - 12.0 * scale);
            Icon::Folder.draw(canvas, center, 26.0 * scale, GRAY, opacity);
            let hint = Rect::new(bounds.left, center.y + 18.0 * scale, bounds.right, center.y + 40.0 * scale);
            canvas.text(&HINT, "Drop files here to keep them for later", hint, NAME_COLOR.faded(opacity));
            return;
        }
        for (position, slot) in slots(self.shelf.len()).into_iter().enumerate() {
            let is_hovered = self.hovered == Some(Target::Slot(position));
            self.draw_tile(canvas, slot_rect(position, bounds, scale), slot, is_hovered, opacity);
        }
        let color = if self.hovered == Some(Target::Clear) { Color::WHITE } else { GRAY };
        canvas.text(&CLEAR, "Clear", clear_rect(bounds, scale), color.faded(opacity));
    }

    fn presence(&self) -> Option<Presence> {
        self.alert.is_visible().then_some(Presence::Transient { width: self.alert_width })
    }

    fn draw_presence(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        ui::draw_message(canvas, area, opacity, Icon::Folder, Color::WHITE, &self.alert_title, Some(&self.alert_value));
    }

    fn accepts_drops(&self) -> bool {
        true
    }

    fn on_drop(&mut self, paths: Vec<PathBuf>, context: &mut Context) {
        let added = self.shelf.add(paths);
        if added > 0 {
            self.save();
            self.announce(added_text(added), context);
        }
        context.request_repaint();
    }

    fn on_start(&mut self, _context: &mut Context) {
        if let Some(storage) = &self.storage {
            self.shelf = Shelf::load(storage);
        }
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let (bounds, scale, count) = (frame.bounds, frame.scale, self.shelf.len());
        match event {
            PointerEvent::Moved(point) => {
                let hovered = target_at(point, bounds, scale, count);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Pressed(point) if frame.is_open() => match target_at(point, bounds, scale, count) {
                Some(Target::Clear) => {
                    self.shelf.clear();
                    self.save();
                    context.request_repaint();
                }
                Some(Target::Slot(position)) => {
                    if let Some(Slot::File(index)) = slots(count).get(position).copied() {
                        platform::open(&self.shelf.items()[index].display().to_string());
                    }
                }
                None => {}
            },
            PointerEvent::SecondaryReleased(point) if frame.is_open() => {
                let Some(Target::Slot(position)) = target_at(point, bounds, scale, count) else { return };
                let Some(Slot::File(index)) = slots(count).get(position).copied() else { return };
                let path = self.shelf.items()[index].clone();
                match context.show_menu(&["Open", "Show in folder", "Copy path", "Remove"]) {
                    Some(0) => platform::open(&path.display().to_string()),
                    Some(1) => platform::reveal(&path),
                    Some(2) => {
                        platform::copy_text(&path.display().to_string());
                    }
                    Some(3) => self.remove(index, context),
                    _ => {}
                }
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                let was_hovered = self.hovered.take().is_some();
                if was_hovered {
                    context.request_repaint();
                }
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if timer == FLASH_TIMER {
            self.alert.on_timer(timer, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::new(0.0, 0.0, 508.0, 124.0);

    #[test]
    fn a_few_files_each_get_a_place() {
        assert!(slots(0).is_empty());
        assert_eq!(slots(3), vec![Slot::File(0), Slot::File(1), Slot::File(2)]);
        assert_eq!(slots(MAX_SLOTS).len(), MAX_SLOTS);
    }

    #[test]
    fn too_many_files_end_in_a_place_that_counts_the_rest() {
        let places = slots(9);
        assert_eq!(places.len(), MAX_SLOTS);
        assert_eq!(places[MAX_SLOTS - 1], Slot::More(9 - (MAX_SLOTS - 1)));
        assert_eq!(places[0], Slot::File(0));
    }

    #[test]
    fn the_places_sit_side_by_side_inside_the_zone() {
        let first = slot_rect(0, BOUNDS, 1.0);
        let second = slot_rect(1, BOUNDS, 1.0);
        assert_eq!(second.left - first.right, FILE_GAP);
        assert!(slot_rect(MAX_SLOTS - 1, BOUNDS, 1.0).right + CLEAR_WIDTH <= BOUNDS.right);
    }

    #[test]
    fn a_press_finds_the_file_or_the_clear_button_under_it() {
        let place = slot_rect(1, BOUNDS, 1.0);
        let inside = Point::new(place.center_x(), place.center_y());
        assert_eq!(target_at(inside, BOUNDS, 1.0, 3), Some(Target::Slot(1)));
        let clear = clear_rect(BOUNDS, 1.0);
        assert_eq!(target_at(Point::new(clear.center_x(), clear.center_y()), BOUNDS, 1.0, 3), Some(Target::Clear));
        assert_eq!(target_at(Point::new(300.0, 100.0), BOUNDS, 1.0, 3), None);
    }

    #[test]
    fn an_empty_shelf_has_nothing_to_press() {
        let clear = clear_rect(BOUNDS, 1.0);
        assert_eq!(target_at(Point::new(clear.center_x(), clear.center_y()), BOUNDS, 1.0, 0), None);
        assert_eq!(target_at(Point::new(40.0, 40.0), BOUNDS, 1.0, 0), None);
    }

    #[test]
    fn the_shelf_is_a_page_to_open_that_takes_drops() {
        let page = ShelfPage::new();
        assert!(page.accepts_drops());
        assert_eq!(page.priority(Mode::Resting), None);
        assert!(page.priority(Mode::Expanded).is_some());
        assert_eq!(page.presence(), None);
    }
}
