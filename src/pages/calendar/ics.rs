//! Recurrences use wall time. Zone names without VTIMEZONE fall back to local time.
//! Zone offsets ignore DST; BYWEEKNO, BYYEARDAY and BYSETPOS are unsupported.

use std::collections::HashMap;

use super::dates::{
    SECONDS_PER_DAY, civil_from_days, day_of, days_from_civil, days_in_month, nth_weekday_of_month, same_day_in_month,
    seconds_from_civil, week_start, weekday,
};

const MAX_STEPS: i64 = 200_000;
const WEEKDAY_CODES: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Floating,
    Utc,
    Offset(i64),
}

/// Turns UTC into local time; production asks Windows, tests use a fixed offset.
pub trait Clock {
    fn utc_to_local(&self, utc: i64) -> i64;
}

#[cfg(test)]
pub struct FixedClock(pub i64);

#[cfg(test)]
impl Clock for FixedClock {
    fn utc_to_local(&self, utc: i64) -> i64 {
        utc + self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub frequency: Frequency,
    pub interval: i64,
    pub count: Option<i64>,
    /// The last moment a repeat may start, as wall seconds in the event's zone.
    pub until: Option<i64>,
    /// Weekdays (Monday = 0) with an optional position in the month: `(2, 1)` is the second Tuesday.
    pub by_day: Vec<(i32, u32)>,
    pub by_month_day: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub location: String,
    pub description: String,
    pub url: String,
    /// Start and end as wall seconds in `zone`; the end of an all-day event is the day after its last day.
    pub start: i64,
    pub end: i64,
    pub zone: Zone,
    pub is_all_day: bool,
    pub rule: Option<Rule>,
    /// Starts (wall seconds) of repeats that were removed.
    pub exceptions: Vec<i64>,
    /// For an event that replaces one repeat of another: the start (wall seconds) it replaces.
    pub replaces: Option<i64>,
    pub is_cancelled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub start: i64,
    pub end: i64,
    pub is_all_day: bool,
    pub summary: String,
    pub location: String,
    pub description: String,
    pub url: String,
}

// Continuation lines begin with whitespace.

fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        match (line.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(previous)) => previous.push_str(rest),
            _ => lines.push(line.to_owned()),
        }
    }
    lines
}

struct Property {
    name: String,
    parameters: Vec<(String, String)>,
    value: String,
}

fn parse_property(line: &str) -> Option<Property> {
    let mut in_quotes = false;
    let split = line.char_indices().find(|&(_, character)| {
        if character == '"' {
            in_quotes = !in_quotes;
        }
        character == ':' && !in_quotes
    })?;
    let (head, value) = (&line[..split.0], &line[split.0 + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_uppercase();
    let parameters = parts
        .filter_map(|part| part.split_once('='))
        .map(|(key, parameter)| (key.to_uppercase(), parameter.trim_matches('"').to_owned()))
        .collect();
    Some(Property { name, parameters, value: value.to_owned() })
}

impl Property {
    fn parameter(&self, key: &str) -> Option<&str> {
        self.parameters.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
    }
}

fn unescape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        match characters.next() {
            Some('n') | Some('N') => result.push('\n'),
            Some(other) => result.push(other),
            None => {}
        }
    }
    result
}

fn parse_moment(value: &str) -> Option<(i64, bool, bool)> {
    let value = value.trim();
    let number = |from: usize, to: usize| value.get(from..to)?.parse::<u32>().ok();
    let (year, month, day) = (i64::from(number(0, 4)?), number(4, 6)?, number(6, 8)?);
    if value.len() == 8 {
        return Some((seconds_from_civil(year, month, day, 0, 0, 0), false, true));
    }
    if value.get(8..9) != Some("T") {
        return None;
    }
    let seconds = seconds_from_civil(year, month, day, number(9, 11)?, number(11, 13)?, number(13, 15).unwrap_or(0));
    Some((seconds, value.ends_with('Z'), false))
}

