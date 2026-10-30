//! Media state comes from session; no progress bar means no per-second redraw while open.

mod artwork;
mod backdrop;
mod controls;
mod expanded;
mod layout;
mod lyric_follower;
mod lyric_panel;
mod lyrics;
mod resting;
mod session;
mod styles;
mod theme;
mod wave;

use controls::Control;
use layout::Layout;
use lyric_follower::LyricFollower;
use lyrics::LyricLine;
use session::{LyricsStatus, MediaInfo, MediaWatcher};

use crate::{
    gfx::{Canvas, Color, Fonts, ImageSlot, Rect, Size, Spring, balance_lines, clamp_lines},
    log,
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    ui::Icon,
};

const SETTING_LYRICS: &str = "media.lyrics";
const LYRIC_TIMER: TimerId = TimerId(1);
const TITLE_TIMER: TimerId = TimerId(2);
const WAVE_TIMER: TimerId = TimerId(3);
const WAVE_FRAME_MILLISECONDS: u32 = 100;
const TITLE_SHOWN_MILLISECONDS: u32 = 3000;
const UNKNOWN_TITLE: &str = "Unknown title";
const NOTHING_PLAYING_TITLE: &str = "Play a song";
const NOTHING_PLAYING_DETAIL: &str = "Nothing is playing";
/// Title lines are fitted a little narrower than their rectangle, so rounding never trims a line that fits.
const TITLE_FIT_MARGIN: f32 = 1.0;
const LYRIC_FIT_MARGIN: f32 = 2.0;

