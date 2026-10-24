use std::sync::Arc;

use super::{lyrics::Lyrics, styles};
use crate::gfx::{Fonts, Spring};

/// Lines are shown slightly early to offset timer and render latency.
const LEAD_TICKS: i64 = 2_000_000;
const TICKS_PER_MILLISECOND: i64 = 10_000;
const MIN_DELAY_MILLISECONDS: i64 = 15;
const MAX_DELAY_MILLISECONDS: i64 = 30_000;

/// What `LyricFollower::update` found out.
#[derive(Default)]
pub struct Update {
    /// The line being sung changed, so a transition started and the page should animate.
    pub line_changed: bool,
    /// Time until the next line starts, for arming a timer; `None` while paused or after the last line.
    pub next_line_in_milliseconds: Option<u32>,
}

/// Tracks which lyric line is being sung and animates the change from one line to the next.
pub struct LyricFollower {
    /// Address of the lyrics being followed, to notice when a different track's lyrics arrive.
    identity: usize,
    index: Option<usize>,
    current: String,
    previous: String,
    /// Width of the current line in device-independent pixels.
    width: f32,
    /// 0 while the previous line is shown, 1 once the current line has taken over.
    transition: Spring,
}

impl LyricFollower {
    pub fn new() -> Self {
        Self {
            identity: 0,
            index: None,
            current: String::new(),
            previous: String::new(),
            width: 0.0,
            transition: Spring::new(1.0, 300.0, 30.0),
        }
    }

    pub fn index(&self) -> Option<usize> {
        self.index
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn previous(&self) -> &str {
        &self.previous
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn transition(&self) -> f32 {
        self.transition.position
    }

    /// Moves to the line sung at `position_ticks`; with no lyrics it clears everything.
    pub fn update(
        &mut self,
        lyrics: Option<&Arc<Lyrics>>,
        position_ticks: i64,
        is_playing: bool,
        fonts: &Fonts,
    ) -> Update {
        let Some(lyrics) = lyrics else {
            self.clear();
            return Update::default();
        };
        let identity = Arc::as_ptr(lyrics) as usize;
        if identity != self.identity {
            self.identity = identity;
            self.clear();
        }

        let position = position_ticks + LEAD_TICKS;
        let index = lyrics.index_at(position);
        let mut line_changed = false;
        if index != self.index {
            self.previous = std::mem::take(&mut self.current);
            self.current = index.map(|line| lyrics.lines[line].text.clone()).unwrap_or_default();
            self.index = index;
            self.width = fonts.measure(&styles::RESTING_LYRIC, &self.current);
            self.transition.jump_to(0.0);
            self.transition.target = 1.0;
            line_changed = true;
        }

        // A paused track keeps its line on screen but needs no timer.
        let next_line_in_milliseconds = is_playing.then(|| lyrics.next_start_after(index)).flatten().map(|start| {
            ((start - position) / TICKS_PER_MILLISECOND).clamp(MIN_DELAY_MILLISECONDS, MAX_DELAY_MILLISECONDS) as u32
        });
        Update { line_changed, next_line_in_milliseconds }
    }

    /// Advances the line transition; returns true while it is still moving.
    pub fn animate(&mut self, elapsed_seconds: f32) -> bool {
        self.transition.step(elapsed_seconds);
        let is_moving = !self.transition.is_settled();
        if !is_moving {
            self.transition.snap_to_target();
        }
        is_moving
    }

    fn clear(&mut self) {
        self.index = None;
        self.current.clear();
        self.previous.clear();
        self.width = 0.0;
        self.transition.jump_to(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::media::lyrics::LyricLine;

    const SECOND: i64 = 10_000_000;

    fn fonts() -> Fonts {
        Fonts::new(1.0).expect("DirectWrite is available on Windows")
    }

    fn lyrics() -> Arc<Lyrics> {
        let lines = [10, 20, 30]
            .into_iter()
            .map(|second| LyricLine { start_ticks: second * SECOND, text: format!("line {second}") })
            .collect();
        Arc::new(Lyrics { lines })
    }

    #[test]
    fn before_the_first_line_nothing_is_shown_and_a_timer_waits_for_it() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        let update = follower.update(Some(&lyrics), 0, true, &fonts);
        assert_eq!(follower.index(), None);
        assert!(!update.line_changed);
        // The line is shown 0.2 s early, so 10 s minus 0.2 s of waiting.
        assert_eq!(update.next_line_in_milliseconds, Some(9_800));
    }

    #[test]
    fn reaching_a_line_shows_it_and_remembers_the_previous_one() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        assert!(follower.update(Some(&lyrics), 10 * SECOND, true, &fonts).line_changed);
        assert_eq!(follower.current(), "line 10");
        assert!(follower.width() > 0.0);

        assert!(follower.update(Some(&lyrics), 21 * SECOND, true, &fonts).line_changed);
        assert_eq!((follower.previous(), follower.current()), ("line 10", "line 20"));
        assert_eq!(follower.index(), Some(1));
    }

    #[test]
    fn staying_on_the_same_line_changes_nothing() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        follower.update(Some(&lyrics), 12 * SECOND, true, &fonts);
        assert!(!follower.update(Some(&lyrics), 15 * SECOND, true, &fonts).line_changed);
    }

    #[test]
    fn a_paused_track_keeps_its_line_but_arms_no_timer() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        let update = follower.update(Some(&lyrics), 12 * SECOND, false, &fonts);
        assert_eq!(follower.current(), "line 10");
        assert_eq!(update.next_line_in_milliseconds, None);
    }

    #[test]
    fn after_the_last_line_no_timer_is_needed() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        let update = follower.update(Some(&lyrics), 99 * SECOND, true, &fonts);
        assert_eq!(update.next_line_in_milliseconds, None);
    }

    #[test]
    fn without_lyrics_everything_is_cleared() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        follower.update(Some(&lyrics), 12 * SECOND, true, &fonts);
        follower.update(None, 12 * SECOND, true, &fonts);
        assert_eq!((follower.index(), follower.current(), follower.width()), (None, "", 0.0));
    }

    #[test]
    fn the_line_transition_runs_and_then_stops() {
        let (mut follower, lyrics, fonts) = (LyricFollower::new(), lyrics(), fonts());
        follower.update(Some(&lyrics), 12 * SECOND, true, &fonts);
        assert!(follower.animate(1.0 / 60.0), "a fresh transition is still moving");
        let mut frames = 0;
        while follower.animate(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 300, "the transition never settled");
        }
        assert_eq!(follower.transition(), 1.0);
    }
}
