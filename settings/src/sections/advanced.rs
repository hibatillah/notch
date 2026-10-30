use gpui::{AnyElement, div, prelude::*};

const VARIABLES: [(&str, &str); 4] = [
    ("NOTCH_PAGES", "comma-separated page ids to switch on"),
    ("NOTCH_LYRICS=off", "never look up lyrics"),
    ("NOTCH_DEBUG=<file>", "append a timestamped log to this file"),
    ("NOTCH_TOP=<pixels>", "move the notch down by this many pixels"),
];

pub fn render() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child("Set these environment variables before starting notch.exe; they are read at start.")
        .children(VARIABLES.iter().map(|(name, meaning)| div().child(format!("{name}: {meaning}"))))
        .into_any_element()
}
