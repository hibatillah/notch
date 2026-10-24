use windows::{
    Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
        },
        UI::{
            Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW},
            WindowsAndMessaging::{
                AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, ICONINFO,
                MF_STRING, PostQuitMessage, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
                WM_APP,
            },
        },
    },
    core::{Result, w},
};

/// Message the shell sends to the window for pointer events on the tray icon.
pub const WM_TRAY: u32 = WM_APP + 2;
const TRAY_ICON_ID: u32 = 1;
const ICON_SIZE: i32 = 32;
const TOOLTIP: &str = "Notch";
const MENU_COMMAND_QUIT: usize = 1;

/// Rounded-rectangle coverage at a pixel, from a signed distance so the edge is anti-aliased.
fn pill_coverage(x: f32, y: f32) -> f32 {
    let (center, half_width, half_height, radius) = (ICON_SIZE as f32 / 2.0, 13.0, 7.0, 7.0);
    let offset_x = ((x - center).abs() - (half_width - radius)).max(0.0);
    let offset_y = ((y - center).abs() - (half_height - radius)).max(0.0);
    let distance = (offset_x * offset_x + offset_y * offset_y).sqrt() - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

fn dot_coverage(x: f32, y: f32) -> f32 {
    let distance = ((x - 9.5).powi(2) + (y - ICON_SIZE as f32 / 2.0).powi(2)).sqrt() - 2.5;
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Draws a small light pill with a dark dot, readable on both light and dark taskbars.
fn create_icon() -> Result<HICON> {
    unsafe {
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ICON_SIZE,
                biHeight: -ICON_SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixel_pointer = std::ptr::null_mut();
        let color_bitmap = CreateDIBSection(None, &bitmap_info, DIB_RGB_COLORS, &mut pixel_pointer, None, 0)?;
        let pixels = std::slice::from_raw_parts_mut(pixel_pointer.cast::<u32>(), (ICON_SIZE * ICON_SIZE) as usize);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let (sample_x, sample_y) = (x as f32 + 0.5, y as f32 + 0.5);
                let pill = pill_coverage(sample_x, sample_y);
                let dot = dot_coverage(sample_x, sample_y);
                // Premultiplied light gray pill with a dark dot blended on top.
                let gray = 0.92 * (1.0 - dot) + 0.12 * dot;
                let channel = (gray * pill * 255.0).round() as u32;
                let alpha = (pill * 255.0).round() as u32;
                pixels[(y * ICON_SIZE + x) as usize] = (alpha << 24) | (channel << 16) | (channel << 8) | channel;
            }
        }
        let mask_bitmap = CreateBitmap(ICON_SIZE, ICON_SIZE, 1, 1, None);
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bitmap,
            hbmColor: color_bitmap,
        });
        let _ = DeleteObject(mask_bitmap.into());
        let _ = DeleteObject(color_bitmap.into());
        icon
    }
}

fn notify_data(window: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: TRAY_ICON_ID,
        ..Default::default()
    }
}

/// Adds the notch's icon to the notification area; also called again if Explorer restarts.
pub fn add(window: HWND) -> Result<()> {
    let mut data = notify_data(window);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = create_icon()?;
    for (slot, unit) in data.szTip.iter_mut().zip(TOOLTIP.encode_utf16()) {
        *slot = unit;
    }
    unsafe { Shell_NotifyIconW(NIM_ADD, &data).ok() }
}

/// Removes the icon; without this a stale icon stays in the tray until the pointer hovers it.
pub fn remove(window: HWND) {
    let data = notify_data(window);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// Shows the context menu at the pointer; it only has "Quit" for now.
pub fn show_context_menu(window: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let _ = AppendMenuW(menu, MF_STRING, MENU_COMMAND_QUIT, w!("Quit"));
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // A popup menu needs a foreground owner to dismiss correctly when clicking elsewhere.
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, cursor.x, cursor.y, None, window, None);
        let _ = DestroyMenu(menu);
        if command.0 as usize == MENU_COMMAND_QUIT {
            PostQuitMessage(0);
        }
    }
}
