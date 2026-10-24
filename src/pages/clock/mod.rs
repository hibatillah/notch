//! The clock page: the time while collapsed, the time and date when open. It is the fallback page, shown whenever
//! nothing with a higher priority has anything to show.

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::{
    gfx::{Align, Canvas, Color, FontStyle, Rect, SEGOE_UI, Size, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, Priority, RESTING_HEIGHT, TimerId},
};

const RESTING_WIDTH: f32 = 104.0;
const EXPANDED_SIZE: Size = Size::new(420.0, 112.0);
const CONTENT_PADDING: f32 = 24.0;

const RESTING_TIME: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Center);
const LARGE_TIME: FontStyle = FontStyle::new(SEGOE_UI, 38.0, Weight::SemiBold, Align::Leading);
const DATE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::Regular, Align::Leading);

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

impl Page for ClockPage {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn priority(&self, _mode: Mode) -> Option<Priority> {
        Some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let time = unsafe { GetLocalTime() };
        let bounds = frame.bounds;
        let margin = 12.0 * frame.scale;
        let row = Rect::new(
            bounds.left + margin,
            bounds.top,
            bounds.right - margin,
            bounds.top + RESTING_HEIGHT * frame.scale,
        );
        canvas.text(&RESTING_TIME, &format!("{:02}:{:02}", time.wHour, time.wMinute), row, Color::WHITE.faded(opacity));
    }

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let time = unsafe { GetLocalTime() };
        let (scale, bounds) = (frame.scale, frame.bounds);
        let left = bounds.left + CONTENT_PADDING * scale;
        let right = bounds.right - CONTENT_PADDING * scale;
        let time_row = Rect::new(left, bounds.top + 14.0 * scale, right, bounds.top + 72.0 * scale);
        canvas.text(
            &LARGE_TIME,
            &format!("{:02}:{:02}", time.wHour, time.wMinute),
            time_row,
            Color::WHITE.faded(opacity),
        );

        let weekday = WEEKDAY_NAMES[usize::from(time.wDayOfWeek) % 7];
        let month = MONTH_NAMES[usize::from(time.wMonth.max(1) - 1) % 12];
        let date_row = Rect::new(left, bounds.top + 76.0 * scale, right, bounds.top + 100.0 * scale);
        canvas.text(&DATE, &format!("{weekday}, {} {month}", time.wDay), date_row, Color::gray(0.65).faded(opacity));
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
        assert!(sizes.resting_width < 150.0);
        assert!(sizes.expanded.height < 150.0);
    }
}