fn parse_offset(value: &str) -> Option<i64> {
    let value = value.trim();
    let sign = match value.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let number = |from: usize, to: usize| value.get(from..to)?.parse::<i64>().ok();
    Some(sign * (number(1, 3)? * 3600 + number(3, 5)? * 60 + number(5, 7).unwrap_or(0)))
}

fn parse_duration(value: &str) -> Option<i64> {
    let value = value.trim();
    let (negative, value) = match value.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, value.strip_prefix('+').unwrap_or(value)),
    };
    let value = value.strip_prefix('P')?;
    let (mut total, mut digits, mut in_time) = (0i64, String::new(), false);
    for character in value.chars() {
        match character {
            'T' => in_time = true,
            '0'..='9' => digits.push(character),
            unit => {
                let amount: i64 = digits.parse().ok()?;
                digits.clear();
                total += amount
                    * match (unit, in_time) {
                        ('W', _) => 7 * SECONDS_PER_DAY,
                        ('D', _) => SECONDS_PER_DAY,
                        ('H', true) => 3600,
                        ('M', true) => 60,
                        ('S', true) => 1,
                        _ => return None,
                    };
            }
        }
    }
    Some(if negative { -total } else { total })
}

fn offset_of(zone: Zone) -> i64 {
    match zone {
        Zone::Offset(offset) => offset,
        _ => 0,
    }
}

struct Moment {
    wall: i64,
    zone: Zone,
    is_date: bool,
}

fn read_moment(property: &Property, zones: &HashMap<String, i64>) -> Option<Moment> {
    let (wall, is_utc, is_date) = parse_moment(&property.value)?;
    let zone = if is_utc {
        Zone::Utc
    } else if is_date {
        Zone::Floating
    } else {
        property
            .parameter("TZID")
            .and_then(|name| zones.get(name))
            .map_or(Zone::Floating, |&offset| Zone::Offset(offset))
    };
    Some(Moment { wall, zone, is_date })
}

/// Expresses a moment as wall seconds in `target`'s zone. A floating moment is already on the target's wall.
fn in_zone(moment: &Moment, target: Zone) -> i64 {
    if moment.zone == Zone::Floating || target == Zone::Floating {
        return moment.wall;
    }
    let utc = moment.wall - offset_of(moment.zone);
    utc + offset_of(target)
}

fn parse_rule(value: &str, zone: Zone) -> Option<Rule> {
    let mut rule = Rule {
        frequency: Frequency::Daily,
        interval: 1,
        count: None,
        until: None,
        by_day: Vec::new(),
        by_month_day: Vec::new(),
    };
    let mut has_frequency = false;
    for part in value.split(';') {
        let Some((key, text)) = part.split_once('=') else { continue };
        match key.to_uppercase().as_str() {
            "FREQ" => {
                rule.frequency = match text.to_uppercase().as_str() {
                    "DAILY" => Frequency::Daily,
                    "WEEKLY" => Frequency::Weekly,
                    "MONTHLY" => Frequency::Monthly,
                    "YEARLY" => Frequency::Yearly,
                    _ => return None,
                };
                has_frequency = true;
            }
            "INTERVAL" => rule.interval = text.parse().ok().filter(|interval| *interval > 0)?,
            "COUNT" => rule.count = text.parse().ok(),
            "UNTIL" => {
                let (seconds, is_utc, is_date) = parse_moment(text)?;
                let until = Moment {
                    wall: seconds + if is_date { SECONDS_PER_DAY - 1 } else { 0 },
                    zone: if is_utc { Zone::Utc } else { Zone::Floating },
                    is_date,
                };
                rule.until = Some(in_zone(&until, zone));
            }
            "BYDAY" => rule.by_day = text.split(',').filter_map(parse_by_day).collect(),
            "BYMONTHDAY" => rule.by_month_day = text.split(',').filter_map(|day| day.trim().parse().ok()).collect(),
            _ => {}
        }
    }
    has_frequency.then_some(rule)
}

