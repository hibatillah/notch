use gpui::{AnyElement, App, SharedString, Window, div, prelude::*};
use gpui_component::{ActiveTheme as _, v_flex};

use crate::{
    field::{self, Kind},
    layout,
    layout::{Group, Item, Page},
};

// Wording follows Google's "Sync your calendar with computer programs" page (support.google.com/calendar/answer/37648).
const GOOGLE_STEPS: [&str; 5] = [
    "Open calendar.google.com on a computer.",
    "Click the gear icon at the top right, then Settings.",
    "Under \"Settings for my calendars\" on the left, click the calendar to show.",
    "Click Integrate calendar.",
    "Copy the link under \"Secret address in iCal format\" and paste it into the Calendar address field.",
];

const GOOGLE_NOTES: [&str; 2] = [
    "The secret address lets anyone who has it read the whole calendar. Keep it to yourself; Reset in the same section replaces it.",
    "A work or school account may hide the address when the admin limits sharing. Ask the admin in that case.",
];

pub fn page() -> Page {
    Page::new(
        "Calendar",
        "Connect the calendar that the calendar page shows.",
        vec![
            Group::titled(
                "Google Calendar",
                vec![
                    Item::new(
                        "Calendar address (.ics)",
                        field::input(
                            "calendar.ics_url",
                            Kind::Secret { width: 280.0 },
                            "Paste the secret iCal address",
                        ),
                    )
                    .description("The private iCal link of your calendar."),
                    Item::new(
                        "Refresh (minutes)",
                        field::input(
                            "calendar.refresh_minutes",
                            Kind::WholeNumber { default: 15, range: 1..=1440 },
                            "15",
                        ),
                    )
                    .description("How often the notch downloads it again, 1 to 1440."),
                ],
            ),
            Group::loose(vec![Item::custom(guide)]),
        ],
    )
}

fn guide(window: &mut Window, cx: &mut App) -> AnyElement {
    let steps = GOOGLE_STEPS.iter().enumerate().map(|(index, step)| div().child(format!("{}. {step}", index + 1)));
    let notes = GOOGLE_NOTES.iter().map(|note| div().text_color(cx.theme().muted_foreground).child(*note));
    let body = v_flex().gap_1().text_sm().children(steps).children(notes).into_any_element();
    layout::accordion(
        "google-steps",
        false,
        vec![(SharedString::from("How to get the address from Google Calendar"), body)],
        window,
        cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_name_the_labels_google_shows() {
        let steps = GOOGLE_STEPS.join(" ");
        for label in ["Settings", "Integrate calendar", "Secret address in iCal format"] {
            assert!(steps.contains(label), "missing {label}");
        }
    }
}
