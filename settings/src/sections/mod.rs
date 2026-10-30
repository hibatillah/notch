use notch_store::Settings;

use crate::notify::tell_notch_to_reload;

pub const SECTION_NAMES: [&str; 8] =
    ["General", "Pages", "Behavior", "Notifications", "Calendar", "Media", "Timer", "Advanced"];

// Section bodies call this once they have toggles.
#[allow(dead_code)]
pub fn apply(settings: &mut Settings, key: &str, value: &str) {
    settings.set_string(key, value);
    tell_notch_to_reload();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_a_key_writes_it_to_the_file() {
        let path = std::env::temp_dir().join(format!("notch-settings-apply-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut settings = Settings::at(Some(path.clone()));
        apply(&mut settings, "notch.example", "on");
        let text = std::fs::read_to_string(&path).expect("settings file written");
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("notch.example=on"), "{text}");
    }
}
