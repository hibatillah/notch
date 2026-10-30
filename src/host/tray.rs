use windows::{
    Win32::{
        Foundation::{HWND, POINT},
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
        UI::{
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW},
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_CHECKED, MF_SEPARATOR, MF_STRING,
                SM_CXSMICON, SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP,
            },
        },
    },
    core::{HSTRING, PCSTR, PCWSTR, Result, w},
};

use super::icon::Icon;

pub const WM_TRAY: u32 = WM_APP + 2;
const TRAY_ICON_ID: u32 = 1;
const TOOLTIP: &str = "Notch";
const MENU_COMMAND_QUIT: usize = 1;
const MENU_COMMAND_HIDE_WHEN_IDLE: usize = 2;
const MENU_COMMAND_START_WITH_WINDOWS: usize = 3;
const MENU_COMMAND_HIDE_FOR_APPS: usize = 4;
const MENU_COMMAND_LYRICS_OVER_MAXIMIZED: usize = 5;
const MENU_COMMAND_NOTCH_OVER_MAXIMIZED: usize = 6;
const MENU_COMMAND_OPEN_SETTINGS: usize = 7;
const MENU_COMMAND_FIRST_ACTIVITY: usize = 100;

/// Commands of the items pages add start here: `PAGE_ITEM_BASE + page * ITEMS_PER_PAGE + item`.
const PAGE_ITEM_BASE: usize = 1000;
const ITEMS_PER_PAGE: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Toggle(usize),
    PageItem {
        page: usize,
        item: usize,
    },
    ToggleHideWhenIdle,
    ToggleStartWithWindows,
    /// Fullscreen hiding is independent of the maximized-window options.
    ToggleHideForApps,
    ToggleLyricsOverMaximized,
    ToggleNotchOverMaximized,
    OpenSettings,
    Quit,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MenuState {
    pub hides_when_idle: bool,
    pub starts_with_windows: bool,
    pub hides_for_apps: bool,
    pub lyrics_over_maximized: bool,
    pub notch_over_maximized: bool,
}

/// The choice a menu command stands for; `None` when the menu was dismissed or the command is unknown.
fn choice_for_command(command: usize, activities: &[(usize, &'static str, bool)]) -> Option<MenuChoice> {
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
        MENU_COMMAND_NOTCH_OVER_MAXIMIZED => return Some(MenuChoice::ToggleNotchOverMaximized),
        MENU_COMMAND_OPEN_SETTINGS => return Some(MenuChoice::OpenSettings),
        _ => {}
    }
    if let Some(offset) = command.checked_sub(PAGE_ITEM_BASE) {
        return Some(MenuChoice::PageItem { page: offset / ITEMS_PER_PAGE, item: offset % ITEMS_PER_PAGE });
    }
    let index = command.checked_sub(MENU_COMMAND_FIRST_ACTIVITY)?;
    activities.iter().any(|&(activity_index, _, _)| activity_index == index).then_some(MenuChoice::Toggle(index))
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
    let dpi = unsafe { GetDpiForWindow(window) };
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) };
    let icon = Icon::load(size)?;
    let mut data = notify_data(window);
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = icon.handle();
    for (slot, unit) in data.szTip.iter_mut().zip(TOOLTIP.encode_utf16()) {
        *slot = unit;
    }
    // The shell copies the icon, so this temporary size-specific handle can be released afterwards.
    unsafe { Shell_NotifyIconW(NIM_ADD, &data).ok() }
}

