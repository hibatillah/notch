//! The media player page: what is playing, with controls, a seek bar and synced lyrics.
//!
//! `session` mirrors the system media session into plain data; the other modules draw it or react to input.

mod app_icon;
mod artwork;
mod controls;
mod expanded;
mod layout;
mod lyric_follower;
mod lyric_panel;
mod lyrics;
mod resting;
mod seek;
mod session;
mod styles;

use controls::Control;
use layout::Layout;
use lyric_follower::LyricFollower;
use lyrics::LyricLine;
use session::{LyricsStatus, MediaInfo, MediaWatcher};

use crate::{
    gfx::{Canvas, ImageSlot, Size},
    log,
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    ui::Icon,
};

const SETTING_LYRICS: &str = "media.lyrics";
/// Repaints the open notch so the progress bar and time left keep moving.
const PROGRESS_TIMER: TimerId = TimerId(0);
const PROGRESS_INTERVAL_MILLISECONDS: u32 = 250;
/// Fires when the next lyric line starts.
const LYRIC_TIMER: TimerId = TimerId(1);

pub struct MediaPage {
    watcher: Option<MediaWatcher>,
    info: MediaInfo,
    artwork: ImageSlot,
    app_icon: ImageSlot,
    follower: LyricFollower,
    /// Whether the user wants lyrics shown; saved so it survives restarts.
    lyrics_enabled: bool,
    /// Measured widths in device-independent pixels, so the collapsed pill can size itself to the text.
    measured_title: String,
    title_width: f32,
    measured_artist: String,
    artist_width: f32,
    hovered_control: Option<Control>,
    /// Fraction under the pointer while the user drags the seek bar.
    seek_drag: Option<f32>,
    is_notch_open: bool,
    is_progress_timer_running: bool,
}

impl MediaPage {
    pub fn new() -> Self {
        Self {
            watcher: None,
            info: MediaInfo::default(),
            artwork: ImageSlot::default(),
            app_icon: ImageSlot::default(),
            follower: LyricFollower::new(),
            lyrics_enabled: true,
            measured_title: String::new(),
            title_width: 0.0,
            measured_artist: String::new(),
            artist_width: 0.0,
            hovered_control: None,
            seek_drag: None,
            is_notch_open: false,
            is_progress_timer_running: false,
        }
    }

    fn is_track_playing(&self) -> bool {
        self.info.is_playing && !self.info.title.is_empty()
    }

    /// The current track has synced lyrics, regardless of the user's on/off choice.
    fn has_lyrics(&self) -> bool {
        matches!(self.info.lyrics, LyricsStatus::Found(_))
    }

    fn is_lyric_panel_visible(&self) -> bool {
        self.lyrics_enabled && self.has_lyrics()
    }

    /// The collapsed pill shows the current lyric line only while a track plays and the user wants lyrics.
    fn is_lyric_mode(&self) -> bool {
        self.info.is_playing && self.is_lyric_panel_visible()
    }

    fn visible_lyric_lines(&self) -> Option<&[LyricLine]> {
        match &self.info.lyrics {
            LyricsStatus::Found(lyrics) if self.lyrics_enabled && self.info.available => Some(&lyrics.lines),
            _ => None,
        }
    }

    /// Takes the latest state from the media session and brings everything derived from it up to date.
    fn apply_snapshot(&mut self, context: &mut Context) {
        if let Some(watcher) = &self.watcher {
            self.info = watcher.snapshot();
        }
        self.artwork.sync(self.info.artwork.as_deref(), |image| context.upload_image(image));
        self.app_icon.sync(self.info.app_icon.as_deref(), |image| context.upload_image(image));
        self.measure_text(context);
        self.update_lyrics(context);
        self.update_progress_timer(context);
        context.request_repaint();
    }

    fn measure_text(&mut self, context: &Context) {
        if self.info.title != self.measured_title {
            self.measured_title = self.info.title.clone();
            self.title_width = context.fonts().measure(&styles::RESTING_TITLE, &self.measured_title);
        }
        if self.info.artist != self.measured_artist {
            self.measured_artist = self.info.artist.clone();
            self.artist_width = context.fonts().measure(&styles::RESTING_TITLE, &self.measured_artist);
        }
    }

