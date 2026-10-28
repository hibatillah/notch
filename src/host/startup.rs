//! Shares the installer's current-user Run entry; no administrator rights are required.

use std::path::Path;

use windows::{
    Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    },
    core::{HSTRING, w},
};

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("Notch");

fn run_command(executable: &Path) -> String {
    format!("\"{}\"", executable.display())
}

pub fn is_enabled() -> bool {
    let mut bytes = 0u32;
    let status =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, RRF_RT_REG_SZ, None, None, Some(&mut bytes)) };
    status.is_ok() && bytes > 0
}

pub fn set_enabled(is_enabled: bool) {
    if !is_enabled {
        let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) };
        return;
    }
    let Ok(executable) = std::env::current_exe() else { return };
    let command = HSTRING::from(run_command(&executable));
    let bytes = (command.len() + 1) * size_of::<u16>();
    let _ = unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, REG_SZ.0, Some(command.as_ptr().cast()), bytes as u32)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_quotes_a_path_with_spaces() {
        assert_eq!(
            run_command(Path::new(r"C:\Users\Some One\Notch\notch.exe")),
            "\"C:\\Users\\Some One\\Notch\\notch.exe\""
        );
    }
}