/// Removes the icon; without this a stale icon stays in the tray until the pointer hovers it.
pub fn remove(window: HWND) {
    let data = notify_data(window);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// Undocumented uxtheme ordinals; resolve them at runtime so unsupported Windows versions still work.
const SET_PREFERRED_APP_MODE: usize = 135;
const FLUSH_MENU_THEMES: usize = 136;
const APP_MODE_ALLOW_DARK: i32 = 1;

/// Missing theme entry points leave the default light menu in place.
fn follow_system_theme() {
    type Entry = unsafe extern "system" fn() -> isize;
    unsafe {
        let Ok(module) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        if let Some(entry) = GetProcAddress(module, PCSTR(SET_PREFERRED_APP_MODE as *const u8)) {
            // SAFETY: this entry point takes the app mode and returns the previous one.
            let set_mode = std::mem::transmute::<Entry, extern "system" fn(i32) -> i32>(entry);
            set_mode(APP_MODE_ALLOW_DARK);
        }
        if let Some(entry) = GetProcAddress(module, PCSTR(FLUSH_MENU_THEMES as *const u8)) {
            // SAFETY: this entry point takes nothing and returns nothing.
            let flush = std::mem::transmute::<Entry, extern "system" fn()>(entry);
            flush();
        }
    }
}

pub fn show_context_menu(
    window: HWND,
    activities: &[(usize, &'static str, bool)],
    page_items: &[(usize, usize, String)],
    state: MenuState,
) -> Option<MenuChoice> {
    // Each time, so a change of theme while the notch runs shows in the next menu.
    follow_system_theme();
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return None };
        for &(index, label, is_on) in activities {
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
            flags(state.notch_over_maximized),
            MENU_COMMAND_NOTCH_OVER_MAXIMIZED,
            w!("Show notch over maximized windows"),
        );
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
        let _ = AppendMenuW(menu, MF_STRING, MENU_COMMAND_OPEN_SETTINGS, w!("Settings…"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_COMMAND_QUIT, w!("Quit"));
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // A popup menu needs a foreground owner to dismiss correctly when clicking elsewhere.
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, cursor.x, cursor.y, None, window, None);
        let _ = DestroyMenu(menu);
        choice_for_command(command.0 as usize, activities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONAL_ACTIVITIES: &[(usize, &str, bool)] = &[(8, "Notifications", true)];

    #[test]
    fn quit_and_the_optional_activity_have_their_own_commands() {
        assert_eq!(choice_for_command(MENU_COMMAND_QUIT, OPTIONAL_ACTIVITIES), Some(MenuChoice::Quit));
        assert_eq!(choice_for_command(MENU_COMMAND_FIRST_ACTIVITY, OPTIONAL_ACTIVITIES), None);
        assert_eq!(
            choice_for_command(MENU_COMMAND_FIRST_ACTIVITY + 8, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::Toggle(8))
        );
    }

    #[test]
    fn hiding_when_idle_has_its_own_command() {
        assert_eq!(
            choice_for_command(MENU_COMMAND_HIDE_WHEN_IDLE, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::ToggleHideWhenIdle)
        );
    }

    #[test]
    fn starting_with_windows_has_its_own_command() {
        assert_eq!(
            choice_for_command(MENU_COMMAND_START_WITH_WINDOWS, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::ToggleStartWithWindows)
        );
    }

    #[test]
    fn the_options_about_other_windows_have_their_own_commands() {
        assert_eq!(
            choice_for_command(MENU_COMMAND_HIDE_FOR_APPS, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::ToggleHideForApps)
        );
        assert_eq!(
            choice_for_command(MENU_COMMAND_LYRICS_OVER_MAXIMIZED, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::ToggleLyricsOverMaximized)
        );
        assert_eq!(
            choice_for_command(MENU_COMMAND_NOTCH_OVER_MAXIMIZED, OPTIONAL_ACTIVITIES),
            Some(MenuChoice::ToggleNotchOverMaximized)
        );
    }

    #[test]
    fn settings_has_its_own_command() {
        assert_eq!(choice_for_command(MENU_COMMAND_OPEN_SETTINGS, OPTIONAL_ACTIVITIES), Some(MenuChoice::OpenSettings));
    }

    #[test]
    fn page_items_name_their_page_and_position() {
        let command = PAGE_ITEM_BASE + 3 * ITEMS_PER_PAGE + 2;
        assert_eq!(choice_for_command(command, OPTIONAL_ACTIVITIES), Some(MenuChoice::PageItem { page: 3, item: 2 }));
    }

    #[test]
    fn a_dismissed_menu_and_unknown_commands_choose_nothing() {
        assert_eq!(choice_for_command(0, OPTIONAL_ACTIVITIES), None);
        assert_eq!(choice_for_command(MENU_COMMAND_FIRST_ACTIVITY + 3, OPTIONAL_ACTIVITIES), None);
        assert_eq!(choice_for_command(50, OPTIONAL_ACTIVITIES), None);
    }

    #[test]
    #[ignore = "depends on the Windows version; run on purpose"]
    fn this_windows_has_the_entry_points_that_make_menus_follow_the_theme() {
        let module = unsafe { LoadLibraryW(w!("uxtheme.dll")) }.expect("uxtheme.dll");
        for ordinal in [SET_PREFERRED_APP_MODE, FLUSH_MENU_THEMES] {
            assert!(unsafe { GetProcAddress(module, PCSTR(ordinal as *const u8)) }.is_some(), "ordinal {ordinal}");
        }
    }
}
