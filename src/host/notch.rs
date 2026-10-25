use std::cell::Cell;

use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Media::{timeBeginPeriod, timeEndPeriod},
    System::{Performance::QueryPerformanceCounter, SystemInformation::GetTickCount64},
    UI::{
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::{GetCursorPos, GetWindowRect, KillTimer, SetTimer},
    },
};

use super::{
    activity_layout::{self, BADGE_EDGE_PADDING, DIVIDER_WIDTH},
    chrome, display, navigation,
    navigation::PageList,
    page_editor::{self, ChipView, Entry, Outcome, PageEditor},
    settings::Settings,
};
use crate::{
    activity::{ACTIVITY_TIMER_BASE, Activity, Presence, SystemMessage},
    gfx::{Canvas, Color, Fonts, Point, Rect, Size, Spring, Surface, lerp},
    log,
    page::{
        Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, RESTING_HEIGHT, Requests, split_owned_timer_key,
        split_timer_key,
    },
};

/// Wide enough for a long lyric line; the window only occupies the pill's area, so this costs no screen space.
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 260.0;

const RESTING_RADIUS: f32 = 14.0;
const EXPANDED_RADIUS: f32 = 22.0;
/// Radius of the outward curve joining the pill's sides to the top of the screen, as on a MacBook notch.
const RESTING_FLARE: f32 = 8.0;
const EXPANDED_FLARE: f32 = 14.0;
const ANIMATION_TIMER_ID: usize = 1;
/// Safety net while the notch is open: the leave notification can be lost when the window resizes under the pointer.
const HOVER_WATCH_TIMER_ID: usize = 2;
const HOVER_WATCH_INTERVAL_MILLISECONDS: u32 = 120;
/// While the notch hides when idle: after the pointer enters the strip it waits this long, then the small pill shows...
const REVEAL_TIMER_ID: usize = 3;
const REVEAL_DELAY_MILLISECONDS: u32 = 150;
/// ...and if the pointer stays, the notch opens this long after that.
const EXPAND_TIMER_ID: usize = 4;
const EXPAND_DELAY_MILLISECONDS: u32 = 450;
/// The strip at the top of the screen that wakes a hidden notch, in device-independent pixels.
const HIDDEN_WIDTH: f32 = 240.0;
const HIDDEN_HEIGHT: f32 = 6.0;
/// How opaque the strip is: just enough for the pointer to find it.
const HIDDEN_ALPHA: f32 = 0.006;
const HIDE_WHEN_IDLE_SETTING: &str = "notch.hide_when_idle";
/// The expansion past which content starts to appear; below it only the collapsed content shows.
const EXPANDED_CONTENT_START: f32 = 0.55;
/// A swipe ends, and the pages settle on the nearest one, when no scroll arrives for this long.
const SWIPE_END_TIMER_ID: usize = 5;
const SWIPE_END_DELAY_MILLISECONDS: u32 = 140;
/// How far a page is faded for each page-width it is from the middle of the notch.
const PAGE_FADE_PER_PAGE: f32 = 0.8;
/// The distance between neighbouring pages in the row, as a share of the content width.
const PAGE_PITCH: f32 = 0.8;
/// The setting that remembers the page that was open last.
const LAST_PAGE_SETTING: &str = "navigation.page";
const PAGE_ORDER_SETTING: &str = "navigation.order";
/// Width of the "Edit pages" view's content, in device-independent pixels.
const EDITOR_WIDTH: f32 = 428.0;
const PAGES_OFF_SETTING: &str = "navigation.disabled";

/// Who is showing something in the collapsed pill: an activity, or a page that also has something to say while closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Activity(usize),
    Page(usize),
}

/// An activity and whether the user has it switched on; it only runs once it has been started.
struct ActivitySlot {
    activity: Box<dyn Activity>,
    is_enabled: bool,
    is_started: bool,
}

/// Owns the window's state: it animates the pill, tracks the pointer and decides which page is shown.
pub struct Notch {
    window: HWND,
    scale: f32,
    screen_position: POINT,
    surface: Surface,
    fonts: Fonts,
    settings: Settings,
    pages: Vec<Box<dyn Page>>,
    activities: Vec<ActivitySlot>,
    /// The order in which the transients now on screen appeared; the newest ones are shown.
    transient_order: Vec<Source>,
    /// The activities drawn in place of the page while collapsed, and the badges drawn beside it, with their widths.
    shown_transients: Vec<(Source, f32)>,
    shown_badges: Vec<(Source, f32)>,
    /// The page drawn while collapsed and the page drawn while open; they can differ.
    resting_index: usize,
    expanded_index: usize,
    /// The page the user moved to last; it stays open until they move again.
    selected_page: Option<usize>,
    /// The page that is on screen right now: the notch is open and shows it.
    shown_page: Option<usize>,
    /// The user's page order and which pages are switched on.
    page_list: PageList,
    /// The "Edit pages" view, while it is open.
    editor: Option<PageEditor>,
    /// The pages that can be opened right now, in order; the dots show them.
    navigable: Vec<usize>,
    is_gear_hovered: bool,
    /// While switching pages: the page being left, the direction (1 toward the next page) and the progress.
    /// Where the row of pages is, as a position among the navigable pages; between two numbers mid-swipe.
    track: Spring,
    /// While fingers swipe on a touchpad: where they have taken the pages, which is not yet a whole page.
    swipe_position: Option<f32>,
    /// Requests collected from page hooks, applied by `flush`.
    pending: Requests,
    expansion: Spring,
    resting_width: Spring,
    /// Height of the collapsed pill: the page height normally, a thin strip while the notch hides.
    resting_height: Spring,
    expanded_width: Spring,
    expanded_height: Spring,
    /// 0 while the page shows in the collapsed pill, 1 while transient activities take its place.
    activity_blend: Spring,
    /// Room kept at the right end of the collapsed pill for badges.
    badge_width: Spring,
    /// Whether the notch hides itself when nothing is happening (a choice in the tray menu).
    hides_when_idle: bool,
    /// While hidden, the pointer has waited on the strip and the small pill is showing.
    is_revealed: bool,
    /// The best page for the collapsed pill has something to say beyond being the clock.
    has_resting_content: bool,
    is_hovering: bool,
    is_animating: bool,
    pointer_captured: bool,
    /// True while a file is dragged over the notch.
    is_dragging_files: bool,
    last_tick: i64,
    ticks_per_second: i64,
    frame_interval_milliseconds: u32,
    /// Top-left of the window within the surface as of the last paint.
    region_origin: Cell<(i32, i32)>,
}

