//! The calendar page: a week strip and today's events, read from a calendar file at an address the user gives (for
//! Google Calendar, the "secret address in iCal format" of a calendar, which needs no sign-in). Open, it lists the
//! day's events and offers Join for a video meeting; collapsed, a reminder appears 15 minutes before an event.
//!
//! The address is set from the tray menu ("Use calendar link from clipboard"), so the notch needs no settings window.
//! It is kept in the settings file, and the downloaded calendar is kept in `calendar.ics` beside it.

mod agenda;
mod dates;
mod fetch;
mod ics;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use windows::Win32::{
    Foundation::FILETIME,
    System::{
        SystemInformation::GetLocalTime,
        Time::{FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime},
    },
};

use self::{
    agenda::{Agenda, Reminder, Row, State, build, reminder},
    dates::{
        MONTH_NAMES, WEEKDAY_LETTERS, WEEKDAY_NAMES, civil_from_days, day_of, seconds_from_civil, week_start, weekday,
    },
    ics::{Clock, Event, Occurrence, occurrences_on, parse},
};
use crate::{
    activity::{FLASH_TIMER, Flash, Presence},
    gfx::{Align, Canvas, Color, FontStyle, Point, Rect, SEGOE_UI, Size, Weight},
    page::{Context, Frame, Mode, Notifier, Page, PageSizes, PointerEvent, Priority, TimerId},
    platform,
    ui::{self, Icon},
};

const EXPANDED_SIZE: Size = Size::new(508.0, 132.0);
const RESTING_WIDTH: f32 = 150.0;
const LEFT_WIDTH: f32 = 200.0;
const COLUMN_GAP: f32 = 16.0;
const DAY_WIDTH: f32 = 26.0;
const ROW_HEIGHT: f32 = 40.0;
const ROW_GAP: f32 = 6.0;
const ROW_RADIUS: f32 = 10.0;
const ROW_PADDING: f32 = 10.0;
const TIME_WIDTH: f32 = 44.0;
const JOIN_WIDTH: f32 = 52.0;
const JOIN_HEIGHT: f32 = 22.0;
const ALERT_MILLISECONDS: u32 = 2500;
const DEFAULT_REFRESH_MINUTES: u64 = 15;
const FIRST_FETCH_MILLISECONDS: u32 = 2000;
/// The longest the reminder shows an event's title, in device-independent pixels.
const REMINDER_TITLE_WIDTH: f32 = 96.0;

const LINK_SETTING: &str = "calendar.ics_url";
const REFRESH_SETTING: &str = "calendar.refresh_minutes";

/// Wakes once a minute, like the clock, so "in 12 min" and the reminder stay right.
const MINUTE_TIMER: TimerId = TimerId(1);
const FETCH_TIMER: TimerId = TimerId(2);

const MONTH: FontStyle = FontStyle::new(SEGOE_UI, 26.0, Weight::SemiBold, Align::Leading);
const WEEKDAY: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
const DAY_LETTER: FontStyle = FontStyle::new(SEGOE_UI, 9.0, Weight::SemiBold, Align::Center);
const DAY_NUMBER: FontStyle = FontStyle::new("Consolas", 13.0, Weight::Regular, Align::Center);
const SUMMARY: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::Regular, Align::Leading);
const EVENT_TIME: FontStyle = FontStyle::new("Consolas", 12.0, Weight::Regular, Align::Leading);
const EVENT_TITLE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
const EVENT_DETAIL: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::Regular, Align::Leading);
const EVENT_TRAILING: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::Regular, Align::Trailing);
const JOIN: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::SemiBold, Align::Center);
const EMPTY: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Center).wrapped();
const BADGE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::SemiBold, Align::Leading);
const BADGE_TIME: FontStyle = FontStyle::new("Consolas", 12.0, Weight::SemiBold, Align::Leading);

const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const DIM: Color = Color::rgb(0.431, 0.431, 0.478);
const LIGHT: Color = Color::rgb(0.769, 0.769, 0.8);
const BLUE: Color = Color::rgb(0.302, 0.671, 0.969);
const PALE_BLUE: Color = Color::rgb(0.455, 0.753, 0.988);
const TODAY_FILL: Color = Color::rgb(0.110, 0.494, 0.839);
const SUNDAY_RED: Color = Color::rgb(1.0, 0.42, 0.42);
const ROW: Color = Color::rgb(0.078, 0.078, 0.094);
const ROW_NEXT: Color = Color::rgb(0.063, 0.141, 0.227);
const DIVIDER: Color = Color::rgb(0.149, 0.149, 0.176);

