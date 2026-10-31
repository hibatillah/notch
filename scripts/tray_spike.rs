//! Throwaway spike: a Windows 11 style tray menu (system backdrop, rounded corners, 40 px rows, Direct2D text).
//! Not run by any script. Run: `cargo run --example tray_spike -- --hold-seconds 10`. See
//! docs/superpowers/spikes/2026-10-30-tray-menu-spike.md. Delete once the decision is recorded.

use std::{cell::RefCell, mem::size_of, time::Instant};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1CreateFactory,
                ID2D1Factory, ID2D1HwndRenderTarget,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat,
            },
            Dwm::{
                DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
                DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWINDOWATTRIBUTE, DwmExtendFrameIntoClientArea,
                DwmGetWindowAttribute, DwmSetWindowAttribute,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::InvalidateRect,
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            ProcessStatus::K32QueryWorkingSet,
            Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW},
            Threading::GetCurrentProcess,
        },
        UI::{
            Controls::MARGINS,
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext},
            Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture, SetCapture, VK_ESCAPE},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW,
                IDC_ARROW, KillTimer, LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SW_SHOWNOACTIVATE,
                SWP_NOACTIVATE, SWP_NOZORDER, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
    core::{PCWSTR, w},
};

/// The labels of the current tray menu items in `src/host/tray.rs` (the activity item is one example).
const LABELS: [&str; 5] = ["Notifications", "Hide when idle", "Start with Windows", "Settings…", "Quit"];
const ITEM_HEIGHT: f32 = 40.0;
const MENU_WIDTH: f32 = 240.0;
const MENU_PADDING: f32 = 4.0;
const TEXT_LEFT: f32 = 16.0;
const FONT_SIZE: f32 = 14.0;
const HIGHLIGHT_INSET: f32 = 4.0;
const HIGHLIGHT_RADIUS: f32 = 4.0;
const POLL_TIMER: usize = 1;
const HOLD_TIMER: usize = 2;
const POLL_MILLISECONDS: u32 = 50;
const PERSONALIZE_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

/// Index of the item under `y` (window pixels), or `None` over the padding or past the last item.
fn item_at(y: f32, item_height: f32, padding: f32, item_count: usize) -> Option<usize> {
    let offset = y - padding;
    if offset < 0.0 {
        return None;
    }
    let index = (offset / item_height) as usize;
    (index < item_count).then_some(index)
}

struct Menu {
    window: HWND,
    scale: f32,
    hovered: Option<usize>,
    foreground_at_open: HWND,
    target: Option<ID2D1HwndRenderTarget>,
    factory: ID2D1Factory,
    format: IDWriteTextFormat,
}

thread_local! {
    static MENU: RefCell<Option<Menu>> = const { RefCell::new(None) };
}

fn registry_dword(root: HKEY, key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(root, key, value, RRF_RT_REG_DWORD, None, Some((&mut data as *mut u32).cast()), Some(&mut size))
    };
    status.is_ok().then_some(data)
}

fn windows_build() -> String {
    let mut buffer = [0u16; 32];
    let mut size = size_of_val(&buffer) as u32;
    let key = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            w!("CurrentBuildNumber"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    let build = if status.is_ok() {
        String::from_utf16_lossy(&buffer).trim_end_matches('\0').to_string()
    } else {
        "unknown".into()
    };
    let revision =
        registry_dword(HKEY_LOCAL_MACHINE, key, w!("UBR")).map(|value| format!(".{value}")).unwrap_or_default();
    format!("{build}{revision}")
}

fn system_is_dark() -> bool {
    registry_dword(HKEY_CURRENT_USER, PERSONALIZE_KEY, w!("AppsUseLightTheme")) == Some(0)
}

fn transparency_enabled() -> Option<bool> {
    registry_dword(HKEY_CURRENT_USER, PERSONALIZE_KEY, w!("EnableTransparency")).map(|value| value != 0)
}

/// Private working set of this process in megabytes, counted like `scripts/perf.rs` does.
fn private_working_set_mb() -> f64 {
    let process = unsafe { GetCurrentProcess() };
    let mut capacity = 8192usize;
    loop {
        let mut buffer = vec![0usize; capacity + 1];
        let size_bytes = (buffer.len() * size_of::<usize>()) as u32;
        let is_complete = unsafe { K32QueryWorkingSet(process, buffer.as_mut_ptr().cast(), size_bytes).as_bool() };
        let count = buffer[0];
        if is_complete {
            let private_pages = buffer[1..=count].iter().filter(|word| (*word >> 8) & 1 == 0).count();
            return (private_pages * 4096) as f64 / (1024.0 * 1024.0);
        }
        capacity = if count > capacity { count + 512 } else { capacity * 2 };
    }
}

fn set_attribute(window: HWND, attribute: DWMWINDOWATTRIBUTE, value: i32) {
    let result =
        unsafe { DwmSetWindowAttribute(window, attribute, (&value as *const i32).cast(), size_of::<i32>() as u32) };
    if let Err(error) = result {
        println!("DwmSetWindowAttribute({}) failed: {error}", attribute.0);
    }
}

fn get_attribute(window: HWND, attribute: DWMWINDOWATTRIBUTE) -> Option<i32> {
    let mut value = 0i32;
    unsafe { DwmGetWindowAttribute(window, attribute, (&mut value as *mut i32).cast(), size_of::<i32>() as u32).ok()? };
    Some(value)
}

fn color(red: f32, green: f32, blue: f32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r: red, g: green, b: blue, a: alpha }
}