    /// Follows the song: shows the line being sung and arms a timer for exactly when the next line starts.
    fn update_lyrics(&mut self, context: &mut Context) {
        let lyrics = match &self.info.lyrics {
            LyricsStatus::Found(lyrics) if self.lyrics_enabled => Some(lyrics.clone()),
            _ => None,
        };
        let update = self.follower.update(
            lyrics.as_ref(),
            self.info.position_ticks_now(),
            self.info.is_playing,
            context.fonts(),
        );
        if update.line_changed {
            log::debug(&format!(
                "lyric line index={:?} chars={}",
                self.follower.index(),
                self.follower.current().chars().count()
            ));
            context.request_animation();
        }
        match update.next_line_in_milliseconds {
            Some(milliseconds) => context.set_timer(LYRIC_TIMER, milliseconds),
            None => context.kill_timer(LYRIC_TIMER),
        }
    }

    /// Ticks only while the open notch shows a playing track, so the idle notch never wakes up.
    fn update_progress_timer(&mut self, context: &mut Context) {
        let needed = self.is_notch_open && self.info.is_playing && self.info.duration_ticks > 0;
        if needed == self.is_progress_timer_running {
            return;
        }
        self.is_progress_timer_running = needed;
        if needed {
            context.set_timer(PROGRESS_TIMER, PROGRESS_INTERVAL_MILLISECONDS);
        } else {
            context.kill_timer(PROGRESS_TIMER);
        }
    }

    fn toggle_lyrics(&mut self, context: &mut Context) {
        self.lyrics_enabled = !self.lyrics_enabled;
        context.settings.set_bool(SETTING_LYRICS, self.lyrics_enabled);
        self.update_lyrics(context);
        context.request_repaint();
    }

    fn activate(&mut self, control: Control, context: &mut Context) {
        if control == Control::Lyrics {
            self.toggle_lyrics(context);
            return;
        }
        let Some(watcher) = &self.watcher else { return };
        match control {
            Control::Lyrics => {}
            Control::Shuffle => watcher.toggle_shuffle(),
            Control::Previous => watcher.skip_previous(),
            Control::PlayPause => watcher.toggle_play_pause(),
            Control::Next => watcher.skip_next(),
            Control::Repeat => watcher.cycle_repeat(),
        }
    }
}