/// Turns UTC into local time through Windows, which knows the time zone and its daylight saving rules.
struct SystemClock;

const UNIX_TO_FILE_TIME_SECONDS: i64 = 11_644_473_600;
const FILE_TIME_TICKS_PER_SECOND: i64 = 10_000_000;

impl Clock for SystemClock {
    fn utc_to_local(&self, utc: i64) -> i64 {
        let ticks = (utc + UNIX_TO_FILE_TIME_SECONDS) * FILE_TIME_TICKS_PER_SECOND;
        let file_time = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
        let (mut universal, mut local, mut local_file_time) =
            (Default::default(), Default::default(), FILETIME::default());
        let converted = unsafe {
            FileTimeToSystemTime(&file_time, &mut universal).is_ok()
                && SystemTimeToTzSpecificLocalTime(None, &universal, &mut local).is_ok()
                && SystemTimeToFileTime(&local, &mut local_file_time).is_ok()
        };
        if !converted {
            return utc;
        }
        let local_ticks = (i64::from(local_file_time.dwHighDateTime) << 32) | i64::from(local_file_time.dwLowDateTime);
        local_ticks / FILE_TIME_TICKS_PER_SECOND - UNIX_TO_FILE_TIME_SECONDS
    }
}

/// The local date and time now, as seconds since 1970-01-01 on the wall clock.
fn local_now() -> i64 {
    let time = unsafe { GetLocalTime() };
    seconds_from_civil(
        i64::from(time.wYear),
        u32::from(time.wMonth),
        u32::from(time.wDay),
        u32::from(time.wHour),
        u32::from(time.wMinute),
        u32::from(time.wSecond),
    )
}

pub struct CalendarPage {
    events: Vec<Event>,
    link: Option<String>,
    storage: Option<PathBuf>,
    fetched: fetch::Outcome,
    notifier: Option<Notifier>,
    /// The local day shown, and what happens on it.
    today: i64,
    occurrences: Vec<Occurrence>,
    agenda: Agenda,
    reminder: Option<Reminder>,
    reminder_width: f32,
    alert: Flash,
    alert_text: String,
    alert_width: f32,
    hovered_join: Option<usize>,
}

impl CalendarPage {
    pub fn new() -> Self {
        let storage =
            std::env::var_os("APPDATA").map(|directory| PathBuf::from(directory).join("notch").join("calendar.ics"));
        Self {
            events: Vec::new(),
            link: None,
            storage,
            fetched: Arc::new(Mutex::new(None)),
            notifier: None,
            today: 0,
            occurrences: Vec::new(),
            agenda: build(&[], 0),
            reminder: None,
            reminder_width: 0.0,
            alert: Flash::default(),
            alert_text: String::new(),
            alert_width: 0.0,
            hovered_join: None,
        }
    }

    /// Works out what the page shows from the events and the time now.
    fn refresh(&mut self, context: &Context) {
        let now = local_now();
        self.today = day_of(now);
        self.occurrences = occurrences_on(&self.events, self.today, &SystemClock);
        self.agenda = build(&self.occurrences, now);
        self.reminder = reminder(&self.occurrences, now);
        if let Some(reminder) = &self.reminder {
            let fonts = context.fonts();
            let title = fonts.measure(&BADGE, &reminder.title).min(REMINDER_TITLE_WIDTH).ceil();
            let time = fonts.measure(&BADGE_TIME, &widest_form(&reminder.text)).ceil();
            self.reminder_width = 14.0 + 6.0 + title + 8.0 + time + 6.0;
        }
    }

    fn arm_minute_timer(context: &mut Context) {
        let time = unsafe { GetLocalTime() };
        let milliseconds_into_minute = u32::from(time.wSecond) * 1000 + u32::from(time.wMilliseconds);
        context.set_timer(MINUTE_TIMER, 60_000 - milliseconds_into_minute + 50);
    }

