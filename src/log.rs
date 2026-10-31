use std::{
    io::Write,
    path::PathBuf,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use notch_store::Settings;

// Read once at the first call: the environment variable wins, then the settings window's key; both apply at start.
fn log_path() -> Option<&'static PathBuf> {
    static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    LOG_PATH
        .get_or_init(|| {
            std::env::var_os("NOTCH_DEBUG").map(PathBuf::from).or_else(|| {
                let settings = Settings::load();
                settings
                    .get_string("advanced.debug_log")
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(PathBuf::from)
            })
        })
        .as_ref()
}

pub fn debug(message: &str) {
    let Some(log_path) = log_path() else { return };
    let milliseconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{milliseconds} ms  {message}");
    }
}