/// Everything an `Notch` is built from.
pub struct NotchParts {
    pub window: HWND,
    pub scale: f32,
    pub screen_position: POINT,
    pub surface: Surface,
    pub fonts: Fonts,
    pub settings: Settings,
    pub pages: Vec<Box<dyn Page>>,
    pub activities: Vec<Box<dyn Activity>>,
    pub ticks_per_second: i64,
}

impl Notch {
    pub fn new(parts: NotchParts) -> Self {
        let NotchParts {
            window,
            scale,
            screen_position,
            surface,
            fonts,
            settings,
            pages,
            activities,
            ticks_per_second,
        } = parts;
        assert!(!pages.is_empty(), "the notch needs at least one page");
        let activities = activities
            .into_iter()
            .map(|activity| {
                let is_enabled = settings.get_bool(&activity_setting_key(activity.id()), true);
                ActivitySlot { activity, is_enabled, is_started: false }
            })
            .collect();
        Self {
            window,
            scale,
            screen_position,
            surface,
            fonts,
            settings,
            pages,
            activities,
            transient_order: Vec::new(),
            shown_transients: Vec::new(),
            shown_badges: Vec::new(),
            resting_index: 0,
            expanded_index: 0,
            selected_page: None,
            shown_page: None,
            page_list: PageList::new(0),
            editor: None,
            navigable: Vec::new(),
            is_gear_hovered: false,
            track: Spring::new(0.0, 260.0, 26.0),
            swipe_position: None,
            pending: Requests::default(),
            expansion: Spring::new(0.0, 260.0, 22.0),
            resting_width: Spring::new(0.0, 220.0, 26.0),
            resting_height: Spring::new(RESTING_HEIGHT, 260.0, 26.0),
            expanded_width: Spring::new(0.0, 200.0, 26.0),
            expanded_height: Spring::new(0.0, 220.0, 26.0),
            activity_blend: Spring::new(0.0, 300.0, 28.0),
            badge_width: Spring::new(0.0, 220.0, 26.0),
            hides_when_idle: false,
            is_revealed: false,
            has_resting_content: false,
            is_hovering: false,
            is_animating: false,
            pointer_captured: false,
            is_dragging_files: false,
            last_tick: 0,
            ticks_per_second,
            frame_interval_milliseconds: display::frame_interval_milliseconds(),
            region_origin: Cell::new((0, 0)),
        }
    }

    /// Starts every page and shows the first frame at the right size, without animating into it.
    pub fn start(&mut self) {
        for (index, page) in self.pages.iter().enumerate() {
            log::debug(&format!("page {index}: {}", page.id()));
        }
        self.selected_page = self
            .settings
            .get_string(LAST_PAGE_SETTING)
            .and_then(|id| self.pages.iter().position(|page| page.id() == id));
        self.hides_when_idle = self.settings.get_bool(HIDE_WHEN_IDLE_SETTING, false);
        let ids = self.page_ids();
        let opt_in: Vec<bool> = self.pages.iter().map(|page| page.is_opt_in()).collect();
        self.page_list = PageList::load(
            &ids,
            &opt_in,
            self.settings.get_string(PAGE_ORDER_SETTING),
            self.settings.get_string(PAGES_OFF_SETTING),
        );
        for index in 0..self.pages.len() {
            self.with_page(index, |page, context| page.on_start(context));
        }
        for index in 0..self.activities.len() {
            if self.activities[index].is_enabled {
                self.start_activity(index);
            }
        }
        self.refresh_layout();
        for spring in self.layout_springs() {
            spring.snap_to_target();
        }
        self.flush();
        self.paint();
    }

    // Input from the window

    pub fn pointer_moved(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        if !self.is_hovering {
            log::debug(&format!("enter move window=({window_x},{window_y}) surface=({:.0},{:.0})", point.x, point.y));
        }
        self.set_hovering(true);
        let frame = self.frame();
        let is_over_gear = frame.is_open() && chrome::gear_button(frame.bounds, self.scale).contains(point);
        if is_over_gear != self.is_gear_hovered {
            self.is_gear_hovered = is_over_gear;
            self.pending.repaint = true;
        }
        if let Some(editor) = self.editor.as_mut() {
            if editor.moved(point) {
                self.pending.repaint = true;
            }
        } else {
            self.route_pointer(PointerEvent::Moved(point));
        }
        self.flush();
    }

    pub fn pointer_pressed(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        if self.editor.is_some() {
            self.press_editor(point);
            self.flush();
            return;
        }
        if self.press_header(point) {
            self.flush();
            return;
        }
        self.route_pointer(PointerEvent::Pressed(point));
        self.flush();
    }

    /// The first enabled page that takes dropped files.
    fn drop_page(&self) -> Option<usize> {
        (0..self.pages.len()).find(|&index| self.page_list.is_enabled(index) && self.pages[index].accepts_drops())
    }

    /// Shows the page that takes dropped files, if the notch is open.
    fn show_drop_page(&mut self, index: usize) {
        if self.is_hovering && self.editor.is_none() {
            self.go_to_page(index);
        }
    }

    /// A file drag reached the notch: open it at the page that takes the files, without the usual hover delay.
    pub fn drag_entered(&mut self) {
        let Some(index) = self.drop_page() else { return };
        self.is_dragging_files = true;
        self.set_hovering(true);
        self.is_revealed = true;
        self.expansion.target = 1.0;
        self.show_drop_page(index);
        self.start_animation();
        self.flush();
    }

    /// The file drag left without dropping: close the notch unless the pointer is on it.
    pub fn drag_left(&mut self) {
        self.is_dragging_files = false;
        if !self.pointer_is_over_window() {
            self.set_hovering(false);
        }
        self.flush();
    }

    /// Files were dropped on the notch: the first page that takes drops gets them, and is shown if the notch is open.
    pub fn files_dropped(&mut self, paths: Vec<std::path::PathBuf>) {
        self.is_dragging_files = false;
        let Some(index) = self.drop_page() else { return };
        self.with_page(index, |page, context| page.on_drop(paths, context));
        self.show_drop_page(index);
        if !self.pointer_is_over_window() {
            self.set_hovering(false);
        }
        self.flush();
    }