    fn refresh_interval_milliseconds(context: &Context) -> u32 {
        let minutes = context
            .settings
            .get_string(REFRESH_SETTING)
            .and_then(|text| text.parse::<u64>().ok())
            .filter(|minutes| *minutes > 0)
            .unwrap_or(DEFAULT_REFRESH_MINUTES);
        (minutes * 60_000).min(u64::from(u32::MAX)) as u32
    }

    fn start_fetch(&self) {
        if let (Some(link), Some(notifier)) = (&self.link, self.notifier) {
            fetch::start(link.clone(), self.fetched.clone(), notifier);
        }
    }

    fn load_events(&mut self, text: &str) {
        self.events = parse(text);
    }

    fn announce(&mut self, text: &str, context: &mut Context) {
        self.alert_width = ui::message_width(context.fonts(), text, None);
        self.alert_text = text.to_owned();
        self.alert.show(context, ALERT_MILLISECONDS);
    }

    fn use_clipboard_link(&mut self, context: &mut Context) {
        let Some(link) = platform::paste_text().and_then(|text| fetch::normalize_link(&text)) else {
            self.announce("No calendar link copied", context);
            return;
        };
        context.settings.set_string(LINK_SETTING, &link);
        self.link = Some(link);
        self.announce("Calendar link saved", context);
        self.start_fetch();
    }

    fn remove_link(&mut self, context: &mut Context) {
        context.settings.set_string(LINK_SETTING, "");
        self.link = None;
        self.events.clear();
        if let Some(storage) = &self.storage {
            let _ = std::fs::remove_file(storage);
        }
        self.announce("Calendar link removed", context);
    }

    /// The join button's place in the row at `position`, if that row has a meeting and is the next one.
    fn join_button(&self, position: usize, bounds: Rect, scale: f32) -> Option<Rect> {
        let row = self.agenda.rows.get(position)?;
        if row.state != State::Next || row.meeting.is_none() {
            return None;
        }
        let area = row_rect(position, bounds, scale);
        Some(Rect::new(
            area.right - (ROW_PADDING + JOIN_WIDTH) * scale,
            area.center_y() - JOIN_HEIGHT * scale / 2.0,
            area.right - ROW_PADDING * scale,
            area.center_y() + JOIN_HEIGHT * scale / 2.0,
        ))
    }

    fn draw_week(&self, canvas: &Canvas, bounds: Rect, opacity: f32) {
        let scale = canvas.scale();
        let (year, month, day) = civil_from_days(self.today);
        let _ = (year, day);
        let column = Rect::new(bounds.left, bounds.top, bounds.left + LEFT_WIDTH * scale, bounds.bottom);

        let summary = Rect::new(column.left, column.bottom - 16.0 * scale, column.right, column.bottom - 2.0 * scale);
        canvas.text(&SUMMARY, &self.summary_text(), summary, GRAY.faded(opacity));

        let strip_top = summary.top - 8.0 * scale - 32.0 * scale;
        let first_day = week_start(self.today);
        let spacing = (LEFT_WIDTH - 7.0 * DAY_WIDTH) / 6.0;
        for position in 0..7 {
            let date = first_day + position;
            let left = column.left + position as f32 * (DAY_WIDTH + spacing) * scale;
            let cell = Rect::new(left, strip_top, left + DAY_WIDTH * scale, strip_top + 32.0 * scale);
            let is_today = date == self.today;
            let letter_color = if is_today { BLUE } else { DIM };
            canvas.text(
                &DAY_LETTER,
                WEEKDAY_LETTERS[position as usize],
                Rect { bottom: cell.top + 12.0 * scale, ..cell },
                letter_color.faded(opacity),
            );
            let number = Rect { top: cell.top + 13.0 * scale, ..cell };
            let (_, _, date_of_month) = civil_from_days(date);
            if is_today {
                let pill = Rect::new(
                    number.center_x() - 12.0 * scale,
                    number.top + 1.0 * scale,
                    number.center_x() + 12.0 * scale,
                    number.bottom - 1.0 * scale,
                );
                canvas.fill_rounded_rect(pill, 6.0 * scale, TODAY_FILL.faded(opacity));
                canvas.text(&DAY_NUMBER, &date_of_month.to_string(), number, Color::WHITE.faded(opacity));
            } else {
                let color = if position == 6 { SUNDAY_RED } else { LIGHT };
                canvas.text(&DAY_NUMBER, &date_of_month.to_string(), number, color.faded(opacity));
            }
        }

        let header_bottom = strip_top - 8.0 * scale;
        let month_name = MONTH_NAMES[(month as usize - 1) % 12];
        let header = Rect::new(column.left, header_bottom - 34.0 * scale, column.right, header_bottom);
        canvas.text(&MONTH, month_name, header, Color::WHITE.faded(opacity));
        let month_width = canvas.measure(&MONTH, month_name) * scale;
        let weekday_area =
            Rect::new(header.left + month_width + 8.0 * scale, header.top, header.right, header.bottom - 4.0 * scale);
        canvas.text(
            &WEEKDAY,
            WEEKDAY_NAMES[weekday(self.today) as usize],
            Rect { top: weekday_area.bottom - 18.0 * scale, ..weekday_area },
            GRAY.faded(opacity),
        );
    }

