//! Collapsed activity contracts; see docs/adding-an-activity.md.

use windows::{Win32::UI::WindowsAndMessaging::WM_APP, core::GUID};

use crate::{
    gfx::{Canvas, Rect},
    page::{Context, PAGE_TIMER_LIMIT, TimerId},
};

pub(crate) const WM_ACTIVITY_NOTIFY: u32 = WM_APP + 3;
/// Win32 timer ids from here up belong to activities; each activity owns 16 of them, like a page.
pub(crate) const ACTIVITY_TIMER_BASE: usize = PAGE_TIMER_LIMIT;

/// How an activity wants to appear in the collapsed pill. Widths are in device-independent pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Presence {
    /// Takes over the pill for a moment, like a volume change. Several at once share the pill, side by side.
    Transient { width: f32 },
    /// A small mark kept at the right end of the pill while something lasts, like the microphone being in use.
    Badge { width: f32 },
    /// Banners preempt transients and block hover expansion; on_hover_changed can extend their lifetime.
    Banner { width: f32, height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SystemMessage {
    /// A Windows power setting the activity listens to changed; the first one after registering is its current value.
    PowerSettingChanged {
        setting: GUID,
        value: u32,
    },
    ClipboardChanged,
}

pub trait Activity {
    /// Stable name used in logs and settings keys (`activity.<id>`).
    fn id(&self) -> &'static str;

    fn label(&self) -> &'static str;

    /// Optional activities expose a tray toggle; required activities ignore saved disable settings.
    fn is_optional(&self) -> bool {
        false
    }

    /// What this activity shows right now; `None` means nothing. Asked again after every event.
    fn presence(&self) -> Option<Presence>;

    /// Draws into `area` (device pixels), which is as wide as the width the presence asked for.
    fn draw(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32);

    /// Header status width in DIP, independent of page selection; None hides it.
    fn header_status_width(&self) -> Option<f32> {
        None
    }

    /// Draws the mark into `area` (device pixels, as high as the header), faded by `opacity`.
    fn draw_header_status(&self, _canvas: &Canvas, _area: Rect, _scale: f32, _opacity: f32) {}

    /// Called once after the window exists, only when the activity is switched on: start listening here.
    fn on_start(&mut self, _context: &mut Context) {}

    fn on_timer(&mut self, _timer: TimerId, _context: &mut Context) {}

    fn on_hover_changed(&mut self, _is_hovering: bool, _context: &mut Context) {}

    fn on_notify(&mut self, _context: &mut Context) {}

    fn on_system_message(&mut self, _message: &SystemMessage, _context: &mut Context) {}

    /// Called on a started activity after another program changed settings.txt: re-read this activity's keys here.
    fn on_settings_changed(&mut self, _context: &mut Context) {}
}

/// The timer a `Flash` uses; an activity that also needs timers of its own starts from `TimerId(1)`.
pub const FLASH_TIMER: TimerId = TimerId(0);

/// Whether a transient is on screen. Showing it again while it is visible keeps it up for a fresh duration.
#[derive(Default)]
pub struct Flash {
    is_visible: bool,
}

impl Flash {
    #[cfg(test)]
    pub fn visible() -> Self {
        Self { is_visible: true }
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn show(&mut self, context: &mut Context, milliseconds: u32) {
        self.is_visible = true;
        context.set_timer(FLASH_TIMER, milliseconds);
        context.request_repaint();
    }

    /// Hides the flash when its timer fires; returns whether `timer` was the flash timer.
    pub fn on_timer(&mut self, timer: TimerId, context: &mut Context) -> bool {
        if timer != FLASH_TIMER {
            return false;
        }
        self.is_visible = false;
        context.kill_timer(FLASH_TIMER);
        context.request_repaint();
        true
    }
}

/// The initial report and repeated values are not changes worth announcing.
pub fn is_change(last: &mut Option<u32>, value: u32) -> bool {
    let previous = last.replace(value);
    previous.is_some_and(|previous| previous != value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_value_is_only_the_starting_point() {
        let mut last = None;
        assert!(!is_change(&mut last, 70));
        assert_eq!(last, Some(70));
    }

    #[test]
    fn a_different_value_is_a_change_and_a_repeat_is_not() {
        let mut last = Some(70);
        assert!(!is_change(&mut last, 70));
        assert!(is_change(&mut last, 80));
        assert!(!is_change(&mut last, 80));
    }

    #[test]
    fn activity_timers_start_where_page_timers_end() {
        assert_eq!(ACTIVITY_TIMER_BASE, PAGE_TIMER_LIMIT);
    }
}
