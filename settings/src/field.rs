use std::ops::RangeInclusive;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, KeyDownEvent, MouseButton, Rgba, Subscription, Window, div,
    prelude::*, px, rgb, rgba,
};
use notch_store::Settings;

use crate::{SettingsWindow, sections::apply};

const ACCENT: u32 = 0x3b82f6;
const SWITCH_OFF: u32 = 0x777777;

pub fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

pub fn set_toggle(settings: &mut Settings, key: &str, enabled: bool) {
    apply(settings, key, on_off(enabled));
}

pub fn parse_minutes(input: &str, range: &RangeInclusive<u32>) -> Option<u32> {
    input.trim().parse::<u32>().ok().filter(|minutes| range.contains(minutes))
}

#[derive(Debug, PartialEq)]
pub enum Press {
    Typing,
    Commit,
    Revert,
}

/// The text being edited next to the last value that was saved; only Enter or blur may save it.
pub struct Draft {
    committed: String,
    text: String,
}

impl Draft {
    pub fn new(value: &str) -> Self {
        Self { committed: value.to_owned(), text: value.to_owned() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn reset_to(&mut self, value: &str) {
        self.committed = value.to_owned();
        self.text = value.to_owned();
    }

    pub fn insert(&mut self, typed: &str) {
        self.text.extend(typed.chars().filter(|character| !character.is_control()));
    }

    pub fn press(&mut self, key: &str, key_char: Option<&str>) -> Press {
        match key {
            "enter" => Press::Commit,
            "escape" => {
                self.text = self.committed.clone();
                Press::Revert
            }
            "backspace" => {
                self.text.pop();
                Press::Typing
            }
            _ => {
                if let Some(typed) = key_char {
                    self.insert(typed);
                }
                Press::Typing
            }
        }
    }

    /// Returns the value to save; a rejected or unchanged draft saves nothing and a rejected one reverts.
    pub fn finish(&mut self, accept: impl FnOnce(&str) -> Option<String>) -> Option<String> {
        match accept(&self.text) {
            Some(value) => {
                self.text = value.clone();
                if value == self.committed {
                    return None;
                }
                self.committed = value.clone();
                Some(value)
            }
            None => {
                self.text = self.committed.clone();
                None
            }
        }
    }
}

enum Kind {
    Text,
    Minutes { default: u32, range: RangeInclusive<u32> },
}

/// A single-line field that writes its key on Enter or when it loses focus.
pub struct TextField {
    key: &'static str,
    kind: Kind,
    draft: Draft,
    // Its own handle on the file: every write re-reads it, so the window's other copy loses nothing.
    settings: Settings,
    focus_handle: FocusHandle,
    _blur_subscription: Subscription,
}

pub fn text(key: &'static str, window: &mut Window, cx: &mut App) -> Entity<TextField> {
    cx.new(|cx| TextField::new(key, Kind::Text, window, cx))
}

pub fn minutes(
    key: &'static str,
    default: u32,
    range: RangeInclusive<u32>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TextField> {
    cx.new(|cx| TextField::new(key, Kind::Minutes { default, range }, window, cx))
}

impl TextField {
    fn new(key: &'static str, kind: Kind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let blur_subscription = cx.on_blur(&focus_handle, window, |field, _window, cx| field.commit(cx));
        let settings = Settings::load();
        let draft = Draft::new(&shown_value(&settings, key, &kind));
        Self { key, kind, draft, settings, focus_handle, _blur_subscription: blur_subscription }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let accepted = self.draft.finish(|text| match &self.kind {
            Kind::Text => Some(text.trim().to_owned()),
            Kind::Minutes { range, .. } => parse_minutes(text, range).map(|minutes| minutes.to_string()),
        });
        if let Some(value) = accepted {
            apply(&mut self.settings, self.key, &value);
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control && keystroke.key == "v" {
            if let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.draft.insert(&pasted);
            }
            cx.notify();
            return;
        }
        if keystroke.modifiers.control || keystroke.modifiers.alt || keystroke.modifiers.platform {
            return;
        }
        match self.draft.press(&keystroke.key, keystroke.key_char.as_deref()) {
            Press::Commit => self.commit(cx),
            Press::Typing | Press::Revert => cx.notify(),
        }
    }
}

fn shown_value(settings: &Settings, key: &str, kind: &Kind) -> String {
    match kind {
        Kind::Text => settings.get_string(key).unwrap_or_default().to_owned(),
        Kind::Minutes { default, range } => {
            settings.get_string(key).and_then(|text| parse_minutes(text, range)).unwrap_or(*default).to_string()
        }
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        if !focused {
            // A hand edit or the notch's own write shows up here instead of being overwritten by a stale value.
            self.settings.reload();
            self.draft.reset_to(&shown_value(&self.settings, self.key, &self.kind));
        }
        let width = match self.kind {
            Kind::Text => 320.0,
            Kind::Minutes { .. } => 72.0,
        };
        let caret = if focused { "|" } else { "" };
        div()
            .track_focus(&self.focus_handle)
            .w(px(width))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(if focused { rgb(ACCENT) } else { rgba(0x80808080) })
            .overflow_hidden()
            .whitespace_nowrap()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|field, _event, window, _cx| field.focus_handle.focus(window)),
            )
            .on_key_down(cx.listener(|field, event: &KeyDownEvent, _window, cx| field.on_key_down(event, cx)))
            .child(format!("{}{caret}", self.draft.text()))
    }
}

fn row(label: &'static str, control: impl IntoElement) -> AnyElement {
    div().flex().items_center().justify_between().gap_4().child(div().child(label)).child(control).into_any_element()
}

pub fn field_row(label: &'static str, field: &Entity<TextField>) -> AnyElement {
    row(label, field.clone())
}

pub fn switch_row(
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&mut SettingsWindow) + 'static,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let track_color: Rgba = rgb(if enabled { ACCENT } else { SWITCH_OFF });
    let switch = div()
        .id(label)
        .flex()
        .items_center()
        .w(px(40.0))
        .h(px(22.0))
        .p(px(2.0))
        .rounded_full()
        .bg(track_color)
        .when(enabled, |track| track.justify_end())
        .child(div().size(px(18.0)).rounded_full().bg(rgb(0xffffff)))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            on_click(this);
            cx.notify();
        }));
    row(label, switch)
}

