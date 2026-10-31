use std::ops::RangeInclusive;

use crate::{
    field::{self, Kind},
    layout::{Group, Item, Page},
    startup,
};

fn minutes(
    title: &'static str,
    key: &'static str,
    default: u32,
    range: RangeInclusive<u32>,
    placeholder: &'static str,
) -> Item {
    Item::new(title, field::input(key, Kind::WholeNumber { default, range }, placeholder))
}

pub fn page() -> Page {
    let start_with_windows = field::switch("start-with-windows", startup::is_enabled, |enabled, cx| {
        startup::set_enabled(enabled);
        cx.refresh_windows();
    });
    Page::new(
        "General",
        "Startup, notifications, music and the focus timer.",
        vec![
            Group::titled(
                "Startup",
                vec![
                    Item::new("Start with Windows", start_with_windows)
                        .description("Starts the notch when you sign in."),
                    Item::new("Hide when idle", field::toggle("notch.hide_when_idle", false)).description(
                        "Hides the notch when nothing is happening. Point at the top edge to bring it back.",
                    ),
                ],
            ),
            Group::titled(
                "Notifications",
                vec![
                    Item::new("Show notifications", field::toggle("activity.notifications", true))
                        .description("Shows a banner when Windows shows a notification."),
                    Item::new("Show notification text", field::toggle("notifications.show_text", true))
                        .description("Shows the message in the banner. Without it, the banner reads \"New message\"."),
                ],
            ),
            Group::titled(
                "Music",
                vec![
                    Item::new("Show lyrics", field::toggle("media.lyrics", true))
                        .description("Shows synced lyrics on the media page."),
                ],
            ),
            Group::titled(
                "Timer",
                vec![
                    minutes("Focus (minutes)", "timer.focus_minutes", 25, 5..=120, "25")
                        .description("Length of a focus session, 5 to 120."),
                    minutes("Short break (minutes)", "timer.short_break_minutes", 5, 1..=120, "5")
                        .description("Rest after a focus session, 1 to 120."),
                    minutes("Long break (minutes)", "timer.long_break_minutes", 15, 1..=120, "15")
                        .description("Longer rest after several sessions, 1 to 120."),
                ],
            ),
        ],
    )
}
