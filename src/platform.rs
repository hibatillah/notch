//! Small Windows actions that pages need: opening a file or a link, showing a file in Explorer, putting text on the
//! clipboard and showing a popup menu. Each is a thin wrapper over one Windows call.

use std::path::Path;

use windows::{
    Win32::{
        Foundation::{HANDLE, HGLOBAL, HWND, POINT},
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, MF_STRING, SW_SHOWNORMAL, SetForegroundWindow,
                TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
            },
        },
    },
    core::{HSTRING, PCWSTR, w},
};

/// The clipboard format for UTF-16 text.
const CF_UNICODETEXT: u32 = 13;

/// Opens a file, folder or web address with the program Windows has for it.
pub fn open(target: &str) {
    unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
}

/// Shows a file selected in a new Explorer window.
pub fn reveal(path: &Path) {
    let arguments = format!("/select,\"{}\"", path.display());
    unsafe {
        ShellExecuteW(None, w!("open"), w!("explorer.exe"), &HSTRING::from(arguments), PCWSTR::null(), SW_SHOWNORMAL)
    };
}

/// Replaces the clipboard with `text`; returns false when the clipboard could not be used.
pub fn copy_text(text: &str) -> bool {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = units.len() * size_of::<u16>();
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let stored = (|| {
            let memory = GlobalAlloc(GMEM_MOVEABLE, bytes).ok()?;
            let target = GlobalLock(memory).cast::<u16>();
            if target.is_null() {
                return None;
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
            let _ = GlobalUnlock(memory);
            // The clipboard owns the memory once it accepts it.
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))).ok()
        })();
        let _ = CloseClipboard();
        stored.is_some()
    }
}

/// The text on the clipboard, if there is any.
pub fn paste_text() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;
        let text = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let memory = HGLOBAL(handle.0);
            let source = GlobalLock(memory).cast::<u16>();
            if source.is_null() {
                return None;
            }
            let length = (0..).take_while(|&offset| *source.add(offset) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));
            let _ = GlobalUnlock(memory);
            Some(text)
        })();
        let _ = CloseClipboard();
        text
    }
}

/// Shows a popup menu of `items` at the pointer and returns the position of the one chosen, if any.
pub fn popup_menu(owner: HWND, items: &[&str]) -> Option<usize> {
    const FIRST_COMMAND: usize = 1;
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for (index, item) in items.iter().enumerate() {
            let _ = AppendMenuW(menu, MF_STRING, FIRST_COMMAND + index, &HSTRING::from(*item));
        }
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        // A popup menu needs a foreground owner to dismiss correctly when clicking elsewhere.
        let _ = SetForegroundWindow(owner);
        let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, cursor.x, cursor.y, None, owner, None);
        let _ = DestroyMenu(menu);
        usize::try_from(command.0).ok()?.checked_sub(FIRST_COMMAND).filter(|index| *index < items.len())
    }
}
