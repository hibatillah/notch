use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

/// Appends a timestamped line to the file named by `NOTCH_DEBUG`; a windowed app has no console to print to.
/// The timestamp is Unix time in milliseconds, so it can be compared with other tools' clocks.
pub fn debug(message: &str) {
    let Some(log_path) = std::env::var_os("NOTCH_DEBUG") else { return };
    let milliseconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{milliseconds} ms  {message}");
    }
}
