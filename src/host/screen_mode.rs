//! Tells whether the window in front covers the screen, so the notch can step out of its way: hide to a thin strip over a
//! maximized window, and hide completely over a full-screen one (a game, a video, F11 in a browser).
//!
//! Nothing polls. Windows reports a change of the window in front, and, for as long as one window is in front, the
//! moves and resizes of that window's own thread; each report is one cheap comparison of its size with the monitor's.

use std::sync::{
    OnceLock,
    atomic::{AtomicIsize, AtomicU8, Ordering},
};

use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
        MonitorFromWindow,
    },
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART,
            GWL_STYLE, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
            IsIconic, IsWindowVisible, IsZoomed, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
            WS_CAPTION,
        },
    },
};

/// How much of the screen the window in front takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenMode {
    Normal,
    /// Maximized: the window fills the screen but for the taskbar, and keeps its title bar.
    Maximized,
    /// Full screen: the window covers the whole monitor, taskbar included, with no title bar.
    Fullscreen,
}

/// A rectangle in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    /// True when this rectangle reaches every edge of `other`.
    fn covers(&self, other: &Bounds) -> bool {
        self.left <= other.left && self.top <= other.top && self.right >= other.right && self.bottom >= other.bottom
    }
}

/// What is known about the window in front.
pub struct Facts {
    pub window: Bounds,
    /// The primary monitor, which is where the notch is.
    pub monitor: Bounds,
    pub is_zoomed: bool,
    pub has_caption: bool,
    /// The desktop, the taskbar, the notch itself, a window that is hidden or minimized, or one on another monitor.
    pub is_ignored: bool,
}

/// A window that covers the whole monitor is full screen, unless it is maximized and still has its title bar (a
/// maximized window can cover the whole monitor when the taskbar hides itself).
pub fn classify(facts: &Facts) -> ScreenMode {
    if facts.is_ignored {
        return ScreenMode::Normal;
    }
    if facts.window.covers(&facts.monitor) && !(facts.is_zoomed && facts.has_caption) {
        ScreenMode::Fullscreen
    } else if facts.is_zoomed {
        ScreenMode::Maximized
    } else {
        ScreenMode::Normal
    }
}

/// Windows that cover the screen without being an app in front of it.
const IGNORED_CLASSES: [&str; 6] =
    ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotchWindow", "NotchDropTarget"];

static ON_CHANGE: OnceLock<fn(ScreenMode)> = OnceLock::new();
static LAST_MODE: AtomicU8 = AtomicU8::new(0);
/// The hook that follows the window in front, replaced whenever another window comes to the front.
static LOCATION_HOOK: AtomicIsize = AtomicIsize::new(0);

fn class_name(window: HWND) -> String {
    let mut buffer = [0u16; 64];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
}

/// Reads the window in front now.
fn read() -> ScreenMode {
    unsafe {
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return ScreenMode::Normal;
        }
        let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        let mut rect = Default::default();
        if !GetMonitorInfoW(primary, &mut info).as_bool() || GetWindowRect(window, &mut rect).is_err() {
            return ScreenMode::Normal;
        }
        let class = class_name(window);
        let facts = Facts {
            window: Bounds { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom },
            monitor: Bounds {
                left: info.rcMonitor.left,
                top: info.rcMonitor.top,
                right: info.rcMonitor.right,
                bottom: info.rcMonitor.bottom,
            },
            is_zoomed: IsZoomed(window).as_bool(),
            has_caption: (GetWindowLongW(window, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0,
            is_ignored: monitor != primary
                || !IsWindowVisible(window).as_bool()
                || IsIconic(window).as_bool()
                || IGNORED_CLASSES.contains(&class.as_str()),
        };
        classify(&facts)
    }
}

/// Reads the window in front and tells the notch if that changed the mode.
fn refresh() {
    let mode = read();
    if LAST_MODE.swap(mode as u8, Ordering::Relaxed) != mode as u8
        && let Some(on_change) = ON_CHANGE.get()
    {
        on_change(mode);
    }
}

/// Follows the moves and resizes of the thread that owns the window in front, and nothing else: asking for every
/// window's would wake the notch on every mouse move.
fn follow_foreground_thread() {
    unsafe {
        let previous = LOCATION_HOOK.swap(0, Ordering::Relaxed);
        if previous != 0 {
            let _ = UnhookWinEvent(HWINEVENTHOOK(previous as *mut _));
        }
        let window = GetForegroundWindow();
        if window.0.is_null() {
            return;
        }
        let thread = GetWindowThreadProcessId(window, None);
        let hook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(on_event),
            0,
            thread,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        LOCATION_HOOK.store(hook.0 as isize, Ordering::Relaxed);
    }
}

unsafe extern "system" fn on_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    match event {
        EVENT_SYSTEM_FOREGROUND => {
            follow_foreground_thread();
            refresh();
        }
        // The caret and other parts of the window report moves too; only the window itself counts.
        EVENT_OBJECT_LOCATIONCHANGE => {
            if object == OBJID_WINDOW.0 && window == unsafe { GetForegroundWindow() } {
                refresh();
            }
        }
        _ => refresh(),
    }
}

