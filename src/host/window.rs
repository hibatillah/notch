use std::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Performance::QueryPerformanceFrequency,
            Power::POWERBROADCAST_SETTING,
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
            Shell::{DragFinish, DragQueryFileW, HDROP},
            WindowsAndMessaging::*,
        },
    },
    core::{Result, w},
};

use super::{
    display,
    notch::{Notch, NotchParts, WINDOW_HEIGHT, WINDOW_WIDTH},
    registry,
    settings::Settings,
    tray,
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

/// Registered message id the shell broadcasts when Explorer restarts and the tray is rebuilt.
static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static NOTCH: RefCell<Option<Notch>> = const { RefCell::new(None) };
}

/// Runs `action` on the notch, skipping it if a message re-enters while the notch is already borrowed.
fn with_notch<R>(action: impl FnOnce(&mut Notch) -> R) -> Option<R> {
    NOTCH.with(|cell| cell.try_borrow_mut().ok().and_then(|mut slot| slot.as_mut().map(action)))
}

/// Shows the tray menu and carries out the choice.
fn show_menu(window: HWND) {
    let activities = with_notch(|notch| notch.activity_menu()).unwrap_or_default();
    let page_items = with_notch(|notch| notch.page_menu()).unwrap_or_default();
    match tray::show_context_menu(window, &activities, &page_items) {
        Some(tray::MenuChoice::PageItem { page, item }) => {
            with_notch(|notch| notch.page_menu_item_chosen(page, item));
        }
        Some(tray::MenuChoice::Toggle(index)) => {
            with_notch(|notch| notch.toggle_activity(index));
        }
        Some(tray::MenuChoice::Quit) => unsafe { PostQuitMessage(0) },
        None => {}
    }
}

/// The power setting a `WM_POWERBROADCAST` reports, if it is one with a 32-bit value.
fn power_setting_message(lparam: LPARAM) -> Option<SystemMessage> {
    let setting = unsafe { (lparam.0 as *const POWERBROADCAST_SETTING).as_ref()? };
    if (setting.DataLength as usize) < size_of::<u32>() {
        return None;
    }
    let value = unsafe { std::ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>()) };
    Some(SystemMessage::PowerSettingChanged { setting: setting.PowerSetting, value })
}

/// The files of a `WM_DROPFILES` message; the drop is finished here.
fn dropped_paths(drop: HDROP) -> Vec<std::path::PathBuf> {
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut paths = Vec::new();
    for index in 0..count {
        let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
        let mut buffer = vec![0u16; length + 1];
        let written = unsafe { DragQueryFileW(drop, index, Some(&mut buffer)) } as usize;
        paths.push(std::path::PathBuf::from(String::from_utf16_lossy(&buffer[..written])));
    }
    unsafe { DragFinish(drop) };
    paths
}

fn low_word_signed(value: isize) -> i32 {
    i32::from((value & 0xFFFF) as u16 as i16)
}

fn high_word_signed(value: isize) -> i32 {
    i32::from(((value >> 16) & 0xFFFF) as u16 as i16)
}

/// Asks Windows to send one `WM_MOUSELEAVE` when the pointer leaves the window; it must be re-armed after each use.
fn track_mouse_leave(window: HWND) {
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
            with_notch(|notch| notch.scrolled(amount, message == WM_MOUSEHWHEEL));
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            with_notch(|notch| notch.capture_lost());
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            if with_notch(|notch| notch.right_clicked(x, y)) != Some(true) {
                show_menu(window);
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            let paths = dropped_paths(HDROP(wparam.0 as *mut _));
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
            LRESULT(1)
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

/// Creates the notch window, starts the pages and runs the message loop until the notch quits.
pub fn run(pages: Vec<Box<dyn Page>>, activities: Vec<Box<dyn Activity>>) -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        RoInitialize(RO_INIT_MULTITHREADED)?;

        let scale = display::system_scale();
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_procedure),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: w!("NotchWindow"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        TASKBAR_CREATED_MESSAGE.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);

        let width = (WINDOW_WIDTH * scale).round() as i32;
        let height = (WINDOW_HEIGHT * scale).round() as i32;
        // NOTCH_TOP shifts the window down in pixels, so it can be compared next to another notch app.
        let top_offset = std::env::var("NOTCH_TOP").ok().and_then(|value| value.parse().ok()).unwrap_or(0);
        let screen_position = POINT { x: (GetSystemMetrics(SM_CXSCREEN) - width) / 2, y: top_offset };
        // A previous instance is asked to close only after this one is visible, so a rebuild swaps without a gap.
        let previous_window = FindWindowW(w!("NotchWindow"), None).ok();
        let window = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_ACCEPTFILES,
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
        with_notch(|notch| notch.start());

        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
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
