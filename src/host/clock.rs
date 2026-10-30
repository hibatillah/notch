//! The clock belongs to the host and moves between the collapsed pill and the open header.

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::gfx::{Align, Canvas, Color, FontStyle, Point, Rect, UI_FONT, Weight, lerp};

/// The collapsed pill never gets narrower than this, so the time sits in the middle of a pill of a steady size.
pub const RESTING_WIDTH: f32 = crate::page::RESTING_MIN_WIDTH;

const TIME: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Medium, Align::Center);

fn time_text(hour: u16, minute: u16) -> String {
    format!("{hour:02}:{minute:02}")
}

pub const HALF_WIDTH: f32 = 50.0;

/// Keep the clock visible through its move; fade it only when page content occupies the collapsed pill.
pub fn opacity(is_shown_collapsed: bool, collapsed_visibility: f32, expanded_opacity: f32) -> f32 {
    let collapsed = if is_shown_collapsed { collapsed_visibility } else { 0.0 };
    collapsed.max(expanded_opacity)
}

pub fn center(collapsed: Point, open: Point, amount: f32) -> Point {
    Point::new(lerp(collapsed.x, open.x, amount), lerp(collapsed.y, open.y, amount))
}

pub fn text_width(canvas: &Canvas) -> f32 {
    let time = unsafe { GetLocalTime() };
    canvas.measure(&TIME, &time_text(time.wHour, time.wMinute))
}

pub fn draw_time(canvas: &Canvas, area: Rect, opacity: f32) {
    let time = unsafe { GetLocalTime() };
    canvas.text(&TIME, &time_text(time.wHour, time.wMinute), area, Color::WHITE.faded(opacity));
}

pub fn milliseconds_until_next_minute() -> u32 {
    let time = unsafe { GetLocalTime() };
    until_next_minute(time.wSecond, time.wMilliseconds)
}

fn until_next_minute(second: u16, millisecond: u16) -> u32 {
    60_000 - (u32::from(second) * 1000 + u32::from(millisecond)) + 50
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_is_written_with_two_digits_for_the_hour_and_the_minute() {
        assert_eq!(time_text(9, 5), "09:05");
        assert_eq!(time_text(23, 59), "23:59");
    }

    #[test]
    fn the_time_stays_visible_all_the_way_from_the_collapsed_pill_to_the_header() {
        for expanded_opacity in [0.0, 0.3, 1.0] {
            assert_eq!(opacity(true, 1.0, expanded_opacity), 1.0);
        }
    }

    #[test]
    fn the_time_waits_for_the_open_notch_when_a_page_fills_the_collapsed_pill() {
        assert_eq!(opacity(false, 1.0, 0.0), 0.0);
        assert_eq!(opacity(false, 1.0, 0.6), 0.6);
    }

    #[test]
    fn the_time_moves_in_a_straight_line_between_its_two_places() {
        let (collapsed, open) = (Point::new(100.0, 14.0), Point::new(140.0, 18.0));
        assert_eq!(center(collapsed, open, 0.0), collapsed);
        assert_eq!(center(collapsed, open, 1.0), open);
        assert_eq!(center(collapsed, open, 0.5), Point::new(120.0, 16.0));
    }

    #[test]
    fn the_timer_fires_just_after_the_next_minute_starts() {
        assert_eq!(until_next_minute(0, 0), 60_050);
        assert_eq!(until_next_minute(59, 999), 51);
    }
}