    /// A right click: inside the open page it goes to the page. Returns false when the notch did not use it.
    pub fn right_clicked(&mut self, window_x: i32, window_y: i32) -> bool {
        let point = self.to_surface_space(window_x, window_y);
        let frame = self.frame();
        if self.editor.is_some() {
            return true;
        }
        if !frame.is_open() || !chrome::content_rect(frame.bounds, self.scale).contains(point) {
            return false;
        }
        self.route_pointer(PointerEvent::SecondaryReleased(point));
        self.flush();
        true
    }

    /// Scrolling the wheel or swiping a touchpad moves between pages while the notch is open.
    pub fn scrolled(&mut self, amount: i32, is_horizontal: bool) {
        if !self.is_hovering || self.editor.is_some() {
            return;
        }
        // Scrolling down, or to the right, goes to the next page.
        let toward_next = if is_horizontal { amount } else { -amount };
        match navigation::classify_scroll(toward_next) {
            navigation::Scroll::Pages(steps) => {
                self.end_swipe();
                self.navigate(steps);
            }
            navigation::Scroll::Drag(pages) => self.drag_pages(pages),
        }
        self.flush();
    }

    /// Moves the row of pages by a fraction of a page and settles on the nearest page once the fingers stop.
    fn drag_pages(&mut self, pages: f32) {
        if self.navigable.len() < 2 {
            return;
        }
        self.wrap_track();
        let position = self.swipe_position.unwrap_or(self.track.target) + pages;
        self.swipe_position = Some(position);
        self.track.target = position;
        self.select_page(self.page_at(position));
        unsafe {
            SetTimer(Some(self.window), SWIPE_END_TIMER_ID, SWIPE_END_DELAY_MILLISECONDS, None);
        }
        self.start_animation();
    }

