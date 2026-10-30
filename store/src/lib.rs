use std::{collections::BTreeMap, fs, path::PathBuf};

/// Small persistent key-value store shared by all pages, saved as `key=value` lines in `%APPDATA%\notch`.
pub struct Settings {
    path: Option<PathBuf>,
    values: BTreeMap<String, String>,
}

impl Settings {
    pub fn load() -> Self {
        Self::at(
            std::env::var_os("APPDATA").map(|directory| PathBuf::from(directory).join("notch").join("settings.txt")),
        )
    }

    pub fn at(path: Option<PathBuf>) -> Self {
        let mut settings = Self { path, values: BTreeMap::new() };
        settings.reload();
        settings
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.get_string(key) {
            Some("on") | Some("true") => true,
            Some("off") | Some("false") => false,
            _ => default,
        }
    }

    pub fn set_string(&mut self, key: &str, value: &str) {
        self.store(key, value.to_owned());
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.store(key, if value { "on" } else { "off" }.to_owned());
    }

    pub fn reload(&mut self) {
        self.values = self.read_file();
    }

    fn read_file(&self) -> BTreeMap<String, String> {
        self.path.as_ref().and_then(|path| fs::read_to_string(path).ok()).map(|text| parse(&text)).unwrap_or_default()
    }

    /// Re-reads the file so keys another process wrote are kept, then replaces it atomically through a temp file.
    fn store(&mut self, key: &str, value: String) {
        let Some(path) = self.path.clone() else {
            self.values.insert(key.to_owned(), value);
            return;
        };
        let mut values = self.read_file();
        values.insert(key.to_owned(), value);
        if let Some(directory) = path.parent() {
            let _ = fs::create_dir_all(directory);
        }
        let text: String = values.iter().map(|(key, value)| format!("{key}={value}\n")).collect();
        let mut temporary_name = path.clone().into_os_string();
        temporary_name.push(".tmp");
        let temporary_path = PathBuf::from(temporary_name);
        if fs::write(&temporary_path, text).is_ok() && fs::rename(&temporary_path, &path).is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        self.values = values;
    }
}

fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("notch-test-{}-{name}", std::process::id())).join("settings.txt")
    }

    #[test]
    fn parses_key_value_lines() {
        let values = parse("a=1\n  b = two \nnot a setting\nc=x=y");
        assert_eq!(values.get("a").map(String::as_str), Some("1"));
        assert_eq!(values.get("b").map(String::as_str), Some("two"));
        assert_eq!(values.get("c").map(String::as_str), Some("x=y"));
        assert_eq!(values.len(), 3);
    }

    #[test]
    fn booleans_accept_on_off_true_false_and_fall_back_to_the_default() {
        let settings = Settings { path: None, values: parse("a=on\nb=off\nc=true\nd=false\ne=maybe") };
        assert!(settings.get_bool("a", false));
        assert!(!settings.get_bool("b", true));
        assert!(settings.get_bool("c", false));
        assert!(!settings.get_bool("d", true));
        assert!(settings.get_bool("e", true));
        assert!(!settings.get_bool("missing", false));
    }

    #[test]
    fn values_survive_a_restart() {
        let path = temporary_file("restart");
        let mut settings = Settings::at(Some(path.clone()));
        settings.set_bool("media.lyrics", false);
        settings.set_bool("clock.seconds", true);

        let reloaded = Settings::at(Some(path.clone()));
        assert!(!reloaded.get_bool("media.lyrics", true));
        assert!(reloaded.get_bool("clock.seconds", false));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn text_values_are_stored_and_replaced() {
        let mut settings = Settings { path: None, values: BTreeMap::new() };
        settings.set_string("navigation.page", "media");
        assert_eq!(settings.get_string("navigation.page"), Some("media"));
        settings.set_string("navigation.page", "clock");
        assert_eq!(settings.get_string("navigation.page"), Some("clock"));
    }

    #[test]
    fn without_a_path_nothing_is_written() {
        let mut settings = Settings { path: None, values: BTreeMap::new() };
        settings.set_bool("a", true);
        assert!(settings.get_bool("a", false));
    }

    #[test]
    fn two_writers_changing_different_keys_keep_both() {
        let path = temporary_file("two-writers");
        let mut notch = Settings::at(Some(path.clone()));
        let mut window = Settings::at(Some(path.clone()));
        notch.set_string("navigation.page", "media");
        window.set_bool("clock.seconds", true);

        let reloaded = Settings::at(Some(path.clone()));
        assert_eq!(reloaded.get_string("navigation.page"), Some("media"));
        assert_eq!(reloaded.get_string("clock.seconds"), Some("on"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_set_keeps_keys_another_process_wrote() {
        let path = temporary_file("other-process");
        let mut settings = Settings::at(Some(path.clone()));
        settings.set_string("a", "1");
        fs::write(&path, "a=1\nb=2\n").unwrap();
        settings.set_string("c", "3");

        let reloaded = Settings::at(Some(path.clone()));
        assert_eq!(reloaded.get_string("b"), Some("2"));
        assert_eq!(reloaded.get_string("c"), Some("3"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn reload_picks_up_a_changed_file() {
        let path = temporary_file("reload");
        let mut settings = Settings::at(Some(path.clone()));
        settings.set_string("a", "1");
        fs::write(&path, "a=9\nb=2\n").unwrap();
        settings.reload();
        assert_eq!(settings.get_string("a"), Some("9"));
        assert_eq!(settings.get_string("b"), Some("2"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_missing_file_is_an_empty_store() {
        let mut settings = Settings::at(Some(temporary_file("missing")));
        assert_eq!(settings.get_string("anything"), None);
        settings.reload();
        assert_eq!(settings.get_string("anything"), None);
    }
}