fn parse_by_day(text: &str) -> Option<(i32, u32)> {
    let text = text.trim().to_uppercase();
    let split = text.len().checked_sub(2)?;
    let code = text.get(split..)?;
    let weekday = WEEKDAY_CODES.iter().position(|known| *known == code)? as u32;
    let ordinal = match text.get(..split)? {
        "" => 0,
        prefix => prefix.parse().ok()?,
    };
    Some((ordinal, weekday))
}

type ZoneBlock = (String, Option<i64>, Option<i64>);

#[derive(Default)]
struct Draft {
    properties: Vec<Property>,
}

fn build_event(draft: &Draft, zones: &HashMap<String, i64>) -> Option<Event> {
    let find = |name: &str| draft.properties.iter().find(|property| property.name == name);
    let start_property = find("DTSTART")?;
    let start = read_moment(start_property, zones)?;
    let zone = start.zone;
    let is_all_day = start.is_date;
    let end = match (find("DTEND"), find("DURATION")) {
        (Some(property), _) => in_zone(&read_moment(property, zones)?, zone),
        (None, Some(property)) => start.wall + parse_duration(&property.value)?,
        (None, None) if is_all_day => start.wall + SECONDS_PER_DAY,
        (None, None) => start.wall,
    };
    let text = |name: &str| find(name).map(|property| unescape(&property.value)).unwrap_or_default();
    let exceptions = draft
        .properties
        .iter()
        .filter(|property| property.name == "EXDATE")
        .flat_map(|property| {
            property.value.split(',').filter_map(|value| {
                let single = Property {
                    name: property.name.clone(),
                    parameters: property.parameters.clone(),
                    value: value.to_owned(),
                };
                Some(in_zone(&read_moment(&single, zones)?, zone))
            })
        })
        .collect();
    Some(Event {
        uid: text("UID"),
        summary: text("SUMMARY"),
        location: text("LOCATION"),
        description: text("DESCRIPTION"),
        url: text("URL"),
        start: start.wall,
        end: end.max(start.wall),
        zone,
        is_all_day,
        rule: find("RRULE").and_then(|property| parse_rule(&property.value, zone)),
        exceptions,
        replaces: find("RECURRENCE-ID").and_then(|property| Some(in_zone(&read_moment(property, zones)?, zone))),
        is_cancelled: find("STATUS").is_some_and(|property| property.value.trim().eq_ignore_ascii_case("CANCELLED")),
    })
}

/// Reads every event of an iCalendar file; what cannot be read is skipped.
pub fn parse(text: &str) -> Vec<Event> {
    let mut zones: HashMap<String, i64> = HashMap::new();
    let mut drafts: Vec<Draft> = Vec::new();
    let (mut current_event, mut current_zone): (Option<Draft>, Option<ZoneBlock>) = (None, None);
    let mut section = String::new();
    for line in unfold(text) {
        let Some(property) = parse_property(&line) else { continue };
        match (property.name.as_str(), property.value.as_str()) {
            ("BEGIN", "VEVENT") => current_event = Some(Draft::default()),
            ("END", "VEVENT") => drafts.extend(current_event.take()),
            ("BEGIN", "VTIMEZONE") => current_zone = Some((String::new(), None, None)),
            ("END", "VTIMEZONE") => {
                if let Some((name, standard, daylight)) = current_zone.take()
                    && let Some(offset) = standard.or(daylight)
                {
                    zones.insert(name, offset);
                }
            }
            ("BEGIN", name) => section = name.to_uppercase(),
            ("END", _) => section.clear(),
            _ => {
                if let Some(draft) = current_event.as_mut() {
                    draft.properties.push(property);
                } else if let Some((name, standard, daylight)) = current_zone.as_mut() {
                    match (property.name.as_str(), section.as_str()) {
                        ("TZID", _) => *name = property.value.clone(),
                        ("TZOFFSETTO", "STANDARD") => *standard = standard.or(parse_offset(&property.value)),
                        ("TZOFFSETTO", "DAYLIGHT") => *daylight = daylight.or(parse_offset(&property.value)),
                        _ => {}
                    }
                }
            }
        }
    }
    drafts.iter().filter_map(|draft| build_event(draft, &zones)).collect()
}

