//! The contract between the notch host and the pages it shows.
//!
//! A page is one self-contained feature of the notch, such as the clock or the media player. It tells the host
//! when it wants to be shown (`priority`), how big it wants to be (`sizes`) and how to draw itself collapsed and
//! expanded. Everything else (input, timers, wake-ups from background threads) is an optional hook.
//! See `ARCHITECTURE.md` for how to add a page.

use windows::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, WPARAM},
        System::{DataExchange::AddClipboardFormatListener, Power::RegisterPowerSettingNotification},
        UI::WindowsAndMessaging::{KillTimer, PostMessageW, REGISTER_NOTIFICATION_FLAGS, SetTimer, WM_APP},
    },
    core::GUID,
};

use crate::{
    activity::Presence,
    gfx::{Canvas, Fonts, ImageBrush, ImageData, Point, Rect, Size, Surface},
    host::Settings,
    ui::Icon,
};

/// Height of the collapsed pill in device-independent pixels; pages center their resting content in it.
pub const RESTING_HEIGHT: f32 = 28.0;

/// Message a `Notifier` posts to wake the host from another thread.
pub(crate) const WM_PAGE_NOTIFY: u32 = WM_APP + 1;
/// Win32 timer ids from this up to `PAGE_TIMER_LIMIT` belong to pages; each page owns `TIMERS_PER_PAGE` of them.
pub(crate) const PAGE_TIMER_BASE: usize = 100;
pub(crate) const TIMERS_PER_PAGE: usize = 16;
/// Timer ids from here up belong to activities.
pub(crate) const PAGE_TIMER_LIMIT: usize = 1000;

/// How much a page wants to be shown right now; when several pages want to, the highest priority wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

impl Priority {
    /// For pages that are always available as a fallback, such as the clock.
    pub const BACKGROUND: Priority = Priority(0);
    /// For pages that show live content, such as music that is playing.
    pub const NORMAL: Priority = Priority(10);
    /// For pages that must interrupt everything else, such as a notification that has just arrived.
    pub const URGENT: Priority = Priority(20);
}

/// Whether the notch is collapsed or open under the pointer. A page can be a different answer in each mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Resting,
    Expanded,
}

/// Sizes a page wants, in device-independent pixels. The host animates toward them. `expanded` is the size of the
/// page's own content: the host adds its padding and the header with the page dots around it.
#[derive(Clone, Copy, Debug)]
pub struct PageSizes {
    pub resting_width: f32,
    pub expanded: Size,
}

/// Where the pill is right now, passed to drawing and pointer hooks.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub scale: f32,
    /// Where the page draws, in device pixels, in the same space as pointer positions. Collapsed, this is the pill;
    /// open, it is the pill without the padding and the header.
    pub bounds: Rect,
    /// 0 collapsed, 1 fully open; the spring can overshoot a little past 1.
    pub expansion: f32,
    /// The animated size of the page's content when open, in device-independent pixels.
    pub expanded: Size,
}

impl Frame {
    /// True once the notch is open far enough for its controls to be usable.
    pub fn is_open(&self) -> bool {
        self.expansion >= 0.8
    }
}

/// Pointer input, delivered to the page that is shown while the notch is open. Positions are device pixels.
#[derive(Clone, Copy, Debug)]
pub enum PointerEvent {
    Moved(Point),
    Pressed(Point),
    Released(Point),
    /// The pointer left the notch.
    Left,
    /// The pointer was captured for a drag in the notch itself, and the capture ended.
    CaptureLost,
    /// The secondary (right) button was released.
    SecondaryReleased(Point),
}

/// Identifies one of a page's timers. Must be below `TIMERS_PER_PAGE` (16).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerId(pub u8);

pub trait Page {
    /// Stable name used in logs and settings keys.
    fn id(&self) -> &'static str;

    /// How much this page wants to be shown in `mode`; `None` means it has nothing to show.
    /// Asked again after every event, so it can follow the page's state.
    fn priority(&self, mode: Mode) -> Option<Priority>;

    /// The sizes this page wants. Asked again after every event, so it can follow the page's content.
    fn sizes(&self) -> PageSizes;

    /// Draws the collapsed pill content. `opacity` fades it out while the notch opens.
    fn draw_resting(&self, canvas: &Canvas, frame: &Frame, opacity: f32);

