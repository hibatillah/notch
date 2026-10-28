//! Timer presentation rules, independent of drawing.

use super::session::{Phase, SESSIONS_PER_ROUND, State, Timer, clock_text, compact_text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Shorter,
    Longer,
    Pause,
    Resume,
    Extend,
    Stop,
    StartNext,
    Skip,
}

pub struct ButtonSpec {
    pub action: Action,
    pub label: String,
    pub is_primary: bool,
    pub is_destructive: bool,
}

pub struct View {
    pub title: String,
    pub detail: Option<String>,
    pub time: Option<String>,
    pub buttons: Vec<ButtonSpec>,
}

fn button(action: Action, label: &str, is_primary: bool) -> ButtonSpec {
    ButtonSpec { action, label: label.to_owned(), is_primary, is_destructive: action == Action::Stop }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Focus => "Focus",
        Phase::ShortBreak => "Break",
        Phase::LongBreak => "Long break",
    }
}

fn running_title(timer: &Timer, phase: Phase) -> String {
    match phase {
        Phase::Focus => format!("Focus · {} of {SESSIONS_PER_ROUND}", timer.session_number()),
        Phase::ShortBreak | Phase::LongBreak => phase_name(phase).to_owned(),
    }
}

fn start_button_label(next: Option<Phase>) -> &'static str {
    match next {
        Some(Phase::ShortBreak) => "Start break",
        Some(Phase::LongBreak) => "Start long break",
        Some(Phase::Focus) | None => "Start focus",
    }
}

pub fn view(timer: &Timer, now: u64) -> View {
    match timer.state {
        State::Idle => View {
            title: "Focus timer".to_owned(),
            detail: Some(format!("Session {} of {SESSIONS_PER_ROUND}", timer.session_number())),
            time: Some(clock_text(timer.lengths.focus * 60_000)),
            buttons: vec![
                button(Action::Shorter, "−5", false),
                button(Action::Longer, "+5", false),
                button(Action::Start, "Start", true),
            ],
        },
        State::Running { phase, .. } => View {
            title: running_title(timer, phase),
            detail: None,
            time: timer.remaining(now).map(clock_text),
            buttons: vec![
                button(Action::Pause, "Pause", false),
                button(Action::Extend, "+5 min", false),
                button(Action::Stop, "Stop", false),
            ],
        },
        State::Paused { phase, .. } => View {
            title: format!("{} · paused", running_title(timer, phase)),
            detail: None,
            time: timer.remaining(now).map(clock_text),
            buttons: vec![
                button(Action::Resume, "Resume", true),
                button(Action::Extend, "+5 min", false),
                button(Action::Stop, "Stop", false),
            ],
        },
        State::Finished { phase } => {
            let next = timer.next_phase();
            let detail = match (phase, next) {
                (Phase::Focus, Some(next)) => {
                    format!(
                        "{} min · {} min {} next",
                        timer.lengths.focus,
                        timer.lengths.minutes(next),
                        phase_name(next).to_lowercase()
                    )
                }
                (_, Some(_)) => format!("Ready for focus {} of {SESSIONS_PER_ROUND}", timer.session_number()),
                _ => String::new(),
            };
            View {
                title: match phase {
                    Phase::Focus => "Focus session done".to_owned(),
                    Phase::ShortBreak | Phase::LongBreak => format!("{} over", phase_name(phase)),
                },
                detail: Some(detail),
                time: None,
                buttons: vec![
                    button(Action::StartNext, start_button_label(next), true),
                    button(Action::Skip, "Skip", false),
                ],
            }
        }
    }
}

pub struct Compact {
    pub text: String,
    pub fraction: f32,
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Focus,
    Break,
    Paused,
}

pub fn compact(timer: &Timer, now: u64) -> Option<Compact> {
    let tone = match timer.state {
        State::Idle => return None,
        State::Paused { .. } => Tone::Paused,
        State::Running { phase: Phase::Focus, .. } => Tone::Focus,
        State::Running { .. } | State::Finished { .. } => Tone::Break,
    };
    let text = match timer.state {
        State::Finished { .. } => "Done".to_owned(),
        _ => compact_text(timer.remaining(now)?),
    };
    Some(Compact { text, fraction: timer.elapsed_fraction(now), tone })
}

pub fn widest_form(text: &str) -> String {
    text.chars().map(|character| if character.is_ascii_digit() { '0' } else { character }).collect()
}