// Recurrences retain wall time across clock changes.

/// The start (wall seconds) of every repeat of `event` from `from` to `to`, the first one included.
fn repeat_starts(event: &Event, from: i64, to: i64) -> Vec<i64> {
    let Some(rule) = &event.rule else { return vec![event.start] };
    let time_of_day = event.start.rem_euclid(SECONDS_PER_DAY);
    let start_day = day_of(event.start);
    let (from_day, to_day) = (day_of(from), day_of(to));
    let mut days: Vec<i64> = Vec::new();
    let mut produced = 0i64;
    let limit = rule.count.unwrap_or(i64::MAX);
    let has_ended = |day: i64| rule.until.is_some_and(|until| day * SECONDS_PER_DAY + time_of_day > until);

    match rule.frequency {
        Frequency::Daily => {
            let first = if rule.count.is_none() { ((from_day - start_day) / rule.interval - 1).max(0) } else { 0 };
            for step in first..first + MAX_STEPS {
                let day = start_day + step * rule.interval;
                if step >= limit || day > to_day || has_ended(day) {
                    break;
                }
                days.push(day);
            }
        }
        Frequency::Weekly => {
            let base = week_start(start_day);
            let weekdays: Vec<u32> = if rule.by_day.is_empty() {
                vec![weekday(start_day)]
            } else {
                let mut listed: Vec<u32> = rule.by_day.iter().map(|&(_, weekday)| weekday).collect();
                listed.sort_unstable();
                listed
            };
            let first = if rule.count.is_none() { ((from_day - base) / (7 * rule.interval) - 1).max(0) } else { 0 };
            'weeks: for week in first..first + MAX_STEPS {
                let week_day = base + week * 7 * rule.interval;
                if week_day > to_day {
                    break;
                }
                for &listed in &weekdays {
                    let day = week_day + i64::from(listed);
                    if day < start_day {
                        continue;
                    }
                    if has_ended(day) || produced >= limit {
                        break 'weeks;
                    }
                    produced += 1;
                    days.push(day);
                }
            }
        }
        Frequency::Monthly | Frequency::Yearly => {
            let (start_year, start_month, start_date) = civil_from_days(start_day);
            let months_per_step = if rule.frequency == Frequency::Yearly { 12 * rule.interval } else { rule.interval };
            let months_before = (civil_from_days(from_day).0 - start_year) * 12
                + i64::from(civil_from_days(from_day).1)
                - i64::from(start_month);
            let first = if rule.count.is_none() { (months_before / months_per_step - 1).max(0) } else { 0 };
            'months: for step in first..first + MAX_STEPS {
                let months = step * months_per_step;
                let index = start_year * 12 + i64::from(start_month) - 1 + months;
                let (year, month) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32);
                if days_from_civil(year, month, 1) > to_day {
                    break;
                }
                let mut in_month: Vec<i64> = Vec::new();
                if rule.frequency == Frequency::Monthly && !rule.by_day.is_empty() {
                    for &(ordinal, listed) in &rule.by_day {
                        if ordinal == 0 {
                            let first_day = days_from_civil(year, month, 1);
                            in_month.extend(
                                (0..i64::from(days_in_month(year, month)))
                                    .map(|offset| first_day + offset)
                                    .filter(|&day| weekday(day) == listed),
                            );
                        } else {
                            in_month.extend(nth_weekday_of_month(year, month, listed, ordinal));
                        }
                    }
                } else if rule.frequency == Frequency::Monthly && !rule.by_month_day.is_empty() {
                    for &number in &rule.by_month_day {
                        let length = days_in_month(year, month) as i32;
                        let day = if number > 0 { number } else { length + 1 + number };
                        if (1..=length).contains(&day) {
                            in_month.push(days_from_civil(year, month, day as u32));
                        }
                    }
                } else {
                    in_month.extend(same_day_in_month(start_year, start_month, start_date, months));
                }
                in_month.sort_unstable();
                for day in in_month {
                    if day < start_day {
                        continue;
                    }
                    if has_ended(day) || produced >= limit {
                        break 'months;
                    }
                    produced += 1;
                    days.push(day);
                }
            }
        }
    }
    days.into_iter()
        .map(|day| day * SECONDS_PER_DAY + time_of_day)
        .filter(|&start| start >= from && start <= to)
        .collect()
}

