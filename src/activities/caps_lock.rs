//! A dedicated hook observes only Caps Lock presses and retains no key data; the activity reads the state.

use std::sync::Mutex;

use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL},
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, KBDLLHOOKSTRUCT, MSG, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
        },
    },
};

use crate::{
    activity::{Activity, Flash, Presence},
    gfx::{Canvas, Color, Point, Rect},
    log,
    page::{Context, Notifier, TimerId},
    ui::{self, Icon},
};

const SHOW_MILLISECONDS: u32 = 1200;
/// The key is handled by Windows after the hook has seen it, so the lock is read a moment later.
const READ_DELAY_MILLISECONDS: u32 = 40;
const READ_TIMER: TimerId = TimerId(1);

static WAKE: Mutex<Option<Notifier>> = Mutex::new(None);

unsafe extern "system" fn on_key(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Negative hook codes must pass through untouched.
    if code >= 0 && matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        // SAFETY: for a low-level keyboard hook, `lparam` points to a valid `KBDLLHOOKSTRUCT` during the call.
        let key = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if key.vkCode == u32::from(VK_CAPITAL.0)
            && let Ok(wake) = WAKE.try_lock()
            && let Some(notifier) = *wake
        {
            notifier.notify();
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Runs the hook on its own thread, because a hook only works on a thread that handles messages.
fn start_hook(notifier: Notifier) {
    if let Ok(mut wake) = WAKE.lock() {
        *wake = Some(notifier);
    }
    let _ = std::thread::Builder::new().name("caps-lock-hook".into()).spawn(|| unsafe {
        let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), None, 0) {
            Ok(hook) => hook,
            Err(error) => {
                log::debug(&format!("caps lock: no keyboard hook: {error}"));
                return;
            }
        };
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = UnhookWindowsHookEx(hook);
    });
}

fn is_caps_lock_on() -> bool {
    unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) & 1 != 0 }
}

fn message(is_on: bool) -> &'static str {
    if is_on { "Caps Lock on" } else { "Caps Lock off" }
}

pub struct CapsLockActivity {
    is_on: bool,
    flash: Flash,
    width: f32,
}

impl CapsLockActivity {
    pub fn new() -> Self {
        Self { is_on: false, flash: Flash::default(), width: 0.0 }
    }
}

impl Activity for CapsLockActivity {
    fn id(&self) -> &'static str {
        "caps_lock"
    }

    fn label(&self) -> &'static str {
        "Caps Lock"
    }

    fn presence(&self) -> Option<Presence> {
        self.flash.is_visible().then_some(Presence::Transient { width: self.width })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, _scale: f32, opacity: f32) {
        let tint = if self.is_on { Color::WHITE } else { Color::gray(0.6) };
        ui::draw_message(canvas, area, opacity, Icon::CapsLock, tint, message(self.is_on), None);
    }

    fn header_status_width(&self) -> Option<f32> {
        self.is_on.then_some(Icon::CapsLock.size())
    }

    fn draw_header_status(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        let center = Point::new(area.center_x(), area.center_y());
        Icon::CapsLock.draw(canvas, center, Icon::CapsLock.size() * scale, Color::WHITE, opacity);
    }

    fn on_start(&mut self, context: &mut Context) {
        self.is_on = is_caps_lock_on();
        start_hook(context.notifier());
    }

    fn on_notify(&mut self, context: &mut Context) {
        context.set_timer(READ_TIMER, READ_DELAY_MILLISECONDS);
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if self.flash.on_timer(timer, context) {
            return;
        }
        if timer != READ_TIMER {
            return;
        }
        context.kill_timer(READ_TIMER);
        let is_on = is_caps_lock_on();
        if is_on == self.is_on {
            return;
        }
        self.is_on = is_on;
        let text = message(is_on);
        self.width = ui::message_width(context.fonts(), text, None).max(ui::message_width(
            context.fonts(),
            message(!is_on),
            None,
        ));
        self.flash.show(context, SHOW_MILLISECONDS);
        context.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_says_which_way_the_lock_went() {
        assert_eq!(message(true), "Caps Lock on");
        assert_eq!(message(false), "Caps Lock off");
    }

    #[test]
    fn the_lock_shows_in_the_header_only_while_it_is_on() {
        let mut activity = CapsLockActivity::new();
        assert_eq!(activity.header_status_width(), None);
        activity.is_on = true;
        assert_eq!(activity.header_status_width(), Some(Icon::CapsLock.size()));
    }

    #[test]
    fn nothing_shows_in_the_pill_until_the_lock_changes() {
        assert_eq!(CapsLockActivity::new().presence(), None);
    }
}
