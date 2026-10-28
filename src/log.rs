use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn debug(message: &str) {
    let Some(log_path) = std::env::var_os("NOTCH_DEBUG") else { return };
    let milliseconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{milliseconds} ms  {message}");
    }
}