/// Starts watching; `on_change` is called on the notch's thread whenever the mode changes. Call it once.
pub fn start(on_change: fn(ScreenMode)) {
    let _ = ON_CHANGE.set(on_change);
    for event in [EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND] {
        let _ = unsafe {
            SetWinEventHook(event, event, None, Some(on_event), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS)
        };
    }
    follow_foreground_thread();
    refresh();
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Bounds = Bounds { left: 0, top: 0, right: 1920, bottom: 1080 };

    fn facts(window: Bounds, is_zoomed: bool, has_caption: bool) -> Facts {
        Facts { window, monitor: MONITOR, is_zoomed, has_caption, is_ignored: false }
    }

    #[test]
    fn a_window_over_the_whole_monitor_with_no_title_bar_is_full_screen() {
        assert_eq!(classify(&facts(MONITOR, false, false)), ScreenMode::Fullscreen);
        // A game that sits a little outside the monitor.
        let outside = Bounds { left: -1, top: -1, right: 1921, bottom: 1081 };
        assert_eq!(classify(&facts(outside, false, false)), ScreenMode::Fullscreen);
    }

    #[test]
    fn a_maximized_window_stops_short_of_the_taskbar() {
        let above_taskbar = Bounds { left: -8, top: -8, right: 1928, bottom: 1040 };
        assert_eq!(classify(&facts(above_taskbar, true, true)), ScreenMode::Maximized);
    }

    #[test]
    fn a_maximized_window_over_a_hidden_taskbar_is_still_maximized() {
        let outside = Bounds { left: -8, top: -8, right: 1928, bottom: 1088 };
        assert_eq!(classify(&facts(outside, true, true)), ScreenMode::Maximized);
    }

    #[test]
    fn a_window_that_does_not_fill_the_screen_is_normal() {
        let window = Bounds { left: 100, top: 100, right: 900, bottom: 700 };
        assert_eq!(classify(&facts(window, false, true)), ScreenMode::Normal);
    }

    #[test]
    fn the_desktop_and_other_ignored_windows_are_never_full_screen() {
        let mut desktop = facts(MONITOR, false, false);
        desktop.is_ignored = true;
        assert_eq!(classify(&desktop), ScreenMode::Normal);
    }

    /// Opens a plain window of the given shape in front and reads the mode back, so what the notch sees on a real
    /// screen is checked, not only the sizes in the tests above.
    #[cfg(test)]
    fn mode_seen_with_window(style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE, show: i32) -> ScreenMode {
        use windows::{
            Win32::{
                Foundation::{LPARAM, WPARAM},
                UI::{
                    Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, VK_MENU, keybd_event},
                    WindowsAndMessaging::{
                        CreateWindowExW, DestroyWindow, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
                        SendMessageW, SetForegroundWindow, ShowWindow, WINDOW_EX_STYLE, WM_NULL,
                    },
                },
            },
            core::w,
        };
        unsafe {
            let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let window = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("screen mode test"),
                style,
                0,
                0,
                width,
                height,
                None,
                None,
                None,
                None,
            )
            .expect("a test window");
            let _ = ShowWindow(window, windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(show));
            // Windows only lets a process take the front when it has just had input; a tap of Alt counts.
            keybd_event(VK_MENU.0 as u8, 0, Default::default(), 0);
            keybd_event(VK_MENU.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            let _ = SetForegroundWindow(window);
            let _ = SendMessageW(window, WM_NULL, Some(WPARAM(0)), Some(LPARAM(0)));
            std::thread::sleep(std::time::Duration::from_millis(400));
            let mode = read();
            let _ = ShowWindow(window, SW_SHOW);
            let _ = DestroyWindow(window);
            mode
        }
    }

    #[test]
    #[ignore = "opens a window over the whole screen for a moment; run on purpose"]
    fn a_real_maximized_window_and_a_real_full_screen_window_are_told_apart() {
        use windows::Win32::UI::WindowsAndMessaging::{
            SW_MAXIMIZE, SW_SHOW, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
        };
        let maximized = mode_seen_with_window(WS_OVERLAPPEDWINDOW | WS_VISIBLE, SW_MAXIMIZE.0);
        let fullscreen = mode_seen_with_window(WS_POPUP | WS_VISIBLE, SW_SHOW.0);
        println!("maximized -> {maximized:?}, full screen -> {fullscreen:?}");
        assert_eq!((maximized, fullscreen), (ScreenMode::Maximized, ScreenMode::Fullscreen));
    }
}
