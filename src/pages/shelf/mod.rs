//! The shelf stores paths; adding, dragging, and removing entries never moves or deletes the originals.

mod items;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use self::items::{Shelf, added_text, count_text, file_name, removed_text, tile_label};
use crate::{
    activity::{FLASH_TIMER, Flash, Presence},
    gfx::{
        Align, Canvas, Color, FontStyle, Fonts, ImageBrush, ImageData, Point, Rect, Size, Spring, UI_FONT, Weight,
        clamp_lines,
    },
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    platform,
    ui::{self, Icon},
};

const EXPANDED_WIDTH: f32 = 2.0 * ZONE_PADDING + MAX_SLOTS as f32 * FILE_WIDTH + (MAX_SLOTS - 1) as f32 * FILE_GAP;
const EXPANDED_SIZE: Size = Size::new(EXPANDED_WIDTH, 86.0);
const RESTING_WIDTH: f32 = 150.0;
const ZONE_RADIUS: f32 = 14.0;
const BORDER_WIDTH: f32 = 1.5;
const DASH: f32 = 10.0;
const DASH_GAP: f32 = 6.0;
const ZONE_PADDING: f32 = 9.5;
const FILE_WIDTH: f32 = 78.0;
const FILE_GAP: f32 = 4.0;
const PREVIEW_SIZE: f32 = 40.0;
const PREVIEW_RADIUS: f32 = 6.0;
const NAME_TOP: f32 = 3.0;
const NAME_LINE_HEIGHT: f32 = 12.0;
const NAME_LINES: usize = 2;
const NAME_WIDTH: f32 = FILE_WIDTH - 2.0;
const COLUMN_HEIGHT: f32 = PREVIEW_SIZE + NAME_TOP + NAME_LINE_HEIGHT * NAME_LINES as f32;
const WASH_REACH: f32 = 3.0;
/// Files shown at once; with more, the last place says how many more there are.
const MAX_SLOTS: usize = 6;
const REMOVE_RADIUS: f32 = 7.0;
const REMOVE_RING: f32 = 1.5;
const REMOVE_HIT_RADIUS: f32 = 9.0;
const DRAG_START_DISTANCE: f32 = 5.0;
/// Pixels asked of Explorer for each preview; enough for a 40 px tile on a screen scaled to 200 %.
const PREVIEW_PIXELS: i32 = 96;
const ALERT_MILLISECONDS: u32 = 2000;
const EMPTY_TITLE_HEIGHT: f32 = 17.0;
const EMPTY_DETAIL_HEIGHT: f32 = 14.0;
const EMPTY_GAP: f32 = 3.0;
const EMPTY_TEXT_HEIGHT: f32 = EMPTY_TITLE_HEIGHT + EMPTY_GAP + EMPTY_DETAIL_HEIGHT;
const ARRIVAL_RESPONSE: (f32, f32) = (200.0, 19.0);
const ARRIVAL_SCALE: f32 = 0.7;
const ARRIVAL_FALL: f32 = 12.0;
const ARRIVAL_STAGGER: f32 = 0.18;

const TILE_LABEL: FontStyle = FontStyle::new("Consolas", 10.0, Weight::Medium, Align::Center);
const NAME: FontStyle = FontStyle::new(UI_FONT, 10.0, Weight::Regular, Align::Center);
const EMPTY_TITLE: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Center);
const EMPTY_DETAIL: FontStyle = FontStyle::new(UI_FONT, 11.0, Weight::Regular, Align::Center);

const BORDER: Color = Color::rgb(0.227, 0.227, 0.267);
const ACCENT: Color = Color::rgb(0.25, 0.56, 1.0);
const NAME_COLOR: Color = Color::rgb(0.769, 0.769, 0.8);
const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const WASH: Color = Color::rgb(1.0, 1.0, 1.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    File(usize),
    More(usize),
}

fn slots(count: usize) -> Vec<Slot> {
    if count <= MAX_SLOTS {
        (0..count).map(Slot::File).collect()
    } else {
        (0..MAX_SLOTS - 1).map(Slot::File).chain(std::iter::once(Slot::More(count - (MAX_SLOTS - 1)))).collect()
    }
}

