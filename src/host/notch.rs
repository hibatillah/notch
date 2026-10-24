use std::cell::Cell;

use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Media::{timeBeginPeriod, timeEndPeriod},
    System::Performance::QueryPerformanceCounter,
    UI::{
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::{GetCursorPos, GetWindowRect, KillTimer, SetTimer},
    },
};

use super::{display, settings::Settings};
use crate::{
    gfx::{Color, Fonts, Point, Rect, Size, Spring, Surface, lerp},
    log,
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, RESTING_HEIGHT, Requests, split_timer_key},
};

/// Wide enough for a long lyric line; the window only occupies the pill's area, so this costs no screen space.
pub const WINDOW_WIDTH: f32 = 1200.0;
pub const WINDOW_HEIGHT: f32 = 175.0;

const RESTING_RADIUS: f32 = 12.0;
const EXPANDED_RADIUS: f32 = 22.0;
/// Radius of the outward curve joining the pill's sides to the top of the screen, as on a MacBook notch.
const RESTING_FLARE: f32 = 8.0;
const EXPANDED_FLARE: f32 = 14.0;
const ANIMATION_TIMER_ID: usize = 1;
/// Safety net while the notch is open: the leave notification can be lost when the window resizes under the pointer.
const HOVER_WATCH_TIMER_ID: usize = 2;
const HOVER_WATCH_INTERVAL_MILLISECONDS: u32 = 120;
/// The expansion past which content starts to appear; below it only the collapsed content shows.
const EXPANDED_CONTENT_START: f32 = 0.55;

/// Owns the window's state: it animates the pill, tracks the pointer and decides which page is shown.
pub struct Notch {
    window: HWND,
    scale: f32,
    screen_position: POINT,
    surface: Surface,
    fonts: Fonts,
    settings: Settings,
    pages: Vec<Box<dyn Page>>,
    /// The page drawn while collapsed and the page drawn while open; they can differ.
    resting_index: usize,
    expanded_index: usize,
    /// Requests collected from page hooks, applied by `flush`.
    pending: Requests,
    expansion: Spring,
    resting_width: Spring,
    expanded_width: Spring,
    expanded_height: Spring,
    is_hovering: bool,
    is_animating: bool,
    pointer_captured: bool,
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
    pub ticks_per_second: i64,
}

