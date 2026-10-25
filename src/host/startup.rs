//! Whether the notch starts when the user signs in to Windows. Windows starts every program listed under the current
//! user's `Run` key, so switching this on adds the notch's own path there and switching it off removes it. The
//! installer adds the same entry; nothing here needs administrator rights.

use std::path::Path;

use windows::{
    Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    },
    core::{HSTRING, w},
};

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("Notch");

/// The command Windows runs at sign-in: the path in quotes, so a space in it does not split the command.
fn run_command(executable: &Path) -> String {
    format!("\"{}\"", executable.display())
}

/// True when the notch is listed to start with Windows.
pub fn is_enabled() -> bool {
    let mut bytes = 0u32;
    let status =
        unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME, RRF_RT_REG_SZ, None, None, Some(&mut bytes)) };
    status.is_ok() && bytes > 0
}

/// Lists this copy of the notch to start with Windows, or takes it off the list.
pub fn set_enabled(is_enabled: bool) {
    if !is_enabled {
        let _ = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE_NAME) };
        return;
    }
    let Ok(executable) = std::env::current_exe() else { return };
    let command = HSTRING::from(run_command(&executable));
    // The value includes the null that ends the text.
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