    fn summary_text(&self) -> String {
        if self.link.is_none() { "Not connected".to_owned() } else { self.agenda.summary.clone() }
    }

    fn draw_row(&self, canvas: &Canvas, position: usize, row: &Row, bounds: Rect, hovered_join: bool, opacity: f32) {
        let scale = canvas.scale();
        let area = row_rect(position, bounds, scale);
        let dimmed = if row.state == State::Done { 0.5 } else { 1.0 };
        let fade = opacity * dimmed;
        let fill = if row.state == State::Next { ROW_NEXT } else { ROW };
        canvas.fill_rounded_rect(area, ROW_RADIUS * scale, fill.faded(fade));

        let time_color = if row.state == State::Next { PALE_BLUE } else { GRAY };
        let time_area = Rect::new(
            area.left + ROW_PADDING * scale,
            area.top,
            area.left + (ROW_PADDING + TIME_WIDTH) * scale,
            area.bottom,
        );
        canvas.text(&EVENT_TIME, &row.time, time_area, time_color.faded(fade));

        let text_left = time_area.right;
        let join = self.join_button(position, bounds, scale);
        let text_right = join.map_or(area.right - ROW_PADDING * scale, |button| button.left - 8.0 * scale);
        let trailing_width = if join.is_none() { 54.0 * scale } else { 0.0 };
        let title_right = text_right - trailing_width;
        let title_color = Color::WHITE.faded(fade);
        match &row.detail {
            Some(detail) => {
                let title = Rect::new(text_left, area.top + 4.0 * scale, title_right, area.top + 21.0 * scale);
                let sub = Rect::new(text_left, area.top + 21.0 * scale, title_right, area.bottom - 4.0 * scale);
                canvas.text(&EVENT_TITLE, &row.occurrence.summary, title, title_color);
                canvas.text(&EVENT_DETAIL, detail, sub, GRAY.faded(fade));
            }
            None => canvas.text(
                &EVENT_TITLE,
                &row.occurrence.summary,
                Rect::new(text_left, area.top, title_right, area.bottom),
                title_color,
            ),
        }
        match join {
            Some(button) => {
                let fill = if hovered_join { Color::rgb(0.2, 0.58, 0.9) } else { TODAY_FILL };
                canvas.fill_rounded_rect(button, button.height() / 2.0, fill.faded(fade));
                canvas.text(&JOIN, "Join", button, Color::WHITE.faded(fade));
            }
            None => {
                let trailing = Rect::new(
                    area.right - (ROW_PADDING + 54.0) * scale,
                    area.top,
                    area.right - ROW_PADDING * scale,
                    area.bottom,
                );
                canvas.text(&EVENT_TRAILING, &row.trailing, trailing, GRAY.faded(fade));
            }
        }
    }
}

/// The digits of a time replaced by zeros, so a width measured for it does not change as the time does.
fn widest_form(text: &str) -> String {
    text.chars().map(|character| if character.is_ascii_digit() { '0' } else { character }).collect()
}

/// The area of the events on the right.
fn events_area(bounds: Rect, scale: f32) -> Rect {
    Rect::new(bounds.left + (LEFT_WIDTH + 2.0 * COLUMN_GAP + 1.0) * scale, bounds.top, bounds.right, bounds.bottom)
}

fn row_rect(position: usize, bounds: Rect, scale: f32) -> Rect {
    let area = events_area(bounds, scale);
    let top = area.top + position as f32 * (ROW_HEIGHT + ROW_GAP) * scale;
    Rect::new(area.left, top, area.right, top + ROW_HEIGHT * scale)
}