pub fn toggle(
    label: &'static str,
    key: &'static str,
    default: bool,
    settings: &Settings,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    switch_row(
        label,
        settings.get_bool(key, default),
        move |window_state| {
            window_state.settings.reload();
            let enabled = window_state.settings.get_bool(key, default);
            set_toggle(&mut window_state.settings, key, !enabled);
        },
        cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_settings(name: &str) -> (Settings, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("notch-settings-field-{}-{name}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (Settings::at(Some(path.clone())), path)
    }

    #[test]
    fn a_toggle_writes_on_and_off() {
        let (mut settings, path) = temporary_settings("toggle");
        set_toggle(&mut settings, "media.lyrics", true);
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "media.lyrics=on");
        set_toggle(&mut settings, "media.lyrics", false);
        assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "media.lyrics=off");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn minutes_outside_the_range_are_rejected() {
        let range = 5..=120;
        assert_eq!(parse_minutes("4", &range), None);
        assert_eq!(parse_minutes("121", &range), None);
        assert_eq!(parse_minutes("abc", &range), None);
        assert_eq!(parse_minutes("", &range), None);
        assert_eq!(parse_minutes(" 5 ", &range), Some(5));
        assert_eq!(parse_minutes("120", &range), Some(120));

        let mut draft = Draft::new("25");
        draft.press("backspace", None);
        draft.press("backspace", None);
        draft.insert("3");
        let saved = draft.finish(|text| parse_minutes(text, &range).map(|minutes| minutes.to_string()));
        assert_eq!(saved, None);
        assert_eq!(draft.text(), "25", "a rejected value reverts the field");
    }

    #[test]
    fn text_commits_only_on_enter_or_blur() {
        let accept = |text: &str| Some(text.trim().to_owned());
        let mut draft = Draft::new("");
        for typed in ["h", "i"] {
            assert_eq!(draft.press(typed, Some(typed)), Press::Typing);
        }
        assert_eq!(draft.text(), "hi");
        assert_eq!(draft.press("enter", None), Press::Commit);
        assert_eq!(draft.finish(accept), Some("hi".to_owned()));
        assert_eq!(draft.finish(accept), None, "blur right after Enter has nothing new to save");

        draft.insert("!");
        assert_eq!(draft.finish(accept), Some("hi!".to_owned()), "blur commits a pending edit");

        draft.insert("?");
        assert_eq!(draft.press("escape", None), Press::Revert);
        assert_eq!(draft.text(), "hi!");
    }
}
