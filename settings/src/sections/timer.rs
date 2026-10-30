use gpui::{AnyElement, div, prelude::*};

use crate::{SettingsWindow, field};

pub fn render(window_state: &SettingsWindow) -> AnyElement {
    let fields = &window_state.fields;
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field::field_row("Focus (minutes)", &fields.focus_minutes))
        .child(field::field_row("Short break (minutes)", &fields.short_break_minutes))
        .child(field::field_row("Long break (minutes)", &fields.long_break_minutes))
        .into_any_element()
}