impl Menu {
    fn size_pixels(&self) -> (i32, i32) {
        let height = MENU_PADDING * 2.0 + ITEM_HEIGHT * LABELS.len() as f32;
        ((MENU_WIDTH * self.scale) as i32, (height * self.scale) as i32)
    }

    fn paint(&mut self) {
        let (width, height) = self.size_pixels();
        if self.target.is_none() {
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let window_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.window,
                pixelSize: D2D_SIZE_U { width: width as u32, height: height as u32 },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            self.target = unsafe { self.factory.CreateHwndRenderTarget(&properties, &window_properties).ok() };
        }
        let Some(target) = self.target.clone() else { return };
        let scale = self.scale;
        let (text_color, highlight_color) = (color(1.0, 1.0, 1.0, 1.0), color(1.0, 1.0, 1.0, 0.10));
        unsafe {
            let (Ok(text_brush), Ok(highlight_brush)) =
                (target.CreateSolidColorBrush(&text_color, None), target.CreateSolidColorBrush(&highlight_color, None))
            else {
                return;
            };
            target.BeginDraw();
            // Opaque dark gray: the system backdrop does not show through this surface.
            target.Clear(Some(&color(0.09, 0.09, 0.10, 1.0)));
            for (index, label) in LABELS.iter().enumerate() {
                let top = (MENU_PADDING + ITEM_HEIGHT * index as f32) * scale;
                let bottom = top + ITEM_HEIGHT * scale;
                if self.hovered == Some(index) {
                    let radius = HIGHLIGHT_RADIUS * scale;
                    target.FillRoundedRectangle(
                        &D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: HIGHLIGHT_INSET * scale,
                                top,
                                right: (MENU_WIDTH - HIGHLIGHT_INSET) * scale,
                                bottom,
                            },
                            radiusX: radius,
                            radiusY: radius,
                        },
                        &highlight_brush,
                    );
                }
                let wide: Vec<u16> = label.encode_utf16().collect();
                target.DrawText(
                    &wide,
                    &self.format,
                    &D2D_RECT_F { left: TEXT_LEFT * scale, top, right: (MENU_WIDTH - TEXT_LEFT) * scale, bottom },
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
            if target.EndDraw(None, None).is_err() {
                // Recreated on the next paint (covers a lost device).
                self.target = None;
            }
        }
    }
}

fn dismiss(window: HWND) {
    unsafe {
        let _ = KillTimer(Some(window), POLL_TIMER);
        let _ = ReleaseCapture();
        PostQuitMessage(0);
    }
}

fn client_point(lparam: LPARAM) -> (f32, f32) {
    (f32::from(lparam.0 as i16), f32::from((lparam.0 >> 16) as i16))
}