fn file_index(position: usize, count: usize) -> Option<usize> {
    match slots(count).get(position) {
        Some(Slot::File(index)) => Some(*index),
        _ => None,
    }
}

/// Keep slots centered while the drop target expands the page.
fn slot_rect(position: usize, bounds: Rect, scale: f32) -> Rect {
    let spare_width = (bounds.width() - EXPANDED_WIDTH * scale).max(0.0) / 2.0;
    let spare_height = (bounds.height() - (COLUMN_HEIGHT + 2.0 * ZONE_PADDING) * scale).max(0.0) / 2.0;
    let left = bounds.left + spare_width + (ZONE_PADDING + position as f32 * (FILE_WIDTH + FILE_GAP)) * scale;
    let top = bounds.top + spare_height + ZONE_PADDING * scale;
    Rect::new(left, top, left + FILE_WIDTH * scale, top + COLUMN_HEIGHT * scale)
}

fn preview_rect(column: Rect, scale: f32) -> Rect {
    let left = column.left + (FILE_WIDTH - PREVIEW_SIZE) / 2.0 * scale;
    Rect::new(left, column.top, left + PREVIEW_SIZE * scale, column.top + PREVIEW_SIZE * scale)
}

fn remove_center(preview: Rect, scale: f32) -> Point {
    Point::new(preview.right - scale, preview.top + scale)
}

