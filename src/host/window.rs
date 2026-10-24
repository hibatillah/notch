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
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent},
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
    gfx::{Fonts, Surface},
    log,
    page::{Page, WM_PAGE_NOTIFY},
};

// Not exported under WindowsAndMessaging by the windows crate.
const WM_MOUSELEAVE: u32 = 0x02A3;

/// Registered message id the shell broadcasts when Explorer restarts and the tray is rebuilt.
static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static NOTCH: RefCell<Option<Notch>> = const { RefCell::new(None) };
}

/// Runs `action` on the notch, skipping it if a message re-enters while the notch is already borrowed.
fn with_notch<R>(action: impl FnOnce(&mut Notch) -> R) -> Option<R> {
    NOTCH.with(|cell| cell.try_borrow_mut().ok().and_then(|mut slot| slot.as_mut().map(action)))
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
        WM_CAPTURECHANGED => {
            with_notch(|notch| notch.capture_lost());
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            tray::show_context_menu(window);
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
        tray::WM_TRAY => {
            // Tray messages carry the originating mouse message in the low word of lparam.
            let mouse_message = (lparam.0 & 0xFFFF) as u32;
            if mouse_message == WM_RBUTTONUP || mouse_message == WM_CONTEXTMENU {
                tray::show_context_menu(window);
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
pub fn run(pages: Vec<Box<dyn Page>>) -> Result<()> {
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
