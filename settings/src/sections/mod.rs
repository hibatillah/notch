mod advanced;
mod calendar;
mod general;
mod media;
mod notifications;
mod pages;
mod timer;

use gpui::{AnyElement, App, Context, Entity, Window, div, prelude::*};
use notch_store::Settings;

use crate::{
    SettingsWindow,
    field::{self, TextField},
    notify::tell_notch_to_reload,
};

pub const SECTION_NAMES: [&str; 8] =
    ["General", "Pages", "Behavior", "Notifications", "Calendar", "Media", "Timer", "Advanced"];

pub fn apply(settings: &mut Settings, key: &str, value: &str) {
    settings.set_string(key, value);
    tell_notch_to_reload();
}

/// The text and number fields live as long as the window so their drafts survive switching sections.
pub struct Fields {
    pub focus_minutes: Entity<TextField>,
    pub short_break_minutes: Entity<TextField>,
    pub long_break_minutes: Entity<TextField>,
    pub calendar_url: Entity<TextField>,
    pub calendar_refresh_minutes: Entity<TextField>,
}

impl Fields {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_minutes: field::minutes("timer.focus_minutes", 25, 5..=120, window, cx),
            short_break_minutes: field::minutes("timer.short_break_minutes", 5, 1..=120, window, cx),
            long_break_minutes: field::minutes("timer.long_break_minutes", 15, 1..=120, window, cx),
            calendar_url: field::text("calendar.ics_url", window, cx),
            calendar_refresh_minutes: field::minutes("calendar.refresh_minutes", 15, 1..=1440, window, cx),
        }
    }
}

pub fn render(section_index: usize, window_state: &SettingsWindow, cx: &mut Context<SettingsWindow>) -> AnyElement {
    match SECTION_NAMES[section_index] {
        "General" => general::render(window_state, cx),
        "Notifications" => notifications::render(window_state, cx),
        "Pages" => pages::render(window_state, cx),
        "Calendar" => calendar::render(window_state),
        "Media" => media::render(window_state, cx),
        "Timer" => timer::render(window_state),
        "Advanced" => advanced::render(),
        _ => div().into_any_element(),
    }
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