impl Page for CalendarPage {
    fn id(&self) -> &'static str {
        "calendar"
    }

    fn name(&self) -> &'static str {
        "Calendar"
    }

    fn icon(&self) -> Icon {
        Icon::Calendar
    }

    fn priority(&self, mode: Mode) -> Option<Priority> {
        (mode == Mode::Expanded).then_some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let (bounds, scale) = (frame.bounds, frame.scale);
        self.draw_week(canvas, bounds, opacity);
        let divider_left = bounds.left + (LEFT_WIDTH + COLUMN_GAP) * scale;
        canvas.fill_rounded_rect(
            Rect::new(divider_left, bounds.top, divider_left + scale, bounds.bottom),
            0.0,
            DIVIDER.faded(opacity),
        );

        if self.agenda.rows.is_empty() {
            let area = events_area(bounds, scale);
            let (message, hint) = if self.link.is_none() {
                (
                    "No calendar linked",
                    Some(
                        "Copy your calendar's iCal address, then right-click the tray icon and choose \"Use calendar link from clipboard\".",
                    ),
                )
            } else {
                ("Nothing for today", None)
            };
            let icon_center =
                Point::new(area.center_x(), area.center_y() - if hint.is_some() { 30.0 } else { 14.0 } * scale);
            Icon::Calendar.draw(canvas, icon_center, 22.0 * scale, DIM, opacity);
            let text = Rect::new(area.left, icon_center.y + 14.0 * scale, area.right, icon_center.y + 34.0 * scale);
            canvas.text(&EMPTY, message, text, LIGHT.faded(opacity));
            if let Some(hint) = hint {
                let hint_area = Rect::new(area.left + 8.0 * scale, text.bottom, area.right - 8.0 * scale, area.bottom);
                canvas.text(&EMPTY_HINT, hint, hint_area, GRAY.faded(opacity));
            }
            return;
        }
        for (position, row) in self.agenda.rows.iter().enumerate() {
            self.draw_row(canvas, position, row, bounds, self.hovered_join == Some(position), opacity);
        }
    }

    fn presence(&self) -> Option<Presence> {
        if self.alert.is_visible() {
            Some(Presence::Transient { width: self.alert_width })
        } else {
            self.reminder.as_ref().map(|_| Presence::Badge { width: self.reminder_width })
        }
    }

    fn draw_presence(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        if self.alert.is_visible() {
            ui::draw_message(canvas, area, opacity, Icon::Calendar, BLUE, &self.alert_text, None);
            return;
        }
        let Some(reminder) = &self.reminder else { return };
        let icon_center = Point::new(area.left + 7.0 * scale, area.center_y());
        Icon::Calendar.draw(canvas, icon_center, 14.0 * scale, Color::WHITE, opacity);
        let title_width = (canvas.measure(&BADGE, &reminder.title).min(REMINDER_TITLE_WIDTH)).ceil() * scale;
        let title_left = area.left + 20.0 * scale;
        canvas.text(
            &BADGE,
            &reminder.title,
            Rect::new(title_left, area.top, title_left + title_width + 2.0 * scale, area.bottom),
            Color::WHITE.faded(opacity),
        );
        let time_left = title_left + title_width + 8.0 * scale;
        canvas.text(
            &BADGE_TIME,
            &reminder.text,
            Rect::new(time_left, area.top, area.right, area.bottom),
            BLUE.faded(opacity),
        );
    }

    fn menu_items(&self) -> Vec<String> {
        vec!["Use calendar link from clipboard".to_owned(), "Remove calendar link".to_owned()]
    }

    fn on_menu_item(&mut self, index: usize, context: &mut Context) {
        match index {
            0 => self.use_clipboard_link(context),
            _ => self.remove_link(context),
        }
        self.refresh(context);
        context.request_repaint();
    }

    fn on_start(&mut self, context: &mut Context) {
        self.notifier = Some(context.notifier());
        self.link = context.settings.get_string(LINK_SETTING).map(str::to_owned).filter(|link| !link.is_empty());
        if let Some(text) = self.storage.as_ref().and_then(|storage| std::fs::read_to_string(storage).ok()) {
            self.load_events(&text);
        }
        self.refresh(context);
        Self::arm_minute_timer(context);
        if self.link.is_some() {
            context.set_timer(FETCH_TIMER, FIRST_FETCH_MILLISECONDS);
        }
    }

    fn on_shown_changed(&mut self, is_shown: bool, context: &mut Context) {
        if is_shown {
            self.refresh(context);
            context.request_repaint();
        } else {
            self.hovered_join = None;
        }
    }

    fn on_notify(&mut self, context: &mut Context) {
        let result = self.fetched.lock().ok().and_then(|mut slot| slot.take());
        match result {
            Some(Ok(text)) => {
                if let Some(storage) = &self.storage {
                    if let Some(directory) = storage.parent() {
                        let _ = std::fs::create_dir_all(directory);
                    }
                    let _ = std::fs::write(storage, &text);
                }
                self.load_events(&text);
                self.refresh(context);
                context.request_repaint();
            }
            Some(Err(message)) => crate::log::debug(&format!("calendar: could not download: {message}")),
            None => {}
        }
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let (bounds, scale) = (frame.bounds, frame.scale);
        match event {
            PointerEvent::Moved(point) => {
                let hovered = (0..self.agenda.rows.len()).find(|&position| {
                    self.join_button(position, bounds, scale).is_some_and(|button| button.contains(point))
                });
                if hovered != self.hovered_join {
                    self.hovered_join = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Pressed(point) if frame.is_open() => {
                for (position, row) in self.agenda.rows.iter().enumerate() {
                    if let (Some(button), Some(meeting)) = (self.join_button(position, bounds, scale), &row.meeting)
                        && button.contains(point)
                    {
                        platform::open(&meeting.url);
                    }
                }
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                let was_hovered = self.hovered_join.take().is_some();
                if was_hovered {
                    context.request_repaint();
                }
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        match timer {
            FLASH_TIMER => {
                self.alert.on_timer(timer, context);
            }
            MINUTE_TIMER => {
                self.refresh(context);
                Self::arm_minute_timer(context);
                context.request_repaint();
            }
            FETCH_TIMER => {
                self.start_fetch();
                context.set_timer(FETCH_TIMER, Self::refresh_interval_milliseconds(context));
            }
            _ => {}
        }
    }
}

const EMPTY_HINT: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::Regular, Align::Center).wrapped();

#[cfg(test)]
mod tests {
    use super::{
        dates::{SECONDS_PER_DAY, days_from_civil},
        *,
    };

    const BOUNDS: Rect = Rect::new(0.0, 0.0, 508.0, 132.0);

    #[test]
    fn three_rows_fill_the_height_of_the_page() {
        let last = row_rect(2, BOUNDS, 1.0);
        assert_eq!(last.bottom, BOUNDS.bottom);
        assert!(row_rect(1, BOUNDS, 1.0).top > row_rect(0, BOUNDS, 1.0).bottom);
    }

    #[test]
    fn the_events_sit_to_the_right_of_the_week() {
        assert!(events_area(BOUNDS, 1.0).left > LEFT_WIDTH + COLUMN_GAP);
        assert_eq!(events_area(BOUNDS, 1.0).right, BOUNDS.right);
    }

    #[test]
    fn the_calendar_is_a_page_to_open_with_a_tray_menu_and_no_collapsed_state() {
        let page = CalendarPage::new();
        assert_eq!(page.priority(Mode::Resting), None);
        assert!(page.priority(Mode::Expanded).is_some());
        assert_eq!(page.menu_items().len(), 2);
        assert_eq!(page.presence(), None);
    }

    #[test]
    fn a_page_without_a_link_says_it_is_not_connected() {
        assert_eq!(CalendarPage::new().summary_text(), "Not connected");
    }

    #[test]
    fn the_widest_form_keeps_the_shape_and_drops_the_digits() {
        assert_eq!(widest_form("in 12m"), "in 00m");
    }

    #[test]
    fn utc_converts_to_local_time_without_going_backwards() {
        // Whatever the zone of the machine running the test, local time is within a day of UTC and consistent.
        let utc = days_from_civil(2026, 10, 24) * SECONDS_PER_DAY + 12 * 3600;
        let local = SystemClock.utc_to_local(utc);
        assert!((local - utc).abs() <= 14 * 3600);
        assert_eq!(SystemClock.utc_to_local(utc + 3600) - local, 3600);
    }
}
