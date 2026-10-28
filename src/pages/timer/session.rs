//! Phase times use Unix milliseconds so sessions survive restarts.

use crate::host::Settings;

pub const SESSIONS_PER_ROUND: u32 = 4;
pub const EXTENSION_MINUTES: u64 = 5;
pub const LENGTH_STEP_MINUTES: u64 = 5;
const MIN_FOCUS_MINUTES: u64 = 5;
const MAX_FOCUS_MINUTES: u64 = 120;
const MILLISECONDS_PER_MINUTE: u64 = 60_000;
const MILLISECONDS_PER_SECOND: u64 = 1000;
pub const SECONDS_SHOWN_BELOW_MINUTES: u64 = 10;
/// Wake a little after a boundary so the clock has surely crossed it.
const WAKE_SLACK_MILLISECONDS: u64 = 20;

const STATE_KEY: &str = "timer.state";
const COMPLETED_KEY: &str = "timer.completed";
const FOCUS_MINUTES_KEY: &str = "timer.focus_minutes";
const SHORT_BREAK_MINUTES_KEY: &str = "timer.short_break_minutes";
const LONG_BREAK_MINUTES_KEY: &str = "timer.long_break_minutes";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Focus => "focus",
            Phase::ShortBreak => "short",
            Phase::LongBreak => "long",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "focus" => Some(Phase::Focus),
            "short" => Some(Phase::ShortBreak),
            "long" => Some(Phase::LongBreak),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    /// `total` is the length of the phase so far, extensions included, in milliseconds.
    Running {
        phase: Phase,
        total: u64,
        ends_at: u64,
    },
    Paused {
        phase: Phase,
        total: u64,
        remaining: u64,
    },
    Finished {
        phase: Phase,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lengths {
    pub focus: u64,
    pub short_break: u64,
    pub long_break: u64,
}

impl Default for Lengths {
    fn default() -> Self {
        Self { focus: 25, short_break: 5, long_break: 15 }
    }
}

impl Lengths {
    pub fn minutes(&self, phase: Phase) -> u64 {
        match phase {
            Phase::Focus => self.focus,
            Phase::ShortBreak => self.short_break,
            Phase::LongBreak => self.long_break,
        }
    }
}

pub struct Timer {
    pub state: State,
    pub completed: u32,
    pub lengths: Lengths,
}

impl Timer {
    pub fn new() -> Self {
        Self { state: State::Idle, completed: 0, lengths: Lengths::default() }
    }

    pub fn session_number(&self) -> u32 {
        self.completed % SESSIONS_PER_ROUND + 1
    }

    pub fn break_after_focus(&self) -> Phase {
        let into_round = self.completed % SESSIONS_PER_ROUND;
        if self.completed > 0 && into_round == 0 { Phase::LongBreak } else { Phase::ShortBreak }
    }

    pub fn next_phase(&self) -> Option<Phase> {
        match self.state {
            State::Finished { phase: Phase::Focus } => Some(self.break_after_focus()),
            State::Finished { .. } => Some(Phase::Focus),
            _ => None,
        }
    }

    pub fn start_focus(&mut self, now: u64) {
        if self.state == State::Idle {
            self.begin(Phase::Focus, now);
        }
    }

    pub fn start_next(&mut self, now: u64) {
        if let Some(phase) = self.next_phase() {
            self.begin(phase, now);
        }
    }

    pub fn skip(&mut self) {
        if matches!(self.state, State::Finished { .. }) {
            self.state = State::Idle;
        }
    }

    pub fn pause(&mut self, now: u64) {
        if let State::Running { phase, total, ends_at } = self.state {
            self.state = State::Paused { phase, total, remaining: ends_at.saturating_sub(now) };
        }
    }

    pub fn resume(&mut self, now: u64) {
        if let State::Paused { phase, total, remaining } = self.state {
            self.state = State::Running { phase, total, ends_at: now + remaining };
        }
    }

    pub fn extend(&mut self) {
        let extra = EXTENSION_MINUTES * MILLISECONDS_PER_MINUTE;
        match &mut self.state {
            State::Running { total, ends_at, .. } => {
                *total += extra;
                *ends_at += extra;
            }
            State::Paused { total, remaining, .. } => {
                *total += extra;
                *remaining += extra;
            }
            _ => {}
        }
    }

    /// Abandons a running or paused phase; an unfinished focus session does not count.
    pub fn stop(&mut self) {
        if matches!(self.state, State::Running { .. } | State::Paused { .. }) {
            self.state = State::Idle;
        }
    }

    pub fn adjust_focus_length(&mut self, steps: i64) {
        if self.state != State::Idle {
            return;
        }
        let minutes = self.lengths.focus as i64 + steps * LENGTH_STEP_MINUTES as i64;
        self.lengths.focus = minutes.clamp(MIN_FOCUS_MINUTES as i64, MAX_FOCUS_MINUTES as i64) as u64;
    }

    pub fn tick(&mut self, now: u64) -> bool {
        let State::Running { phase, ends_at, .. } = self.state else { return false };
        if now < ends_at {
            return false;
        }
        if phase == Phase::Focus {
            self.completed += 1;
        }
        self.state = State::Finished { phase };
        true
    }

    pub fn remaining(&self, now: u64) -> Option<u64> {
        match self.state {
            State::Running { ends_at, .. } => Some(ends_at.saturating_sub(now)),
            State::Paused { remaining, .. } => Some(remaining),
            _ => None,
        }
    }

    pub fn elapsed_fraction(&self, now: u64) -> f32 {
        let (total, remaining) = match self.state {
            State::Running { total, ends_at, .. } => (total, ends_at.saturating_sub(now)),
            State::Paused { total, remaining, .. } => (total, remaining),
            State::Finished { .. } => return 1.0,
            State::Idle => return 0.0,
        };
        if total == 0 { 1.0 } else { (1.0 - remaining as f32 / total as f32).clamp(0.0, 1.0) }
    }

    pub fn phase(&self) -> Option<Phase> {
        match self.state {
            State::Running { phase, .. } | State::Paused { phase, .. } | State::Finished { phase } => Some(phase),
            State::Idle => None,
        }
    }

    fn begin(&mut self, phase: Phase, now: u64) {
        let total = self.lengths.minutes(phase) * MILLISECONDS_PER_MINUTE;
        self.state = State::Running { phase, total, ends_at: now + total };
        if phase == Phase::Focus && self.completed >= SESSIONS_PER_ROUND {
            self.completed = 0;
        }
    }

    // Expired sessions restore as finished.

    /// Restores the timer saved by `save`; a phase that ran out while the notch was closed comes back finished.
    pub fn load(settings: &Settings, now: u64) -> Self {
        let defaults = Lengths::default();
        let read = |key: &str, default: u64| {
            settings
                .get_string(key)
                .and_then(|text| text.parse().ok())
                .filter(|minutes| *minutes > 0)
                .unwrap_or(default)
        };
        let mut timer = Self {
            state: settings.get_string(STATE_KEY).and_then(parse_state).unwrap_or(State::Idle),
            completed: settings.get_string(COMPLETED_KEY).and_then(|text| text.parse().ok()).unwrap_or(0),
            lengths: Lengths {
                focus: read(FOCUS_MINUTES_KEY, defaults.focus).clamp(MIN_FOCUS_MINUTES, MAX_FOCUS_MINUTES),
                short_break: read(SHORT_BREAK_MINUTES_KEY, defaults.short_break),
                long_break: read(LONG_BREAK_MINUTES_KEY, defaults.long_break),
            },
        };
        timer.tick(now);
        timer
    }

    pub fn save(&self, settings: &mut Settings) {
        settings.set_string(STATE_KEY, &format_state(self.state));
        settings.set_string(COMPLETED_KEY, &self.completed.to_string());
        settings.set_string(FOCUS_MINUTES_KEY, &self.lengths.focus.to_string());
    }
}

fn format_state(state: State) -> String {
    match state {
        State::Idle => "idle".to_owned(),
        State::Running { phase, total, ends_at } => format!("running {} {total} {ends_at}", phase.name()),
        State::Paused { phase, total, remaining } => format!("paused {} {total} {remaining}", phase.name()),
        State::Finished { phase } => format!("finished {}", phase.name()),
    }
}

fn parse_state(text: &str) -> Option<State> {
    let mut words = text.split_whitespace();
    let kind = words.next()?;
    if kind == "idle" {
        return Some(State::Idle);
    }
    let phase = Phase::from_name(words.next()?)?;
    if kind == "finished" {
        return Some(State::Finished { phase });
    }
    let total: u64 = words.next()?.parse().ok()?;
    let time: u64 = words.next()?.parse().ok()?;
    match kind {
        "running" => Some(State::Running { phase, total, ends_at: time }),
        "paused" => Some(State::Paused { phase, total, remaining: time }),
        _ => None,
    }
}

// Compact timers switch to seconds in the last ten minutes.

pub fn compact_text(remaining: u64) -> String {
    if remaining > SECONDS_SHOWN_BELOW_MINUTES * MILLISECONDS_PER_MINUTE {
        format!("{}m", remaining / MILLISECONDS_PER_MINUTE)
    } else {
        clock_text(remaining)
    }
}

/// The time left as "m:ss", counting a started second as a whole one so zero only shows when it is over.
pub fn clock_text(remaining: u64) -> String {
    let seconds = remaining.div_ceil(MILLISECONDS_PER_SECOND);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Milliseconds until the displayed time changes or the phase ends; open views update each second.
pub fn next_wake(remaining: u64, is_open: bool) -> u64 {
    let counts_seconds = is_open || remaining <= SECONDS_SHOWN_BELOW_MINUTES * MILLISECONDS_PER_MINUTE;
    let unit = if counts_seconds { MILLISECONDS_PER_SECOND } else { MILLISECONDS_PER_MINUTE };
    let past_boundary = remaining % unit;
    let until_boundary = if past_boundary == 0 { unit } else { past_boundary };
    until_boundary.min(remaining) + WAKE_SLACK_MILLISECONDS
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = MILLISECONDS_PER_MINUTE;
    const NOW: u64 = 1_000_000_000_000;

    fn running() -> Timer {
        let mut timer = Timer::new();
        timer.start_focus(NOW);
        timer
    }

    #[test]
    fn a_focus_session_runs_for_its_length() {
        let timer = running();
        assert_eq!(timer.state, State::Running { phase: Phase::Focus, total: 25 * MINUTE, ends_at: NOW + 25 * MINUTE });
        assert_eq!(timer.remaining(NOW + 5 * MINUTE), Some(20 * MINUTE));
    }

    #[test]
    fn starting_only_works_from_idle() {
        let mut timer = running();
        timer.start_focus(NOW + MINUTE);
        assert_eq!(timer.remaining(NOW + MINUTE), Some(24 * MINUTE));
    }

    #[test]
    fn pausing_freezes_the_time_and_resuming_continues_from_it() {
        let mut timer = running();
        timer.pause(NOW + 10 * MINUTE);
        assert_eq!(timer.remaining(NOW + 99 * MINUTE), Some(15 * MINUTE));
        timer.resume(NOW + 30 * MINUTE);
        assert_eq!(timer.remaining(NOW + 30 * MINUTE), Some(15 * MINUTE));
        assert!(!timer.tick(NOW + 44 * MINUTE));
        assert!(timer.tick(NOW + 45 * MINUTE));
    }

    #[test]
    fn extending_adds_five_minutes_to_a_running_or_paused_phase() {
        let mut timer = running();
        timer.extend();
        assert_eq!(timer.remaining(NOW), Some(30 * MINUTE));
        timer.pause(NOW);
        timer.extend();
        assert_eq!(timer.remaining(NOW), Some(35 * MINUTE));
    }

    #[test]
    fn extending_keeps_the_ring_honest() {
        let mut timer = running();
        timer.extend();
        let fraction = timer.elapsed_fraction(NOW + 15 * MINUTE);
        assert!((fraction - 0.5).abs() < 0.001, "{fraction}");
    }

    #[test]
    fn stopping_returns_to_idle_without_counting_the_session() {
        let mut timer = running();
        timer.stop();
        assert_eq!(timer.state, State::Idle);
        assert_eq!(timer.completed, 0);
    }

    #[test]
    fn running_out_finishes_the_phase_and_counts_a_focus_session() {
        let mut timer = running();
        assert!(!timer.tick(NOW + 25 * MINUTE - 1));
        assert!(timer.tick(NOW + 25 * MINUTE));
        assert_eq!(timer.state, State::Finished { phase: Phase::Focus });
        assert_eq!(timer.completed, 1);
        assert!(!timer.tick(NOW + 26 * MINUTE));
    }

    #[test]
    fn a_short_break_follows_a_focus_session_and_focus_follows_the_break() {
        let mut timer = running();
        timer.tick(NOW + 25 * MINUTE);
        assert_eq!(timer.next_phase(), Some(Phase::ShortBreak));
        timer.start_next(NOW + 26 * MINUTE);
        assert_eq!(timer.phase(), Some(Phase::ShortBreak));
        assert_eq!(timer.remaining(NOW + 26 * MINUTE), Some(5 * MINUTE));
        timer.tick(NOW + 31 * MINUTE);
        assert_eq!(timer.completed, 1);
        assert_eq!(timer.next_phase(), Some(Phase::Focus));
    }

    #[test]
    fn the_fourth_focus_session_earns_the_long_break_and_then_a_new_round_starts() {
        let mut timer = Timer::new();
        let mut now = NOW;
        for session in 1..=SESSIONS_PER_ROUND {
            assert_eq!(timer.session_number(), session);
            timer.start_focus(now);
            now += 25 * MINUTE;
            timer.tick(now);
            if session < SESSIONS_PER_ROUND {
                timer.start_next(now);
                now += 5 * MINUTE;
                timer.tick(now);
                timer.start_next(now);
                timer.stop();
            }
        }
        assert_eq!(timer.next_phase(), Some(Phase::LongBreak));
        timer.start_next(now);
        assert_eq!(timer.remaining(now), Some(15 * MINUTE));
        now += 15 * MINUTE;
        timer.tick(now);
        timer.start_next(now);
        assert_eq!(timer.completed, 0);
        assert_eq!(timer.session_number(), 1);
    }

    #[test]
    fn skipping_a_finished_phase_goes_back_to_idle() {
        let mut timer = running();
        timer.tick(NOW + 25 * MINUTE);
        timer.skip();
        assert_eq!(timer.state, State::Idle);
        assert_eq!(timer.completed, 1);
    }

    #[test]
    fn the_length_changes_in_steps_inside_its_limits_and_only_while_idle() {
        let mut timer = Timer::new();
        timer.adjust_focus_length(1);
        assert_eq!(timer.lengths.focus, 30);
        timer.adjust_focus_length(-100);
        assert_eq!(timer.lengths.focus, MIN_FOCUS_MINUTES);
        timer.adjust_focus_length(1000);
        assert_eq!(timer.lengths.focus, MAX_FOCUS_MINUTES);
        timer.adjust_focus_length(-1);
        timer.start_focus(NOW);
        timer.adjust_focus_length(1);
        assert_eq!(timer.lengths.focus, MAX_FOCUS_MINUTES - 5);
    }

    #[test]
    fn the_ring_fills_as_time_passes() {
        let timer = running();
        assert_eq!(timer.elapsed_fraction(NOW), 0.0);
        assert!((timer.elapsed_fraction(NOW + 5 * MINUTE) - 0.2).abs() < 0.001);
        assert_eq!(Timer::new().elapsed_fraction(NOW), 0.0);
    }

    #[test]
    fn compact_text_shows_minutes_above_ten_and_seconds_below() {
        assert_eq!(compact_text(24 * MINUTE + 13_000), "24m");
        assert_eq!(compact_text(25 * MINUTE), "25m");
        assert_eq!(compact_text(10 * MINUTE + 1), "10m");
        assert_eq!(compact_text(10 * MINUTE), "10:00");
        assert_eq!(compact_text(4 * MINUTE + 12_000), "4:12");
    }

    #[test]
    fn a_started_second_counts_as_a_whole_one() {
        assert_eq!(clock_text(1), "0:01");
        assert_eq!(clock_text(59_001), "1:00");
        assert_eq!(clock_text(0), "0:00");
    }

    #[test]
    fn the_wake_comes_when_the_text_changes() {
        assert_eq!(next_wake(24 * MINUTE + 13_000, false), 13_000 + WAKE_SLACK_MILLISECONDS);
        assert_eq!(next_wake(25 * MINUTE, false), MINUTE + WAKE_SLACK_MILLISECONDS);
        assert_eq!(next_wake(4 * MINUTE + 12_300, false), 300 + WAKE_SLACK_MILLISECONDS);
        assert_eq!(next_wake(24 * MINUTE + 13_300, true), 300 + WAKE_SLACK_MILLISECONDS);
    }

    #[test]
    fn the_last_wake_is_when_the_phase_ends() {
        assert_eq!(next_wake(400, false), 400 + WAKE_SLACK_MILLISECONDS);
        assert_eq!(next_wake(1000, false), 1000 + WAKE_SLACK_MILLISECONDS);
    }

    #[test]
    fn states_survive_saving_and_loading() {
        let states = [
            State::Idle,
            State::Running { phase: Phase::Focus, total: 1_500_000, ends_at: NOW + 700_000 },
            State::Paused { phase: Phase::ShortBreak, total: 300_000, remaining: 120_000 },
            State::Finished { phase: Phase::LongBreak },
        ];
        for state in states {
            assert_eq!(parse_state(&format_state(state)), Some(state), "{state:?}");
        }
    }

    #[test]
    fn unreadable_saved_states_are_ignored() {
        for text in ["", "running", "running focus x 1", "paused nap 1 2", "flying focus 1 2", "finished"] {
            assert_eq!(parse_state(text), None, "{text:?}");
        }
    }

    #[test]
    fn a_saved_timer_comes_back_and_one_that_ran_out_meanwhile_comes_back_finished() {
        let path = std::env::temp_dir().join(format!("notch-test-{}-timer", std::process::id())).join("settings.txt");
        let mut settings = Settings::at(Some(path.clone()));
        let mut timer = running();
        timer.completed = 2;
        timer.save(&mut settings);

        let restored = Timer::load(&Settings::at(Some(path.clone())), NOW + 10 * MINUTE);
        assert_eq!(restored.state, timer.state);
        assert_eq!(restored.completed, 2);

        let late = Timer::load(&Settings::at(Some(path.clone())), NOW + 40 * MINUTE);
        assert_eq!(late.state, State::Finished { phase: Phase::Focus });
        assert_eq!(late.completed, 3);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_focus_length_is_saved_and_other_lengths_can_be_set_by_hand() {
        let path = std::env::temp_dir().join(format!("notch-test-{}-lengths", std::process::id())).join("settings.txt");
        let mut settings = Settings::at(Some(path.clone()));
        settings.set_string(SHORT_BREAK_MINUTES_KEY, "7");
        let mut timer = Timer::new();
        timer.adjust_focus_length(1);
        timer.save(&mut settings);
        let restored = Timer::load(&Settings::at(Some(path.clone())), NOW);
        assert_eq!(restored.lengths, Lengths { focus: 30, short_break: 7, long_break: 15 });
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