    /// Draws the open content. `opacity` fades it in while the notch opens.
    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32);

    /// Called once after the window exists: start background work and timers here.
    fn on_start(&mut self, _context: &mut Context) {}

    /// Called on every page when the pointer enters or leaves the notch.
    fn on_hover_changed(&mut self, _is_hovering: bool, _context: &mut Context) {}

    /// Items this page adds to the tray menu, such as "Use calendar link from clipboard".
    fn menu_items(&self) -> Vec<String> {
        Vec::new()
    }

    /// Called when the user picks the item at `index` of `menu_items`.
    fn on_menu_item(&mut self, _index: usize, _context: &mut Context) {}

    /// True for a page that takes files dropped on the notch; the first such page that is switched on gets them.
    fn accepts_drops(&self) -> bool {
        false
    }

    /// Called with the files dropped on the notch, when `accepts_drops` is true.
    fn on_drop(&mut self, _paths: Vec<std::path::PathBuf>, _context: &mut Context) {}

    /// Called when this page starts or stops being the one on screen: the notch is open and showing it. A page that
    /// measures something only while it is looked at starts and stops its work here.
    fn on_shown_changed(&mut self, _is_shown: bool, _context: &mut Context) {}

    fn on_pointer(&mut self, _event: PointerEvent, _frame: &Frame, _context: &mut Context) {}

    fn on_timer(&mut self, _timer: TimerId, _context: &mut Context) {}

    /// Called after a background thread used this page's `Notifier`.
    fn on_notify(&mut self, _context: &mut Context) {}

    /// What this page shows in the collapsed pill while it is not the page on show, as an activity does: a moment
    /// of its own (`Transient`) or a small mark kept beside the page that is on show (`Badge`). Asked after every event.
    fn presence(&self) -> Option<Presence> {
        None
    }

    /// Draws what `presence` asked for into `area` (device pixels), faded by `opacity`.
    fn draw_presence(&self, _canvas: &Canvas, _area: Rect, _scale: f32, _opacity: f32) {}

    /// The name shown for the page when the user edits the page list.
    fn name(&self) -> &'static str {
        self.id()
    }

    /// The glyph shown beside the name when the user edits the page list.
    fn icon(&self) -> Icon {
        Icon::Clock
    }

    /// True for a page that starts switched off until the user adds it.
    fn is_opt_in(&self) -> bool {
        false
    }

    /// Advances the page's own animations; return true while any is still moving. Only called while animating.
    fn animate(&mut self, _elapsed_seconds: f32) -> bool {
        false
    }
}

/// Things a page asked for during one hook; the host applies them afterwards.
#[derive(Default)]
pub(crate) struct Requests {
    pub repaint: bool,
    pub animate: bool,
}

/// The page's way to ask the host for things. Created by the host for each hook call.
pub struct Context<'a> {
    window: HWND,
    /// Index of the owning page or activity, which selects its timers and its wake-up messages.
    owner_index: usize,
    timer_base: usize,
    notify_message: u32,
    surface: &'a Surface,
    fonts: &'a Fonts,
    requests: &'a mut Requests,
    /// Persistent settings shared by all pages and activities; prefix keys with the page or activity id.
    pub settings: &'a mut Settings,
}

impl<'a> Context<'a> {
    pub(crate) fn new(
        window: HWND,
        page_index: usize,
        surface: &'a Surface,
        fonts: &'a Fonts,
        requests: &'a mut Requests,
        settings: &'a mut Settings,
    ) -> Self {
        Self {
            window,
            owner_index: page_index,
            timer_base: PAGE_TIMER_BASE,
            notify_message: WM_PAGE_NOTIFY,
            surface,
            fonts,
            requests,
            settings,
        }
    }

    /// The same context for an activity: its timers and wake-ups are kept apart from the pages'.
    pub(crate) fn for_activity(
        window: HWND,
        activity_index: usize,
        surface: &'a Surface,
        fonts: &'a Fonts,
        requests: &'a mut Requests,
        settings: &'a mut Settings,
    ) -> Self {
        Self {
            window,
            owner_index: activity_index,
            timer_base: crate::activity::ACTIVITY_TIMER_BASE,
            notify_message: crate::activity::WM_ACTIVITY_NOTIFY,
            surface,
            fonts,
            requests,
            settings,
        }
    }

    pub fn fonts(&self) -> &Fonts {
        self.fonts
    }

    /// Shows a popup menu of `items` at the pointer and waits for the choice; the position of the one chosen, if any.
    pub fn show_menu(&mut self, items: &[&str]) -> Option<usize> {
        crate::platform::popup_menu(self.window, items)
    }

    /// Draw the notch again after this hook returns.
    pub fn request_repaint(&mut self) {
        self.requests.repaint = true;
    }

    /// Start the frame timer so `Page::animate` gets called; use after starting an animation of the page's own.
    pub fn request_animation(&mut self) {
        self.requests.animate = true;
    }

    /// Calls `Page::on_timer` with `timer` after `milliseconds`, repeating until killed. Setting it again re-arms it.
    pub fn set_timer(&mut self, timer: TimerId, milliseconds: u32) {
        unsafe { SetTimer(Some(self.window), self.timer_key(timer), milliseconds, None) };
    }