    /// Stops following the fingers: the row settles on the nearest whole page.
    fn end_swipe(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.window), SWIPE_END_TIMER_ID);
        }
        if self.swipe_position.take().is_some() {
            self.track.target = self.track.target.round();
            self.start_animation();
        }
    }

    pub fn pointer_released(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        let was_captured = self.pointer_captured;
        if self.editor.is_some() {
            self.release_editor(point);
            if was_captured {
                self.pointer_captured = false;
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
        } else {
            self.route_pointer(PointerEvent::Released(point));
        }
        self.flush();
        // A drag can end outside the pill, where no leave message follows.
        if was_captured && !self.frame().bounds.contains(point) {
            self.set_hovering(false);
            self.flush();
        }
    }

    /// Handles `WM_MOUSELEAVE`; returns false when the leave was not real and tracking must be re-armed.
    pub fn pointer_left_window(&mut self) -> bool {
        // While dragging, the pointer may leave the pill; the drag keeps the notch open.
        if self.pointer_captured || self.is_dragging_files {
            return true;
        }
        // A resize under a stationary pointer can report a leave that did not happen; only believe a real one.
        let over_window = self.pointer_is_over_window();
        log::debug(&format!("mouse leave message, pointer still over window: {over_window}"));
        if over_window {
            return false;
        }
        self.set_hovering(false);
        self.flush();
        true
    }

    pub fn capture_lost(&mut self) {
        self.pointer_captured = false;
        self.route_pointer(PointerEvent::CaptureLost);
        self.flush();
    }

    pub fn timer_fired(&mut self, timer_key: usize) {
        match timer_key {
            ANIMATION_TIMER_ID => self.advance_animation(),
            SWIPE_END_TIMER_ID => {
                self.end_swipe();
                self.flush();
            }
            REVEAL_TIMER_ID => {
                unsafe {
                    let _ = KillTimer(Some(self.window), REVEAL_TIMER_ID);
                    SetTimer(Some(self.window), EXPAND_TIMER_ID, EXPAND_DELAY_MILLISECONDS, None);
                }
                if self.is_hovering {
                    self.is_revealed = true;
                    self.flush();
                }
            }
            EXPAND_TIMER_ID => {
                unsafe {
                    let _ = KillTimer(Some(self.window), EXPAND_TIMER_ID);
                }
                if self.is_hovering {
                    self.expansion.target = 1.0;
                    self.start_animation();
                    self.flush();
                }
            }
            HOVER_WATCH_TIMER_ID => {
                if !self.pointer_captured && !self.is_dragging_files && !self.pointer_is_over_window() {
                    let (cursor, bounds) = self.pointer_and_window_bounds();
                    log::debug(&format!(
                        "hover watch: pointer left the window; cursor=({},{}) window=({},{})-({},{}) expansion={:.2}",
                        cursor.x,
                        cursor.y,
                        bounds.left,
                        bounds.top,
                        bounds.right,
                        bounds.bottom,
                        self.expansion.position
                    ));
                    self.set_hovering(false);
                    self.flush();
                }
            }
            key if key >= ACTIVITY_TIMER_BASE => {
                let Some((index, timer)) = split_owned_timer_key(ACTIVITY_TIMER_BASE, usize::MAX, key) else { return };
                if index < self.activities.len() {
                    self.with_activity(index, |activity, context| activity.on_timer(timer, context));
                    self.flush();
                }
            }
            key => {
                let Some((index, timer)) = split_timer_key(key) else { return };
                if index < self.pages.len() {
                    self.with_page(index, |page, context| page.on_timer(timer, context));
                    self.flush();
                }
            }
        }
    }

    pub fn page_notified(&mut self, index: usize) {
        if index < self.pages.len() {
            self.with_page(index, |page, context| page.on_notify(context));
            self.flush();
        }
    }

    pub fn activity_notified(&mut self, index: usize) {
        if index < self.activities.len() && self.activities[index].is_started {
            self.with_activity(index, |activity, context| activity.on_notify(context));
            self.flush();
        }
    }

    /// Hands a system message to every activity that is running, whether or not it is switched on right now.
    pub fn system_message(&mut self, message: SystemMessage) {
        for index in 0..self.activities.len() {
            if self.activities[index].is_started {
                self.with_activity(index, |activity, context| activity.on_system_message(&message, context));
            }
        }
        self.flush();
    }

    /// The name and on/off state of every activity, for the tray menu.
    pub fn activity_menu(&self) -> Vec<(&'static str, bool)> {
        self.activities.iter().map(|slot| (slot.activity.label(), slot.is_enabled)).collect()
    }

    /// The items pages add to the tray menu: page index, item index and label.
    pub fn page_menu(&self) -> Vec<(usize, usize, String)> {
        let mut items = Vec::new();
        for (page, entry) in self.pages.iter().enumerate().filter(|(index, _)| self.page_list.is_enabled(*index)) {
            items.extend(entry.menu_items().into_iter().enumerate().map(|(item, label)| (page, item, label)));
        }
        items
    }

    pub fn page_menu_item_chosen(&mut self, page: usize, item: usize) {
        if page < self.pages.len() {
            self.with_page(page, |entry, context| entry.on_menu_item(item, context));
            self.flush();
        }
    }

    /// Switches an activity on or off and remembers the choice; switching on starts it the first time.
    pub fn toggle_activity(&mut self, index: usize) {
        let Some(slot) = self.activities.get_mut(index) else { return };
        slot.is_enabled = !slot.is_enabled;
        let (key, is_enabled) = (activity_setting_key(slot.activity.id()), slot.is_enabled);
        self.settings.set_bool(&key, is_enabled);
        if is_enabled {
            self.start_activity(index);
        }
        self.flush();
    }

    fn start_activity(&mut self, index: usize) {
        if self.activities[index].is_started {
            return;
        }
        self.activities[index].is_started = true;
        log::debug(&format!("activity {index}: {}", self.activities[index].activity.id()));
        self.with_activity(index, |activity, context| activity.on_start(context));
    }

    // Page plumbing

    /// Runs an activity hook with a `Context`; whatever the activity requests is applied by the next `flush`.
    fn with_activity<R>(&mut self, index: usize, hook: impl FnOnce(&mut dyn Activity, &mut Context) -> R) -> R {
        let mut context = Context::for_activity(
            self.window,
            index,
            &self.surface,
            &self.fonts,
            &mut self.pending,
            &mut self.settings,
        );
        hook(self.activities[index].activity.as_mut(), &mut context)
    }

    /// Runs a page hook with a `Context`; whatever the page requests is applied by the next `flush`.
    fn with_page<R>(&mut self, index: usize, hook: impl FnOnce(&mut dyn Page, &mut Context) -> R) -> R {
        let mut context =
            Context::new(self.window, index, &self.surface, &self.fonts, &mut self.pending, &mut self.settings);
        hook(self.pages[index].as_mut(), &mut context)
    }

    /// Moves `step` pages along (negative goes back); past the last page the row comes round to the first.
    fn navigate(&mut self, step: i32) {
        if self.navigable.len() < 2 {
            return;
        }
        self.wrap_track();
        let target = self.track.target.round() + step as f32;
        self.track.target = target;
        self.select_page(self.page_at(target));
        self.start_animation();
    }

    /// The page at `position` in the loop of navigable pages.
    fn page_at(&self, position: f32) -> usize {
        self.navigable[(position.round() as i64).rem_euclid(self.navigable.len() as i64) as usize]
    }

    /// Brings a row that has come to rest back into 0..count, so it does not keep growing as the pages go round.
    fn wrap_track(&mut self) {
        let count = self.navigable.len() as f32;
        if self.swipe_position.is_none() && self.track.is_settled() && count > 0.0 {
            self.track.jump_to(self.track.target.rem_euclid(count));
        }
    }

    /// Makes `target` the open page; the row of pages slides to it the next time the layout is refreshed.
    fn go_to_page(&mut self, target: usize) {
        if target != self.expanded_index {
            self.select_page(target);
            self.start_animation();
        }
    }

    /// Remembers `target` as the open page.
    fn select_page(&mut self, target: usize) {
        self.selected_page = Some(target);
        let id = self.pages[target].id();
        self.settings.set_string(LAST_PAGE_SETTING, id);
    }

    /// Presses in the header: a page dot opens that page. Returns true when the press was in the header.
    fn press_header(&mut self, point: Point) -> bool {
        let frame = self.frame();
        if !frame.is_open() {
            return false;
        }
        let dots = chrome::dot_rects(self.navigable.len(), self.track.position, frame.bounds, self.scale);
        if let Some(position) = chrome::dot_at(&dots, point, self.scale) {
            self.end_swipe();
            self.go_to_page(self.navigable[position]);
            return true;
        }
        if chrome::gear_button(frame.bounds, self.scale).contains(point) {
            self.editor = Some(PageEditor::default());
            return true;
        }
        false
    }

    // Editing the page list

    /// The pages as the editor lists them: the ones that are on in the user's order, then the ones that are off.
    fn editor_entries(&self) -> Vec<Entry> {
        let chip_width =
            |page: usize| page_editor::chip_width(self.fonts.measure(&page_editor::CHIP, self.pages[page].name()));
        let on = self.page_list.order().iter().copied().filter(|&page| self.page_list.is_enabled(page));
        let off = (0..self.pages.len()).filter(|&page| !self.page_list.is_enabled(page));
        on.map(|page| (page, true))
            .chain(off.map(|page| (page, false)))
            .map(|(page, is_on)| Entry { page, is_on, chip_width: chip_width(page) })
            .collect()
    }

    fn editor_layout(&self, frame: &Frame) -> page_editor::Layout {
        page_editor::layout(
            &self.editor_entries(),
            chrome::content_rect(frame.bounds, self.scale),
            chrome::header_rect(frame.bounds, self.scale),
            self.scale,
        )
    }

    /// The size of the editor's content, which decides how big the open notch is while it is shown.
    fn editor_size(&self) -> Size {
        let bounds = Rect::new(0.0, 0.0, EDITOR_WIDTH, 0.0);
        let layout = page_editor::layout(&self.editor_entries(), bounds, Rect::new(0.0, 0.0, EDITOR_WIDTH, 16.0), 1.0);
        Size::new(EDITOR_WIDTH, layout.content_height)
    }

    fn press_editor(&mut self, point: Point) {
        let frame = self.frame();
        if !frame.is_open() {
            return;
        }
        let layout = self.editor_layout(&frame);
        let Some(editor) = self.editor.as_mut() else { return };
        let outcome = editor.press(&layout, point, unsafe { GetTickCount64() });
        if editor.is_holding() {
            self.pointer_captured = true;
            unsafe { SetCapture(self.window) };
        }
        self.apply_edit(outcome);
        self.pending.repaint = true;
    }

    fn release_editor(&mut self, point: Point) {
        let frame = self.frame();
        let layout = self.editor_layout(&frame);
        let Some(editor) = self.editor.as_mut() else { return };
        let outcome = editor.release(&layout, point);
        self.apply_edit(outcome);
        self.pending.repaint = true;
    }

    fn apply_edit(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Nothing => return,
            Outcome::Done => {
                self.editor = None;
            }
            Outcome::Move { page, position } => {
                let on_pages: Vec<usize> =
                    self.page_list.order().iter().copied().filter(|&index| self.page_list.is_enabled(index)).collect();
                if let Some(&target) = on_pages.get(position)
                    && let Some(order_position) = self.page_list.order().iter().position(|&index| index == target)
                {
                    self.page_list.move_to(page, order_position);
                }
            }
            Outcome::Remove(page) => {
                let on_count = (0..self.pages.len()).filter(|&index| self.page_list.is_enabled(index)).count();
                // The notch always keeps at least one page.
                if on_count > 1 {
                    self.page_list.set_enabled(page, false);
                }
            }
            Outcome::Add(page) => self.page_list.set_enabled(page, true),
        }
        let ids = self.page_ids();
        let (order, disabled) = (self.page_list.order_text(&ids), self.page_list.disabled_text(&ids));
        self.settings.set_string(PAGE_ORDER_SETTING, &order);
        self.settings.set_string(PAGES_OFF_SETTING, &disabled);
    }

    /// Draws the "Edit pages" view in place of the page dots and the page.
    fn draw_editor(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let layout = self.editor_layout(frame);
        canvas.text(&page_editor::TITLE, "Edit pages", layout.title, Color::WHITE.faded(opacity));
        page_editor::draw_done(canvas, &layout, opacity);
        page_editor::draw_labels(canvas, &layout, opacity);
        let dragged = self.editor.as_ref().and_then(PageEditor::dragged);
        for (is_on, chips) in [(true, &layout.on_chips), (false, &layout.off_chips)] {
            for &(page, rect) in chips {
                let is_dragged = is_on && dragged.is_some_and(|(dragged_page, _)| dragged_page == page);
                let rect = match dragged {
                    Some((_, offset)) if is_dragged => Rect::new(
                        rect.left + offset.x,
                        rect.top + offset.y,
                        rect.right + offset.x,
                        rect.bottom + offset.y,
                    ),
                    _ => rect,
                };
                let view = ChipView { name: self.pages[page].name(), icon: self.pages[page].icon() };
                page_editor::draw_chip(canvas, rect, &view, is_on, is_dragged, opacity);
            }
        }
    }

    /// The ids of the pages in registration order.
    fn page_ids(&self) -> Vec<&'static str> {
        self.pages.iter().map(|page| page.id()).collect()
    }

    /// The frame a page draws and receives pointer events in: the open notch without its padding and header.
    fn page_frame(&self, frame: &Frame) -> Frame {
        Frame { bounds: chrome::content_rect(frame.bounds, self.scale), ..*frame }
    }

    fn route_pointer(&mut self, event: PointerEvent) {
        if self.editor.is_some() {
            return;
        }
        let frame = self.page_frame(&self.frame());
        let index = self.expanded_index;
        self.with_page(index, |page, context| page.on_pointer(event, &frame, context));
    }

    /// Applies what pages asked for, re-selects the shown pages and repaints if anything changed.
    fn flush(&mut self) {
        let requests = std::mem::take(&mut self.pending);
        let layout_changed = self.refresh_layout();
        if requests.animate {
            self.start_animation();
        }
        if requests.repaint || layout_changed {
            self.paint();
        }
        // Telling pages what is on screen can make them ask for more, which the next flush applies.
        if self.sync_shown_page() {
            self.flush();
        }
    }

    /// Tells the pages that started or stopped being on screen; returns true when that was any.
    fn sync_shown_page(&mut self) -> bool {
        let shown = (self.is_hovering && self.editor.is_none()).then_some(self.expanded_index);
        if shown == self.shown_page {
            return false;
        }
        let previous = std::mem::replace(&mut self.shown_page, shown);
        if let Some(index) = previous {
            self.with_page(index, |page, context| page.on_shown_changed(false, context));
        }
        if let Some(index) = shown {
            self.with_page(index, |page, context| page.on_shown_changed(true, context));
        }
        true
    }

    /// Picks the page to show in each mode and animates toward its sizes; returns true if the selection changed.
    fn refresh_layout(&mut self) -> bool {
        let resting_index = self.best_page(Mode::Resting).unwrap_or(self.resting_index);
        let expanded_priorities: Vec<Option<Priority>> =
            self.pages.iter().map(|page| page.priority(Mode::Expanded)).collect();
        self.navigable = self.page_list.navigable(&expanded_priorities);
        let expanded_index = navigation::choose(self.selected_page, &self.page_list.effective(&expanded_priorities))
            .unwrap_or(self.expanded_index);
        let activities_changed = self.refresh_activities();
        let selection_changed =
            resting_index != self.resting_index || expanded_index != self.expanded_index || activities_changed;
        self.resting_index = resting_index;
        self.expanded_index = expanded_index;
        self.follow_open_page();

        let PageSizes { resting_width, .. } = self.pages[resting_index].sizes();
        let PageSizes { mut expanded, .. } = self.pages[expanded_index].sizes();
        if self.editor.is_some() {
            expanded = self.editor_size();
        }
        let transient_widths: Vec<f32> = self.shown_transients.iter().map(|&(_, width)| width).collect();
        let badge_space = badge_space(&self.shown_badges);
        let resting_target = if transient_widths.is_empty() {
            resting_width + badge_space
        } else {
            activity_layout::total_width(&transient_widths)
        };
        let resting_priority = self.pages[resting_index].priority(Mode::Resting);
        self.has_resting_content = resting_priority.is_some_and(|priority| priority > Priority::BACKGROUND);
        let is_hidden = self.is_hidden();
        let (resting_target, resting_height_target) =
            if is_hidden { (HIDDEN_WIDTH, HIDDEN_HEIGHT) } else { (resting_target, RESTING_HEIGHT) };
        let mut targets_changed = false;
        for (spring, target) in [
            (&mut self.resting_width, resting_target),
            (&mut self.resting_height, resting_height_target),
            (&mut self.expanded_width, expanded.width + chrome::WIDTH_EXTRA),
            (&mut self.expanded_height, expanded.height + chrome::HEIGHT_EXTRA),
            (&mut self.activity_blend, if transient_widths.is_empty() { 0.0 } else { 1.0 }),
            (&mut self.badge_width, badge_space),
        ] {
            if (spring.target - target).abs() > f32::EPSILON {
                spring.target = target;
                targets_changed = true;
            }
        }
        if targets_changed {
            self.start_animation();
        }
        selection_changed
    }

    /// Points the row of pages at the open page: the row slides there while the notch is open (also when a page takes
    /// over, such as a new notification) and jumps there otherwise. A swipe in progress steers the row itself.
    fn follow_open_page(&mut self) {
        if self.swipe_position.is_some() {
            return;
        }
        let Some(position) = self.navigable.iter().position(|&index| index == self.expanded_index) else { return };
        let count = self.navigable.len() as f32;
        // How far the row has to go to reach the open page, the short way round.
        let mut gap = (position as f32 - self.track.target).rem_euclid(count);
        if gap > count / 2.0 {
            gap -= count;
        }
        if gap.abs() < 0.01 {
            return;
        }
        if self.is_hovering && self.editor.is_none() {
            self.track.target += gap;
            self.start_animation();
        } else {
            self.track.jump_to(position as f32);
        }
    }

    /// Collects what the running activities and the pages want to show in the collapsed pill; returns true if the
    /// transients or badges changed.
    fn refresh_activities(&mut self) -> bool {
        let mut transients = Vec::new();
        let mut badges = Vec::new();
        let activity_presences = self
            .activities
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_enabled && slot.is_started)
            .map(|(index, slot)| (Source::Activity(index), slot.activity.presence()));
        let page_presences = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, _)| self.page_list.is_enabled(*index))
            .map(|(index, page)| (Source::Page(index), page.presence()));
        for (source, presence) in activity_presences.chain(page_presences) {
            match presence {
                Some(Presence::Transient { width }) => transients.push((source, width)),
                Some(Presence::Badge { width }) => badges.push((source, width)),
                None => {}
            }
        }
        let active: Vec<Source> = transients.iter().map(|&(source, _)| source).collect();
        activity_layout::track_order(&mut self.transient_order, &active);
        let shown: Vec<(Source, f32)> = activity_layout::newest(&self.transient_order)
            .iter()
            .filter_map(|&source| transients.iter().copied().find(|&(candidate, _)| candidate == source))
            .collect();
        let changed = shown != self.shown_transients || badges != self.shown_badges;
        self.shown_transients = shown;
        self.shown_badges = badges;
        changed
    }

    /// The springs that decide the size of the pill.
    fn layout_springs(&mut self) -> [&mut Spring; 7] {
        [
            &mut self.resting_width,
            &mut self.resting_height,
            &mut self.expanded_width,
            &mut self.expanded_height,
            &mut self.activity_blend,
            &mut self.badge_width,
            &mut self.track,
        ]
    }

    /// The page with the highest priority in `mode`; the earliest registered page wins ties.
    fn best_page(&self, mode: Mode) -> Option<usize> {
        highest_priority(self.pages.iter().map(|page| page.priority(mode)))
    }

    // Hover

    /// True while the notch is only the thin strip: it hides when idle, nothing is going on, and nobody is pointing at it.
    fn is_hidden(&self) -> bool {
        self.hides_when_idle
            && !self.is_revealed
            && self.expansion.target == 0.0
            && self.editor.is_none()
            && self.shown_transients.is_empty()
            && self.shown_badges.is_empty()
            && !self.has_resting_content
    }

    /// Switches hiding when idle on or off and remembers the choice.
    pub fn toggle_hide_when_idle(&mut self) {
        self.hides_when_idle = !self.hides_when_idle;
        self.settings.set_bool(HIDE_WHEN_IDLE_SETTING, self.hides_when_idle);
        self.flush();
    }

    pub fn hides_when_idle(&self) -> bool {
        self.hides_when_idle
    }

    fn set_hovering(&mut self, is_hovering: bool) {
        if self.is_hovering == is_hovering {
            return;
        }
        self.is_hovering = is_hovering;
        if !is_hovering {
            self.editor = None;
            self.is_gear_hovered = false;
        }
        log::debug(&format!("hovering={is_hovering}"));
        super::drop_target::set_hovering(is_hovering);
        // A hidden notch first shows its small pill, and opens only if the pointer stays.
        let waits_to_reveal = is_hovering && self.is_hidden();
        if waits_to_reveal {
            unsafe {
                SetTimer(Some(self.window), REVEAL_TIMER_ID, REVEAL_DELAY_MILLISECONDS, None);
            }
        } else {
            self.expansion.target = if is_hovering { 1.0 } else { 0.0 };
        }
        if !is_hovering {
            self.is_revealed = false;
            unsafe {
                let _ = KillTimer(Some(self.window), REVEAL_TIMER_ID);
                let _ = KillTimer(Some(self.window), EXPAND_TIMER_ID);
            }
        }
        self.start_animation();
        for index in 0..self.pages.len() {
            self.with_page(index, |page, context| page.on_hover_changed(is_hovering, context));
        }
        if !is_hovering {
            self.route_pointer(PointerEvent::Left);
        }
        unsafe {
            if is_hovering {
                SetTimer(Some(self.window), HOVER_WATCH_TIMER_ID, HOVER_WATCH_INTERVAL_MILLISECONDS, None);
            } else {
                let _ = KillTimer(Some(self.window), HOVER_WATCH_TIMER_ID);
            }
        }
    }

    fn pointer_is_over_window(&self) -> bool {
        let (cursor, bounds) = self.pointer_and_window_bounds();
        cursor.x >= bounds.left && cursor.x < bounds.right && cursor.y >= bounds.top && cursor.y < bounds.bottom
    }

    fn pointer_and_window_bounds(&self) -> (POINT, RECT) {
        let mut cursor = POINT::default();
        let mut bounds = RECT::default();
        unsafe {
            let _ = GetCursorPos(&mut cursor);
            let _ = GetWindowRect(self.window, &mut bounds);
        }
        (cursor, bounds)
    }

    /// Mouse coordinates arrive relative to the window, which only covers the pill; pages use surface space.
    fn to_surface_space(&self, window_x: i32, window_y: i32) -> Point {
        let (origin_x, origin_y) = self.region_origin.get();
        Point::new((window_x + origin_x) as f32, (window_y + origin_y) as f32)
    }

    // Animation

    fn start_animation(&mut self) {
        if self.is_animating {
            return;
        }
        self.is_animating = true;
        unsafe {
            timeBeginPeriod(1);
            let _ = QueryPerformanceCounter(&mut self.last_tick);
            SetTimer(Some(self.window), ANIMATION_TIMER_ID, self.frame_interval_milliseconds, None);
        }
    }

    fn stop_animation(&mut self) {
        self.is_animating = false;
        unsafe {
            let _ = KillTimer(Some(self.window), ANIMATION_TIMER_ID);
            timeEndPeriod(1);
        }
    }

    fn advance_animation(&mut self) {
        let mut now = 0;
        unsafe {
            let _ = QueryPerformanceCounter(&mut now);
        }
        let elapsed_seconds = ((now - self.last_tick) as f32 / self.ticks_per_second as f32).clamp(0.0, 0.05);
        self.last_tick = now;

        let mut is_settled = true;
        self.expansion.step(elapsed_seconds);
        is_settled &= self.expansion.is_settled();
        for spring in self.layout_springs() {
            spring.step(elapsed_seconds);
            is_settled &= spring.is_settled();
        }
        let mut pages_busy = false;
        for page in &mut self.pages {
            pages_busy |= page.animate(elapsed_seconds);
        }

        if is_settled && !pages_busy {
            self.expansion.snap_to_target();
            for spring in self.layout_springs() {
                spring.snap_to_target();
            }
            self.stop_animation();
        }
        self.paint();
    }

    // Painting

    fn frame(&self) -> Frame {
        let amount = self.expansion.position.clamp(-0.05, 1.1);
        let width = lerp(self.resting_width.position, self.expanded_width.position, amount) * self.scale;
        let height = lerp(self.resting_height.position, self.expanded_height.position, amount) * self.scale;
        let left = (WINDOW_WIDTH * self.scale - width) / 2.0;
        Frame {
            scale: self.scale,
            bounds: Rect::new(left, 0.0, left + width, height),
            expansion: self.expansion.position,
            expanded: Size::new(
                self.expanded_width.position - chrome::WIDTH_EXTRA,
                self.expanded_height.position - chrome::HEIGHT_EXTRA,
            ),
        }
    }

    /// Radius of the outward-curving fillets where the pill's sides meet the top of the screen, in device pixels.
    fn flare_size(&self, frame: &Frame) -> f32 {
        lerp(RESTING_FLARE, EXPANDED_FLARE, frame.expansion.clamp(0.0, 1.0)) * self.scale * self.visibility()
    }

    /// 0 while the notch is only a thin strip, 1 once it is a full pill.
    fn visibility(&self) -> f32 {
        ((self.resting_height.position - HIDDEN_HEIGHT) / (RESTING_HEIGHT - HIDDEN_HEIGHT))
            .clamp(0.0, 1.0)
            .max(self.expansion.position.clamp(0.0, 1.0))
    }

    /// Smallest surface rectangle that holds the pill and its flares, with a margin for anti-aliased edges.
    fn pill_region(&self, frame: &Frame) -> RECT {
        const MARGIN: i32 = 2;
        let (surface_width, surface_height) = (self.surface.width, self.surface.height);
        let side_extent = MARGIN + self.flare_size(frame).ceil() as i32;
        let left = (frame.bounds.left.floor() as i32 - side_extent).clamp(0, surface_width - 1);
        let right = (frame.bounds.right.ceil() as i32 + side_extent).clamp(left + 1, surface_width);
        let bottom = (frame.bounds.bottom.ceil() as i32 + MARGIN).clamp(1, surface_height);
        RECT { left, top: 0, right, bottom }
    }

    pub fn paint(&self) {
        let frame = self.frame();
        let region = self.pill_region(&frame);
        self.region_origin.set((region.left, region.top));
        let visibility = self.visibility();
        let resting_opacity = (1.0 - frame.expansion * 2.2).clamp(0.0, 1.0) * visibility;
        let fill = Color::BLACK.with_alpha(lerp(HIDDEN_ALPHA, 1.0, visibility));
        let blend = self.activity_blend.position.clamp(0.0, 1.0);
        let page_opacity = resting_opacity * (1.0 - blend);
        let activity_opacity = resting_opacity * blend;
        let expanded_opacity =
            ((frame.expansion - EXPANDED_CONTENT_START) / (1.0 - EXPANDED_CONTENT_START)).clamp(0.0, 1.0);

        // The drop target covers the collapsed pill, wherever it is now.
        if !self.is_hovering && frame.expansion <= 0.001 {
            super::drop_target::set_resting_area(RECT {
                left: self.screen_position.x + region.left,
                top: self.screen_position.y + region.top,
                right: self.screen_position.x + region.right,
                bottom: self.screen_position.y + region.bottom,
            });
        }
        let result = self.surface.render_and_present(
            self.window,
            self.screen_position,
            region,
            &self.fonts,
            self.scale,
            |canvas| {
                let bounds = frame.bounds;
                let radius = (lerp(RESTING_RADIUS, EXPANDED_RADIUS, frame.expansion.clamp(0.0, 1.0)) * self.scale)
                    .min(bounds.height() / 2.0);
                // The shape starts one radius above the screen edge so only the bottom corners show as rounded.
                canvas.fill_rounded_rect(
                    Rect::new(bounds.left, bounds.top - radius, bounds.right, bounds.bottom),
                    radius,
                    fill,
                );
                if visibility > 0.05 {
                    canvas.fill_top_flares(bounds, self.flare_size(&frame), Color::BLACK);
                }
                canvas.clipped(bounds, || {
                    if page_opacity > 0.01 {
                        // Badges take the right end of the pill, so the page draws in what is left of it.
                        let badge_room = self.badge_width.position.max(0.0) * self.scale;
                        let page_frame = Frame { bounds: Rect { right: bounds.right - badge_room, ..bounds }, ..frame };
                        self.pages[self.resting_index].draw_resting(canvas, &page_frame, page_opacity);
                        self.draw_badges(canvas, &frame, page_opacity);
                    }
                    if activity_opacity > 0.01 {
                        self.draw_transients(canvas, &frame, activity_opacity);
                    }
                    if expanded_opacity > 0.01 {
                        if self.editor.is_some() {
                            self.draw_editor(canvas, &frame, expanded_opacity);
                        } else {
                            self.draw_header(canvas, &frame, expanded_opacity);
                            self.draw_open_pages(canvas, &frame, expanded_opacity);
                        }
                    }
                });
            },
        );
        if let Err(error) = result {
            log::debug(&format!("paint failed: {error}"));
        }
    }

    /// Draws the open pages as a loop that slides sideways: mid-swipe the two pages next to the middle show together,
    /// and past the last page comes the first.
    fn draw_open_pages(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let page_frame = self.page_frame(frame);
        let count = self.navigable.len();
        let track = if count > 0 { self.track.position.rem_euclid(count as f32) } else { 0.0 };
        if count < 2 || (track - track.round()).abs() < 0.001 {
            self.pages[self.expanded_index].draw_expanded(canvas, &page_frame, opacity);
            return;
        }
        let pitch = page_frame.bounds.width() * PAGE_PITCH;
        let before = track.floor();
        for step in 0..2 {
            let position = before + step as f32;
            let offset = (position - track) * pitch;
            let mut shifted = page_frame;
            shifted.bounds.left += offset;
            shifted.bounds.right += offset;
            let fade = (1.0 - (position - track).abs() * PAGE_FADE_PER_PAGE).clamp(0.0, 1.0);
            let page = self.navigable[position as usize % count];
            self.pages[page].draw_expanded(canvas, &shifted, opacity * fade);
        }
    }

    /// Draws the page dots and the gear along the top of the open notch.
    fn draw_header(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        if self.navigable.len() >= 2 {
            let dots = chrome::dot_rects(self.navigable.len(), self.track.position, frame.bounds, self.scale);
            chrome::draw_dots(canvas, &dots, self.track.position, opacity);
        }
        chrome::draw_gear(canvas, frame.bounds, self.scale, self.is_gear_hovered, opacity);
    }

    /// Draws the transient activities side by side, centered in the pill, with a thin line between them.
    fn draw_transients(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let scale = self.scale;
        let widths: Vec<f32> = self.shown_transients.iter().map(|&(_, width)| width).collect();
        let offsets = activity_layout::offsets(&widths);
        let total = activity_layout::total_width(&widths) * scale;
        let first_left = frame.bounds.left + (frame.bounds.width() - total) / 2.0;
        let (top, bottom) = (frame.bounds.top, frame.bounds.top + RESTING_HEIGHT * scale);
        for (position, &(source, width)) in self.shown_transients.iter().enumerate() {
            let left = first_left + offsets[position] * scale;
            let area = Rect::new(left, top, left + width * scale, bottom);
            self.draw_source(source, canvas, area, opacity);
            if position > 0 {
                let divider = Rect::new(left - DIVIDER_WIDTH * scale, top + 9.0 * scale, left, bottom - 9.0 * scale);
                canvas.fill_rounded_rect(divider, 0.0, Color::gray(0.35).faded(opacity));
            }
        }
    }

    /// Draws the badges from the right end of the pill inward.
    fn draw_badges(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let scale = self.scale;
        let (top, bottom) = (frame.bounds.top, frame.bounds.top + RESTING_HEIGHT * scale);
        let mut right = frame.bounds.right - BADGE_EDGE_PADDING * scale;
        for &(source, width) in &self.shown_badges {
            let area = Rect::new(right - width * scale, top, right, bottom);
            self.draw_source(source, canvas, area, opacity);
            right = area.left;
        }
    }

    fn draw_source(&self, source: Source, canvas: &Canvas, area: Rect, opacity: f32) {
        match source {
            Source::Activity(index) => self.activities[index].activity.draw(canvas, area, self.scale, opacity),
            Source::Page(index) => self.pages[index].draw_presence(canvas, area, self.scale, opacity),
        }
    }
}