fn fitted(size: (u32, u32), area: Rect) -> Rect {
    let (width, height) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let factor = (area.width() / width).min(area.height() / height);
    Rect::around(Point::new(area.center_x(), area.center_y()), width * factor / 2.0, height * factor / 2.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Slot(usize),
    Remove(usize),
}

fn target_at(point: Point, bounds: Rect, scale: f32, count: usize) -> Option<Target> {
    let places = slots(count);
    for (position, slot) in places.iter().enumerate() {
        if !matches!(slot, Slot::File(_)) {
            continue;
        }
        let center = remove_center(preview_rect(slot_rect(position, bounds, scale), scale), scale);
        if (point.x - center.x).hypot(point.y - center.y) <= REMOVE_HIT_RADIUS * scale {
            return Some(Target::Remove(position));
        }
    }
    (0..places.len()).find(|&position| slot_rect(position, bounds, scale).contains(point)).map(Target::Slot)
}

fn name_lines(name: &str, fonts: &Fonts) -> Vec<String> {
    clamp_lines(name, NAME_LINES, NAME_WIDTH, |text| fonts.measure(&NAME, text))
}

fn is_double_click(previous: Option<&(PathBuf, Instant)>, path: &Path, now: Instant, limit: Duration) -> bool {
    previous.is_some_and(|(last_path, at)| last_path == path && now.duration_since(*at) <= limit)
}

#[derive(Clone, Copy, Debug)]
struct Press {
    target: Target,
    origin: Point,
}

struct Preview {
    size: (u32, u32),
    waiting: Option<ImageData>,
    brush: Option<ImageBrush>,
}

#[derive(Default)]
struct Inbox {
    previews: Vec<(PathBuf, Option<ImageData>)>,
    drags: Vec<(PathBuf, bool)>,
}

pub struct ShelfPage {
    shelf: Shelf,
    storage: Option<PathBuf>,
    alert: Flash,
    alert_title: String,
    alert_value: String,
    alert_width: f32,
    hovered: Option<Target>,
    press: Option<Press>,
    is_drag_over: bool,
    dragging: Option<PathBuf>,
    names: Vec<Vec<String>>,
    previews: RefCell<HashMap<PathBuf, Preview>>,
    arrivals: HashMap<PathBuf, Spring>,
    last_click: Option<(PathBuf, Instant)>,
    requested: HashSet<PathBuf>,
    inbox: Arc<Mutex<Inbox>>,
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
            press: None,
            is_drag_over: false,
            dragging: None,
            names: Vec::new(),
            previews: RefCell::new(HashMap::new()),
            arrivals: HashMap::new(),
            last_click: None,
            requested: HashSet::new(),
            inbox: Arc::default(),
        }
    }

    #[cfg(test)]
    pub fn sample(directory: &Path, fonts: &Fonts, is_drag_over: bool) -> Self {
        let mut page = Self::new();
        page.storage = None;
        page.is_drag_over = is_drag_over;
        let files: Vec<PathBuf> =
            ["hover-state-v3.png", "wallpaper-1440p.jpg", "notch-brief.pdf", "budget-q4.xlsx", "notes.txt", "tasks.md"]
                .iter()
                .map(|name| {
                    let path = directory.join(name);
                    let _ = std::fs::write(&path, "x");
                    path
                })
                .collect();
        page.shelf.add(files);
        page.refresh_names(fonts);
        for path in page.shelf.items().iter().take(MAX_SLOTS) {
            if let Some(image) = platform::file_preview(path, PREVIEW_PIXELS) {
                page.previews.get_mut().insert(path.clone(), Preview::new(image));
            }
        }
        page
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

    fn refresh_names(&mut self, fonts: &Fonts) {
        self.names =
            self.shelf.items().iter().take(MAX_SLOTS).map(|path| name_lines(&file_name(path), fonts)).collect();
    }

    fn request_previews(&mut self, context: &Context) {
        let items = self.shelf.items();
        self.requested.retain(|path| items.contains(path));
        self.previews.get_mut().retain(|path, _| items.contains(path));
        let notifier = context.notifier();
        for path in items.iter().take(MAX_SLOTS) {
            if !self.requested.insert(path.clone()) {
                continue;
            }
            let (path, inbox) = (path.clone(), Arc::clone(&self.inbox));
            let _ = std::thread::Builder::new().name("shelf-preview".into()).spawn(move || {
                let image = platform::file_preview(&path, PREVIEW_PIXELS);
                if let Ok(mut inbox) = inbox.lock() {
                    inbox.previews.push((path, image));
                }
                notifier.notify();
            });
        }
    }

    fn shelf_changed(&mut self, context: &mut Context) {
        self.save();
        self.refresh_names(context.fonts());
        self.request_previews(context);
        let items = self.shelf.items();
        self.arrivals.retain(|path, _| items.contains(path));
        self.hovered = None;
        context.request_repaint();
    }

    fn remove(&mut self, index: usize, context: &mut Context) {
        if let Some(path) = self.shelf.remove(index) {
            self.shelf_changed(context);
            self.announce(removed_text(&file_name(&path)), context);
        }
    }

    fn begin_drag(&mut self, index: usize, context: &mut Context) {
        let path = self.shelf.items()[index].clone();
        let (inbox, notifier) = (Arc::clone(&self.inbox), context.notifier());
        self.dragging = Some(path.clone());
        let started = platform::drag_file_out(path, move |path, was_taken| {
            if let Ok(mut inbox) = inbox.lock() {
                inbox.drags.push((path, was_taken));
            }
            notifier.notify();
        });
        if !started {
            self.dragging = None;
        }
        context.request_repaint();
    }

    fn draw_preview(&self, canvas: &Canvas, path: &Path, area: Rect, opacity: f32) {
        let scale = canvas.scale();
        if let Some(preview) = self.previews.borrow_mut().get_mut(path) {
            if preview.brush.is_none()
                && let Some(image) = preview.waiting.take()
            {
                preview.brush = canvas.upload(&image);
            }
            if let Some(brush) = &preview.brush {
                canvas.image(brush, fitted(preview.size, area), PREVIEW_RADIUS * scale, opacity);
                return;
            }
        }
        canvas.fill_rounded_rect(area, PREVIEW_RADIUS * scale, Color::gray(0.16).faded(opacity));
        if path.is_dir() {
            Icon::Folder.draw(canvas, Point::new(area.center_x(), area.center_y()), 22.0 * scale, GRAY, opacity);
        } else {
            canvas.text(&TILE_LABEL, &tile_label(path, false), area, GRAY.faded(opacity));
        }
    }

    fn draw_remove_button(&self, canvas: &Canvas, preview: Rect, is_pressed_target: bool, opacity: f32) {
        let scale = canvas.scale();
        let center = remove_center(preview, scale);
        let fill = if is_pressed_target { Color::gray(0.42) } else { Color::gray(0.26) };
        canvas.fill_circle(center, (REMOVE_RADIUS + REMOVE_RING) * scale, Color::BLACK.faded(opacity));
        canvas.fill_circle(center, REMOVE_RADIUS * scale, fill.faded(opacity));
        Icon::Close.draw(canvas, center, 10.0 * scale, Color::WHITE, opacity);
    }

    fn draw_slot(&self, canvas: &Canvas, position: usize, slot: Slot, bounds: Rect, opacity: f32) {
        let scale = canvas.scale();
        let column = slot_rect(position, bounds, scale);
        let arrival = match slot {
            Slot::File(index) => self.arrivals.get(&self.shelf.items()[index]).map(|spring| spring.position),
            Slot::More(_) => None,
        };
        let Some(progress) = arrival else { return self.draw_column(canvas, position, slot, column, opacity) };
        let shown = progress.max(0.0);
        let rise = (1.0 - shown) * ARRIVAL_FALL * scale;
        let column = Rect::new(column.left, column.top - rise, column.right, column.bottom - rise);
        let factor = ARRIVAL_SCALE + (1.0 - ARRIVAL_SCALE) * shown;
        let center = Point::new(column.center_x(), column.center_y());
        let faded = opacity * progress.clamp(0.0, 1.0);
        canvas.scaled_about(center, factor, || self.draw_column(canvas, position, slot, column, faded));
    }

    fn draw_column(&self, canvas: &Canvas, position: usize, slot: Slot, column: Rect, opacity: f32) {
        let scale = canvas.scale();
        let is_hovered = matches!(self.hovered, Some(Target::Slot(at) | Target::Remove(at)) if at == position);
        if is_hovered {
            let wash = Rect::new(
                column.left,
                column.top - WASH_REACH * scale,
                column.right,
                column.bottom + WASH_REACH * scale,
            );
            canvas.fill_rounded_rect(wash, 8.0 * scale, WASH.with_alpha(0.07 * opacity));
        }
        let preview = preview_rect(column, scale);
        let name_top = preview.bottom + NAME_TOP * scale;
        let name_color = if is_hovered { Color::WHITE } else { NAME_COLOR };
        match slot {
            Slot::File(index) => {
                let path = &self.shelf.items()[index];
                let faded = if self.dragging.as_ref() == Some(path) { opacity * 0.35 } else { opacity };
                self.draw_preview(canvas, path, preview, faded);
                for (line, text) in self.names.get(index).into_iter().flatten().enumerate() {
                    let top = name_top + line as f32 * NAME_LINE_HEIGHT * scale;
                    let rect = Rect::new(column.left, top, column.right, top + NAME_LINE_HEIGHT * scale);
                    canvas.text(&NAME, text, rect, name_color.faded(faded));
                }
                if is_hovered {
                    self.draw_remove_button(canvas, preview, self.hovered == Some(Target::Remove(position)), opacity);
                }
            }
            Slot::More(count) => {
                canvas.fill_rounded_rect(preview, PREVIEW_RADIUS * scale, Color::gray(0.16).faded(opacity));
                canvas.text(&TILE_LABEL, &format!("+{count}"), preview, Color::WHITE.faded(opacity));
                let rect = Rect::new(column.left, name_top, column.right, name_top + NAME_LINE_HEIGHT * scale);
                canvas.text(&NAME, "more", rect, name_color.faded(opacity));
            }
        }
    }
}