fn to_local(wall: i64, zone: Zone, clock: &dyn Clock) -> i64 {
    match zone {
        Zone::Floating => wall,
        Zone::Utc => clock.utc_to_local(wall),
        Zone::Offset(offset) => clock.utc_to_local(wall - offset),
    }
}

pub fn occurrences_on(events: &[Event], day: i64, clock: &dyn Clock) -> Vec<Occurrence> {
    let (window_start, window_end) = (day * SECONDS_PER_DAY, (day + 1) * SECONDS_PER_DAY);
    let replaced: Vec<(&str, i64)> =
        events.iter().filter_map(|event| event.replaces.map(|start| (event.uid.as_str(), start))).collect();
    let mut found = Vec::new();
    for event in events.iter().filter(|event| !event.is_cancelled) {
        let length = event.end - event.start;
        // Wall time can differ from local time by a day either way, so look a day further on both sides.
        let from = window_start - length - 2 * SECONDS_PER_DAY;
        let to = window_end + 2 * SECONDS_PER_DAY;
        for start in repeat_starts(event, from, to) {
            let is_removed = event.exceptions.contains(&start) || replaced.contains(&(event.uid.as_str(), start));
            if is_removed && event.replaces.is_none() {
                continue;
            }
            let local_start = if event.is_all_day { start } else { to_local(start, event.zone, clock) };
            let local_end = local_start + length;
            let overlaps = local_start < window_end && local_end > window_start;
            let is_instant_inside = length == 0 && (window_start..window_end).contains(&local_start);
            if overlaps || is_instant_inside {
                found.push(Occurrence {
                    start: local_start,
                    end: local_end,
                    is_all_day: event.is_all_day,
                    summary: event.summary.clone(),
                    location: event.location.clone(),
                    description: event.description.clone(),
                    url: event.url.clone(),
                });
            }
        }
    }
    found.sort_by_key(|occurrence| (!occurrence.is_all_day, occurrence.start, occurrence.end));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOCK: FixedClock = FixedClock(7 * 3600);

    fn calendar(body: &str) -> String {
        format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{body}END:VCALENDAR\r\n")
    }

    fn day(year: i64, month: u32, date: u32) -> i64 {
        days_from_civil(year, month, date)
    }

    fn titles(events: &[Event], when: i64) -> Vec<String> {
        occurrences_on(events, when, &CLOCK).into_iter().map(|occurrence| occurrence.summary).collect()
    }

    #[test]
    fn a_utc_event_shows_in_local_time() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nUID:a\r\nDTSTART:20261024T063000Z\r\nDTEND:20261024T073000Z\r\nSUMMARY:Design review\r\nEND:VEVENT\r\n",
        ));
        let found = occurrences_on(&events, day(2026, 10, 24), &CLOCK);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].summary, "Design review");
        assert_eq!(found[0].start % SECONDS_PER_DAY, 13 * 3600 + 30 * 60);
        assert_eq!(found[0].end - found[0].start, 3600);
        assert!(occurrences_on(&events, day(2026, 10, 25), &CLOCK).is_empty());
    }

    #[test]
    fn a_late_utc_event_belongs_to_the_next_local_day() {
        let events = parse(&calendar("BEGIN:VEVENT\r\nDTSTART:20261024T200000Z\r\nSUMMARY:Late\r\nEND:VEVENT\r\n"));
        assert!(titles(&events, day(2026, 10, 24)).is_empty());
        assert_eq!(titles(&events, day(2026, 10, 25)), vec!["Late"]);
    }

    #[test]
    fn a_zoned_event_uses_the_offset_of_its_timezone_block() {
        let events = parse(&calendar(
            "BEGIN:VTIMEZONE\r\nTZID:Asia/Jakarta\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0700\r\nTZOFFSETTO:+0700\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n\
             BEGIN:VEVENT\r\nDTSTART;TZID=Asia/Jakarta:20261024T090000\r\nDTEND;TZID=Asia/Jakarta:20261024T093000\r\nSUMMARY:Standup\r\nEND:VEVENT\r\n",
        ));
        let found = occurrences_on(&events, day(2026, 10, 24), &CLOCK);
        assert_eq!(found[0].start % SECONDS_PER_DAY, 9 * 3600);
        assert_eq!(found[0].end - found[0].start, 1800);
    }

    #[test]
    fn a_floating_time_is_the_wall_clock() {
        let events = parse(&calendar("BEGIN:VEVENT\r\nDTSTART:20261024T090000\r\nSUMMARY:Local\r\nEND:VEVENT\r\n"));
        assert_eq!(occurrences_on(&events, day(2026, 10, 24), &CLOCK)[0].start % SECONDS_PER_DAY, 9 * 3600);
    }

    #[test]
    fn an_all_day_event_covers_its_days_only_and_comes_first() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261024T090000Z\r\nSUMMARY:Meeting\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261024\r\nDTEND;VALUE=DATE:20261026\r\nSUMMARY:Trip\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&events, day(2026, 10, 24)), vec!["Trip", "Meeting"]);
        assert_eq!(titles(&events, day(2026, 10, 25)), vec!["Trip"]);
        assert!(titles(&events, day(2026, 10, 26)).is_empty());
    }

    #[test]
    fn a_duration_replaces_the_end() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261024T060000Z\r\nDURATION:PT1H30M\r\nSUMMARY:Long\r\nEND:VEVENT\r\n",
        ));
        let found = occurrences_on(&events, day(2026, 10, 24), &CLOCK);
        assert_eq!(found[0].end - found[0].start, 5400);
    }

    #[test]
    fn text_is_unfolded_and_unescaped() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261024T060000Z\r\nSUMMARY:Plan\\, review\r\n  and ship\r\nDESCRIPTION:one\\ntwo\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(events[0].summary, "Plan, review and ship");
        assert_eq!(events[0].description, "one\ntwo");
    }

    #[test]
    fn cancelled_events_and_events_without_a_start_are_left_out() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261024T060000Z\r\nSTATUS:CANCELLED\r\nSUMMARY:Gone\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nSUMMARY:No start\r\nEND:VEVENT\r\n",
        ));
        assert!(titles(&events, day(2026, 10, 24)).is_empty());
    }

    #[test]
    fn a_daily_repeat_shows_on_every_day_after_it_starts() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261020T020000Z\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Daily\r\nEND:VEVENT\r\n",
        ));
        assert!(titles(&events, day(2026, 10, 19)).is_empty());
        assert_eq!(titles(&events, day(2026, 10, 20)), vec!["Daily"]);
        assert_eq!(titles(&events, day(2027, 3, 3)), vec!["Daily"]);
    }

    #[test]
    fn a_repeat_with_a_count_or_an_end_date_stops() {
        let counted = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261020T020000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nSUMMARY:Three\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&counted, day(2026, 10, 22)), vec!["Three"]);
        assert!(titles(&counted, day(2026, 10, 23)).is_empty());
        let ended = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261020T020000Z\r\nRRULE:FREQ=DAILY;UNTIL=20261022T020000Z\r\nSUMMARY:Until\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&ended, day(2026, 10, 22)), vec!["Until"]);
        assert!(titles(&ended, day(2026, 10, 23)).is_empty());
    }

    #[test]
    fn a_weekly_repeat_on_chosen_weekdays_skips_the_others() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261019T020000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE\r\nSUMMARY:Class\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&events, day(2026, 10, 21)), vec!["Class"]);
        assert!(titles(&events, day(2026, 10, 22)).is_empty());
        assert_eq!(titles(&events, day(2026, 11, 2)), vec!["Class"]);
        assert_eq!(titles(&events, day(2026, 11, 4)), vec!["Class"]);
    }

    #[test]
    fn a_two_weekly_repeat_skips_the_weeks_in_between() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261019T020000Z\r\nRRULE:FREQ=WEEKLY;INTERVAL=2\r\nSUMMARY:Sprint\r\nEND:VEVENT\r\n",
        ));
        assert!(titles(&events, day(2026, 10, 26)).is_empty());
        assert_eq!(titles(&events, day(2026, 11, 2)), vec!["Sprint"]);
    }

    #[test]
    fn a_monthly_repeat_keeps_the_day_of_the_month_or_the_nth_weekday() {
        let by_date = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261031T020000Z\r\nRRULE:FREQ=MONTHLY\r\nSUMMARY:Month end\r\nEND:VEVENT\r\n",
        ));
        assert!(titles(&by_date, day(2026, 11, 30)).is_empty());
        assert_eq!(titles(&by_date, day(2026, 12, 31)), vec!["Month end"]);
        let by_weekday = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261013T020000Z\r\nRRULE:FREQ=MONTHLY;BYDAY=2TU\r\nSUMMARY:Second Tuesday\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&by_weekday, day(2026, 11, 10)), vec!["Second Tuesday"]);
        assert!(titles(&by_weekday, day(2026, 11, 17)).is_empty());
    }

    #[test]
    fn a_yearly_repeat_comes_back_on_the_same_date() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20200229\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Leap\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&events, day(2028, 2, 29)), vec!["Leap"]);
        assert!(titles(&events, day(2027, 2, 28)).is_empty());
    }

    #[test]
    fn a_removed_repeat_is_skipped() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20261020T020000Z\r\nRRULE:FREQ=DAILY\r\nEXDATE:20261022T020000Z\r\nSUMMARY:Daily\r\nEND:VEVENT\r\n",
        ));
        assert!(titles(&events, day(2026, 10, 22)).is_empty());
        assert_eq!(titles(&events, day(2026, 10, 23)), vec!["Daily"]);
    }

    #[test]
    fn a_moved_repeat_replaces_the_original() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nUID:x\r\nDTSTART:20261020T020000Z\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Daily\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:x\r\nRECURRENCE-ID:20261022T020000Z\r\nDTSTART:20261022T080000Z\r\nSUMMARY:Moved\r\nEND:VEVENT\r\n",
        ));
        let found = occurrences_on(&events, day(2026, 10, 22), &CLOCK);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].summary, "Moved");
        assert_eq!(found[0].start % SECONDS_PER_DAY, 15 * 3600);
    }

    #[test]
    fn a_repeat_that_started_years_ago_is_found_quickly() {
        let events = parse(&calendar(
            "BEGIN:VEVENT\r\nDTSTART:20100104T020000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO\r\nSUMMARY:Old\r\nEND:VEVENT\r\n",
        ));
        assert_eq!(titles(&events, day(2026, 10, 19)), vec!["Old"]);
    }

    #[test]
    fn durations_parse_in_every_unit() {
        assert_eq!(parse_duration("PT1H30M"), Some(5400));
        assert_eq!(parse_duration("P1D"), Some(86_400));
        assert_eq!(parse_duration("P2W"), Some(1_209_600));
        assert_eq!(parse_duration("-PT15M"), Some(-900));
        assert_eq!(parse_duration("nonsense"), None);
    }

    #[test]
    fn offsets_parse_with_either_sign() {
        assert_eq!(parse_offset("+0700"), Some(25_200));
        assert_eq!(parse_offset("-0330"), Some(-12_600));
        assert_eq!(parse_offset("0700"), None);
    }

    #[test]
    fn garbage_gives_no_events() {
        assert!(parse("").is_empty());
        assert!(parse("not a calendar\r\nat all").is_empty());
    }
}
