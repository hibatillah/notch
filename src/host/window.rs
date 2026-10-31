use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Performance::QueryPerformanceFrequency,
            Power::POWERBROADCAST_SETTING,
            RemoteDesktop::{
                NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
            },
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetSystemMetricsForDpi, SetProcessDpiAwarenessContext,
            },
            Input::{
                GetCurrentInputMessageSource, IMDT_TOUCHPAD, INPUT_MESSAGE_SOURCE,
                KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
            },
            WindowsAndMessaging::*,
        },
    },
    core::{Result, w},
};

use super::{
    display, drop_target,
    icon::Icon,
    notch::{Notch, NotchParts, WINDOW_HEIGHT, WINDOW_WIDTH},
    registry,
    screen_mode::{self, ScreenMode},
    settings::Settings,
    settings_launcher, startup, tray,
};
use crate::{
    activity::{Activity, SystemMessage, WM_ACTIVITY_NOTIFY},
    gfx::{Fonts, Surface},
    log,
    page::{Page, WM_PAGE_NOTIFY},
};

// Not exported under WindowsAndMessaging by the windows crate.
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_POWERBROADCAST: u32 = 0x0218;
const WM_CLIPBOARDUPDATE: u32 = 0x031D;
/// The `wparam` of `WM_POWERBROADCAST` when a power setting registered with `listen_to_power_setting` changed.
const PBT_POWERSETTINGCHANGE: usize = 0x8013;
/// Posted to the `NotchWindow` window by another program after it changed settings.txt.
pub const WM_SETTINGS_CHANGED: u32 = WM_APP + 30;

/// Registered message id the shell broadcasts when Explorer restarts and the tray is rebuilt.
static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);
static SESSION_NOTIFICATIONS_REGISTERED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static NOTCH: RefCell<Option<Notch>> = const { RefCell::new(None) };
}

/// Runs `action` on the notch, skipping it if a message re-enters while the notch is already borrowed.
fn with_notch<R>(action: impl FnOnce(&mut Notch) -> R) -> Option<R> {
    NOTCH.with(|cell| cell.try_borrow_mut().ok().and_then(|mut slot| slot.as_mut().map(action)))
}

fn show_menu(window: HWND) {
    let activities = with_notch(|notch| notch.activity_menu()).unwrap_or_default();
    let page_items = with_notch(|notch| notch.page_menu()).unwrap_or_default();
    let state =
        with_notch(|notch| tray::MenuState { hides_when_idle: notch.hides_when_idle(), starts_with_windows: false })
            .unwrap_or_default();
    let state = tray::MenuState { starts_with_windows: startup::is_enabled(), ..state };
    match tray::show_context_menu(window, &activities, &page_items, state) {
        Some(tray::MenuChoice::ToggleHideWhenIdle) => {
            with_notch(|notch| notch.toggle_hide_when_idle());
        }
        Some(tray::MenuChoice::ToggleStartWithWindows) => startup::set_enabled(!state.starts_with_windows),
        Some(tray::MenuChoice::PageItem { page, item }) => {
            with_notch(|notch| notch.page_menu_item_chosen(page, item));
        }
        Some(tray::MenuChoice::Toggle(index)) => {
            with_notch(|notch| notch.toggle_activity(index));
        }
        Some(tray::MenuChoice::OpenSettings) => settings_launcher::open_settings_window(),
        Some(tray::MenuChoice::Quit) => unsafe { PostQuitMessage(0) },
        None => {}
    }
}

fn power_setting_message(lparam: LPARAM) -> Option<SystemMessage> {
    let setting = unsafe { (lparam.0 as *const POWERBROADCAST_SETTING).as_ref()? };
    if (setting.DataLength as usize) < size_of::<u32>() {
        return None;
    }
    let value = unsafe { std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>()) };
    Some(SystemMessage::PowerSettingChanged { setting: setting.PowerSetting, value })
}

fn low_word_signed(value: isize) -> i32 {
    i32::from((value & 0xFFFF) as u16 as i16)
}

