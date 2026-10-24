//! The contract between the notch host and the activities it shows in the collapsed pill.
//!
//! A page is what the notch shows when you open it; an activity is something that happens while it is closed: a
//! volume change, a charger being plugged in, the microphone turning on. An activity tells the host whether it has
//! something to show (`presence`) and draws that into the area the host gives it. Everything else (timers, wake-ups
//! from background threads, system messages) is an optional hook.
//! See `docs/adding-an-activity.md` for how to add one.

use windows::{Win32::UI::WindowsAndMessaging::WM_APP, core::GUID};

use crate::{
    gfx::{Canvas, Rect},
    page::{Context, PAGE_TIMER_LIMIT, TimerId},
};

/// Message an activity's `Notifier` posts to wake the host from another thread.
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
}

/// Something that happened to the system, delivered to every activity.
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

    /// Name shown in the tray menu, where the activity can be switched on and off.
    fn label(&self) -> &'static str;

    /// What this activity shows right now; `None` means nothing. Asked again after every event.
    fn presence(&self) -> Option<Presence>;

    /// Draws into `area` (device pixels), which is as wide as the width the presence asked for.
    fn draw(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32);

    /// Called once after the window exists, only when the activity is switched on: start listening here.
    fn on_start(&mut self, _context: &mut Context) {}

    fn on_timer(&mut self, _timer: TimerId, _context: &mut Context) {}

    /// Called after a background thread used this activity's `Notifier`.
    fn on_notify(&mut self, _context: &mut Context) {}

    fn on_system_message(&mut self, _message: &SystemMessage, _context: &mut Context) {}
}

/// The timer a `Flash` uses; an activity that also needs timers of its own starts from `TimerId(1)`.
pub const FLASH_TIMER: TimerId = TimerId(0);

/// Whether a transient is on screen. Showing it again while it is visible keeps it up for a fresh duration.
#[derive(Default)]
pub struct Flash {
    is_visible: bool,
}

impl Flash {
    /// A flash that is showing, for building samples of what a page looks like.
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

/// Remembers the last value of a setting that reports its current value on registration. Returns true when `value`
/// is a change worth showing: the first value is only the starting point, and a repeat is not a change.
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
