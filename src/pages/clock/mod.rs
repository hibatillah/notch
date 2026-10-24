//! The clock page: the time and the date while collapsed, the time and the date in large type when open. It is the
//! fallback page, shown whenever nothing with a higher priority has anything to show.

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::{
    gfx::{Align, Canvas, Color, FontStyle, Rect, SEGOE_UI, Size, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, Priority, RESTING_HEIGHT, TimerId},
    ui::Icon,
};

const RESTING_WIDTH: f32 = 150.0;
const EXPANDED_SIZE: Size = Size::new(260.0, 52.0);
/// Width of the time in the open page; the date column starts after it.
const TIME_WIDTH: f32 = 150.0;
const DATE_COLUMN_GAP: f32 = 10.0;

const RESTING_TIME: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
const RESTING_DATE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
const LARGE_TIME: FontStyle = FontStyle::new(SEGOE_UI, 38.0, Weight::SemiBold, Align::Leading);
const WEEKDAY: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
const DATE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);

const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const DIVIDER: Color = Color::rgb(0.2, 0.2, 0.24);

/// Fires just after each minute starts, so the displayed time never lags.
const MINUTE_TIMER: TimerId = TimerId(0);

const WEEKDAY_NAMES: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_NAMES: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

pub struct ClockPage;

impl ClockPage {
    pub fn new() -> Self {
        Self
    }
}

fn arm_minute_timer(context: &mut Context) {
    let time = unsafe { GetLocalTime() };
    let milliseconds_into_minute = u32::from(time.wSecond) * 1000 + u32::from(time.wMilliseconds);
    context.set_timer(MINUTE_TIMER, 60_000 - milliseconds_into_minute + 50);
}

/// "Sat 24 Oct": the short date shown beside the time in the collapsed pill.
fn short_date(day_of_week: u16, day: u16, month: u16) -> String {
    let weekday = WEEKDAY_NAMES[usize::from(day_of_week) % 7];
    let month = MONTH_NAMES[usize::from(month.max(1) - 1) % 12];
    format!("{} {day} {month}", &weekday[..3])
}

impl Page for ClockPage {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn name(&self) -> &'static str {
        "Clock"
    }

    fn icon(&self) -> Icon {
        Icon::Clock
    }

    fn priority(&self, _mode: Mode) -> Option<Priority> {
        Some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let time = unsafe { GetLocalTime() };
        let (bounds, scale) = (frame.bounds, frame.scale);
        let text = format!("{:02}:{:02}", time.wHour, time.wMinute);
        let date = short_date(time.wDayOfWeek, time.wDay, time.wMonth);
        let time_width = canvas.measure(&RESTING_TIME, &text) * scale;
        let date_width = canvas.measure(&RESTING_DATE, &date) * scale;
        // The time, a thin divider and the date, centered as a group.
        let gap = 8.0 * scale;
        let group = time_width + gap + 1.0 * scale + gap + date_width;
        let left = bounds.center_x() - group / 2.0;
        let row = |from: f32, width: f32| {
            Rect::new(from, bounds.top, from + width + 2.0 * scale, bounds.top + RESTING_HEIGHT * scale)
        };
        canvas.text(&RESTING_TIME, &text, row(left, time_width), GRAY.faded(opacity));
        let divider_left = left + time_width + gap;
        let center_y = bounds.top + RESTING_HEIGHT * scale / 2.0;
        canvas.fill_rounded_rect(
            Rect::new(divider_left, center_y - 6.0 * scale, divider_left + 1.0 * scale, center_y + 6.0 * scale),
            0.0,
            DIVIDER.faded(opacity),
        );
        canvas.text(&RESTING_DATE, &date, row(divider_left + 1.0 * scale + gap, date_width), GRAY.faded(opacity));
    }

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let time = unsafe { GetLocalTime() };
        let (scale, bounds) = (frame.scale, frame.bounds);
        let time_row = Rect::new(bounds.left, bounds.top, bounds.left + TIME_WIDTH * scale, bounds.bottom);
        canvas.text(
            &LARGE_TIME,
            &format!("{:02}:{:02}", time.wHour, time.wMinute),
            time_row,
            Color::WHITE.faded(opacity),
        );

        let column_left = bounds.left + (TIME_WIDTH + DATE_COLUMN_GAP) * scale;
        let weekday = WEEKDAY_NAMES[usize::from(time.wDayOfWeek) % 7];
        let month = MONTH_NAMES[usize::from(time.wMonth.max(1) - 1) % 12];
        let center_y = bounds.center_y();
        let weekday_row = Rect::new(column_left, center_y - 20.0 * scale, bounds.right, center_y);
        let date_row = Rect::new(column_left, center_y, bounds.right, center_y + 20.0 * scale);
        canvas.text(&WEEKDAY, weekday, weekday_row, Color::WHITE.faded(opacity));
        canvas.text(&DATE, &format!("{} {month} {}", time.wDay, time.wYear), date_row, GRAY.faded(opacity));
    }

    fn on_start(&mut self, context: &mut Context) {
        arm_minute_timer(context);
    }

    fn on_timer(&mut self, _timer: TimerId, context: &mut Context) {
        context.request_repaint();
        arm_minute_timer(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_is_always_available_as_the_fallback() {
        let page = ClockPage::new();
        assert_eq!(page.priority(Mode::Resting), Some(Priority::BACKGROUND));
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::BACKGROUND));
    }

    #[test]
    fn the_clock_asks_for_a_small_collapsed_pill() {
        let sizes = ClockPage::new().sizes();
        assert!(sizes.resting_width < 200.0);
        assert!(sizes.expanded.height < 150.0);
    }

    #[test]
    fn the_short_date_names_the_weekday_day_and_month() {
        assert_eq!(short_date(6, 24, 10), "Sat 24 Oct");
        assert_eq!(short_date(0, 1, 1), "Sun 1 Jan");
    }
}
