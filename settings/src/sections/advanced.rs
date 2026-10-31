use std::ops::RangeInclusive;

use crate::{
    field::{self, Kind},
    layout::{Group, Item, Page},
};

const TOP_RANGE: RangeInclusive<u32> = 0..=2000;

pub fn page() -> Page {
    Page::new(
        "Advanced",
        "Troubleshooting and placement. Changes apply after you restart the notch.",
        vec![Group::new(vec![
            Item::new(
                "Debug log file",
                field::input("advanced.debug_log", Kind::Text { width: 280.0 }, "Path to a log file"),
            )
            .description("Appends timestamped lines to this file. Empty means no log."),
            Item::new(
                "Move down (pixels)",
                field::input("notch.top", Kind::WholeNumber { default: 0, range: TOP_RANGE }, "0"),
            )
            .description("Distance below the top edge, 0 to 2000."),
        ])],
    )
}
