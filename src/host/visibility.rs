use super::{screen_mode::ScreenMode, settings::Settings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Showing {
    Hidden,
    ActivitiesOnly,
    MusicWithLyrics,
    Music,
}

/// Matches the old defaults: lyrics stay visible over maximized windows, full-screen windows hide the notch.
pub const DEFAULT_MAXIMIZED: Showing = Showing::MusicWithLyrics;
pub const DEFAULT_FULLSCREEN: Showing = Showing::Hidden;

pub const MAXIMIZED_KEY: &str = "notch.maximized";
pub const FULLSCREEN_KEY: &str = "notch.fullscreen";

const OLD_HIDE_FOR_FULLSCREEN_KEY: &str = "notch.hide_for_fullscreen";
const OLD_SHOW_OVER_MAXIMIZED_KEY: &str = "notch.show_over_maximized";
const OLD_LYRICS_OVER_MAXIMIZED_KEY: &str = "notch.lyrics_over_maximized";

impl Showing {
    pub fn parse(text: &str) -> Option<Showing> {
        match text {
            "hidden" => Some(Showing::Hidden),
            "activities" => Some(Showing::ActivitiesOnly),
            "lyrics" => Some(Showing::MusicWithLyrics),
            "music" => Some(Showing::Music),
            _ => None,
        }
    }

    pub fn as_text(self) -> &'static str {
        match self {
            Showing::Hidden => "hidden",
            Showing::ActivitiesOnly => "activities",
            Showing::MusicWithLyrics => "lyrics",
            Showing::Music => "music",
        }
    }
}

pub fn showing_for(mode: ScreenMode, maximized: Showing, fullscreen: Showing) -> Option<Showing> {
    match mode {
        ScreenMode::Normal => None,
        ScreenMode::Maximized => Some(maximized),
        ScreenMode::Fullscreen => Some(fullscreen),
    }
}

/// Writes a new key only when it is absent and an old key exists; with no old key the default already matches.
pub fn migrate(settings: &mut Settings) {
    if settings.get_string(FULLSCREEN_KEY).is_none() && settings.get_string(OLD_HIDE_FOR_FULLSCREEN_KEY).is_some() {
        let hides = settings.get_bool(OLD_HIDE_FOR_FULLSCREEN_KEY, true);
        settings.set_string(FULLSCREEN_KEY, if hides { Showing::Hidden } else { Showing::Music }.as_text());
    }
    let has_old_maximized_key = settings.get_string(OLD_SHOW_OVER_MAXIMIZED_KEY).is_some()
        || settings.get_string(OLD_LYRICS_OVER_MAXIMIZED_KEY).is_some();
    if settings.get_string(MAXIMIZED_KEY).is_none() && has_old_maximized_key {
        let showing = if settings.get_bool(OLD_SHOW_OVER_MAXIMIZED_KEY, false) {
            Showing::Music
        } else if settings.get_bool(OLD_LYRICS_OVER_MAXIMIZED_KEY, true) {
            Showing::MusicWithLyrics
        } else {
            Showing::ActivitiesOnly
        };
        settings.set_string(MAXIMIZED_KEY, showing.as_text());
    }
}

