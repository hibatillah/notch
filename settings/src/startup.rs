//! The same current-user Run value as `src/host/startup.rs`, so the tray item and this window agree.

use std::path::PathBuf;

use windows::{
    Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    },
    core::{HSTRING, PCWSTR, w},
};

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: PCWSTR = w!("Notch");

/// `notch.exe` sits in the same folder as this program.
fn notch_executable() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?.with_file_name("notch.exe");
    executable.is_file().then_some(executable)
}

pub fn is_enabled() -> bool {
    let mut bytes = 0u32;
    // SAFETY: only the size of the value is requested; no output buffer is passed.
    let status =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, RRF_RT_REG_SZ, None, None, Some(&mut bytes)) };
    status.is_ok() && bytes > 0
}

pub fn set_enabled(enabled: bool) {
    if !enabled {
        // SAFETY: both strings are static and null-terminated.
        let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) };
        return;
    }
    let Some(executable) = notch_executable() else { return };
    let command = HSTRING::from(format!("\"{}\"", executable.display()));
    let bytes = (command.len() + 1) * size_of::<u16>();
    // SAFETY: `command` is a null-terminated UTF-16 buffer of `bytes` bytes that outlives the call.
    let _ = unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, REG_SZ.0, Some(command.as_ptr().cast()), bytes as u32)
    };
}
