use std::cell::Cell;

use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Media::{timeBeginPeriod, timeEndPeriod},
    System::{Performance::QueryPerformanceCounter, SystemInformation::GetTickCount64},
    UI::{
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::{GetCursorPos, KillTimer, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer, ShowWindow},
    },
};

use super::{
    activity_layout::{self, BADGE_EDGE_PADDING, DIVIDER_WIDTH},
    chrome, clock, display, navigation,
    navigation::PageList,
    page_editor::{self, ChipView, Entry, Outcome, PageEditor},
    screen_mode::ScreenMode,
    settings::Settings,
};
use crate::{
    activity::{ACTIVITY_TIMER_BASE, Activity, Presence, SystemMessage},
    gfx::{Canvas, Color, Fonts, Point, Rect, Size, Spring, Surface, lerp},
    log,
    page::{
        Context, Frame, Mode, Page, PointerEvent, Priority, RESTING_HEIGHT, Requests, split_owned_timer_key,
        split_timer_key,
    },
};

pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 260.0;
const DRAG_GROWTH: Size = Size::new(28.0, 14.0);

const RESTING_RADIUS: f32 = 12.0;
const BANNER_RADIUS: f32 = 18.0;
const BANNER_RADIUS_RANGE: f32 = 20.0;
const RESTING_BOTTOM_PADDING: f32 = 3.0;
const RESTING_PILL_HEIGHT: f32 = RESTING_HEIGHT + RESTING_BOTTOM_PADDING;
const EXPANDED_RADIUS: f32 = 22.0;
const SHADOW_LAYERS: usize = 10;
const SHADOW_STEP: f32 = 2.0;
const SHADOW_OFFSET: f32 = 4.0;
const SHADOW_LAYER_ALPHA: f32 = 0.028;
const RESTING_FLARE: f32 = 6.0;
const EXPANDED_FLARE: f32 = 12.0;
const ANIMATION_TIMER_ID: usize = 1;
/// Safety net while the notch is open: the leave notification can be lost when the window resizes under the pointer.
const HOVER_WATCH_TIMER_ID: usize = 2;
const HOVER_WATCH_INTERVAL_MILLISECONDS: u32 = 120;
const REVEAL_TIMER_ID: usize = 3;
const REVEAL_DELAY_MILLISECONDS: u32 = 150;
/// Over a maximized window the pointer must stay longer at the top, which is where its tabs and title bar are.
const REVEAL_DELAY_OVER_WINDOW_MILLISECONDS: u32 = 350;
const EXPAND_TIMER_ID: usize = 4;
const EXPAND_DELAY_MILLISECONDS: u32 = 450;
const HIDDEN_WIDTH: f32 = 240.0;
const HIDDEN_HEIGHT: f32 = 6.0;
const HIDDEN_HEIGHT_OVER_WINDOW: f32 = 3.0;
const HIDDEN_ALPHA: f32 = 0.006;
const HIDE_WHEN_IDLE_SETTING: &str = "notch.hide_when_idle";
const HIDE_FOR_APPS_SETTING: &str = "notch.hide_for_fullscreen";
const LYRICS_OVER_WINDOWS_SETTING: &str = "notch.lyrics_over_maximized";
const NOTCH_OVER_WINDOWS_SETTING: &str = "notch.show_over_maximized";
/// Expansion stages: 0 collapsed, HEADER_STAGE header-only, 1 fully open.
const HEADER_STAGE: f32 = 0.5;
const HEADER_STAGE_WIDTH: f32 = 240.0;
const HEADER_STAGE_HEIGHT: f32 = 52.0;
const STATUS_GAP: f32 = 8.0;
const RESTING_FADE_END: f32 = 0.25;
const HEADER_FADE: (f32, f32) = (0.2, 0.45);
const CONTENT_FADE: (f32, f32) = (0.62, 0.9);
const CLOCK_TIMER_ID: usize = 6;
const STAGE_TIMER_ID: usize = 7;
const CLOSE_TIMER_ID: usize = 8;
const RAISE_TIMER_ID: usize = 9;
/// The window the user switched to finishes raising itself after the foreground event, so raise the notch again.
const RAISE_DELAY_MILLISECONDS: u32 = 250;
const CLOSE_DELAY_MILLISECONDS: u32 = 200;
const HOVER_OPEN_DELAY_MILLISECONDS: u32 = 100;
const OPEN_STAGE_MILLISECONDS: u32 = 165;
const CLOSE_STAGE_MILLISECONDS: u32 = 110;
const OPEN_RESPONSE: (f32, f32) = (480.0, 29.0);
const CLOSE_RESPONSE: (f32, f32) = (330.0, 22.0);
const SWIPE_END_TIMER_ID: usize = 5;
const PAGE_FADE_PER_PAGE: f32 = 0.5;
const PAGE_GAP: f32 = 24.0;
/// The page area of the open notch while no page has anything to show: only the header remains.
const EMPTY_PAGE_SIZE: Size = Size::new(220.0, 0.0);
const LAST_PAGE_SETTING: &str = "navigation.page";
const PAGE_ORDER_SETTING: &str = "navigation.order";
const EDITOR_WIDTH: f32 = 428.0;
const PAGES_OFF_SETTING: &str = "navigation.disabled";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Activity(usize),
    Page(usize),
}

#[derive(Clone, Copy)]
enum HeaderMark {
    Page(usize),
    Activity(usize),
}

struct ActivitySlot {
    activity: Box<dyn Activity>,
    is_enabled: bool,
    is_started: bool,
}

