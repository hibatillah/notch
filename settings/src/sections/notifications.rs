use gpui::{AnyElement, Context, div, prelude::*};

use crate::{SettingsWindow, field};

pub fn render(window_state: &SettingsWindow, cx: &mut Context<SettingsWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field::toggle("Show notifications", "activity.notifications", true, &window_state.settings, cx))
        .child(field::toggle("Show notification text", "notifications.show_text", true, &window_state.settings, cx))
        .into_any_element()
}