impl Preview {
    fn new(image: ImageData) -> Self {
        Self { size: (image.width, image.height), waiting: Some(image), brush: None }
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
        if self.is_drag_over {
            canvas.fill_rounded_rect(bounds, ZONE_RADIUS * scale, ACCENT.with_alpha(0.1 * opacity));
        }
        let border = if self.is_drag_over { ACCENT } else { BORDER };
        ui::draw_dashed_outline(
            canvas,
            bounds,
            ZONE_RADIUS * scale,
            BORDER_WIDTH * scale,
            DASH * scale,
            DASH_GAP * scale,
            border.faded(opacity),
        );

        if self.shelf.len() == 0 {
            let (title, detail, title_color, detail_color) = if self.is_drag_over {
                ("Release to add", "Files stay where they are", Color::WHITE, ACCENT)
            } else {
                ("Drop files here", "Drag them back out when you need them", NAME_COLOR, GRAY)
            };
            let top = bounds.center_y() - EMPTY_TEXT_HEIGHT / 2.0 * scale;
            let title_area = Rect::new(bounds.left, top, bounds.right, top + EMPTY_TITLE_HEIGHT * scale);
            let detail_top = title_area.bottom + EMPTY_GAP * scale;
            let detail_area =
                Rect::new(bounds.left, detail_top, bounds.right, detail_top + EMPTY_DETAIL_HEIGHT * scale);
            canvas.text(&EMPTY_TITLE, title, title_area, title_color.faded(opacity));
            canvas.text(&EMPTY_DETAIL, detail, detail_area, detail_color.faded(opacity));
            return;
        }
        for (position, slot) in slots(self.shelf.len()).into_iter().enumerate() {
            self.draw_slot(canvas, position, slot, bounds, opacity);
        }
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

    fn on_file_drag_changed(&mut self, is_over: bool, context: &mut Context) {
        if self.is_drag_over != is_over {
            self.is_drag_over = is_over;
            context.request_repaint();
        }
    }

    fn on_drop(&mut self, paths: Vec<PathBuf>, context: &mut Context) {
        let before = self.shelf.len();
        let added = self.shelf.add(paths);
        if added > 0 {
            for (order, path) in self.shelf.items()[before..].iter().take(MAX_SLOTS).enumerate() {
                // Starting below zero holds each later file back until its turn.
                let mut spring = Spring::new(-ARRIVAL_STAGGER * order as f32, ARRIVAL_RESPONSE.0, ARRIVAL_RESPONSE.1);
                spring.target = 1.0;
                self.arrivals.insert(path.clone(), spring);
            }
            context.request_animation();
            self.shelf_changed(context);
            self.announce(added_text(added), context);
        }
        context.request_repaint();
    }

    fn on_start(&mut self, context: &mut Context) {
        if let Some(storage) = &self.storage {
            self.shelf = Shelf::load(storage);
        }
        self.refresh_names(context.fonts());
        self.request_previews(context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        let (previews, drags) = match self.inbox.lock() {
            Ok(mut inbox) => (std::mem::take(&mut inbox.previews), std::mem::take(&mut inbox.drags)),
            Err(_) => return,
        };
        for (path, image) in previews {
            if let Some(image) = image
                && self.shelf.items().contains(&path)
            {
                self.previews.get_mut().insert(path, Preview::new(image));
            }
        }
        for (path, was_taken) in drags {
            if self.dragging.as_ref() == Some(&path) {
                self.dragging = None;
            }
            if was_taken && let Some(index) = self.shelf.items().iter().position(|item| *item == path) {
                self.remove(index, context);
            }
        }
        context.request_repaint();
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
                let Some(press) = self.press else { return };
                let Target::Slot(position) = press.target else { return };
                let moved = (point.x - press.origin.x).hypot(point.y - press.origin.y);
                if moved >= DRAG_START_DISTANCE * scale
                    && let Some(index) = file_index(position, count)
                {
                    self.press = None;
                    self.begin_drag(index, context);
                }
            }
            PointerEvent::Pressed(point) if frame.is_open() => {
                self.press = target_at(point, bounds, scale, count).map(|target| Press { target, origin: point });
            }
            PointerEvent::Released(point) => {
                let Some(press) = self.press.take() else { return };
                if !frame.is_open() || target_at(point, bounds, scale, count) != Some(press.target) {
                    return;
                }
                match press.target {
                    Target::Remove(position) => {
                        if let Some(index) = file_index(position, count) {
                            self.remove(index, context);
                        }
                    }
                    Target::Slot(position) => {
                        let Some(index) = file_index(position, count) else { return };
                        let path = self.shelf.items()[index].clone();
                        let (now, limit) = (Instant::now(), platform::double_click_time());
                        if is_double_click(self.last_click.as_ref(), &path, now, limit) {
                            self.last_click = None;
                            platform::open(&path.display().to_string());
                        } else {
                            self.last_click = Some((path, now));
                        }
                    }
                }
            }
            PointerEvent::SecondaryReleased(point) if frame.is_open() => {
                let Some(Target::Slot(position) | Target::Remove(position)) = target_at(point, bounds, scale, count)
                else {
                    return;
                };
                if let Some(index) = file_index(position, count) {
                    self.remove(index, context);
                }
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                self.press = None;
                if self.hovered.take().is_some() {
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

    fn animate(&mut self, elapsed_seconds: f32) -> bool {
        for spring in self.arrivals.values_mut() {
            spring.step(elapsed_seconds);
        }
        self.arrivals.retain(|_, spring| !spring.is_settled());
        !self.arrivals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::new(0.0, 0.0, EXPANDED_WIDTH, 86.0);

    #[test]
    fn six_files_fit_and_a_seventh_uses_the_last_place_for_overflow() {
        assert_eq!(slots(6), (0..6).map(Slot::File).collect::<Vec<_>>());
        assert_eq!(
            slots(7),
            vec![Slot::File(0), Slot::File(1), Slot::File(2), Slot::File(3), Slot::File(4), Slot::More(2)]
        );
        assert_eq!(EXPANDED_SIZE.width, 507.0);
    }

    fn middle(rect: Rect) -> Point {
        Point::new(rect.center_x(), rect.center_y())
    }

    #[test]
    fn a_few_files_each_get_a_place() {
        assert!(slots(0).is_empty());
        assert_eq!(slots(3), vec![Slot::File(0), Slot::File(1), Slot::File(2)]);
        assert_eq!(slots(MAX_SLOTS).len(), MAX_SLOTS);
    }

    #[test]
    fn too_many_files_end_in_a_place_that_counts_the_rest() {
        let places = slots(12);
        assert_eq!(places.len(), MAX_SLOTS);
        assert_eq!(places[MAX_SLOTS - 1], Slot::More(12 - (MAX_SLOTS - 1)));
        assert_eq!(places[0], Slot::File(0));
    }

    #[test]
    fn only_a_place_that_shows_a_file_has_a_file() {
        assert_eq!(file_index(1, 3), Some(1));
        assert_eq!(file_index(MAX_SLOTS - 1, 12), None);
        assert_eq!(file_index(4, 3), None);
    }

    #[test]
    fn the_places_sit_side_by_side_and_fill_the_page_from_edge_to_edge_of_its_padding() {
        let first = slot_rect(0, BOUNDS, 1.0);
        let second = slot_rect(1, BOUNDS, 1.0);
        assert_eq!(second.left - first.right, FILE_GAP);
        assert_eq!(first.left, ZONE_PADDING);
        let last = slot_rect(MAX_SLOTS - 1, BOUNDS, 1.0);
        assert!((BOUNDS.right - last.right - ZONE_PADDING).abs() < 0.01, "{last:?}");
    }

    #[test]
    fn the_padding_is_the_same_on_every_side_of_a_column() {
        let column = slot_rect(0, BOUNDS, 1.0);
        assert_eq!(column.left - BOUNDS.left, column.top - BOUNDS.top);
        assert!((BOUNDS.bottom - column.bottom - ZONE_PADDING).abs() < 0.01, "{column:?}");
    }

    #[test]
    fn a_press_finds_the_file_or_the_cross_under_it() {
        let column = slot_rect(1, BOUNDS, 1.0);
        let inside = Point::new(column.center_x(), column.bottom - 2.0);
        assert_eq!(target_at(inside, BOUNDS, 1.0, 3), Some(Target::Slot(1)));
        let cross = remove_center(preview_rect(column, 1.0), 1.0);
        assert_eq!(target_at(cross, BOUNDS, 1.0, 3), Some(Target::Remove(1)));
        assert_eq!(target_at(Point::new(BOUNDS.right - 5.0, 80.0), BOUNDS, 1.0, 3), None);
    }

    #[test]
    fn the_more_place_has_no_cross() {
        let more = slot_rect(MAX_SLOTS - 1, BOUNDS, 1.0);
        let corner = remove_center(preview_rect(more, 1.0), 1.0);
        assert_eq!(target_at(corner, BOUNDS, 1.0, 12), Some(Target::Slot(MAX_SLOTS - 1)));
    }

    #[test]
    fn an_empty_shelf_has_nothing_to_press() {
        assert_eq!(target_at(Point::new(40.0, 40.0), BOUNDS, 1.0, 0), None);
        assert_eq!(target_at(middle(slot_rect(0, BOUNDS, 1.0)), BOUNDS, 1.0, 0), None);
    }

    #[test]
    fn the_page_is_exactly_as_wide_as_its_places_and_padding() {
        let places = MAX_SLOTS as f32 * FILE_WIDTH + (MAX_SLOTS - 1) as f32 * FILE_GAP;
        assert_eq!(EXPANDED_SIZE.width, places + 2.0 * ZONE_PADDING);
    }

    #[test]
    fn a_picture_keeps_its_proportions_inside_its_box() {
        let area = Rect::new(0.0, 0.0, 40.0, 40.0);
        let wide = fitted((96, 54), area);
        assert_eq!(wide.width(), 40.0);
        assert!((wide.height() - 22.5).abs() < 0.01);
        assert_eq!((wide.center_x(), wide.center_y()), (20.0, 20.0));
        let tall = fitted((50, 100), area);
        assert_eq!((tall.width(), tall.height()), (20.0, 40.0));
    }

    #[test]
    fn a_long_name_takes_two_lines_and_ends_in_an_ellipsis() {
        let fonts = Fonts::new(1.0).unwrap();
        assert_eq!(name_lines("notes.txt", &fonts), vec!["notes.txt"]);
        let lines = name_lines("a-very-long-file-name-for-the-quarterly-budget-review.xlsx", &fonts);
        assert_eq!(lines.len(), NAME_LINES);
        assert!(lines[NAME_LINES - 1].ends_with('\u{2026}'));
        assert!(lines.iter().all(|line| fonts.measure(&NAME, line) <= NAME_WIDTH + 0.5));
    }

    #[test]
    fn two_clicks_on_the_same_file_in_time_make_a_double_click() {
        let (path, now) = (PathBuf::from("a.txt"), Instant::now());
        let limit = Duration::from_millis(500);
        let first = Some((path.clone(), now));
        assert!(is_double_click(first.as_ref(), &path, now + Duration::from_millis(300), limit));
        assert!(!is_double_click(first.as_ref(), &path, now + Duration::from_millis(900), limit));
        assert!(!is_double_click(first.as_ref(), Path::new("b.txt"), now + Duration::from_millis(100), limit));
        assert!(!is_double_click(None, &path, now, limit));
    }

    #[test]
    fn the_shelf_is_a_page_to_open_that_takes_drops() {
        let page = ShelfPage::new();
        assert!(page.accepts_drops());
        assert_eq!(page.priority(Mode::Resting), None);
        assert!(page.priority(Mode::Expanded).is_some());
        assert_eq!(page.presence(), None);
    }

    #[test]
    fn a_new_file_arrives_and_then_the_page_stops_animating() {
        let mut page = ShelfPage::new();
        let mut spring = Spring::new(0.0, ARRIVAL_RESPONSE.0, ARRIVAL_RESPONSE.1);
        spring.target = 1.0;
        page.arrivals.insert(PathBuf::from("a.txt"), spring);
        assert!(page.animate(0.016));
        for _ in 0..200 {
            page.animate(0.016);
        }
        assert!(!page.animate(0.016));
        assert!(page.arrivals.is_empty());
    }
}