/// An activity (a notification banner, a volume change) shows in every mode except Hidden; Music keeps the pill up
/// even with nothing playing, so the idle clock stays visible.
pub fn allows(showing: Showing, has_activity: bool, is_playing: bool, has_lyrics: bool) -> bool {
    match showing {
        Showing::Hidden => false,
        Showing::ActivitiesOnly => has_activity,
        Showing::MusicWithLyrics => has_activity || (is_playing && has_lyrics),
        Showing::Music => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(contents: &str) -> (Settings, std::path::PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("notch-visibility-{}-{id}.txt", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        (Settings::at(Some(path.clone())), path)
    }

    #[test]
    fn parse_rejects_unknown_words_and_the_default_applies() {
        assert_eq!(Showing::parse("sometimes"), None);
        assert_eq!(Showing::parse(""), None);
        assert_eq!(Showing::parse("Music"), None);
        assert_eq!(Showing::parse(" music"), None);
        assert_eq!(Showing::parse("bogus").unwrap_or(DEFAULT_MAXIMIZED), Showing::MusicWithLyrics);
        assert_eq!(Showing::parse("bogus").unwrap_or(DEFAULT_FULLSCREEN), Showing::Hidden);
    }

    #[test]
    fn every_showing_round_trips_through_its_text() {
        for showing in [Showing::Hidden, Showing::ActivitiesOnly, Showing::MusicWithLyrics, Showing::Music] {
            assert_eq!(Showing::parse(showing.as_text()), Some(showing));
        }
        assert_eq!(Showing::parse("hidden"), Some(Showing::Hidden));
        assert_eq!(Showing::parse("activities"), Some(Showing::ActivitiesOnly));
        assert_eq!(Showing::parse("lyrics"), Some(Showing::MusicWithLyrics));
        assert_eq!(Showing::parse("music"), Some(Showing::Music));
    }

    #[test]
    fn a_normal_window_has_no_showing_override() {
        assert_eq!(showing_for(ScreenMode::Normal, Showing::Music, Showing::Music), None);
        assert_eq!(showing_for(ScreenMode::Maximized, Showing::Music, Showing::Hidden), Some(Showing::Music));
        assert_eq!(showing_for(ScreenMode::Fullscreen, Showing::Music, Showing::Hidden), Some(Showing::Hidden));
    }

    #[test]
    fn fullscreen_hide_on_migrates_to_hidden() {
        let (mut settings, path) = settings_with("notch.hide_for_fullscreen=on\n");
        migrate(&mut settings);
        assert_eq!(settings.get_string(FULLSCREEN_KEY), Some("hidden"));
        let (mut settings, path_off) = settings_with("notch.hide_for_fullscreen=off\n");
        migrate(&mut settings);
        assert_eq!(settings.get_string(FULLSCREEN_KEY), Some("music"));
        let _ = (std::fs::remove_file(path), std::fs::remove_file(path_off));
    }

    #[test]
    fn maximized_show_over_maximized_migrates_to_music() {
        let (mut settings, path) = settings_with("notch.show_over_maximized=on\nnotch.lyrics_over_maximized=off\n");
        migrate(&mut settings);
        assert_eq!(settings.get_string(MAXIMIZED_KEY), Some("music"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn maximized_defaults_migrate_to_lyrics() {
        let (mut settings, path) = settings_with("notch.show_over_maximized=off\n");
        migrate(&mut settings);
        assert_eq!(settings.get_string(MAXIMIZED_KEY), Some("lyrics"));
        let (mut settings, path_off) = settings_with("notch.lyrics_over_maximized=off\n");
        migrate(&mut settings);
        assert_eq!(settings.get_string(MAXIMIZED_KEY), Some("activities"));
        let _ = (std::fs::remove_file(path), std::fs::remove_file(path_off));
    }

    #[test]
    fn migration_never_overwrites_a_new_key() {
        let (mut settings, path) = settings_with(
            "notch.maximized=hidden\nnotch.fullscreen=music\nnotch.show_over_maximized=on\nnotch.hide_for_fullscreen=on\n",
        );
        migrate(&mut settings);
        migrate(&mut settings);
        assert_eq!(settings.get_string(MAXIMIZED_KEY), Some("hidden"));
        assert_eq!(settings.get_string(FULLSCREEN_KEY), Some("music"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migration_writes_nothing_when_no_old_key_exists_and_is_idempotent() {
        let (mut settings, path) = settings_with("");
        migrate(&mut settings);
        assert_eq!(settings.get_string(MAXIMIZED_KEY), None);
        assert_eq!(settings.get_string(FULLSCREEN_KEY), None);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        let _ = std::fs::remove_file(path);

        let (mut settings, path) = settings_with("notch.show_over_maximized=on\n");
        migrate(&mut settings);
        let once = std::fs::read_to_string(&path).unwrap();
        migrate(&mut settings);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
        assert_eq!(settings.get_string(FULLSCREEN_KEY), None);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn music_with_lyrics_needs_lyrics_but_music_does_not() {
        assert!(allows(Showing::MusicWithLyrics, false, true, true));
        assert!(!allows(Showing::MusicWithLyrics, false, true, false));
        assert!(!allows(Showing::MusicWithLyrics, false, false, true));
        assert!(allows(Showing::MusicWithLyrics, true, false, false));
        assert!(allows(Showing::Music, false, true, false));
    }

    #[test]
    fn music_keeps_the_pill_up_even_with_nothing_playing() {
        assert!(allows(Showing::Music, false, false, false));
    }

    #[test]
    fn activities_only_shows_only_with_an_activity() {
        assert!(allows(Showing::ActivitiesOnly, true, false, false));
        assert!(!allows(Showing::ActivitiesOnly, false, true, true));
        assert!(!allows(Showing::Hidden, true, true, true));
    }
}