pub struct MediaPage {
    watcher: Option<MediaWatcher>,
    info: MediaInfo,
    artwork: ImageSlot,
    app_icon: ImageSlot,
    follower: LyricFollower,
    lyrics_enabled: bool,
    measured_title: String,
    title_width: f32,
    measured_artist: String,
    artist_width: f32,
    hovered_control: Option<Control>,
    is_hovered: bool,
    theme_color: Option<Color>,
    theme_version: u64,
    backdrop: Spring,
    is_title_shown: bool,
    /// Rebuild title lines only when the title, artist, or text-column width changes.
    title_lines: Vec<String>,
    title_lines_source: (String, String, bool),
    lyric_wraps: Vec<Vec<String>>,
    lyric_wraps_source: usize,
    text_width: f32,
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
            is_hovered: false,
            theme_color: None,
            theme_version: 0,
            backdrop: Spring::new(0.0, 180.0, 26.0),
            is_title_shown: false,
            title_lines: Vec::new(),
            title_lines_source: (String::new(), String::new(), false),
            lyric_wraps: Vec::new(),
            lyric_wraps_source: 0,
            text_width: layout::TEXT_MIN_WIDTH,
        }
    }

    #[cfg(test)]
    pub fn sample_idle(fonts: &crate::gfx::Fonts) -> Self {
        let mut page = Self::new();
        page.refresh_title_lines(fonts);
        page
    }

    #[cfg(test)]
    pub fn sample(fonts: &crate::gfx::Fonts, lyrics_on: bool, show_title: bool) -> Self {
        let mut page = Self::new();
        page.info.available = true;
        page.info.is_playing = true;
        page.info.title = "Location Unknown".to_owned();
        page.info.artist = "HONNE, NIKI".to_owned();
        page.info.album = "10 Years".to_owned();
        let texts = [
            "Hold on to what you had",
            "I will find you in the dark",
            "Wherever you are, wherever you go",
            "Take me to the place you know",
            "And I will stay",
        ];
        let lines: Vec<LyricLine> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| LyricLine { start_ticks: index as i64 * 50_000_000, text: (*text).to_owned() })
            .collect();
        let lyrics = std::sync::Arc::new(lyrics::Lyrics { lines });
        page.info.lyrics = LyricsStatus::Found(lyrics.clone());
        page.lyrics_enabled = lyrics_on;
        page.refresh_lyric_wraps(Some(&lyrics), fonts);
        if lyrics_on {
            page.theme_color = Some(Color::rgb(0.2, 0.65, 0.75));
            page.backdrop.jump_to(1.0);
        }
        page.measured_title = page.info.title.clone();
        page.title_width = fonts.measure(&styles::COMPACT_TITLE, &page.info.title);
        page.measured_artist = page.info.artist.clone();
        page.artist_width = fonts.measure(&styles::COMPACT_ARTIST, &page.info.artist);
        if lyrics_on {
            page.follower.update(Some(&lyrics), 2 * 50_000_000 + 10_000_000, true, fonts);
            page.follower.animate(5.0);
        }
        page.is_title_shown = show_title;
        page.refresh_title_lines(fonts);
        page
    }

    fn is_wave_shown(&self) -> bool {
        self.is_track_playing()
    }

    fn is_wave_visible(&self) -> bool {
        self.is_wave_shown() && (self.is_hovered || self.resting_view() == resting::View::Wave)
    }

    fn sync_backdrop(&mut self, context: &mut Context) {
        let wanted = if self.is_lyric_panel_visible() && self.theme_color.is_some() { 1.0 } else { 0.0 };
        if self.backdrop.target != wanted {
            self.backdrop.target = wanted;
            context.request_animation();
        }
    }

    fn sync_wave_timer(&self, context: &mut Context) {
        if self.is_wave_visible() {
            context.set_timer(WAVE_TIMER, WAVE_FRAME_MILLISECONDS);
        } else {
            context.kill_timer(WAVE_TIMER);
        }
    }

    fn is_track_playing(&self) -> bool {
        self.info.is_playing && !self.info.title.is_empty()
    }

    fn has_lyrics(&self) -> bool {
        matches!(self.info.lyrics, LyricsStatus::Found(_))
    }

    fn is_lyric_panel_visible(&self) -> bool {
        self.lyrics_enabled && self.has_lyrics()
    }

    fn is_lyric_mode(&self) -> bool {
        self.info.is_playing && self.is_lyric_panel_visible()
    }

    fn resting_view(&self) -> resting::View {
        if self.is_title_shown {
            resting::View::TrackChange
        } else if self.is_lyric_mode() {
            // Stays in lyric mode through a part of the song with no words; the pill then shows a note.
            resting::View::Lyrics
        } else {
            resting::View::Wave
        }
    }

    fn visible_lyric_lines(&self) -> Option<&[LyricLine]> {
        match &self.info.lyrics {
            LyricsStatus::Found(lyrics) if self.lyrics_enabled && self.info.available => Some(&lyrics.lines),
            _ => None,
        }
    }

    fn apply_snapshot(&mut self, context: &mut Context) {
        let previous_title = std::mem::take(&mut self.measured_title);
        if let Some(watcher) = &self.watcher {
            self.info = watcher.snapshot();
        }
        self.artwork.sync(self.info.artwork.as_deref(), |image| context.upload_image(image));
        self.app_icon.sync(self.info.app_icon.as_deref(), |image| context.upload_image(image));
        let cover_version = self.info.artwork.as_deref().map_or(0, |image| image.version);
        if cover_version != self.theme_version {
            self.theme_version = cover_version;
            self.theme_color = self.info.artwork.as_deref().and_then(theme::color_of);
        }
        self.measure_text(context);
        self.refresh_title_lines(context.fonts());
        self.sync_wave_timer(context);
        self.sync_backdrop(context);
        if self.measured_title != previous_title && !self.measured_title.is_empty() && self.info.is_playing {
            self.is_title_shown = true;
            context.set_timer(TITLE_TIMER, TITLE_SHOWN_MILLISECONDS);
        }
        self.update_lyrics(context);
        context.request_repaint();
    }

    fn displayed_title(&self) -> &str {
        if !self.info.available {
            NOTHING_PLAYING_TITLE
        } else if self.info.title.is_empty() {
            UNKNOWN_TITLE
        } else {
            &self.info.title
        }
    }

    fn displayed_detail(&self) -> &str {
        if self.info.available { &self.info.artist } else { NOTHING_PLAYING_DETAIL }
    }

    fn refresh_title_lines(&mut self, fonts: &Fonts) {
        let source =
            (self.displayed_title().to_owned(), self.displayed_detail().to_owned(), self.is_lyric_panel_visible());
        if source == self.title_lines_source && !self.title_lines.is_empty() {
            return;
        }
        let title = &source.0;
        let width = layout::title_width(source.2) - TITLE_FIT_MARGIN;
        let measure_title = |text: &str| fonts.measure(&styles::TITLE, text);
        self.title_lines = clamp_lines(title, layout::TITLE_MAX_LINES, width, measure_title);
        let widest = self
            .title_lines
            .iter()
            .map(|line| measure_title(line))
            .fold(fonts.measure(&styles::DETAIL, &source.1), f32::max);
        self.text_width = (widest + TITLE_FIT_MARGIN).clamp(layout::TEXT_MIN_WIDTH, layout::TEXT_MAX_WIDTH);
        self.title_lines_source = source;
    }

    fn refresh_lyric_wraps(&mut self, lyrics: Option<&std::sync::Arc<lyrics::Lyrics>>, fonts: &Fonts) {
        let Some(lyrics) = lyrics else {
            self.lyric_wraps.clear();
            self.lyric_wraps_source = 0;
            return;
        };
        let source = std::sync::Arc::as_ptr(lyrics) as usize;
        if source == self.lyric_wraps_source {
            return;
        }
        let width = layout::lyric_panel_width() - LYRIC_FIT_MARGIN;
        self.lyric_wraps = lyrics
            .lines
            .iter()
            .map(|line| {
                balance_lines(&line.text, layout::LYRIC_MAX_LINES, width, |text| {
                    fonts.measure(&styles::LYRIC_LINE, text)
                })
            })
            .collect();
        self.lyric_wraps_source = source;
    }

    fn measure_text(&mut self, context: &Context) {
        if self.info.title != self.measured_title {
            self.measured_title = self.info.title.clone();
            self.title_width = context.fonts().measure(&styles::COMPACT_TITLE, &self.measured_title);
        }
        if self.info.artist != self.measured_artist {
            self.measured_artist = self.info.artist.clone();
            self.artist_width = context.fonts().measure(&styles::COMPACT_ARTIST, &self.measured_artist);
        }
    }

    fn update_lyrics(&mut self, context: &mut Context) {
        let lyrics = match &self.info.lyrics {
            LyricsStatus::Found(lyrics) if self.lyrics_enabled => Some(lyrics.clone()),
            _ => None,
        };
        self.refresh_lyric_wraps(lyrics.as_ref(), context.fonts());
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

    fn toggle_lyrics(&mut self, context: &mut Context) {
        self.lyrics_enabled = !self.lyrics_enabled;
        context.settings.set_bool(SETTING_LYRICS, self.lyrics_enabled);
        self.refresh_title_lines(context.fonts());
        self.sync_wave_timer(context);
        self.sync_backdrop(context);
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
            Control::Previous => watcher.skip_previous(),
            Control::PlayPause => watcher.toggle_play_pause(),
            Control::Next => watcher.skip_next(),
            Control::Lyrics => {}
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
        // Collapsed requires playback; expanded remains available while paused or empty.
        match mode {
            Mode::Resting => self.is_track_playing().then_some(Priority::NORMAL),
            Mode::Expanded if self.info.available => Some(Priority::NORMAL),
            Mode::Expanded => Some(Priority::BACKGROUND),
        }
    }

    fn sizes(&self) -> PageSizes {
        let width =
            if self.is_lyric_panel_visible() { layout::LYRICS_WIDTH } else { layout::plain_width(self.text_width) };
        PageSizes {
            resting_width: resting::width(
                self.resting_view(),
                self.title_width,
                self.artist_width,
                self.follower.width(),
            ),
            expanded: Size::new(width, layout::PLAIN_HEIGHT),
        }
    }

    fn shows_live_text(&self) -> bool {
        matches!(self.resting_view(), resting::View::Lyrics)
    }

    fn header_status_width(&self) -> Option<f32> {
        self.is_wave_shown().then_some(wave::WIDTH)
    }

    fn draw_header_status(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        wave::draw(canvas, area, scale, wave::now_seconds(), opacity);
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

    fn on_hover_changed(&mut self, is_hovering: bool, context: &mut Context) {
        self.is_hovered = is_hovering;
        self.sync_wave_timer(context);
    }

    fn on_notify(&mut self, context: &mut Context) {
        self.apply_snapshot(context);
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let layout = Layout::new(frame, self.title_lines.len(), self.text_width);
        let can_use_controls = frame.is_open() && self.info.available;
        match event {
            PointerEvent::Moved(point) => {
                let hovered = if can_use_controls { controls::hit_test(&layout, point) } else { None };
                if hovered != self.hovered_control {
                    self.hovered_control = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Released(point) if can_use_controls => {
                if let Some(control) = controls::hit_test(&layout, point) {
                    self.activate(control, context);
                }
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                self.hovered_control = None;
                context.request_repaint();
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        match timer {
            LYRIC_TIMER => {
                self.update_lyrics(context);
                context.request_repaint();
            }
            WAVE_TIMER => context.request_repaint(),
            TITLE_TIMER => {
                self.is_title_shown = false;
                self.sync_wave_timer(context);
                context.kill_timer(TITLE_TIMER);
                context.request_repaint();
            }
            _ => {}
        }
    }

    fn animate(&mut self, elapsed_seconds: f32) -> bool {
        let is_following = self.follower.animate(elapsed_seconds);
        self.backdrop.step(elapsed_seconds);
        let is_fading = !self.backdrop.is_settled();
        if !is_fading {
            self.backdrop.snap_to_target();
        }
        is_following || is_fading
    }

    fn draw_backdrop(&self, canvas: &Canvas, frame: &Frame, pill: Rect, radius: f32, opacity: f32) {
        if let Some(color) = self.theme_color {
            backdrop::draw(canvas, pill, radius, frame.scale, color, self.backdrop.position.clamp(0.0, 1.0) * opacity);
        }
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
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::BACKGROUND));
    }

    #[test]
    fn with_nothing_playing_the_open_player_asks_for_a_song() {
        let mut page = MediaPage::new();
        assert_eq!(page.displayed_title(), NOTHING_PLAYING_TITLE);
        assert_eq!(page.displayed_detail(), NOTHING_PLAYING_DETAIL);
        page.info.available = true;
        page.info.title = "Song".to_owned();
        page.info.artist = "Artist".to_owned();
        assert_eq!((page.displayed_title(), page.displayed_detail()), ("Song", "Artist"));
        page.info.title.clear();
        assert_eq!(page.displayed_title(), UNKNOWN_TITLE);
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
    fn the_open_player_widens_only_while_lyrics_are_shown() {
        let plain = playing_page().sizes().expanded;
        let mut page = with_lyrics(playing_page());
        assert_eq!(page.sizes().expanded.width, layout::LYRICS_WIDTH);
        assert!(page.sizes().expanded.width > plain.width);

        page.lyrics_enabled = false;
        assert_eq!(page.sizes().expanded.width, plain.width);
        assert_eq!(page.sizes().expanded.height, plain.height);
    }

    #[test]
    fn lyrics_that_were_not_found_leave_the_layout_alone() {
        let mut page = playing_page();
        page.info.lyrics = LyricsStatus::NotFound;
        assert!(!page.is_lyric_panel_visible());
        assert!(!page.is_lyric_mode());
        assert_eq!(page.sizes().expanded.width, layout::plain_width(layout::TEXT_MIN_WIDTH));
    }

    #[test]
    fn the_collapsed_pill_shows_lyrics_only_while_playing() {
        let mut page = with_lyrics(playing_page());
        assert!(page.is_lyric_mode());
        page.info.is_playing = false;
        assert!(!page.is_lyric_mode());
        assert!(page.is_lyric_panel_visible(), "the open view still shows them while paused");
    }

    #[test]
    fn the_collapsed_pill_shows_still_bars_by_default_and_the_title_after_a_track_change() {
        let mut page = playing_page();
        assert_eq!(page.resting_view(), resting::View::Wave);
        page.is_title_shown = true;
        assert_eq!(page.resting_view(), resting::View::TrackChange);
        assert!(page.sizes().resting_width >= 150.0);
    }

    #[test]
    fn the_header_has_the_wave_whenever_music_plays_whatever_the_lyrics_do() {
        let mut page = playing_page();
        assert_eq!(page.header_status_width(), Some(wave::WIDTH));
        page.lyrics_enabled = true;
        assert_eq!(page.header_status_width(), Some(wave::WIDTH));
        let page = with_lyrics(page);
        assert_eq!(page.header_status_width(), Some(wave::WIDTH));

        let mut paused = playing_page();
        paused.info.is_playing = false;
        assert_eq!(paused.header_status_width(), None, "paused music has nothing moving");
    }

    #[test]
    fn the_collapsed_pill_has_the_wave_only_while_the_lyrics_are_off() {
        let mut page = playing_page();
        assert!(page.is_wave_visible());
        page = with_lyrics(page);
        assert!(!page.is_wave_visible(), "lyric mode shows the lyrics in the pill");
        page.is_hovered = true;
        assert!(page.is_wave_visible(), "the open notch has the wave in its header");
    }

    #[test]
    fn a_rest_in_the_lyrics_keeps_the_collapsed_pill_in_lyric_mode() {
        let page = with_lyrics(playing_page());
        assert!(page.follower.current().is_empty());
        assert_eq!(page.resting_view(), resting::View::Lyrics);
    }

    #[test]
    fn the_collapsed_pill_with_a_short_title_is_small() {
        assert_eq!(playing_page().sizes().resting_width, crate::page::RESTING_MIN_WIDTH);
    }
}
