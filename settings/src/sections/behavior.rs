use gpui::{AnyElement, Context, SharedString, div, prelude::*, px, rgb, rgba};
use notch_store::Settings;

use crate::{SettingsWindow, sections::apply};

const ACCENT: u32 = 0x3b82f6;

pub struct Choice {
    pub word: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

// Mirrors `Showing` in `src/host/visibility.rs`: the same words, in the order the window offers them.
pub const CHOICES: [Choice; 4] = [
    Choice { word: "hidden", label: "Hidden", description: "Takes the notch off the screen, activities included." },
    Choice {
        word: "activities",
        label: "Activities only",
        description: "Shows volume, notification banners and badges, nothing else.",
    },
    Choice {
        word: "lyrics",
        label: "Music with lyrics",
        description: "Also shows the pill while music plays with lyrics.",
    },
    Choice {
        word: "music",
        label: "Music, even without lyrics",
        description: "Keeps the pill up always, the idle clock included.",
    },
];

pub const MAXIMIZED_KEY: &str = "notch.maximized";
pub const FULLSCREEN_KEY: &str = "notch.fullscreen";
const DEFAULT_MAXIMIZED: &str = "lyrics";
const DEFAULT_FULLSCREEN: &str = "hidden";

/// The stored word when the notch would accept it, else the documented default.
pub fn shown_word(settings: &Settings, key: &str, default: &'static str) -> &'static str {
    settings
        .get_string(key)
        .and_then(|stored| CHOICES.iter().find(|choice| choice.word == stored))
        .map_or(default, |choice| choice.word)
}

pub fn select(settings: &mut Settings, key: &str, word: &str) {
    apply(settings, key, word);
}

fn choice_group(
    title: &'static str,
    key: &'static str,
    default: &'static str,
    window_state: &SettingsWindow,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let selected_word = shown_word(&window_state.settings, key, default);
    let options = CHOICES.iter().map(|choice| {
        let is_selected = choice.word == selected_word;
        let word = choice.word;
        div()
            .id(SharedString::from(format!("{key}-{word}")))
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(if is_selected { rgb(ACCENT) } else { rgba(0x80808055) })
            .when(is_selected, |option| option.bg(rgba(0x3b82f633)))
            .cursor_pointer()
            .on_click(cx.listener(move |window_state, _event, _window, cx| {
                window_state.settings.reload();
                select(&mut window_state.settings, key, word);
                cx.notify();
            }))
            .child(div().child(choice.label))
            .child(div().text_sm().opacity(0.7).child(choice.description))
    });
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
        .children(options)
        .into_any_element()
}

pub fn render(window_state: &SettingsWindow, cx: &mut Context<SettingsWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .max_w(px(480.0))
        .child(choice_group("Over a maximized window", MAXIMIZED_KEY, DEFAULT_MAXIMIZED, window_state, cx))
        .child(choice_group("Over a full-screen app", FULLSCREEN_KEY, DEFAULT_FULLSCREEN, window_state, cx))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(name: &str, contents: &str) -> (Settings, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("notch-settings-behavior-{}-{name}.txt", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        (Settings::at(Some(path.clone())), path)
    }

    #[test]
    fn the_dropdown_offers_the_four_choices_in_order() {
        let offered: Vec<(&str, &str)> = CHOICES.iter().map(|choice| (choice.word, choice.label)).collect();
        assert_eq!(
            offered,
            [
                ("hidden", "Hidden"),
                ("activities", "Activities only"),
                ("lyrics", "Music with lyrics"),
                ("music", "Music, even without lyrics"),
            ]
        );
        assert!(CHOICES.iter().all(|choice| !choice.description.is_empty()));
    }

    #[test]
    fn a_missing_or_unknown_word_shows_the_default_for_each_key() {
        let (settings, path) = settings_with("unknown", "notch.maximized=banana\n");
        let _ = std::fs::remove_file(&path);
        assert_eq!(shown_word(&settings, MAXIMIZED_KEY, DEFAULT_MAXIMIZED), "lyrics");
        assert_eq!(shown_word(&settings, FULLSCREEN_KEY, DEFAULT_FULLSCREEN), "hidden");
    }

    #[test]
    fn a_stored_word_is_shown_as_stored() {
        let (settings, path) = settings_with("stored", "notch.maximized=music\nnotch.fullscreen=activities\n");
        let _ = std::fs::remove_file(&path);
        assert_eq!(shown_word(&settings, MAXIMIZED_KEY, DEFAULT_MAXIMIZED), "music");
        assert_eq!(shown_word(&settings, FULLSCREEN_KEY, DEFAULT_FULLSCREEN), "activities");
    }

    #[test]
    fn selecting_a_choice_writes_its_exact_word() {
        let (mut settings, path) = settings_with("select", "");
        select(&mut settings, FULLSCREEN_KEY, "music");
        select(&mut settings, MAXIMIZED_KEY, "activities");
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("notch.fullscreen=music") && text.contains("notch.maximized=activities"), "{text}");
    }
}
