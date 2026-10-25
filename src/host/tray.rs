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
                MF_CHECKED, MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
                TrackPopupMenu, WM_APP,
            },
        },
    },
    core::{HSTRING, PCWSTR, Result, w},
};

/// Message the shell sends to the window for pointer events on the tray icon.
pub const WM_TRAY: u32 = WM_APP + 2;
const TRAY_ICON_ID: u32 = 1;
const ICON_SIZE: i32 = 32;
const TOOLTIP: &str = "Notch";
const MENU_COMMAND_QUIT: usize = 1;
const MENU_COMMAND_HIDE_WHEN_IDLE: usize = 2;
const MENU_COMMAND_START_WITH_WINDOWS: usize = 3;
const MENU_COMMAND_HIDE_FOR_APPS: usize = 4;
const MENU_COMMAND_LYRICS_OVER_MAXIMIZED: usize = 5;
/// The command of the first activity in the menu; the rest follow in order.
const MENU_COMMAND_FIRST_ACTIVITY: usize = 100;

/// Commands of the items pages add start here: `PAGE_ITEM_BASE + page * ITEMS_PER_PAGE + item`.
const PAGE_ITEM_BASE: usize = 1000;
const ITEMS_PER_PAGE: usize = 16;

/// What the user picked from the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    /// Switch the activity at this index on or off.
    Toggle(usize),
    /// An item a page added to the menu.
    PageItem {
        page: usize,
        item: usize,
    },
    /// Switch hiding the notch when nothing is happening on or off.
    ToggleHideWhenIdle,
    /// Switch starting the notch when Windows starts on or off.
    ToggleStartWithWindows,
    /// Switch hiding the notch for maximized and full-screen windows on or off.
    ToggleHideForApps,
    /// Switch keeping the lyric line over maximized windows on or off.
    ToggleLyricsOverMaximized,
    Quit,
}

/// Which of the notch's own options are on, to show them checked.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuState {
    pub hides_when_idle: bool,
    pub starts_with_windows: bool,
    pub hides_for_apps: bool,
    pub lyrics_over_maximized: bool,
}

/// The choice a menu command stands for; `None` when the menu was dismissed or the command is unknown.
fn choice_for_command(command: usize, activity_count: usize) -> Option<MenuChoice> {
    if command == MENU_COMMAND_QUIT {
        return Some(MenuChoice::Quit);
    }
    if command == MENU_COMMAND_HIDE_WHEN_IDLE {
        return Some(MenuChoice::ToggleHideWhenIdle);
    }
    match command {
        MENU_COMMAND_START_WITH_WINDOWS => return Some(MenuChoice::ToggleStartWithWindows),
        MENU_COMMAND_HIDE_FOR_APPS => return Some(MenuChoice::ToggleHideForApps),
        MENU_COMMAND_LYRICS_OVER_MAXIMIZED => return Some(MenuChoice::ToggleLyricsOverMaximized),
        _ => {}
    }
    if let Some(offset) = command.checked_sub(PAGE_ITEM_BASE) {
        return Some(MenuChoice::PageItem { page: offset / ITEMS_PER_PAGE, item: offset % ITEMS_PER_PAGE });
    }
    let index = command.checked_sub(MENU_COMMAND_FIRST_ACTIVITY)?;
    (index < activity_count).then_some(MenuChoice::Toggle(index))
}

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

/// Shows the context menu at the pointer: a checkable item for each activity, the items pages add (page index, item
/// index, label), then the notch's own options and "Quit". Returns what was picked.
pub fn show_context_menu(
    window: HWND,
    activities: &[(&'static str, bool)],
    page_items: &[(usize, usize, String)],
    state: MenuState,
) -> Option<MenuChoice> {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return None };
        for (index, &(label, is_on)) in activities.iter().enumerate() {
            let flags = if is_on { MF_STRING | MF_CHECKED } else { MF_STRING };
            let _ = AppendMenuW(menu, flags, MENU_COMMAND_FIRST_ACTIVITY + index, &HSTRING::from(label));
        }
        if !activities.is_empty() {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        }
        for (page, item, label) in page_items {
            let command = PAGE_ITEM_BASE + page * ITEMS_PER_PAGE + item;
            let _ = AppendMenuW(menu, MF_STRING, command, &HSTRING::from(label.as_str()));
        }
        if !page_items.is_empty() {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        }
        let flags = |is_on: bool| if is_on { MF_STRING | MF_CHECKED } else { MF_STRING };
        let _ = AppendMenuW(menu, flags(state.hides_when_idle), MENU_COMMAND_HIDE_WHEN_IDLE, w!("Hide when idle"));
        let _ =
            AppendMenuW(menu, flags(state.hides_for_apps), MENU_COMMAND_HIDE_FOR_APPS, w!("Hide for full-screen apps"));
        let _ = AppendMenuW(
            menu,
            flags(state.lyrics_over_maximized),
            MENU_COMMAND_LYRICS_OVER_MAXIMIZED,
            w!("Show lyrics over maximized windows"),
        );
        let _ = AppendMenuW(
            menu,
            flags(state.starts_with_windows),
            MENU_COMMAND_START_WITH_WINDOWS,
            w!("Start with Windows"),
        );
        let _ = AppendMenuW(menu, MF_STRING, MENU_COMMAND_QUIT, w!("Quit"));
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // A popup menu needs a foreground owner to dismiss correctly when clicking elsewhere.
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, cursor.x, cursor.y, None, window, None);
        let _ = DestroyMenu(menu);
        choice_for_command(command.0 as usize, activities.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_and_each_activity_have_their_own_command() {
        assert_eq!(choice_for_command(MENU_COMMAND_QUIT, 3), Some(MenuChoice::Quit));
        assert_eq!(choice_for_command(MENU_COMMAND_FIRST_ACTIVITY, 3), Some(MenuChoice::Toggle(0)));
        assert_eq!(choice_for_command(MENU_COMMAND_FIRST_ACTIVITY + 2, 3), Some(MenuChoice::Toggle(2)));
    }

    #[test]
    fn hiding_when_idle_has_its_own_command() {
        assert_eq!(choice_for_command(MENU_COMMAND_HIDE_WHEN_IDLE, 3), Some(MenuChoice::ToggleHideWhenIdle));
    }

    #[test]
    fn starting_with_windows_has_its_own_command() {
        assert_eq!(choice_for_command(MENU_COMMAND_START_WITH_WINDOWS, 3), Some(MenuChoice::ToggleStartWithWindows));
    }

    #[test]
    fn the_options_about_other_windows_have_their_own_commands() {
        assert_eq!(choice_for_command(MENU_COMMAND_HIDE_FOR_APPS, 3), Some(MenuChoice::ToggleHideForApps));
        assert_eq!(
            choice_for_command(MENU_COMMAND_LYRICS_OVER_MAXIMIZED, 3),
            Some(MenuChoice::ToggleLyricsOverMaximized)
        );
    }

    #[test]
    fn page_items_name_their_page_and_position() {
        let command = PAGE_ITEM_BASE + 3 * ITEMS_PER_PAGE + 2;
        assert_eq!(choice_for_command(command, 3), Some(MenuChoice::PageItem { page: 3, item: 2 }));
    }

    #[test]
    fn a_dismissed_menu_and_unknown_commands_choose_nothing() {
        assert_eq!(choice_for_command(0, 3), None);
        assert_eq!(choice_for_command(MENU_COMMAND_FIRST_ACTIVITY + 3, 3), None);
        assert_eq!(choice_for_command(50, 3), None);
    }
}
