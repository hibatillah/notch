//! Calendar arithmetic on plain numbers: days since 1970-01-01 and seconds, with no time zone in sight. Pure, so it
//! is tested without a window. Weekdays count from Monday = 0.

pub const SECONDS_PER_DAY: i64 = 86_400;
pub const SECONDS_PER_MINUTE: i64 = 60;

pub const WEEKDAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
pub const WEEKDAY_LETTERS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];
pub const MONTH_NAMES: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Days since 1970-01-01 of a civil date (the proleptic Gregorian calendar).
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date (year, month 1 to 12, day 1 to 31) of a day number.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The weekday of a day number, Monday = 0. 1970-01-01 was a Thursday.
pub fn weekday(days: i64) -> u32 {
    (days + 3).rem_euclid(7) as u32
}

/// The day number that contains this many seconds since 1970-01-01.
pub fn day_of(seconds: i64) -> i64 {
    seconds.div_euclid(SECONDS_PER_DAY)
}

pub fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap_year(year) => 29,
        _ => 28,
    }
}

/// The first day of the week (Monday) that contains `day`.
pub fn week_start(day: i64) -> i64 {
    day - i64::from(weekday(day))
}

/// The date `months` months after (or before) the month of `day`, on the same day of the month, or `None` when that
/// month has no such day.
pub fn same_day_in_month(year: i64, month: u32, day: u32, months: i64) -> Option<i64> {
    let index = year * 12 + i64::from(month) - 1 + months;
    let (year, month) = (index.div_euclid(12), (index.rem_euclid(12) + 1) as u32);
    (day <= days_in_month(year, month)).then(|| days_from_civil(year, month, day))
}

/// The day number of the `ordinal`th `weekday` of a month; a negative ordinal counts from the end (-1 is the last).
pub fn nth_weekday_of_month(year: i64, month: u32, weekday_number: u32, ordinal: i32) -> Option<i64> {
    let first = days_from_civil(year, month, 1);
    let last = first + i64::from(days_in_month(year, month)) - 1;
    if ordinal > 0 {
        let offset = (i64::from(weekday_number) - i64::from(weekday(first))).rem_euclid(7);
        let day = first + offset + 7 * i64::from(ordinal - 1);
        (day <= last).then_some(day)
    } else if ordinal < 0 {
        let offset = (i64::from(weekday(last)) - i64::from(weekday_number)).rem_euclid(7);
        let day = last - offset - 7 * i64::from(-ordinal - 1);
        (day >= first).then_some(day)
    } else {
        None
    }
}

/// Seconds since 1970-01-01 of a civil date and time of day.
pub fn seconds_from_civil(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    days_from_civil(year, month, day) * SECONDS_PER_DAY + i64::from(hour * 3600 + minute * 60 + second)
}

/// "13:30" for a time of day in seconds.
pub fn clock_text(seconds_in_day: i64) -> String {
    let minutes = seconds_in_day.rem_euclid(SECONDS_PER_DAY) / SECONDS_PER_MINUTE;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_day_zero_and_a_thursday() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(weekday(0), 3);
    }

    #[test]
    fn dates_round_trip_through_day_numbers() {
        for (year, month, day) in
            [(1970, 1, 1), (2000, 2, 29), (2026, 10, 24), (1969, 12, 31), (2100, 3, 1), (1900, 1, 1)]
        {
            assert_eq!(civil_from_days(days_from_civil(year, month, day)), (year, month, day));
        }
    }

    #[test]
    fn known_dates_have_the_right_weekday() {
        // 2026-10-24 is a Saturday, 2024-02-29 a Thursday, 2000-01-01 a Saturday.
        assert_eq!(weekday(days_from_civil(2026, 10, 24)), 5);
        assert_eq!(weekday(days_from_civil(2024, 2, 29)), 3);
        assert_eq!(weekday(days_from_civil(2000, 1, 1)), 5);
    }

    #[test]
    fn the_week_starts_on_monday() {
        let saturday = days_from_civil(2026, 10, 24);
        assert_eq!(civil_from_days(week_start(saturday)), (2026, 10, 19));
        assert_eq!(week_start(week_start(saturday)), week_start(saturday));
    }

    #[test]
    fn months_have_the_right_lengths() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2026, 4), 30);
    }

    #[test]
    fn the_same_day_in_another_month_skips_months_that_lack_it() {
        assert_eq!(same_day_in_month(2026, 1, 31, 1), None);
        assert_eq!(same_day_in_month(2026, 1, 31, 2), Some(days_from_civil(2026, 3, 31)));
        assert_eq!(same_day_in_month(2026, 11, 15, 3), Some(days_from_civil(2027, 2, 15)));
        assert_eq!(same_day_in_month(2026, 3, 10, -4), Some(days_from_civil(2025, 11, 10)));
    }

    #[test]
    fn the_nth_weekday_of_a_month_counts_from_either_end() {
        // October 2026: Tuesdays are the 6th, 13th, 20th and 27th.
        assert_eq!(nth_weekday_of_month(2026, 10, 1, 2), Some(days_from_civil(2026, 10, 13)));
        assert_eq!(nth_weekday_of_month(2026, 10, 1, -1), Some(days_from_civil(2026, 10, 27)));
        assert_eq!(nth_weekday_of_month(2026, 10, 1, 5), None);
        assert_eq!(nth_weekday_of_month(2026, 10, 1, 0), None);
    }

    #[test]
    fn seconds_and_day_numbers_agree() {
        let seconds = seconds_from_civil(2026, 10, 24, 13, 30, 15);
        assert_eq!(day_of(seconds), days_from_civil(2026, 10, 24));
        assert_eq!(seconds.rem_euclid(SECONDS_PER_DAY), 13 * 3600 + 30 * 60 + 15);
        assert_eq!(day_of(-1), -1);
    }

    #[test]
    fn times_of_day_are_written_with_two_digits() {
        assert_eq!(clock_text(9 * 3600), "09:00");
        assert_eq!(clock_text(13 * 3600 + 30 * 60 + 59), "13:30");
    }
}