    pub fn kill_timer(&mut self, timer: TimerId) {
        unsafe {
            let _ = KillTimer(Some(self.window), self.timer_key(timer));
        }
    }

    /// A handle that background threads use to wake this page through `Page::on_notify`.
    pub fn notifier(&self) -> Notifier {
        Notifier { window_address: self.window.0 as isize, owner_index: self.owner_index, message: self.notify_message }
    }

    /// Delivers changes of a Windows power setting as `SystemMessage::PowerSettingChanged`, starting with its
    /// current value. The registration lives as long as the notch.
    pub fn listen_to_power_setting(&self, setting: &GUID) {
        let flags = REGISTER_NOTIFICATION_FLAGS(0);
        let _ = unsafe { RegisterPowerSettingNotification(HANDLE(self.window.0), setting, flags) };
    }

    /// Delivers clipboard changes as `SystemMessage::ClipboardChanged`.
    pub fn listen_to_clipboard(&self) {
        let _ = unsafe { AddClipboardFormatListener(self.window) };
    }

    /// Uploads an image so `Canvas::image` can draw it. Usually called through `ImageSlot::sync`.
    pub fn upload_image(&self, image: &ImageData) -> Option<ImageBrush> {
        self.surface.create_image_brush(image).ok()
    }

    fn timer_key(&self, timer: TimerId) -> usize {
        owned_timer_key(self.timer_base, self.owner_index, timer)
    }
}

/// The Win32 timer key for one of a page's timers.
#[cfg(test)]
pub(crate) fn timer_key(page_index: usize, timer: TimerId) -> usize {
    owned_timer_key(PAGE_TIMER_BASE, page_index, timer)
}

/// The Win32 timer key for a timer of the page or activity at `owner_index`, with timers starting at `timer_base`.
pub(crate) fn owned_timer_key(timer_base: usize, owner_index: usize, timer: TimerId) -> usize {
    debug_assert!(usize::from(timer.0) < TIMERS_PER_PAGE, "timer ids must be below {TIMERS_PER_PAGE}");
    timer_base + owner_index * TIMERS_PER_PAGE + usize::from(timer.0)
}

/// The page and timer a Win32 timer key belongs to; `None` for keys that are not page timers.
pub(crate) fn split_timer_key(key: usize) -> Option<(usize, TimerId)> {
    split_owned_timer_key(PAGE_TIMER_BASE, PAGE_TIMER_LIMIT, key)
}

/// The owner and timer of a key in `timer_base..limit`.
pub(crate) fn split_owned_timer_key(timer_base: usize, limit: usize, key: usize) -> Option<(usize, TimerId)> {
    if key >= limit {
        return None;
    }
    let offset = key.checked_sub(timer_base)?;
    Some((offset / TIMERS_PER_PAGE, TimerId((offset % TIMERS_PER_PAGE) as u8)))
}

/// Wakes a page or activity from any thread. Cheap to clone; keep one in the background worker.
#[derive(Clone, Copy, Debug)]
pub struct Notifier {
    // The window handle crosses threads as an integer, since the raw handle type is not thread-safe.
    window_address: isize,
    owner_index: usize,
    message: u32,
}

impl Notifier {
    pub fn notify(&self) {
        let window = HWND(self.window_address as *mut _);
        unsafe {
            let _ = PostMessageW(Some(window), self.message, WPARAM(self.owner_index), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities_are_ordered() {
        assert!(Priority::BACKGROUND < Priority::NORMAL);
        assert!(Priority::NORMAL < Priority::URGENT);
    }

    #[test]
    fn the_notch_counts_as_open_from_80_percent() {
        let frame = |expansion| Frame { scale: 1.0, bounds: Rect::default(), expansion, expanded: Size::new(0.0, 0.0) };
        assert!(!frame(0.79).is_open());
        assert!(frame(0.8).is_open());
        assert!(frame(1.05).is_open());
    }

    #[test]
    fn timer_keys_round_trip_to_page_and_timer() {
        for (page, timer) in [(0, 0), (0, 15), (1, 0), (3, 7)] {
            let key = timer_key(page, TimerId(timer));
            assert_eq!(split_timer_key(key), Some((page, TimerId(timer))));
        }
    }

    #[test]
    fn timers_of_different_pages_never_collide() {
        assert_ne!(timer_key(0, TimerId(15)), timer_key(1, TimerId(0)));
    }

    #[test]
    fn keys_below_the_page_range_belong_to_the_host() {
        assert_eq!(split_timer_key(0), None);
        assert_eq!(split_timer_key(PAGE_TIMER_BASE - 1), None);
    }

    #[test]
    fn keys_from_the_activity_range_up_are_not_page_timers() {
        assert_eq!(split_timer_key(PAGE_TIMER_LIMIT), None);
        assert!(split_timer_key(PAGE_TIMER_LIMIT - 1).is_some());
    }
}
