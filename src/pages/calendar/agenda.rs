//! Calendar presentation uses local seconds.

use super::{
    dates::{SECONDS_PER_DAY, SECONDS_PER_MINUTE, clock_text},
    ics::Occurrence,
};

pub const MAX_ROWS: usize = 2;
const REMINDER_LEAD_MINUTES: i64 = 15;
const REMINDER_TRAIL_MINUTES: i64 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meeting {
    pub url: String,
    pub service: &'static str,
}

const SERVICES: [(&str, &str); 6] = [
    ("meet.google.com", "Google Meet"),
    ("zoom.us", "Zoom"),
    ("teams.microsoft.com", "Teams"),
    ("teams.live.com", "Teams"),
    ("webex.com", "Webex"),
    ("whereby.com", "Whereby"),
];

fn addresses(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("http") {
        let candidate = &rest[start..];
        if candidate.starts_with("https://") || candidate.starts_with("http://") {
            let end = candidate
                .find(|character: char| character.is_whitespace() || "<>\")'".contains(character))
                .unwrap_or(candidate.len());
            found.push(candidate[..end].trim_end_matches(['.', ',', ';']));
            rest = &candidate[end..];
        } else {
            rest = &candidate[4..];
        }
    }
    found
}

/// The meeting link of an event: a known video service in its location, link or description, else a link in its location.
pub fn find_meeting(occurrence: &Occurrence) -> Option<Meeting> {
    let places = [occurrence.location.as_str(), occurrence.url.as_str(), occurrence.description.as_str()];
    for place in places {
        for address in addresses(place) {
            let lower = address.to_lowercase();
            if let Some((_, service)) = SERVICES.iter().find(|(host, _)| lower.contains(host)) {
                return Some(Meeting { url: address.to_owned(), service });
            }
        }
    }
    addresses(&occurrence.location).first().map(|address| Meeting { url: (*address).to_owned(), service: "Link" })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Done,
    Next,
    Later,
}

pub struct Row {
    pub occurrence: Occurrence,
    pub state: State,
    pub time: String,
    pub detail: Option<String>,
    pub trailing: String,
    pub meeting: Option<Meeting>,
}

pub struct Agenda {
    pub rows: Vec<Row>,
}