pub struct Notch {
    window: HWND,
    scale: f32,
    screen_position: POINT,
    surface: Surface,
    fonts: Fonts,
    settings: Settings,
    pages: Vec<Box<dyn Page>>,
    activities: Vec<ActivitySlot>,
    transient_order: Vec<Source>,
    shown_transients: Vec<(Source, f32)>,
    shown_banner: Option<(Source, f32, f32)>,
    shown_badges: Vec<(Source, f32)>,
    resting_index: Option<usize>,
    expanded_index: Option<usize>,
    selected_page: Option<usize>,
    shown_page: Option<usize>,
    page_list: PageList,
    editor: Option<PageEditor>,
    navigable: Vec<usize>,
    is_settings_hovered: bool,
    track: Spring,
    swipe: Option<navigation::Swipe>,
    /// Finger input owns the fractional row position until its quiet period ends.
    holds_page_position: bool,
    pending: Requests,
    expansion: Spring,
    resting_width: Spring,
    resting_height: Spring,
    expanded_width: Spring,
    expanded_height: Spring,
    activity_blend: Spring,
    badge_width: Spring,
    hides_when_idle: bool,
    hides_for_apps: bool,
    keeps_lyrics_over_windows: bool,
    keeps_notch_over_windows: bool,
    screen_mode: ScreenMode,
    is_revealed: bool,
    has_resting_content: bool,
    is_hovering: bool,
    is_close_pending: bool,
    is_animating: bool,
    pointer_captured: bool,
    is_dragging_files: bool,
    last_tick: i64,
    ticks_per_second: i64,
    frame_interval_milliseconds: u32,
    region_origin: Cell<(i32, i32)>,
}

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
        let activities = activities
            .into_iter()
            .map(|activity| {
                let is_enabled =
                    !activity.is_optional() || settings.get_bool(&activity_setting_key(activity.id()), true);
                ActivitySlot { activity, is_enabled, is_started: false }
            })
            .collect();
        let hides_when_idle = settings.get_bool(HIDE_WHEN_IDLE_SETTING, false);
        let hides_for_apps = settings.get_bool(HIDE_FOR_APPS_SETTING, true);
        let keeps_lyrics_over_windows = settings.get_bool(LYRICS_OVER_WINDOWS_SETTING, true);
        let keeps_notch_over_windows = settings.get_bool(NOTCH_OVER_WINDOWS_SETTING, false);
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
            shown_banner: None,
            shown_badges: Vec::new(),
            resting_index: None,
            expanded_index: None,
            selected_page: None,
            shown_page: None,
            page_list: PageList::new(0),
            editor: None,
            navigable: Vec::new(),
            is_settings_hovered: false,
            track: navigation::page_transition(0.0),
            swipe: None,
            holds_page_position: false,
            pending: Requests::default(),
            expansion: Spring::new(0.0, OPEN_RESPONSE.0, OPEN_RESPONSE.1).with_rebound_damping(1.4),
            resting_width: Spring::new(0.0, 220.0, 26.0),
            resting_height: Spring::new(RESTING_PILL_HEIGHT, 260.0, 26.0),
            expanded_width: Spring::new(0.0, 200.0, 26.0),
            expanded_height: Spring::new(0.0, 220.0, 26.0),
            activity_blend: Spring::new(0.0, 300.0, 28.0),
            badge_width: Spring::new(0.0, 220.0, 26.0),
            hides_when_idle,
            hides_for_apps,
            keeps_lyrics_over_windows,
            keeps_notch_over_windows,
            screen_mode: ScreenMode::Normal,
            is_revealed: false,
            has_resting_content: false,
            is_hovering: false,
            is_close_pending: false,
            is_animating: false,
            pointer_captured: false,
            is_dragging_files: false,
            last_tick: 0,
            ticks_per_second,
            frame_interval_milliseconds: display::frame_interval_milliseconds(),
            region_origin: Cell::new((0, 0)),
        }
    }

    pub fn start(&mut self) {
        for (index, page) in self.pages.iter().enumerate() {
            log::debug(&format!("page {index}: {}", page.id()));
        }
        self.selected_page = self
            .settings
            .get_string(LAST_PAGE_SETTING)
            .and_then(|id| self.pages.iter().position(|page| page.id() == id));
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
        self.arm_clock_timer();
        self.refresh_layout();
        for spring in self.layout_springs() {
            spring.snap_to_target();
        }
        self.flush();
        self.paint();
    }

    fn arm_clock_timer(&self) {
        unsafe { SetTimer(Some(self.window), CLOCK_TIMER_ID, clock::milliseconds_until_next_minute(), None) };
    }

    pub fn pointer_moved(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        if !self.is_hovering {
            log::debug(&format!("enter move window=({window_x},{window_y}) surface=({:.0},{:.0})", point.x, point.y));
        }
        // The shadow remains during closing but must never count as hovering the pill.
        if !self.pointer_captured && !self.is_dragging_files && !self.pointer_is_over_window() {
            if self.is_hovering {
                self.pointer_left();
                self.flush();
            }
            return;
        }
        self.cancel_close();
        self.set_hovering(true);
        let frame = self.frame();
        let is_over_settings = chrome::SETTINGS_BUTTON_SHOWN
            && frame.is_open()
            && chrome::settings_button(frame.bounds, self.scale).contains(point);
        if is_over_settings != self.is_settings_hovered {
            self.is_settings_hovered = is_over_settings;
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

    fn drop_page(&self) -> Option<usize> {
        (0..self.pages.len()).find(|&index| self.page_list.is_enabled(index) && self.pages[index].accepts_drops())
    }

    fn show_drop_page(&mut self, index: usize) {
        if self.is_hovering && self.editor.is_none() {
            self.go_to_page(index);
        }
    }

    pub fn drag_entered(&mut self) {
        let Some(index) = self.drop_page() else { return };
        self.is_dragging_files = true;
        self.set_hovering(true);
        self.is_revealed = true;
        self.begin_opening();
        self.with_page(index, |page, context| page.on_file_drag_changed(true, context));
        self.show_drop_page(index);
        self.start_animation();
        self.flush();
    }

    pub fn drag_left(&mut self) {
        self.is_dragging_files = false;
        if let Some(index) = self.drop_page() {
            self.with_page(index, |page, context| page.on_file_drag_changed(false, context));
        }
        if !self.pointer_is_over_window() {
            self.set_hovering(false);
        }
        self.flush();
    }

    pub fn files_dropped(&mut self, paths: Vec<std::path::PathBuf>) {
        self.is_dragging_files = false;
        let Some(index) = self.drop_page() else { return };
        self.with_page(index, |page, context| {
            page.on_file_drag_changed(false, context);
            page.on_drop(paths, context);
        });
        self.show_drop_page(index);
        if !self.pointer_is_over_window() {
            self.set_hovering(false);
        }
        self.flush();
    }

    pub fn right_clicked(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        let frame = self.frame();
        if self.editor.is_some() || !frame.is_open() || !chrome::content_rect(frame.bounds, self.scale).contains(point)
        {
            return;
        }
        self.route_pointer(PointerEvent::SecondaryReleased(point));
        self.flush();
    }

    pub fn scrolled(&mut self, amount: i32, is_horizontal: bool, is_touchpad: bool) {
        if !self.is_hovering || self.editor.is_some() {
            return;
        }
        let toward_next = if is_horizontal { amount } else { -amount };
        match navigation::classify_scroll(toward_next, is_touchpad || is_horizontal || self.swipe.is_some()) {
            navigation::Scroll::Pages(steps) => {
                self.end_swipe();
                self.navigate(steps.clamp(-(navigation::MAX_WHEEL_TRAVEL as i32), navigation::MAX_WHEEL_TRAVEL as i32));
            }
            navigation::Scroll::Drag(pages) => self.drag_pages(pages),
        }
        self.flush();
    }

    fn drag_pages(&mut self, pages: f32) {
        if self.navigable.len() < 2 {
            return;
        }
        self.wrap_track();
        self.holds_page_position = true;
        let position = self.swipe.get_or_insert_with(|| navigation::Swipe::new(self.track.position)).drag_by(pages);
        // Smooth discrete wheel-message deltas without restarting the visible position or velocity.
        self.track.set_response(navigation::PAGE_DRAG_RESPONSE.0, navigation::PAGE_DRAG_RESPONSE.1);
        self.track.target = position;
        self.arm_swipe_end();
        self.start_animation();
    }

    fn arm_swipe_end(&self) {
        if let Some(swipe) = &self.swipe {
            let remaining_milliseconds = swipe.quiet_remaining().as_millis().max(1) as u32;
            unsafe {
                SetTimer(Some(self.window), SWIPE_END_TIMER_ID, remaining_milliseconds, None);
            }
        }
    }

    fn end_swipe(&mut self) {
        unsafe {
            let _ = KillTimer(Some(self.window), SWIPE_END_TIMER_ID);
        }
        if let Some(swipe) = self.swipe.take() {
            self.holds_page_position = false;
            self.track.set_response(navigation::TOUCHPAD_SNAP_RESPONSE.0, navigation::TOUCHPAD_SNAP_RESPONSE.1);
            self.track.target = swipe.destination();
            if !self.navigable.is_empty() {
                self.select_page(self.page_at(self.track.target));
            }
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
            self.pointer_left();
            self.flush();
        }
    }

    /// Handles `WM_MOUSELEAVE`; returns false when the leave was not real and tracking must be re-armed.
    pub fn pointer_left_window(&mut self) -> bool {
        if self.pointer_captured || self.is_dragging_files {
            return true;
        }
        // A resize under a stationary pointer can report a leave that did not happen; only believe a real one.
        let over_window = self.pointer_is_over_window();
        log::debug(&format!("mouse leave message, pointer still over window: {over_window}"));
        if over_window {
            return false;
        }
        self.pointer_left();
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
            CLOCK_TIMER_ID => {
                self.arm_clock_timer();
                if self.resting_index.is_none() || self.expansion.position > 0.001 {
                    self.paint();
                }
            }
            SWIPE_END_TIMER_ID => {
                if self.swipe.as_ref().is_some_and(|swipe| !swipe.quiet_remaining().is_zero()) {
                    // A WM_TIMER may already be queued when fresh scroll input resets its deadline.
                    self.arm_swipe_end();
                } else {
                    self.end_swipe();
                    self.flush();
                }
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
                if self.is_hovering && self.shown_banner.is_none() {
                    self.begin_opening();
                    self.start_animation();
                    self.flush();
                }
            }
            STAGE_TIMER_ID => {
                unsafe {
                    let _ = KillTimer(Some(self.window), STAGE_TIMER_ID);
                }
                self.expansion.target = if self.is_hovering { 1.0 } else { 0.0 };
                self.start_animation();
            }
            HOVER_WATCH_TIMER_ID => {
                if !self.pointer_captured
                    && !self.is_dragging_files
                    && !self.is_close_pending
                    && !self.pointer_is_over_window()
                {
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
                    self.pointer_left();
                    self.flush();
                }
            }
            RAISE_TIMER_ID => {
                unsafe {
                    let _ = KillTimer(Some(self.window), RAISE_TIMER_ID);
                }
                if !self.is_suppressed() {
                    super::window::raise_topmost(self.window);
                }
            }
            CLOSE_TIMER_ID => {
                unsafe {
                    let _ = KillTimer(Some(self.window), CLOSE_TIMER_ID);
                }
                self.is_close_pending = false;
                // The pointer may have come back without moving, which sends no message.
                if !self.pointer_captured && !self.is_dragging_files && !self.pointer_is_over_window() {
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

    pub fn system_message(&mut self, message: SystemMessage) {
        for index in 0..self.activities.len() {
            if self.activities[index].is_started {
                self.with_activity(index, |activity, context| activity.on_system_message(&message, context));
            }
        }
        self.flush();
    }

    pub fn activity_menu(&self) -> Vec<(usize, &'static str, bool)> {
        self.activities
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.activity.is_optional())
            .map(|(index, slot)| (index, slot.activity.label(), slot.is_enabled))
            .collect()
    }

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

    pub fn toggle_activity(&mut self, index: usize) {
        let Some(slot) = self.activities.get_mut(index) else { return };
        if !slot.activity.is_optional() {
            return;
        }
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

    fn with_page<R>(&mut self, index: usize, hook: impl FnOnce(&mut dyn Page, &mut Context) -> R) -> R {
        let mut context =
            Context::new(self.window, index, &self.surface, &self.fonts, &mut self.pending, &mut self.settings);
        hook(self.pages[index].as_mut(), &mut context)
    }

    fn navigate(&mut self, step: i32) {
        if self.navigable.len() < 2 {
            return;
        }
        self.wrap_track();
        self.holds_page_position = false;
        self.track.set_response(navigation::PAGE_SNAP_RESPONSE.0, navigation::PAGE_SNAP_RESPONSE.1);
        let target =
            navigation::limit_wheel_travel(self.track.position.round(), self.track.target.round() + step as f32);
        self.track.target = target;
        self.select_page(self.page_at(target));
        self.start_animation();
    }

    fn page_at(&self, position: f32) -> usize {
        self.navigable[(position.round() as i64).rem_euclid(self.navigable.len() as i64) as usize]
    }

    fn wrap_track(&mut self) {
        let count = self.navigable.len() as f32;
        if self.swipe.is_none() && self.track.is_settled() && count > 0.0 {
            self.track.jump_to(self.track.target.rem_euclid(count));
        }
    }

    fn go_to_page(&mut self, target: usize) {
        if Some(target) != self.expanded_index || self.holds_page_position {
            self.end_swipe();
            self.holds_page_position = false;
            self.track.set_response(navigation::PAGE_SNAP_RESPONSE.0, navigation::PAGE_SNAP_RESPONSE.1);
            self.select_page(target);
            self.start_animation();
        }
    }

    fn select_page(&mut self, target: usize) {
        self.selected_page = Some(target);
        let id = self.pages[target].id();
        self.settings.set_string(LAST_PAGE_SETTING, id);
    }

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
        if chrome::SETTINGS_BUTTON_SHOWN && chrome::settings_button(frame.bounds, self.scale).contains(point) {
            self.editor = Some(PageEditor::default());
            return true;
        }
        false
    }

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

    fn page_ids(&self) -> Vec<&'static str> {
        self.pages.iter().map(|page| page.id()).collect()
    }

    fn page_frame(&self, frame: &Frame) -> Frame {
        Frame { bounds: chrome::content_rect(frame.bounds, self.scale), ..*frame }
    }

    fn natural_page_frame(&self, index: usize, viewport: &Frame, center_offset: f32) -> Frame {
        let mut frame = self.page_frame(viewport);
        let width = self.pages[index].sizes().expanded.width;
        let center = frame.bounds.center_x() + center_offset * self.scale;
        frame.bounds.left = center - width * self.scale / 2.0;
        frame.bounds.right = center + width * self.scale / 2.0;
        frame.expanded.width = width;
        frame
    }

    fn page_blend(&self) -> Option<(usize, usize, f32)> {
        let count = self.navigable.len();
        if count < 2 {
            return None;
        }
        let position = self.track.position.rem_euclid(count as f32);
        let first = position.floor() as usize;
        Some((self.navigable[first], self.navigable[(first + 1) % count], position.fract()))
    }

    fn blended_page_layout(&self) -> Option<navigation::PageLayout> {
        let (first, next, progress) = self.page_blend()?;
        Some(navigation::page_layout(
            self.pages[first].sizes().expanded.width,
            self.pages[next].sizes().expanded.width,
            progress,
            PAGE_GAP,
        ))
    }

    fn visible_page_frames(&self, viewport: &Frame) -> Option<[(usize, Frame, f32); 2]> {
        let (first, next, progress) = self.page_blend()?;
        if progress < 0.001 || 1.0 - progress < 0.001 {
            return None;
        }
        let layout = self.blended_page_layout()?;
        Some([
            (first, self.natural_page_frame(first, viewport, layout.center_offsets[0]), progress),
            (next, self.natural_page_frame(next, viewport, layout.center_offsets[1]), 1.0 - progress),
        ])
    }

    fn follow_page_width(&mut self) {
        if self.editor.is_none()
            && let Some(layout) = self.blended_page_layout()
        {
            let growth = if self.is_dragging_files { DRAG_GROWTH.width } else { 0.0 };
            self.expanded_width.jump_to(layout.viewport_width + chrome::WIDTH_EXTRA + growth);
        }
    }

    fn route_pointer(&mut self, event: PointerEvent) {
        if self.editor.is_some() {
            return;
        }
        let viewport = self.frame();
        if let Some(pages) = self.visible_page_frames(&viewport) {
            for (index, frame, _) in pages {
                self.with_page(index, |page, context| page.on_pointer(event, &frame, context));
            }
            return;
        }
        let Some(index) = self.expanded_index else { return };
        let index = if self.navigable.len() < 2 { index } else { self.page_at(self.track.position) };
        let frame = self.natural_page_frame(index, &viewport, 0.0);
        self.with_page(index, |page, context| page.on_pointer(event, &frame, context));
    }

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

    fn sync_shown_page(&mut self) -> bool {
        let shown = (self.is_hovering && self.editor.is_none()).then_some(self.expanded_index).flatten();
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

    fn refresh_layout(&mut self) -> bool {
        let resting_index = self.best_page(Mode::Resting);
        let expanded_priorities: Vec<Option<Priority>> =
            self.pages.iter().map(|page| page.priority(Mode::Expanded)).collect();
        self.navigable = self.page_list.navigable(&expanded_priorities);
        let expanded_index = navigation::choose(self.selected_page, &self.page_list.effective(&expanded_priorities));
        let activities_changed = self.refresh_activities();
        let selection_changed =
            resting_index != self.resting_index || expanded_index != self.expanded_index || activities_changed;
        self.resting_index = resting_index;
        self.expanded_index = expanded_index;
        self.follow_open_page();

        let resting_width = resting_index.map_or(clock::RESTING_WIDTH, |index| self.pages[index].sizes().resting_width);
        let mut expanded = expanded_index.map_or(EMPTY_PAGE_SIZE, |index| self.pages[index].sizes().expanded);
        if (self.holds_page_position || !self.track.is_settled())
            && let Some(layout) = self.blended_page_layout()
        {
            expanded.width = layout.viewport_width;
        }
        if self.editor.is_some() {
            expanded = self.editor_size();
        }
        let transient_widths: Vec<f32> = self.shown_transients.iter().map(|&(_, width)| width).collect();
        let badge_space = badge_space(&self.shown_badges);
        let banner = self.shown_banner;
        let resting_target = if let Some((_, width, _)) = banner {
            width
        } else if transient_widths.is_empty() {
            resting_width + badge_space
        } else {
            activity_layout::total_width(&transient_widths)
        };
        self.has_resting_content = resting_index.is_some();
        let is_hidden = self.is_hidden();
        let banner_height = banner.map_or(RESTING_PILL_HEIGHT, |(_, _, height)| height + RESTING_BOTTOM_PADDING);
        let (resting_target, resting_height_target) = if is_hidden {
            (HIDDEN_WIDTH, self.hidden_height())
        } else {
            (resting_target.max(crate::page::RESTING_MIN_WIDTH), banner_height)
        };
        let growth = if self.is_dragging_files { DRAG_GROWTH } else { Size::new(0.0, 0.0) };
        let mut targets_changed = false;
        for (spring, target) in [
            (&mut self.resting_width, resting_target),
            (&mut self.resting_height, resting_height_target),
            (&mut self.expanded_width, expanded.width + chrome::WIDTH_EXTRA + growth.width),
            (&mut self.expanded_height, chrome::CONTENT_HEIGHT + chrome::HEIGHT_EXTRA + growth.height),
            (&mut self.activity_blend, if transient_widths.is_empty() && banner.is_none() { 0.0 } else { 1.0 }),
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

    /// Follow selection only when finger input does not own the row; jump while closed.
    fn follow_open_page(&mut self) {
        if self.swipe.is_some() {
            return;
        }
        if self.holds_page_position {
            if self.expanded_index == self.selected_page && self.navigable.len() > 1 {
                return;
            }
            self.holds_page_position = false;
        }
        let Some(position) = self.navigable.iter().position(|&index| Some(index) == self.expanded_index) else {
            return;
        };
        let count = self.navigable.len() as f32;
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

    fn refresh_activities(&mut self) -> bool {
        let mut transients = Vec::new();
        let mut badges = Vec::new();
        let mut banners = Vec::new();
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
                Some(Presence::Banner { width, height }) => banners.push((source, width, height)),
                None => {}
            }
        }
        let active: Vec<Source> = transients.iter().map(|&(source, _)| source).collect();
        activity_layout::track_order(&mut self.transient_order, &active);
        let banner = banners.last().copied();
        let shown: Vec<(Source, f32)> = if banner.is_some() {
            Vec::new()
        } else {
            activity_layout::newest(&self.transient_order)
                .iter()
                .filter_map(|&source| transients.iter().copied().find(|&(candidate, _)| candidate == source))
                .collect()
        };
        let changed = shown != self.shown_transients || badges != self.shown_badges || banner != self.shown_banner;
        self.shown_banner = banner;
        self.shown_transients = shown;
        self.shown_badges = badges;
        changed
    }

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

    fn best_page(&self, mode: Mode) -> Option<usize> {
        highest_priority(self.pages.iter().map(|page| page.priority(mode)))
    }

    fn is_hidden(&self) -> bool {
        let is_over_window = self.is_over_maximized_window();
        if is_over_window && self.keeps_notch_over_windows {
            return false;
        }
        // Over maximized windows, only active lyrics with the user setting can keep the pill visible.
        let resting_content_stays = if is_over_window {
            self.keeps_lyrics_over_windows
                && self.resting_index.is_some_and(|index| self.pages[index].shows_live_text())
        } else {
            self.has_resting_content
        };
        (self.hides_when_idle || is_over_window)
            && !self.is_revealed
            && self.expansion.target == 0.0
            && self.editor.is_none()
            && self.shown_transients.is_empty()
            && self.shown_banner.is_none()
            && self.shown_badges.is_empty()
            && !resting_content_stays
    }

    fn is_over_maximized_window(&self) -> bool {
        self.screen_mode == ScreenMode::Maximized
    }

    fn is_suppressed(&self) -> bool {
        self.hides_for_apps && self.screen_mode == ScreenMode::Fullscreen
    }

    fn hidden_height(&self) -> f32 {
        if self.hides_when_idle { HIDDEN_HEIGHT } else { HIDDEN_HEIGHT_OVER_WINDOW }
    }

    fn reveal_delay(&self) -> u32 {
        if self.hides_when_idle { REVEAL_DELAY_MILLISECONDS } else { REVEAL_DELAY_OVER_WINDOW_MILLISECONDS }
    }

    pub fn set_screen_mode(&mut self, mode: ScreenMode) {
        if mode != ScreenMode::Fullscreen || !self.hides_for_apps {
            super::window::raise_topmost(self.window);
            unsafe {
                SetTimer(Some(self.window), RAISE_TIMER_ID, RAISE_DELAY_MILLISECONDS, None);
            }
        }
        if mode == self.screen_mode {
            return;
        }
        let was_suppressed = self.is_suppressed();
        self.screen_mode = mode;
        self.apply_suppression(was_suppressed);
        self.flush();
    }

    pub fn reset_session_interaction(&mut self) {
        if self.pointer_captured {
            unsafe {
                let _ = ReleaseCapture();
            }
            self.pointer_captured = false;
        }
        self.is_dragging_files = false;
        self.set_hovering(false);
        self.is_revealed = false;
        self.cancel_close();
        self.end_swipe();
        for timer_key in [REVEAL_TIMER_ID, EXPAND_TIMER_ID, STAGE_TIMER_ID, HOVER_WATCH_TIMER_ID] {
            unsafe {
                let _ = KillTimer(Some(self.window), timer_key);
            }
        }
        self.route_pointer(PointerEvent::CaptureLost);
        self.expansion.jump_to(0.0);
        super::drop_target::reset_interaction();
        self.flush();
    }

    fn apply_suppression(&mut self, was_suppressed: bool) {
        let is_suppressed = self.is_suppressed();
        if is_suppressed == was_suppressed {
            return;
        }
        log::debug(&format!("suppressed={is_suppressed}"));
        if is_suppressed {
            self.set_hovering(false);
            unsafe {
                let _ = ShowWindow(self.window, SW_HIDE);
            }
        } else {
            unsafe {
                let _ = ShowWindow(self.window, SW_SHOWNOACTIVATE);
            }
        }
        super::drop_target::set_suppressed(is_suppressed);
        if !is_suppressed {
            self.paint();
        }
    }

    pub fn toggle_hide_for_apps(&mut self) {
        let was_suppressed = self.is_suppressed();
        self.hides_for_apps = !self.hides_for_apps;
        self.settings.set_bool(HIDE_FOR_APPS_SETTING, self.hides_for_apps);
        self.apply_suppression(was_suppressed);
        self.flush();
    }

    pub fn hides_for_apps(&self) -> bool {
        self.hides_for_apps
    }

    pub fn toggle_lyrics_over_maximized(&mut self) {
        self.keeps_lyrics_over_windows = !self.keeps_lyrics_over_windows;
        self.settings.set_bool(LYRICS_OVER_WINDOWS_SETTING, self.keeps_lyrics_over_windows);
        self.flush();
    }

    pub fn keeps_lyrics_over_windows(&self) -> bool {
        self.keeps_lyrics_over_windows
    }

    pub fn toggle_notch_over_maximized(&mut self) {
        self.keeps_notch_over_windows = !self.keeps_notch_over_windows;
        self.settings.set_bool(NOTCH_OVER_WINDOWS_SETTING, self.keeps_notch_over_windows);
        self.flush();
    }

    pub fn keeps_notch_over_windows(&self) -> bool {
        self.keeps_notch_over_windows
    }

    pub fn toggle_hide_when_idle(&mut self) {
        self.hides_when_idle = !self.hides_when_idle;
        self.settings.set_bool(HIDE_WHEN_IDLE_SETTING, self.hides_when_idle);
        self.flush();
    }

    pub fn hides_when_idle(&self) -> bool {
        self.hides_when_idle
    }

    /// Re-reads settings.txt after `WM_SETTINGS_CHANGED`; a missing or unreadable value keeps the current one.
    /// Hover, the open page and the page shown stay as they are.
    pub fn reload_settings(&mut self) {
        self.settings.reload();
        let was_suppressed = self.is_suppressed();
        self.hides_when_idle = self.settings.get_bool(HIDE_WHEN_IDLE_SETTING, self.hides_when_idle);
        self.hides_for_apps = self.settings.get_bool(HIDE_FOR_APPS_SETTING, self.hides_for_apps);
        self.keeps_lyrics_over_windows =
            self.settings.get_bool(LYRICS_OVER_WINDOWS_SETTING, self.keeps_lyrics_over_windows);
        self.keeps_notch_over_windows =
            self.settings.get_bool(NOTCH_OVER_WINDOWS_SETTING, self.keeps_notch_over_windows);
        let order_changed = self.reload_page_list();
        for index in 0..self.pages.len() {
            self.with_page(index, |page, context| page.on_settings_changed(context));
        }
        for index in 0..self.activities.len() {
            let slot = &mut self.activities[index];
            if slot.activity.is_optional() {
                slot.is_enabled = self.settings.get_bool(&activity_setting_key(slot.activity.id()), slot.is_enabled);
            }
            // A started listener thread cannot be stopped; switching off only hides the activity's presence.
            if self.activities[index].is_enabled {
                self.start_activity(index);
            }
            if self.activities[index].is_started {
                self.with_activity(index, |activity, context| activity.on_settings_changed(context));
            }
        }
        self.apply_suppression(was_suppressed);
        if order_changed {
            self.keep_shown_page_in_place();
        }
        self.flush();
    }

    /// Returns whether the page order changed; a list that switches every page off is ignored.
    fn reload_page_list(&mut self) -> bool {
        let ids = self.page_ids();
        let opt_in: Vec<bool> = self.pages.iter().map(|page| page.is_opt_in()).collect();
        let (current_order, current_disabled) = (self.page_list.order_text(&ids), self.page_list.disabled_text(&ids));
        let page_list = PageList::load(
            &ids,
            &opt_in,
            Some(self.settings.get_string(PAGE_ORDER_SETTING).unwrap_or(&current_order)),
            Some(self.settings.get_string(PAGES_OFF_SETTING).unwrap_or(&current_disabled)),
        );
        if !(0..self.pages.len()).any(|index| page_list.is_enabled(index)) {
            return false;
        }
        let order_changed = page_list.order() != self.page_list.order();
        self.page_list = page_list;
        order_changed
    }

    /// A new order moves the open page to another row position; jump there so the open notch does not scroll.
    fn keep_shown_page_in_place(&mut self) {
        self.refresh_layout();
        if self.swipe.is_some() || self.holds_page_position {
            return;
        }
        if let Some(position) = self.navigable.iter().position(|&index| Some(index) == self.expanded_index) {
            self.track.jump_to(position as f32);
        }
    }

    fn pointer_left(&mut self) {
        if self.is_close_pending {
            return;
        }
        self.route_pointer(PointerEvent::Left);
        if self.is_settings_hovered {
            self.is_settings_hovered = false;
            self.pending.repaint = true;
        }
        self.is_close_pending = true;
        unsafe {
            SetTimer(Some(self.window), CLOSE_TIMER_ID, CLOSE_DELAY_MILLISECONDS, None);
        }
    }

    fn cancel_close(&mut self) {
        if self.is_close_pending {
            self.is_close_pending = false;
            unsafe {
                let _ = KillTimer(Some(self.window), CLOSE_TIMER_ID);
            }
        }
    }

    fn begin_opening(&mut self) {
        self.expansion.set_response(OPEN_RESPONSE.0, OPEN_RESPONSE.1);
        if self.expansion.position < HEADER_STAGE - 0.05 {
            self.expansion.target = HEADER_STAGE;
            unsafe {
                SetTimer(Some(self.window), STAGE_TIMER_ID, OPEN_STAGE_MILLISECONDS, None);
            }
        } else {
            self.expansion.target = 1.0;
            unsafe {
                let _ = KillTimer(Some(self.window), STAGE_TIMER_ID);
            }
        }
    }

    fn begin_closing(&mut self) {
        self.expansion.set_response(CLOSE_RESPONSE.0, CLOSE_RESPONSE.1);
        if self.expansion.position > HEADER_STAGE + 0.05 {
            self.expansion.target = HEADER_STAGE;
            unsafe {
                SetTimer(Some(self.window), STAGE_TIMER_ID, CLOSE_STAGE_MILLISECONDS, None);
            }
        } else {
            self.expansion.target = 0.0;
            unsafe {
                let _ = KillTimer(Some(self.window), STAGE_TIMER_ID);
            }
        }
    }

    fn set_hovering(&mut self, is_hovering: bool) {
        if is_hovering && !self.is_suppressed() {
            super::window::restore_topmost(self.window);
        }
        if self.is_hovering == is_hovering {
            return;
        }
        self.is_hovering = is_hovering;
        if !is_hovering {
            self.editor = None;
            self.is_settings_hovered = false;
            self.cancel_close();
        }
        log::debug(&format!("hovering={is_hovering}"));
        super::drop_target::set_hovering(is_hovering);
        let waits_to_reveal = is_hovering && self.is_hidden();
        if waits_to_reveal {
            unsafe {
                SetTimer(Some(self.window), REVEAL_TIMER_ID, self.reveal_delay(), None);
            }
        } else {
            if is_hovering {
                unsafe {
                    SetTimer(Some(self.window), EXPAND_TIMER_ID, HOVER_OPEN_DELAY_MILLISECONDS, None);
                }
            } else {
                self.begin_closing();
            }
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
        for index in 0..self.activities.len() {
            if self.activities[index].is_started {
                self.with_activity(index, |activity, context| activity.on_hover_changed(is_hovering, context));
            }
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
        unsafe {
            let _ = GetCursorPos(&mut cursor);
        }
        let region = self.pill_region(&self.frame(), false);
        let bounds = RECT {
            left: self.screen_position.x + region.left,
            top: self.screen_position.y + region.top,
            right: self.screen_position.x + region.right,
            bottom: self.screen_position.y + region.bottom,
        };
        (cursor, bounds)
    }

    /// Mouse coordinates arrive relative to the window, which only covers the pill; pages use surface space.
    fn to_surface_space(&self, window_x: i32, window_y: i32) -> Point {
        let (origin_x, origin_y) = self.region_origin.get();
        Point::new((window_x + origin_x) as f32, (window_y + origin_y) as f32)
    }

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

        let follows_page_width = self.holds_page_position || !self.track.is_settled();
        self.expansion.step(elapsed_seconds);
        for spring in self.layout_springs() {
            spring.step(elapsed_seconds);
        }
        if follows_page_width {
            self.follow_page_width();
        }
        let is_settled = self.expansion.is_settled() && self.layout_springs().iter().all(|spring| spring.is_settled());
        let mut pages_busy = false;
        for page in &mut self.pages {
            pages_busy |= page.animate(elapsed_seconds);
        }

        if is_settled && !pages_busy {
            self.expansion.snap_to_target();
            for spring in self.layout_springs() {
                spring.snap_to_target();
            }
            if follows_page_width {
                self.follow_page_width();
            }
            self.stop_animation();
        }
        self.paint();
    }

    fn frame(&self) -> Frame {
        let amount = self.expansion.position.clamp(-0.08, 1.1);
        let (resting_width, full_width) = (self.resting_width.position, self.expanded_width.position);
        let (resting_height, full_height) = (self.resting_height.position, self.expanded_height.position);
        let header_width = HEADER_STAGE_WIDTH.min(full_width).max(resting_width);
        let header_height = HEADER_STAGE_HEIGHT.min(full_height).max(resting_height);
        let width = staged(resting_width, header_width, full_width, amount) * self.scale;
        let height = staged(resting_height, header_height, full_height, amount) * self.scale;
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

    fn flare_size(&self, frame: &Frame) -> f32 {
        lerp(RESTING_FLARE, EXPANDED_FLARE, frame.expansion.clamp(0.0, 1.0)) * self.scale * self.visibility()
    }

    fn visibility(&self) -> f32 {
        ((self.resting_height.position - self.hidden_height()) / (RESTING_PILL_HEIGHT - self.hidden_height()))
            .clamp(0.0, 1.0)
            .max(self.expansion.position.clamp(0.0, 1.0))
    }

    fn pill_region(&self, frame: &Frame, include_shadow: bool) -> RECT {
        const MARGIN: i32 = 2;
        let (surface_width, surface_height) = (self.surface.width, self.surface.height);
        let shadow = if include_shadow { frame.expansion.clamp(0.0, 1.0) * self.scale } else { 0.0 };
        let shadow_sides = (SHADOW_LAYERS as f32 * SHADOW_STEP * shadow).ceil() as i32;
        let shadow_below = ((SHADOW_LAYERS as f32 * SHADOW_STEP + SHADOW_OFFSET) * shadow).ceil() as i32;
        let side_extent = MARGIN + self.flare_size(frame).ceil() as i32 + shadow_sides;
        let left = (frame.bounds.left.floor() as i32 - side_extent).clamp(0, surface_width - 1);
        let right = (frame.bounds.right.ceil() as i32 + side_extent).clamp(left + 1, surface_width);
        let bottom = (frame.bounds.bottom.ceil() as i32 + MARGIN + shadow_below).clamp(1, surface_height);
        RECT { left, top: 0, right, bottom }
    }

    pub fn paint(&self) {
        if self.is_suppressed() {
            return;
        }
        let frame = self.frame();
        let region = self.pill_region(&frame, true);
        self.region_origin.set((region.left, region.top));
        let visibility = self.visibility();
        let resting_opacity = (1.0 - frame.expansion / RESTING_FADE_END).clamp(0.0, 1.0) * visibility;
        let fill = Color::BLACK.with_alpha(lerp(HIDDEN_ALPHA, 1.0, visibility));
        let blend = self.activity_blend.position.clamp(0.0, 1.0);
        let page_opacity = resting_opacity * (1.0 - blend);
        let activity_opacity = resting_opacity * blend;
        let header_opacity = ramp(frame.expansion, HEADER_FADE);
        let expanded_opacity = ramp(frame.expansion, CONTENT_FADE);

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
                let banner_amount =
                    ((self.resting_height.position - RESTING_PILL_HEIGHT) / BANNER_RADIUS_RANGE).clamp(0.0, 1.0);
                let resting_radius = lerp(RESTING_RADIUS, BANNER_RADIUS, banner_amount);
                let radius = (lerp(resting_radius, EXPANDED_RADIUS, frame.expansion.clamp(0.0, 1.0)) * self.scale)
                    .min(bounds.height() / 2.0);
                self.draw_shadow(canvas, bounds, radius, frame.expansion.clamp(0.0, 1.0) * visibility);
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
                    if expanded_opacity > 0.01 && self.editor.is_none() {
                        self.draw_backdrops(canvas, &frame, radius, expanded_opacity);
                    }
                    if page_opacity > 0.01 {
                        let badge_room = self.badge_width.position.max(0.0) * self.scale;
                        let page_frame = Frame { bounds: Rect { right: bounds.right - badge_room, ..bounds }, ..frame };
                        if let Some(index) = self.resting_index {
                            self.pages[index].draw_resting(canvas, &page_frame, page_opacity);
                        }
                        self.draw_badges(canvas, &frame, page_opacity);
                    }
                    let clock_opacity =
                        clock::opacity(self.resting_index.is_none(), visibility * (1.0 - blend), header_opacity);
                    if clock_opacity > 0.01 {
                        self.draw_clock(canvas, &frame, clock_opacity, header_opacity);
                    }
                    if activity_opacity > 0.01 {
                        if self.shown_banner.is_some() {
                            self.draw_banner(canvas, &frame, activity_opacity);
                        } else {
                            self.draw_transients(canvas, &frame, activity_opacity);
                        }
                    }
                    if header_opacity > 0.01 && self.editor.is_none() {
                        self.draw_header(canvas, &frame, header_opacity);
                    }
                    if expanded_opacity > 0.01 {
                        if self.editor.is_some() {
                            self.draw_editor(canvas, &frame, expanded_opacity);
                        } else {
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

    /// Backdrops crossfade in place while page content slides.
    fn draw_backdrops(&self, canvas: &Canvas, frame: &Frame, radius: f32, opacity: f32) {
        let Some(expanded_index) = self.expanded_index else { return };
        if let Some(pages) = self.visible_page_frames(frame) {
            for (index, _, distance) in pages {
                let page_frame = self.natural_page_frame(index, frame, 0.0);
                self.pages[index].draw_backdrop(canvas, &page_frame, frame.bounds, radius, opacity * (1.0 - distance));
            }
            return;
        }
        let index = if self.navigable.len() < 2 { expanded_index } else { self.page_at(self.track.position) };
        let page_frame = self.natural_page_frame(index, frame, 0.0);
        self.pages[index].draw_backdrop(canvas, &page_frame, frame.bounds, radius, opacity);
    }

    fn draw_open_pages(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let Some(expanded_index) = self.expanded_index else { return };
        if let Some(pages) = self.visible_page_frames(frame) {
            for (index, page_frame, distance) in pages {
                let fade = (1.0 - distance * PAGE_FADE_PER_PAGE).clamp(0.0, 1.0);
                self.pages[index].draw_expanded(canvas, &page_frame, opacity * fade);
            }
            return;
        }
        let index = if self.navigable.len() < 2 { expanded_index } else { self.page_at(self.track.position) };
        let page_frame = self.natural_page_frame(index, frame, 0.0);
        self.pages[index].draw_expanded(canvas, &page_frame, opacity);
    }

    fn draw_clock(&self, canvas: &Canvas, frame: &Frame, opacity: f32, header_opacity: f32) {
        let (bounds, scale) = (frame.bounds, self.scale);
        let badge_room = self.badge_width.position.max(0.0) * scale;
        let collapsed =
            Point::new((bounds.left + bounds.right - badge_room) / 2.0, bounds.top + RESTING_HEIGHT * scale / 2.0);
        let time_width = clock::text_width(canvas) * scale;
        let header = chrome::header_rect(bounds, scale);
        let open = Point::new(header.right - time_width / 2.0, header.center_y());
        let center = clock::center(collapsed, open, (frame.expansion / HEADER_STAGE).clamp(0.0, 1.0));
        let area = Rect::around(center, clock::HALF_WIDTH * scale, RESTING_HEIGHT * scale / 2.0);
        clock::draw_time(canvas, area, opacity);
        if header_opacity > 0.01 && self.editor.is_none() {
            self.draw_header_statuses(canvas, center.x - time_width / 2.0, center.y, header_opacity);
        }
    }

    fn draw_header_statuses(&self, canvas: &Canvas, right: f32, center_y: f32, opacity: f32) {
        let scale = self.scale;
        let mut entries: Vec<(u8, f32, HeaderMark)> = Vec::new();
        for (index, page) in self.pages.iter().enumerate() {
            if !self.page_list.is_enabled(index) {
                continue;
            }
            if let Some(width) = page.header_status_width() {
                entries.push((page.header_status_rank(), width, HeaderMark::Page(index)));
            }
        }
        for (index, slot) in self.activities.iter().enumerate() {
            if !slot.is_enabled || !slot.is_started {
                continue;
            }
            if let Some(width) = slot.activity.header_status_width() {
                entries.push((0, width, HeaderMark::Activity(index)));
            }
        }
        entries.sort_by_key(|&(rank, _, _)| rank);
        let half_height = chrome::HEADER_HEIGHT * scale / 2.0;
        let mut right = right - STATUS_GAP * scale;
        for (_, width, mark) in entries {
            let area = Rect::new(right - width * scale, center_y - half_height, right, center_y + half_height);
            match mark {
                HeaderMark::Page(index) => self.pages[index].draw_header_status(canvas, area, scale, opacity),
                HeaderMark::Activity(index) => {
                    self.activities[index].activity.draw_header_status(canvas, area, scale, opacity)
                }
            }
            right = area.left - STATUS_GAP * scale;
        }
    }

    fn draw_shadow(&self, canvas: &Canvas, bounds: Rect, radius: f32, strength: f32) {
        if strength <= 0.01 {
            return;
        }
        let color = Color::BLACK.with_alpha(SHADOW_LAYER_ALPHA * strength);
        for layer in (1..=SHADOW_LAYERS).rev() {
            let grow = layer as f32 * SHADOW_STEP * self.scale;
            let rect = Rect::new(
                bounds.left - grow,
                bounds.top - radius,
                bounds.right + grow,
                bounds.bottom + grow + SHADOW_OFFSET * self.scale,
            );
            canvas.fill_rounded_rect(rect, radius + grow, color);
        }
    }

    fn draw_header(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        if self.navigable.len() >= 2 {
            let dots = chrome::dot_rects(self.navigable.len(), self.track.position, frame.bounds, self.scale);
            chrome::draw_dots(canvas, &dots, self.track.position, opacity);
        }
        if chrome::SETTINGS_BUTTON_SHOWN {
            chrome::draw_settings(canvas, frame.bounds, self.scale, self.is_settings_hovered, opacity);
        }
    }

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

    fn draw_banner(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let Some((source, _, _)) = self.shown_banner else { return };
        let bounds = frame.bounds;
        let area =
            Rect::new(bounds.left, bounds.top, bounds.right, bounds.bottom - RESTING_BOTTOM_PADDING * self.scale);
        self.draw_source(source, canvas, area, opacity);
    }

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

/// Interpolate through the header stage and extrapolate past the endpoints for spring overshoot.
fn staged(resting: f32, header: f32, full: f32, expansion: f32) -> f32 {
    if expansion <= HEADER_STAGE {
        lerp(resting, header, expansion / HEADER_STAGE)
    } else {
        lerp(header, full, (expansion - HEADER_STAGE) / (1.0 - HEADER_STAGE))
    }
}

fn ramp(expansion: f32, range: (f32, f32)) -> f32 {
    ((expansion - range.0) / (range.1 - range.0)).clamp(0.0, 1.0)
}

fn activity_setting_key(id: &str) -> String {
    format!("activity.{id}")
}

fn badge_space(badges: &[(Source, f32)]) -> f32 {
    if badges.is_empty() { 0.0 } else { badges.iter().map(|&(_, width)| width).sum::<f32>() + BADGE_EDGE_PADDING }
}

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

    #[test]
    fn a_staged_size_passes_through_the_header_pill_on_the_way_to_the_open_notch() {
        assert_eq!(staged(96.0, 240.0, 280.0, 0.0), 96.0);
        assert_eq!(staged(96.0, 240.0, 280.0, HEADER_STAGE), 240.0);
        assert_eq!(staged(96.0, 240.0, 280.0, 1.0), 280.0);
        assert_eq!(staged(96.0, 240.0, 280.0, 0.25), 168.0);
        assert_eq!(staged(96.0, 240.0, 280.0, 0.75), 260.0);
    }

    #[test]
    fn a_staged_size_goes_past_both_ends_when_the_spring_overshoots() {
        assert!(staged(96.0, 240.0, 280.0, 1.05) > 280.0);
        assert!(staged(96.0, 240.0, 280.0, -0.05) < 96.0);
    }

    #[test]
    fn the_header_shows_before_the_page_content_and_the_collapsed_content_is_gone_first() {
        assert!(RESTING_FADE_END <= HEADER_FADE.0 + 0.1);
        assert!(HEADER_FADE.1 <= CONTENT_FADE.0);
        assert_eq!(ramp(HEADER_STAGE, HEADER_FADE), 1.0);
        assert_eq!(ramp(HEADER_STAGE, CONTENT_FADE), 0.0);
        assert_eq!(ramp(1.0, CONTENT_FADE), 1.0);
    }

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