impl Page for MediaPage {
    fn id(&self) -> &'static str {
        "media"
    }

    fn name(&self) -> &'static str {
        "Media"
    }

    fn icon(&self) -> Icon {
        Icon::Music
    }

    fn priority(&self, mode: Mode) -> Option<Priority> {
        // Collapsed it only shows music that is playing; open it also stays up while paused, so playback can resume.
        let wanted = match mode {
            Mode::Resting => self.is_track_playing(),
            Mode::Expanded => self.info.available,
        };
        wanted.then_some(Priority::NORMAL)
    }

    fn sizes(&self) -> PageSizes {
        let lyric_width = self.is_lyric_mode().then(|| self.follower.width());
        let expanded_width = if self.is_lyric_panel_visible() { layout::LYRICS_WIDTH } else { layout::PLAIN_WIDTH };
        PageSizes {
            resting_width: resting::width(self.title_width, self.artist_width, lyric_width),
            expanded: Size::new(expanded_width, layout::EXPANDED_HEIGHT),
        }
    }

    fn draw_resting(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        resting::draw(self, canvas, frame, opacity);
    }

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        expanded::draw(self, canvas, frame, opacity);
    }

    fn on_start(&mut self, context: &mut Context) {
        self.lyrics_enabled = context.settings.get_bool(SETTING_LYRICS, true);
        let notifier = context.notifier();
        self.watcher = MediaWatcher::start(move || notifier.notify())
            .map_err(|error| log::debug(&format!("media watcher unavailable: {error}")))
            .ok();
        self.apply_snapshot(context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        self.apply_snapshot(context);
    }

    fn on_hover_changed(&mut self, is_hovering: bool, context: &mut Context) {
        self.is_notch_open = is_hovering;
        self.update_progress_timer(context);
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let layout = Layout::new(frame);
        let can_use_controls = frame.is_open() && self.info.available;
        match event {
            PointerEvent::Moved(point) => {
                if self.seek_drag.is_some() {
                    self.seek_drag = Some(layout.progress_bar().fraction_at(point.x));
                    context.request_repaint();
                }
                let hovered = if can_use_controls { controls::hit_test(&layout, &self.info, point) } else { None };
                if hovered != self.hovered_control {
                    self.hovered_control = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Pressed(point) => {
                if can_use_controls && seek::can_grab(&layout, &self.info, point) {
                    context.capture_pointer();
                    self.seek_drag = Some(layout.progress_bar().fraction_at(point.x));
                    context.request_repaint();
                }
            }
            PointerEvent::Released(point) => {
                if self.seek_drag.take().is_some() {
                    context.release_pointer();
                    let fraction = layout.progress_bar().fraction_at(point.x);
                    if let Some(watcher) = &self.watcher {
                        watcher.seek((fraction * self.info.duration_ticks as f32) as i64);
                    }
                    context.request_repaint();
                } else if can_use_controls && let Some(control) = controls::hit_test(&layout, &self.info, point) {
                    self.activate(control, context);
                }
            }
            PointerEvent::Left => {
                self.hovered_control = None;
                context.request_repaint();
            }
            PointerEvent::CaptureLost => {
                self.seek_drag = None;
                context.request_repaint();
            }
            PointerEvent::SecondaryReleased(_) => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        match timer {
            PROGRESS_TIMER => context.request_repaint(),
            LYRIC_TIMER => {
                self.update_lyrics(context);
                context.request_repaint();
            }
            _ => {}
        }
    }

    fn animate(&mut self, elapsed_seconds: f32) -> bool {
        self.follower.animate(elapsed_seconds)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::page::Priority;

    fn playing_page() -> MediaPage {
        let mut page = MediaPage::new();
        page.info.available = true;
        page.info.is_playing = true;
        page.info.title = "Song".to_owned();
        page
    }

    fn with_lyrics(mut page: MediaPage) -> MediaPage {
        let lines = vec![LyricLine { start_ticks: 0, text: "la".to_owned() }];
        page.info.lyrics = LyricsStatus::Found(Arc::new(lyrics::Lyrics { lines }));
        page
    }

    #[test]
    fn with_no_music_the_page_stays_out_of_the_way() {
        let page = MediaPage::new();
        assert_eq!(page.priority(Mode::Resting), None);
        assert_eq!(page.priority(Mode::Expanded), None);
    }

    #[test]
    fn playing_music_is_shown_collapsed_and_open() {
        let page = playing_page();
        assert_eq!(page.priority(Mode::Resting), Some(Priority::NORMAL));
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::NORMAL));
    }

    #[test]
    fn paused_music_leaves_the_collapsed_pill_but_stays_up_when_open() {
        let mut page = playing_page();
        page.info.is_playing = false;
        assert_eq!(page.priority(Mode::Resting), None);
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::NORMAL));
    }

    #[test]
    fn the_open_pill_widens_only_while_lyrics_are_shown() {
        let plain = playing_page().sizes().expanded.width;
        let mut page = with_lyrics(playing_page());
        assert_eq!(page.sizes().expanded.width, layout::LYRICS_WIDTH);
        assert!(page.sizes().expanded.width > plain);

        page.lyrics_enabled = false;
        assert_eq!(page.sizes().expanded.width, plain);
    }

    #[test]
    fn lyrics_that_were_not_found_leave_the_layout_alone() {
        let mut page = playing_page();
        page.info.lyrics = LyricsStatus::NotFound;
        assert!(!page.is_lyric_panel_visible());
        assert!(!page.is_lyric_mode());
        assert_eq!(page.sizes().expanded.width, layout::PLAIN_WIDTH);
    }

    #[test]
    fn the_collapsed_pill_shows_lyrics_only_while_playing() {
        let mut page = with_lyrics(playing_page());
        assert!(page.is_lyric_mode());
        page.info.is_playing = false;
        assert!(!page.is_lyric_mode());
        assert!(page.is_lyric_panel_visible(), "the open view still shows them while paused");
    }
}