impl Notch {
    pub fn new(parts: NotchParts) -> Self {
        let NotchParts { window, scale, screen_position, surface, fonts, settings, pages, ticks_per_second } = parts;
        assert!(!pages.is_empty(), "the notch needs at least one page");
        Self {
            window,
            scale,
            screen_position,
            surface,
            fonts,
            settings,
            pages,
            resting_index: 0,
            expanded_index: 0,
            pending: Requests::default(),
            expansion: Spring::new(0.0, 260.0, 22.0),
            resting_width: Spring::new(0.0, 220.0, 26.0),
            expanded_width: Spring::new(0.0, 200.0, 26.0),
            expanded_height: Spring::new(0.0, 220.0, 26.0),
            is_hovering: false,
            is_animating: false,
            pointer_captured: false,
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
        for index in 0..self.pages.len() {
            self.with_page(index, |page, context| page.on_start(context));
        }
        self.refresh_layout();
        for spring in [&mut self.resting_width, &mut self.expanded_width, &mut self.expanded_height] {
            spring.snap_to_target();
        }
        self.flush();
        self.paint();
    }

    // ---- input from the window ------------------------------------------------------------------------------

    pub fn pointer_moved(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        if !self.is_hovering {
            log::debug(&format!("enter move window=({window_x},{window_y}) surface=({:.0},{:.0})", point.x, point.y));
        }
        self.set_hovering(true);
        self.route_pointer(PointerEvent::Moved(point));
        self.flush();
    }

    pub fn pointer_pressed(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        self.route_pointer(PointerEvent::Pressed(point));
        self.flush();
    }

    pub fn pointer_released(&mut self, window_x: i32, window_y: i32) {
        let point = self.to_surface_space(window_x, window_y);
        let was_captured = self.pointer_captured;
        self.route_pointer(PointerEvent::Released(point));
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
        if self.pointer_captured {
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
            HOVER_WATCH_TIMER_ID => {
                if !self.pointer_captured && !self.pointer_is_over_window() {
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

    // ---- page plumbing --------------------------------------------------------------------------------------

    /// Runs a page hook with a `Context`; whatever the page requests is applied by the next `flush`.
    fn with_page<R>(&mut self, index: usize, hook: impl FnOnce(&mut dyn Page, &mut Context) -> R) -> R {
        let mut context =
            Context::new(self.window, index, &self.surface, &self.fonts, &mut self.pending, &mut self.settings);
        hook(self.pages[index].as_mut(), &mut context)
    }

    fn route_pointer(&mut self, event: PointerEvent) {
        let frame = self.frame();
        let index = self.expanded_index;
        self.with_page(index, |page, context| page.on_pointer(event, &frame, context));
    }

    /// Applies what pages asked for, re-selects the shown pages and repaints if anything changed.
    fn flush(&mut self) {
        let requests = std::mem::take(&mut self.pending);
        match requests.pointer_capture {
            Some(true) => {
                self.pointer_captured = true;
                unsafe { SetCapture(self.window) };
            }
            Some(false) => {
                self.pointer_captured = false;
                unsafe {
                    let _ = ReleaseCapture();
                }
            }
            None => {}
        }
        let layout_changed = self.refresh_layout();
        if requests.animate {
            self.start_animation();
        }
        if requests.repaint || layout_changed {
            self.paint();
        }
    }

    /// Picks the page to show in each mode and animates toward its sizes; returns true if the selection changed.
    fn refresh_layout(&mut self) -> bool {
        let resting_index = self.best_page(Mode::Resting).unwrap_or(self.resting_index);
        let expanded_index = self.best_page(Mode::Expanded).unwrap_or(self.expanded_index);
        let selection_changed = resting_index != self.resting_index || expanded_index != self.expanded_index;
        self.resting_index = resting_index;
        self.expanded_index = expanded_index;

        let PageSizes { resting_width, .. } = self.pages[resting_index].sizes();
        let PageSizes { expanded, .. } = self.pages[expanded_index].sizes();
        let mut targets_changed = false;
        for (spring, target) in [
            (&mut self.resting_width, resting_width),
            (&mut self.expanded_width, expanded.width),
            (&mut self.expanded_height, expanded.height),
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

    /// The page with the highest priority in `mode`; the earliest registered page wins ties.
    fn best_page(&self, mode: Mode) -> Option<usize> {
        highest_priority(self.pages.iter().map(|page| page.priority(mode)))
    }

    // ---- hover ----------------------------------------------------------------------------------------------

    fn set_hovering(&mut self, is_hovering: bool) {
        if self.is_hovering == is_hovering {
            return;
        }
        self.is_hovering = is_hovering;
        log::debug(&format!("hovering={is_hovering}"));
        self.expansion.target = if is_hovering { 1.0 } else { 0.0 };
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

    // ---- animation ------------------------------------------------------------------------------------------

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

        let springs =
            [&mut self.expansion, &mut self.resting_width, &mut self.expanded_width, &mut self.expanded_height];
        let mut is_settled = true;
        for spring in springs {
            spring.step(elapsed_seconds);
            is_settled &= spring.is_settled();
        }
        let mut pages_busy = false;
        for page in &mut self.pages {
            pages_busy |= page.animate(elapsed_seconds);
        }

        if is_settled && !pages_busy {
            for spring in
                [&mut self.expansion, &mut self.resting_width, &mut self.expanded_width, &mut self.expanded_height]
            {
                spring.snap_to_target();
            }
            self.stop_animation();
        }
        self.paint();
    }

    // ---- painting -------------------------------------------------------------------------------------------

    fn frame(&self) -> Frame {
        let amount = self.expansion.position.clamp(-0.05, 1.1);
        let width = lerp(self.resting_width.position, self.expanded_width.position, amount) * self.scale;
        let height = lerp(RESTING_HEIGHT, self.expanded_height.position, amount) * self.scale;
        let left = (WINDOW_WIDTH * self.scale - width) / 2.0;
        Frame {
            scale: self.scale,
            bounds: Rect::new(left, 0.0, left + width, height),
            expansion: self.expansion.position,
            expanded: Size::new(self.expanded_width.position, self.expanded_height.position),
        }
    }

    /// Radius of the outward-curving fillets where the pill's sides meet the top of the screen, in device pixels.
    fn flare_size(&self, frame: &Frame) -> f32 {
        lerp(RESTING_FLARE, EXPANDED_FLARE, frame.expansion.clamp(0.0, 1.0)) * self.scale
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
        let resting_opacity = (1.0 - frame.expansion * 2.2).clamp(0.0, 1.0);
        let expanded_opacity =
            ((frame.expansion - EXPANDED_CONTENT_START) / (1.0 - EXPANDED_CONTENT_START)).clamp(0.0, 1.0);

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
                    Color::BLACK,
                );
                canvas.fill_top_flares(bounds, self.flare_size(&frame), Color::BLACK);
                canvas.clipped(bounds, || {
                    if resting_opacity > 0.01 {
                        self.pages[self.resting_index].draw_resting(canvas, &frame, resting_opacity);
                    }
                    if expanded_opacity > 0.01 {
                        self.pages[self.expanded_index].draw_expanded(canvas, &frame, expanded_opacity);
                    }
                });
            },
        );
        if let Err(error) = result {
            log::debug(&format!("paint failed: {error}"));
        }
    }
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
}
