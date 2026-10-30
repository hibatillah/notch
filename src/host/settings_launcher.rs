use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::log;

const SETTINGS_PROGRAM: &str = "notch-settings.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn settings_program_path(notch_exe: &Path) -> PathBuf {
    notch_exe.with_file_name(SETTINGS_PROGRAM)
}

/// Starts the settings program without waiting; it shows its existing window when one is already open.
pub fn open_settings_window() {
    let result = std::env::current_exe()
        .and_then(|notch_exe| Command::new(settings_program_path(&notch_exe)).creation_flags(CREATE_NO_WINDOW).spawn());
    if let Err(error) = result {
        log::debug(&format!("could not start settings: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_program_sits_beside_the_notch() {
        let path = settings_program_path(Path::new(r"C:\Apps\Notch\notch.exe"));
        assert_eq!(path, PathBuf::from(r"C:\Apps\Notch\notch-settings.exe"));
    }
}