/// The settings key that switches an activity on or off.
fn activity_setting_key(id: &str) -> String {
    format!("activity.{id}")
}

/// Room the collapsed pill keeps for these badges, in device-independent pixels.
fn badge_space(badges: &[(Source, f32)]) -> f32 {
    if badges.is_empty() { 0.0 } else { badges.iter().map(|&(_, width)| width).sum::<f32>() + BADGE_EDGE_PADDING }
}

/// Index of the highest priority; `None` entries are skipped and the earliest entry wins ties.
fn highest_priority(priorities: impl IntoIterator<Item = Option<Priority>>) -> Option<usize> {
    let mut best: Option<(usize, Priority)> = None;
    for (index, priority) in priorities.into_iter().enumerate() {
        let Some(priority) = priority else { continue };
        if best.is_none_or(|(_, best_priority)| priority > best_priority) {
            best = Some((index, priority));
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
mod snapshots;

#[cfg(test)]
mod tests {
    use super::*;

    const LOW: Priority = Priority(1);
    const HIGH: Priority = Priority(5);

    #[test]
    fn no_page_wants_to_show() {
        assert_eq!(highest_priority([None, None]), None);
        assert_eq!(highest_priority(Vec::new()), None);
    }

    #[test]
    fn the_highest_priority_wins() {
        assert_eq!(highest_priority([Some(LOW), Some(HIGH), Some(LOW)]), Some(1));
    }

    #[test]
    fn the_earliest_page_wins_a_tie() {
        assert_eq!(highest_priority([Some(HIGH), Some(HIGH)]), Some(0));
    }

    #[test]
    fn pages_with_nothing_to_show_are_skipped() {
        assert_eq!(highest_priority([None, Some(LOW), None]), Some(1));
    }

    #[test]
    fn an_activity_is_switched_off_by_its_own_settings_key() {
        assert_eq!(activity_setting_key("volume"), "activity.volume");
    }

    #[test]
    fn badges_reserve_their_widths_plus_the_edge_padding() {
        assert_eq!(badge_space(&[]), 0.0);
        assert_eq!(badge_space(&[(Source::Activity(0), 14.0), (Source::Page(3), 14.0)]), 28.0 + BADGE_EDGE_PADDING);
    }
}
