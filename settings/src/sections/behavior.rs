use gpui::{SharedString, div, prelude::*};
use gpui_component::ActiveTheme as _;
use notch_store::Settings;

use crate::{
    field, layout,
    layout::{Group, Item, Page},
};

pub struct Choice {
    pub word: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

// The words `Showing::parse` accepts in `src/host/visibility.rs`; a test compares them, the labels are this window's own.
pub const CHOICES: [Choice; 4] = [
    Choice {
        word: "hidden",
        label: "Hidden",
        description: "The notch hides, even for notification banners and volume changes.",
    },
    Choice {
        word: "activities",
        label: "Activities only",
        description: "The notch shows only notification banners, volume changes and badges.",
    },
    Choice {
        word: "lyrics",
        label: "Lyrics only",
        description: "The notch shows only while a lyric line is active, and for activities. Otherwise it hides.",
    },
    Choice {
        word: "music",
        label: "Always visible",
        description: "The notch always shows, with the idle clock when nothing is playing.",
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

fn choice_item(title: &'static str, description: &'static str, key: &'static str, default: &'static str) -> Item {
    let choices =
        CHOICES.iter().map(|choice| (SharedString::from(choice.word), SharedString::from(choice.label))).collect();
    let control = field::dropdown(key, choices, move |settings| SharedString::from(shown_word(settings, key, default)));
    Item::new(title, control).description(description)
}

fn meanings() -> Item {
    Item::custom(|window, cx| {
        let value_color = cx.theme().foreground;
        let entries = CHOICES
            .iter()
            .map(|choice| {
                let description = div().text_sm().text_color(value_color).child(choice.description).into_any_element();
                (SharedString::from(choice.label), description)
            })
            .collect();
        layout::accordion("behavior-choices", true, entries, window, cx)
    })
}

pub fn page() -> Page {
    Page::new(
        "Behavior",
        "Choose what the notch shows while another window is in front.",
        vec![
            Group::titled(
                "Over other windows",
                vec![
                    choice_item(
                        "Over a maximized window",
                        "Applies while a maximized window is in front.",
                        MAXIMIZED_KEY,
                        DEFAULT_MAXIMIZED,
                    ),
                    choice_item(
                        "Over a full-screen app",
                        "Applies while a full-screen app is in front.",
                        FULLSCREEN_KEY,
                        DEFAULT_FULLSCREEN,
                    ),
                ],
            ),
            Group::loose_titled("What each choice does", vec![meanings()]),
        ],
    )
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
    fn the_choices_come_in_order() {
        let offered: Vec<(&str, &str)> = CHOICES.iter().map(|choice| (choice.word, choice.label)).collect();
        assert_eq!(
            offered,
            [
                ("hidden", "Hidden"),
                ("activities", "Activities only"),
                ("lyrics", "Lyrics only"),
                ("music", "Always visible"),
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
    fn every_choice_word_is_accepted_by_the_hosts_parser() {
        let host = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/host/visibility.rs"))
            .expect("host visibility source");
        for choice in &CHOICES {
            assert!(
                host.contains(&format!("\"{}\" => Some(Showing::", choice.word)),
                "{} missing in host",
                choice.word
            );
        }
    }
}
