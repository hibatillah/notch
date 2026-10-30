use gpui::{AnyElement, div, prelude::*};

use crate::{SettingsWindow, field};

pub fn render(window_state: &SettingsWindow) -> AnyElement {
    let fields = &window_state.fields;
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field::field_row("Calendar address (.ics)", &fields.calendar_url))
        .child(field::field_row("Refresh (minutes)", &fields.calendar_refresh_minutes))
        .into_any_element()
}
