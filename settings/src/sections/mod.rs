mod advanced;
mod behavior;
mod calendar;
mod general;
mod pages;

use notch_store::Settings;

use crate::{layout::Page, notify::tell_notch_to_reload};

pub fn apply(settings: &mut Settings, key: &str, value: &str) {
    settings.set_string(key, value);
    tell_notch_to_reload();
}

pub fn all() -> Vec<Page> {
    vec![general::page(), pages::page(), behavior::page(), calendar::page(), advanced::page()]
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
