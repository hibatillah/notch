use gpui::{AnyElement, Context, div, prelude::*};

use crate::{SettingsWindow, field, startup};

pub fn render(window_state: &SettingsWindow, cx: &mut Context<SettingsWindow>) -> AnyElement {
    let start_with_windows = field::switch_row(
        "Start with Windows",
        startup::is_enabled(),
        |_window_state| startup::set_enabled(!startup::is_enabled()),
        cx,
    );
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field::toggle("Hide when idle", "notch.hide_when_idle", false, &window_state.settings, cx))
        .child(start_with_windows)
        .into_any_element()
}