fn high_word_signed(value: isize) -> i32 {
    i32::from(((value >> 16) & 0xFFFF) as u16 as i16)
}

/// Re-arming an occluded window immediately posts another leave, so check the actual pointer target first.
fn track_mouse_leave(window: HWND) {
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_err() || unsafe { WindowFromPoint(cursor) } != window {
        return;
    }
    let mut tracking = TRACKMOUSEEVENT {
        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: window,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut tracking);
    }
}

/// Moves the window to the top of the topmost band; another topmost window can sit above it with the flag still set.
pub(super) fn raise_topmost(window: HWND) {
    let result = unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    };
    if let Err(error) = result {
        log::debug(&format!("could not raise topmost: {error}"));
    }
}

pub(super) fn restore_topmost(window: HWND) {
    if unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0 != 0 {
        return;
    }
    match unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    } {
        Ok(()) => log::debug("restored topmost"),
        Err(error) => log::debug(&format!("could not restore topmost: {error}")),
    }
}

extern "system" fn window_procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let (x, y) = (low_word_signed(lparam.0), high_word_signed(lparam.0));
    match message {
        WM_MOUSEMOVE => {
            track_mouse_leave(window);
            with_notch(|notch| notch.pointer_moved(x, y));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if with_notch(|notch| notch.pointer_left_window()) == Some(false) {
                track_mouse_leave(window);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            with_notch(|notch| notch.pointer_pressed(x, y));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            with_notch(|notch| notch.pointer_released(x, y));
            LRESULT(0)
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let amount = high_word_signed(wparam.0 as isize);
            let mut source = INPUT_MESSAGE_SOURCE::default();
            // Drivers without source metadata still use the delta/active-gesture fallback in scrolled().
            let is_touchpad =
                unsafe { GetCurrentInputMessageSource(&mut source) }.is_ok() && source.deviceType == IMDT_TOUCHPAD;
            with_notch(|notch| notch.scrolled(amount, message == WM_MOUSEHWHEEL, is_touchpad));
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            with_notch(|notch| notch.capture_lost());
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            with_notch(|notch| notch.right_clicked(x, y));
            LRESULT(0)
        }
        drop_target::WM_POINTER_MOVED => {
            with_notch(|notch| notch.pointer_moved(x, y));
            LRESULT(0)
        }
        drop_target::WM_DRAG_ENTERED => {
            with_notch(|notch| notch.drag_entered());
            LRESULT(0)
        }
        drop_target::WM_DRAG_LEFT => {
            with_notch(|notch| notch.drag_left());
            LRESULT(0)
        }
        drop_target::WM_DRAG_DROPPED => {
            let paths = drop_target::take_dropped_paths();
            with_notch(|notch| notch.files_dropped(paths));
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_TIMER => {
            with_notch(|notch| notch.timer_fired(wparam.0));
            LRESULT(0)
        }
        WM_PAGE_NOTIFY => {
            with_notch(|notch| notch.page_notified(wparam.0));
            LRESULT(0)
        }
        WM_ACTIVITY_NOTIFY => {
            with_notch(|notch| notch.activity_notified(wparam.0));
            LRESULT(0)
        }
        WM_POWERBROADCAST => {
            if wparam.0 == PBT_POWERSETTINGCHANGE
                && let Some(message) = power_setting_message(lparam)
            {
                with_notch(|notch| notch.system_message(message));
            }
            if wparam.0 == PBT_APMRESUMEAUTOMATIC as usize {
                screen_mode::refresh_foreground();
                with_notch(|notch| notch.reset_session_interaction());
            }
            LRESULT(1)
        }
        WM_WTSSESSION_CHANGE if wparam.0 == WTS_SESSION_LOCK as usize || wparam.0 == WTS_SESSION_UNLOCK as usize => {
            log::debug(if wparam.0 == WTS_SESSION_LOCK as usize { "session locked" } else { "session unlocked" });
            if wparam.0 == WTS_SESSION_UNLOCK as usize {
                screen_mode::refresh_foreground();
            }
            with_notch(|notch| notch.reset_session_interaction());
            LRESULT(0)
        }
        WM_SETTINGS_CHANGED => {
            log::debug("settings changed by another program");
            with_notch(|notch| notch.reload_settings());
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            with_notch(|notch| notch.system_message(SystemMessage::ClipboardChanged));
            LRESULT(0)
        }
        tray::WM_TRAY => {
            // Tray messages carry the originating mouse message in the low word of lparam.
            let mouse_message = (lparam.0 & 0xFFFF) as u32;
            if mouse_message == WM_RBUTTONUP || mouse_message == WM_CONTEXTMENU {
                show_menu(window);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            if SESSION_NOTIFICATIONS_REGISTERED.swap(false, Ordering::Relaxed)
                && let Err(error) = unsafe { WTSUnRegisterSessionNotification(window) }
            {
                log::debug(&format!("could not unregister session notifications: {error}"));
            }
            tray::remove(window);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ if message != 0 && message == TASKBAR_CREATED_MESSAGE.load(Ordering::Relaxed) => {
            let _ = tray::add(window);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn on_screen_mode(mode: ScreenMode) {
    with_notch(|notch| notch.set_screen_mode(mode));
}

pub fn run(pages: Vec<Box<dyn Page>>, activities: Vec<Box<dyn Activity>>) -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        RoInitialize(RO_INIT_MULTITHREADED)?;

        let scale = display::system_scale();
        let instance = GetModuleHandleW(None)?;
        let dpi = (scale * 96.0).round() as u32;
        let large_icon = Icon::load(GetSystemMetricsForDpi(SM_CXICON, dpi))?;
        let small_icon = Icon::load(GetSystemMetricsForDpi(SM_CXSMICON, dpi))?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_procedure),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: large_icon.handle(),
            hIconSm: small_icon.handle(),
            lpszClassName: w!("NotchWindow"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        TASKBAR_CREATED_MESSAGE.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);

        let width = (WINDOW_WIDTH * scale).round() as i32;
        let height = (WINDOW_HEIGHT * scale).round() as i32;
        // NOTCH_TOP or the notch.top setting shifts the window down in pixels, so it can be compared next to another notch app.
        let top_offset = std::env::var("NOTCH_TOP")
            .ok()
            .or_else(|| Settings::load().get_string("notch.top").map(str::to_owned))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        let screen_position = POINT { x: (GetSystemMetrics(SM_CXSCREEN) - width) / 2, y: top_offset };
        // A previous instance is asked to close only after this one is visible, so a rebuild swaps without a gap.
        let previous_window = FindWindowW(w!("NotchWindow"), None).ok();
        let window = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("NotchWindow"),
            w!("Notch"),
            WS_POPUP,
            screen_position.x,
            screen_position.y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;

        drop_target::start(window, scale, screen_position.x + width / 2, screen_position.y);

        let mut ticks_per_second = 0;
        QueryPerformanceFrequency(&mut ticks_per_second)?;

        let settings = Settings::load();
        let pages = registry::enabled_pages(pages, &settings);
        let notch = Notch::new(NotchParts {
            window,
            scale,
            screen_position,
            surface: Surface::new(width, height)?,
            fonts: Fonts::new(scale)?,
            settings,
            pages,
            activities,
            ticks_per_second,
        });
        NOTCH.with(|cell| *cell.borrow_mut() = Some(notch));
        match WTSRegisterSessionNotification(window, NOTIFY_FOR_THIS_SESSION) {
            Ok(()) => SESSION_NOTIFICATIONS_REGISTERED.store(true, Ordering::Relaxed),
            Err(error) => {
                log::debug(&format!("session notifications unavailable; foreground recovery remains active: {error}"))
            }
        }
        with_notch(|notch| notch.start());

        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        screen_mode::start(on_screen_mode);
        if let Err(error) = tray::add(window) {
            log::debug(&format!("tray icon unavailable: {error}"));
        }
        if let Some(previous_window) = previous_window {
            let _ = PostMessageW(Some(previous_window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}