pub fn relative_text(seconds: i64) -> String {
    let minutes = (seconds + SECONDS_PER_MINUTE - 1).div_euclid(SECONDS_PER_MINUTE).max(0);
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

pub fn duration_text(seconds: i64) -> String {
    let minutes = seconds.div_euclid(SECONDS_PER_MINUTE).max(0);
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

fn next_index(occurrences: &[Occurrence], now: i64) -> Option<usize> {
    occurrences.iter().position(|occurrence| !occurrence.is_all_day && occurrence.end.max(occurrence.start + 1) > now)
}

/// The rows to list: all events when they fit, else a window that has the next event in it.
fn visible_range(total: usize, next: Option<usize>) -> std::ops::Range<usize> {
    if total <= MAX_ROWS {
        return 0..total;
    }
    let start = next.map_or(total - MAX_ROWS, |next| next.saturating_sub(1)).min(total - MAX_ROWS);
    start..start + MAX_ROWS
}

pub fn build(occurrences: &[Occurrence], now: i64) -> Agenda {
    rows_for(occurrences, now, true)
}

pub fn list(occurrences: &[Occurrence]) -> Agenda {
    rows_for(occurrences, 0, false)
}

fn rows_for(occurrences: &[Occurrence], now: i64, is_today: bool) -> Agenda {
    let next = if is_today { next_index(occurrences, now) } else { None };
    let total = occurrences.len();
    let rows = visible_range(total, next)
        .map(|index| {
            let occurrence = &occurrences[index];
            let state = match next {
                Some(next) if index == next => State::Next,
                Some(next) if index > next => State::Later,
                _ if occurrence.is_all_day || !is_today => State::Later,
                _ => State::Done,
            };
            let meeting = find_meeting(occurrence);
            let time = if occurrence.is_all_day {
                String::new()
            } else {
                clock_text(occurrence.start.rem_euclid(SECONDS_PER_DAY))
            };
            let (detail, trailing) = match state {
                State::Next => {
                    let when = if occurrence.start <= now {
                        "now".to_owned()
                    } else {
                        format!("in {}", relative_text(occurrence.start - now))
                    };
                    let place = meeting.as_ref().map(|meeting| meeting.service.to_owned()).or_else(|| {
                        Some(occurrence.location.trim().to_owned())
                            .filter(|location| !location.is_empty() && !location.starts_with("http"))
                    });
                    (
                        Some(place.map_or(when.clone(), |place| format!("{when} \u{00b7} {place}"))),
                        duration_text(occurrence.end - occurrence.start),
                    )
                }
                State::Done => (None, "Done".to_owned()),
                State::Later if occurrence.is_all_day => (None, "All day".to_owned()),
                State::Later => (None, duration_text(occurrence.end - occurrence.start)),
            };
            Row { occurrence: occurrence.clone(), state, time, detail, trailing, meeting }
        })
        .collect();
    Agenda { rows }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reminder {
    pub title: String,
    pub text: String,
}

pub fn reminder(occurrences: &[Occurrence], now: i64) -> Option<Reminder> {
    let occurrence = occurrences.iter().filter(|occurrence| !occurrence.is_all_day).find(|occurrence| {
        occurrence.start - now <= REMINDER_LEAD_MINUTES * SECONDS_PER_MINUTE
            && now - occurrence.start <= REMINDER_TRAIL_MINUTES * SECONDS_PER_MINUTE
    })?;
    let text = if occurrence.start <= now {
        "now".to_owned()
    } else {
        format!("in {}m", (occurrence.start - now + SECONDS_PER_MINUTE - 1) / SECONDS_PER_MINUTE)
    };
    Some(Reminder { title: occurrence.summary.clone(), text })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const MINUTE: i64 = 60;
    const DAY_START: i64 = 20_000 * SECONDS_PER_DAY;

    fn event(start_hour: i64, minutes: i64, summary: &str) -> Occurrence {
        let start = DAY_START + start_hour * HOUR;
        Occurrence {
            start,
            end: start + minutes * MINUTE,
            is_all_day: false,
            summary: summary.to_owned(),
            location: String::new(),
            description: String::new(),
            url: String::new(),
        }
    }

    #[test]
    fn a_known_video_service_is_found_in_the_location_or_the_description() {
        let mut meeting = event(13, 60, "Review");
        meeting.location = "https://meet.google.com/abc-defg-hij".to_owned();
        assert_eq!(
            find_meeting(&meeting),
            Some(Meeting { url: "https://meet.google.com/abc-defg-hij".to_owned(), service: "Google Meet" })
        );
        let mut described = event(13, 60, "Call");
        described.description = "Join: https://us02web.zoom.us/j/12345?pwd=x. See you!".to_owned();
        assert_eq!(find_meeting(&described).unwrap().service, "Zoom");
        assert_eq!(find_meeting(&described).unwrap().url, "https://us02web.zoom.us/j/12345?pwd=x");
    }

    #[test]
    fn another_link_in_the_location_counts_as_a_plain_link_and_no_link_means_no_meeting() {
        let mut other = event(9, 30, "Talk");
        other.location = "https://example.com/room".to_owned();
        assert_eq!(find_meeting(&other).unwrap().service, "Link");
        assert_eq!(find_meeting(&event(9, 30, "Plain")), None);
    }

    #[test]
    fn times_are_worded_in_minutes_and_hours() {
        assert_eq!(relative_text(12 * MINUTE), "12 min");
        assert_eq!(relative_text(12 * MINUTE - 20), "12 min");
        assert_eq!(relative_text(2 * HOUR), "2 h");
        assert_eq!(relative_text(HOUR + 30 * MINUTE), "1 h 30 min");
        assert_eq!(duration_text(45 * MINUTE), "45 min");
        assert_eq!(duration_text(HOUR), "1 h");
    }

    #[test]
    fn the_next_event_is_the_first_one_that_has_not_finished() {
        let day = [event(9, 30, "Standup"), event(13, 60, "Review"), event(16, 60, "Reading")];
        let agenda = build(&day, DAY_START + 13 * HOUR - 12 * MINUTE);
        let states: Vec<State> = agenda.rows.iter().map(|row| row.state).collect();
        assert_eq!(states, vec![State::Done, State::Next]);
        assert_eq!(agenda.rows[1].detail.as_deref(), Some("in 12 min"));
        assert_eq!(agenda.rows[0].trailing, "Done");
        let morning = build(&day, DAY_START + 8 * HOUR);
        let states: Vec<State> = morning.rows.iter().map(|row| row.state).collect();
        assert_eq!(states, vec![State::Next, State::Later]);
        assert_eq!(morning.rows[1].trailing, "1 h");
    }

    #[test]
    fn the_next_event_names_its_meeting_service_or_place() {
        let mut review = event(13, 60, "Review");
        review.location = "https://meet.google.com/x".to_owned();
        let mut visit = event(15, 60, "Visit");
        visit.location = "Room 4".to_owned();
        let agenda = build(&[review.clone()], review.start - 12 * MINUTE);
        assert_eq!(agenda.rows[0].detail.as_deref(), Some("in 12 min \u{00b7} Google Meet"));
        assert!(agenda.rows[0].meeting.is_some());
        let agenda = build(&[visit.clone()], visit.start - 30 * MINUTE);
        assert_eq!(agenda.rows[0].detail.as_deref(), Some("in 30 min \u{00b7} Room 4"));
    }

    #[test]
    fn an_event_in_progress_is_next_and_says_now() {
        let day = [event(13, 60, "Review")];
        let agenda = build(&day, DAY_START + 13 * HOUR + 10 * MINUTE);
        assert_eq!(agenda.rows[0].state, State::Next);
        assert_eq!(agenda.rows[0].detail.as_deref(), Some("now"));
    }

    #[test]
    fn after_the_last_event_everything_is_done() {
        let agenda = build(&[event(9, 30, "Standup")], DAY_START + 20 * HOUR);
        assert_eq!(agenda.rows[0].state, State::Done);
    }

    #[test]
    fn another_day_lists_its_events_with_none_next_and_none_over() {
        let agenda = list(&[event(9, 30, "Standup"), event(13, 60, "Review")]);
        let states: Vec<State> = agenda.rows.iter().map(|row| row.state).collect();
        assert_eq!(states, vec![State::Later, State::Later]);
        assert_eq!(agenda.rows[1].trailing, "1 h");
        assert!(agenda.rows.iter().all(|row| row.detail.is_none()));
    }

    #[test]
    fn an_empty_day_has_no_rows() {
        let agenda = build(&[], DAY_START);
        assert!(agenda.rows.is_empty());
    }

    #[test]
    fn all_day_events_have_no_clock_time_and_are_never_next() {
        let mut trip = event(0, 0, "Trip");
        trip.is_all_day = true;
        trip.end = trip.start + SECONDS_PER_DAY;
        let agenda = build(&[trip, event(13, 60, "Review")], DAY_START + 9 * HOUR);
        assert_eq!(agenda.rows[0].time, "");
        assert_eq!(agenda.rows[0].trailing, "All day");
        assert_eq!(agenda.rows[1].state, State::Next);
    }

    #[test]
    fn a_long_day_lists_a_window_around_the_next_event() {
        let day: Vec<Occurrence> = (8..14).map(|hour| event(hour, 30, &format!("Event {hour}"))).collect();
        let agenda = build(&day, DAY_START + 10 * HOUR + 40 * MINUTE);
        let names: Vec<&str> = agenda.rows.iter().map(|row| row.occurrence.summary.as_str()).collect();
        assert_eq!(names, vec!["Event 10", "Event 11"]);
        assert_eq!(agenda.rows.len(), MAX_ROWS);
        let late = build(&day, DAY_START + 23 * HOUR);
        assert_eq!(late.rows[0].occurrence.summary, "Event 12");
    }

    #[test]
    fn the_reminder_appears_fifteen_minutes_before_and_leaves_five_after() {
        let day = [event(13, 60, "Design review")];
        let start = day[0].start;
        assert_eq!(reminder(&day, start - 16 * MINUTE), None);
        assert_eq!(reminder(&day, start - 15 * MINUTE).unwrap().text, "in 15m");
        assert_eq!(reminder(&day, start - 12 * MINUTE + 10).unwrap().text, "in 12m");
        assert_eq!(reminder(&day, start).unwrap().text, "now");
        assert_eq!(reminder(&day, start + 5 * MINUTE).unwrap().text, "now");
        assert_eq!(reminder(&day, start + 5 * MINUTE + 1), None);
        assert_eq!(reminder(&day, start - 12 * MINUTE).unwrap().title, "Design review");
    }

    #[test]
    fn all_day_events_never_trigger_a_reminder() {
        let mut trip = event(0, 0, "Trip");
        trip.is_all_day = true;
        assert_eq!(reminder(&[trip], DAY_START), None);
    }
}