extern "system" fn window_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_PAINT => {
            MENU.with(|menu| {
                if let Some(menu) = menu.borrow_mut().as_mut() {
                    menu.paint();
                }
            });
            unsafe {
                let _ = windows::Win32::Graphics::Gdi::ValidateRect(Some(window), None);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = client_point(lparam);
            MENU.with(|menu| {
                if let Some(menu) = menu.borrow_mut().as_mut() {
                    let (width, height) = menu.size_pixels();
                    let is_inside = x >= 0.0 && y >= 0.0 && x < width as f32 && y < height as f32;
                    let hovered = if is_inside {
                        item_at(y, ITEM_HEIGHT * menu.scale, MENU_PADDING * menu.scale, LABELS.len())
                    } else {
                        None
                    };
                    if hovered != menu.hovered {
                        menu.hovered = hovered;
                        unsafe {
                            let _ = InvalidateRect(Some(window), None, false);
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP => {
            // The mouse is captured, so a click outside the menu also arrives here, with no item hovered.
            let chosen = MENU.with(|menu| menu.borrow().as_ref().and_then(|menu| menu.hovered));
            if let Some(index) = chosen {
                if message == WM_LBUTTONUP {
                    println!("clicked: {}", LABELS[index]);
                    dismiss(window);
                }
            } else if message == WM_LBUTTONDOWN {
                println!("dismissed by click outside");
                dismiss(window);
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == POLL_TIMER => {
            // WS_EX_NOACTIVATE means no focus messages arrive, so poll for Escape and a changed foreground window.
            let is_escape_down = unsafe { GetAsyncKeyState(i32::from(VK_ESCAPE.0)) } as u16 & 0x8000 != 0;
            let has_foreground_changed = MENU.with(|menu| {
                menu.borrow().as_ref().is_some_and(|menu| unsafe { GetForegroundWindow() } != menu.foreground_at_open)
            });
            if is_escape_down || has_foreground_changed {
                println!("dismissed by {}", if is_escape_down { "Escape" } else { "foreground change" });
                dismiss(window);
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == HOLD_TIMER => {
            unsafe {
                let _ = KillTimer(Some(window), HOLD_TIMER);
            }
            println!("private working set after hold: {:.1} MB", private_working_set_mb());
            dismiss(window);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn hold_seconds_argument() -> Option<u32> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--hold-seconds" {
            return arguments.next()?.parse().ok();
        }
    }
    None
}

fn main() -> windows::core::Result<()> {
    let started = Instant::now();
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("NotchTraySpike");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let foreground_at_open = GetForegroundWindow();
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            class_name,
            w!("Notch tray spike"),
            WS_POPUP,
            cursor.x,
            cursor.y,
            100,
            100,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let scale = GetDpiForWindow(window) as f32 / 96.0;

        let is_dark = system_is_dark();
        set_attribute(window, DWMWA_USE_IMMERSIVE_DARK_MODE, i32::from(is_dark));
        set_attribute(window, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW.0);
        set_attribute(window, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND.0);
        // A negative margin extends the frame over the whole client area so the backdrop is not covered.
        let _ = DwmExtendFrameIntoClientArea(
            window,
            &MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 },
        );

        println!("Windows build: {}", windows_build());
        println!("system theme: {}", if is_dark { "dark" } else { "light" });
        println!("transparency effects (registry EnableTransparency): {:?}", transparency_enabled());
        println!("DPI scale: {scale}");
        println!(
            "requested backdrop type {} (DWMSBT_TRANSIENTWINDOW), corner preference {} (DWMWCP_ROUND)",
            DWMSBT_TRANSIENTWINDOW.0, DWMWCP_ROUND.0
        );
        println!(
            "effective backdrop type: {:?}, corner preference: {:?}",
            get_attribute(window, DWMWA_SYSTEMBACKDROP_TYPE),
            get_attribute(window, DWMWA_WINDOW_CORNER_PREFERENCE)
        );

        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let write_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let format = write_factory.CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            FONT_SIZE * scale,
            w!("en-us"),
        )?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        let menu = Menu { window, scale, hovered: None, foreground_at_open, target: None, factory, format };
        let (width, height) = menu.size_pixels();
        MENU.with(|slot| *slot.borrow_mut() = Some(menu));
        SetWindowPos(window, None, cursor.x, cursor.y, width, height, SWP_NOACTIVATE | SWP_NOZORDER)?;
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        // Captured so a click outside the menu dismisses it, as no focus loss is reported.
        SetCapture(window);
        SetTimer(Some(window), POLL_TIMER, POLL_MILLISECONDS, None);
        if let Some(seconds) = hold_seconds_argument() {
            SetTimer(Some(window), HOLD_TIMER, seconds * 1000, None);
        }
        println!(
            "startup to shown: {} ms; private working set now: {:.1} MB",
            started.elapsed().as_millis(),
            private_working_set_mb()
        );

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::item_at;

    #[test]
    fn items_are_found_by_their_row() {
        assert_eq!(item_at(2.0, 40.0, 4.0, 5), None);
        assert_eq!(item_at(4.0, 40.0, 4.0, 5), Some(0));
        assert_eq!(item_at(43.9, 40.0, 4.0, 5), Some(0));
        assert_eq!(item_at(44.0, 40.0, 4.0, 5), Some(1));
        assert_eq!(item_at(204.0, 40.0, 4.0, 5), None);
    }
}