pub fn alert(timer: &Timer) -> Option<(String, String)> {
    let State::Finished { phase } = timer.state else { return None };
    Some(match phase {
        Phase::Focus => ("Focus session done".to_owned(), format!("{} min", timer.lengths.focus)),
        Phase::ShortBreak | Phase::LongBreak => (format!("{} over", phase_name(phase)), "back to focus".to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: u64 = 60_000;
    const NOW: u64 = 1_000_000_000_000;

    fn actions(view: &View) -> Vec<Action> {
        view.buttons.iter().map(|button| button.action).collect()
    }

    fn running() -> Timer {
        let mut timer = Timer::new();
        timer.start_focus(NOW);
        timer
    }

    #[test]
    fn idle_offers_a_length_and_a_start() {
        let view = view(&Timer::new(), NOW);
        assert_eq!(view.title, "Focus timer");
        assert_eq!(view.time.as_deref(), Some("25:00"));
        assert_eq!(actions(&view), vec![Action::Shorter, Action::Longer, Action::Start]);
        assert!(view.buttons[2].is_primary);
        assert!(!view.buttons[2].is_destructive);
    }

    #[test]
    fn a_running_session_shows_its_number_and_the_time_left() {
        let view = view(&running(), NOW + 60_000 + 13_000);
        assert_eq!(view.title, "Focus · 1 of 4");
        assert_eq!(view.time.as_deref(), Some("23:47"));
        assert_eq!(actions(&view), vec![Action::Pause, Action::Extend, Action::Stop]);
    }

    #[test]
    fn only_the_stop_button_is_red() {
        let view = view(&running(), NOW + 1_000);
        let red: Vec<Action> = view.buttons.iter().filter(|spec| spec.is_destructive).map(|spec| spec.action).collect();
        assert_eq!(red, vec![Action::Stop]);
    }

    #[test]
    fn a_paused_session_offers_resume_first() {
        let mut timer = running();
        timer.pause(NOW);
        let view = view(&timer, NOW + 5 * MINUTE);
        assert_eq!(view.title, "Focus · 1 of 4 · paused");
        assert_eq!(view.time.as_deref(), Some("25:00"));
        assert_eq!(actions(&view), vec![Action::Resume, Action::Extend, Action::Stop]);
    }

    #[test]
    fn a_finished_focus_session_offers_the_break() {
        let mut timer = running();
        timer.tick(NOW + 25 * MINUTE);
        let view = view(&timer, NOW + 26 * MINUTE);
        assert_eq!(view.title, "Focus session done");
        assert_eq!(view.detail.as_deref(), Some("25 min · 5 min break next"));
        assert_eq!(view.buttons[0].label, "Start break");
        assert_eq!(actions(&view), vec![Action::StartNext, Action::Skip]);
        assert!(view.time.is_none());
    }

    #[test]
    fn a_finished_break_offers_the_next_focus_session() {
        let mut timer = running();
        timer.tick(NOW + 25 * MINUTE);
        timer.start_next(NOW + 25 * MINUTE);
        timer.tick(NOW + 30 * MINUTE);
        let view = view(&timer, NOW + 31 * MINUTE);
        assert_eq!(view.title, "Break over");
        assert_eq!(view.detail.as_deref(), Some("Ready for focus 2 of 4"));
        assert_eq!(view.buttons[0].label, "Start focus");
    }

    #[test]
    fn the_compact_view_counts_minutes_then_seconds() {
        let timer = running();
        assert_eq!(compact(&timer, NOW + MINUTE + 13_000).unwrap().text, "23m");
        assert_eq!(compact(&timer, NOW + 21 * MINUTE).unwrap().text, "4:00");
    }

    #[test]
    fn the_compact_view_is_toned_by_what_is_going_on() {
        let mut timer = running();
        assert_eq!(compact(&timer, NOW).unwrap().tone, Tone::Focus);
        timer.pause(NOW);
        assert_eq!(compact(&timer, NOW).unwrap().tone, Tone::Paused);
        assert!(compact(&Timer::new(), NOW).is_none());
    }

    #[test]
    fn a_finished_phase_shows_done_in_the_compact_view() {
        let mut timer = running();
        timer.tick(NOW + 25 * MINUTE);
        assert_eq!(compact(&timer, NOW + 26 * MINUTE).unwrap().text, "Done");
    }

    #[test]
    fn the_widest_form_keeps_the_shape_and_drops_the_digits() {
        assert_eq!(widest_form("24m"), "00m");
        assert_eq!(widest_form("4:12"), "0:00");
    }

    #[test]
    fn the_alert_says_what_ended() {
        let mut timer = running();
        assert!(alert(&timer).is_none());
        timer.tick(NOW + 25 * MINUTE);
        assert_eq!(alert(&timer), Some(("Focus session done".to_owned(), "25 min".to_owned())));
    }
}
