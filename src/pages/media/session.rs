//! Mirrors the system's media session (Spotify, browsers, ...) into plain data the page can read.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use windows::{
    Foundation::TypedEventHandler,
    Media::{
        Control::{
            GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus,
        },
        MediaPlaybackAutoRepeatMode,
    },
    Win32::System::SystemInformation::GetSystemTimeAsFileTime,
    core::Result,
};

use super::{
    app_icon, artwork,
    lyrics::{self, Lyrics},
};
use crate::gfx::ImageData;

const TICKS_PER_SECOND: i64 = 10_000_000;

/// Delay before a lyrics lookup, so the new track's length has arrived and the match is exact.
const LYRICS_LOOKUP_DELAY: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    Track,
    List,
}

#[derive(Clone, Default)]
pub enum LyricsStatus {
    #[default]
    Unknown,
    Loading,
    Found(Arc<Lyrics>),
    NotFound,
}

#[derive(Clone, Default)]
pub struct MediaInfo {
    pub available: bool,
    pub is_playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork: Option<Arc<ImageData>>,
    /// Windows app identifier of the player, such as `Spotify.exe`.
    pub source_app: String,
    pub app_icon: Option<Arc<ImageData>>,
    pub shuffle_enabled: bool,
    pub shuffle_active: bool,
    pub repeat_enabled: bool,
    pub repeat_mode: RepeatMode,
    pub lyrics: LyricsStatus,
    track_key: String,
    /// Track length in 100 ns ticks; zero when the source reports no timeline.
    pub duration_ticks: i64,
    pub can_seek: bool,
    start_ticks: i64,
    position_ticks: i64,
    position_updated_at_ticks: i64,
}

impl MediaInfo {
    /// Playback position now, extrapolated from the last report while playing.
    pub fn position_ticks_now(&self) -> i64 {
        let elapsed = if self.is_playing { (now_ticks() - self.position_updated_at_ticks).max(0) } else { 0 };
        (self.position_ticks + elapsed).clamp(0, self.duration_ticks.max(0))
    }
}

/// Current time as Windows file-time ticks (100 ns since 1601), the same epoch the media session reports.
pub fn now_ticks() -> i64 {
    let file_time = unsafe { GetSystemTimeAsFileTime() };
    (i64::from(file_time.dwHighDateTime) << 32) | i64::from(file_time.dwLowDateTime)
}

pub fn format_seconds(ticks: i64) -> String {
    let total_seconds = (ticks.max(0) / TICKS_PER_SECOND) as u64;
    let (hours, minutes, seconds) = (total_seconds / 3600, (total_seconds / 60) % 60, total_seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

struct SessionSubscription {
    session: GlobalSystemMediaTransportControlsSession,
    properties_token: i64,
    playback_token: i64,
    timeline_token: i64,
}

struct Shared {
    info: Mutex<MediaInfo>,
    /// Lookup results by track, so replaying a song never asks the service again; `None` means it has no lyrics.
    lyrics_cache: Mutex<HashMap<String, Option<Arc<Lyrics>>>>,
    subscription: Mutex<Option<SessionSubscription>>,
    notify_changed: Box<dyn Fn() + Send + Sync>,
}

/// Mirrors the system's current media session into `MediaInfo` and calls `notify_changed` on updates.
pub struct MediaWatcher {
    manager: GlobalSystemMediaTransportControlsSessionManager,
    shared: Arc<Shared>,
}

impl MediaWatcher {
    pub fn start(notify_changed: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
        let shared = Arc::new(Shared {
            info: Mutex::new(MediaInfo::default()),
            lyrics_cache: Mutex::new(HashMap::new()),
            subscription: Mutex::new(None),
            notify_changed: Box::new(notify_changed),
        });

        let session_changed_manager = manager.clone();
        let session_changed_shared = shared.clone();
        manager.CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
            attach_current_session(&session_changed_manager, &session_changed_shared);
            Ok(())
        }))?;

        attach_current_session(&manager, &shared);
        Ok(Self { manager, shared })
    }

    pub fn snapshot(&self) -> MediaInfo {
        self.shared.info.lock().map(|info| info.clone()).unwrap_or_default()
    }

    pub fn toggle_play_pause(&self) {
        if let Ok(session) = self.manager.GetCurrentSession() {
            let _ = session.TryTogglePlayPauseAsync();
        }
    }

    pub fn skip_next(&self) {
        if let Ok(session) = self.manager.GetCurrentSession() {
            let _ = session.TrySkipNextAsync();
        }
    }

    pub fn skip_previous(&self) {
        if let Ok(session) = self.manager.GetCurrentSession() {
            let _ = session.TrySkipPreviousAsync();
        }
    }

    pub fn toggle_shuffle(&self) {
        let is_active = self.shared.info.lock().map(|info| info.shuffle_active).unwrap_or(false);
        if let Ok(session) = self.manager.GetCurrentSession() {
            let _ = session.TryChangeShuffleActiveAsync(!is_active);
        }
    }

    /// Cycles off, repeat all, repeat one, which is the order players show them in.
    pub fn cycle_repeat(&self) {
        let current = self.shared.info.lock().map(|info| info.repeat_mode).unwrap_or_default();
        let next = match current {
            RepeatMode::Off => MediaPlaybackAutoRepeatMode::List,
            RepeatMode::List => MediaPlaybackAutoRepeatMode::Track,
            RepeatMode::Track => MediaPlaybackAutoRepeatMode::None,
        };
        if let Ok(session) = self.manager.GetCurrentSession() {
            let _ = session.TryChangeAutoRepeatModeAsync(next);
        }
    }

    /// Jumps to `position_ticks` from the track start and updates the local position without waiting for the source.
    pub fn seek(&self, position_ticks: i64) {
        let Ok(session) = self.manager.GetCurrentSession() else { return };
        let start_ticks = self.shared.info.lock().map(|info| info.start_ticks).unwrap_or(0);
        let _ = session.TryChangePlaybackPositionAsync(start_ticks + position_ticks);
        update_info(&self.shared, |info| {
            info.position_ticks = position_ticks;
            info.position_updated_at_ticks = now_ticks();
        });
    }
}

fn attach_current_session(manager: &GlobalSystemMediaTransportControlsSessionManager, shared: &Arc<Shared>) {
    let Ok(mut subscription) = shared.subscription.lock() else { return };
    if let Some(previous) = subscription.take() {
        let _ = previous.session.RemoveMediaPropertiesChanged(previous.properties_token);
        let _ = previous.session.RemovePlaybackInfoChanged(previous.playback_token);
        let _ = previous.session.RemoveTimelinePropertiesChanged(previous.timeline_token);
    }

    let Ok(session) = manager.GetCurrentSession() else {
        drop(subscription);
        update_info(shared, |info| *info = MediaInfo::default());
        return;
    };

    let properties_token = session.MediaPropertiesChanged(&TypedEventHandler::new({
        let (shared, session) = (shared.clone(), session.clone());
        move |_, _| {
            refresh_properties(&shared, &session);
            Ok(())
        }
    }));
    let playback_token = session.PlaybackInfoChanged(&TypedEventHandler::new({
        let (shared, session) = (shared.clone(), session.clone());
        move |_, _| {
            refresh_playback(&shared, &session);
            Ok(())
        }
    }));
    let timeline_token = session.TimelinePropertiesChanged(&TypedEventHandler::new({
        let (shared, session) = (shared.clone(), session.clone());
        move |_, _| {
            refresh_playback(&shared, &session);
            Ok(())
        }
    }));

    if let (Ok(properties_token), Ok(playback_token), Ok(timeline_token)) =
        (properties_token, playback_token, timeline_token)
    {
        *subscription =
            Some(SessionSubscription { session: session.clone(), properties_token, playback_token, timeline_token });
    }
    drop(subscription);
    load_app_icon(shared, session.SourceAppUserModelId().map(|id| id.to_string()).unwrap_or_default());
    refresh_properties(shared, &session);
    refresh_playback(shared, &session);
}

/// Loads the player's icon on its own thread, since resolving it touches the shell and can be slow.
fn load_app_icon(shared: &Arc<Shared>, source_app: String) {
    let is_new_app = update_info(shared, |info| {
        let changed = info.source_app != source_app;
        if changed {
            info.source_app = source_app.clone();
            info.app_icon = None;
        }
        changed
    });
    if !is_new_app {
        return;
    }
    let shared = shared.clone();
    thread::spawn(move || {
        let icon = app_icon::load(&source_app).map(Arc::new);
        update_info(&shared, |info| {
            if info.source_app == source_app {
                info.app_icon = icon;
            }
        });
    });
}

fn refresh_properties(shared: &Arc<Shared>, session: &GlobalSystemMediaTransportControlsSession) {
    let Ok(properties) = session.TryGetMediaPropertiesAsync().and_then(|operation| operation.join()) else { return };
    let title = properties.Title().map(|text| text.to_string()).unwrap_or_default();
    let artist = properties.Artist().map(|text| text.to_string()).unwrap_or_default();
    let album = properties.AlbumTitle().map(|text| text.to_string()).unwrap_or_default();
    let artwork = artwork::read(&properties).ok().map(Arc::new);
    let track_key = format!("{artist}\u{1}{title}\u{1}{album}");
    let is_new_track = update_info(shared, |info| {
        info.available = true;
        info.title = title.clone();
        info.artist = artist.clone();
        info.album = album.clone();
        info.artwork = artwork;
        let is_new_track = info.track_key != track_key;
        if is_new_track {
            info.track_key = track_key.clone();
            info.lyrics = LyricsStatus::Loading;
        }
        is_new_track
    });
    if is_new_track {
        spawn_lyrics_lookup(shared.clone(), track_key, artist, title, album);
    }
}

/// Fetches synced lyrics off-thread and stores them only if the track has not changed meanwhile.
fn spawn_lyrics_lookup(shared: Arc<Shared>, track_key: String, artist: String, title: String, album: String) {
    thread::spawn(move || {
        let lookups_disabled = std::env::var("NOTCH_LYRICS").is_ok_and(|value| value.eq_ignore_ascii_case("off"));
        let cached = shared.lyrics_cache.lock().ok().and_then(|cache| cache.get(&track_key).cloned());
        let result = if let Some(cached) = cached {
            cached
        } else if lookups_disabled || title.is_empty() {
            None
        } else {
            thread::sleep(LYRICS_LOOKUP_DELAY);
            let (still_current, duration_seconds) = shared
                .info
                .lock()
                .map(|info| (info.track_key == track_key, info.duration_ticks / 10_000_000))
                .unwrap_or((false, 0));
            if !still_current {
                return;
            }
            match lyrics::fetch(&artist, &title, &album, duration_seconds) {
                Ok(found) => {
                    let found = found.map(Arc::new);
                    if let Ok(mut cache) = shared.lyrics_cache.lock() {
                        // A small cache is plenty: it only has to cover replays and skipping back.
                        if cache.len() >= 64 {
                            cache.clear();
                        }
                        cache.insert(track_key.clone(), found.clone());
                    }
                    found
                }
                // Network trouble is not remembered, so the next play of this track tries again.
                Err(_) => None,
            }
        };
        update_info(&shared, |info| {
            if info.track_key == track_key {
                info.lyrics = result.map_or(LyricsStatus::NotFound, LyricsStatus::Found);
            }
        });
    });
}

fn refresh_playback(shared: &Arc<Shared>, session: &GlobalSystemMediaTransportControlsSession) {
    let Ok(playback) = session.GetPlaybackInfo() else { return };
    let is_playing = playback
        .PlaybackStatus()
        .is_ok_and(|status| status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
    let can_seek = playback.Controls().and_then(|controls| controls.IsPlaybackPositionEnabled()).unwrap_or(false);
    let controls = playback.Controls().ok();
    let shuffle_enabled = controls.as_ref().and_then(|controls| controls.IsShuffleEnabled().ok()).unwrap_or(false);
    let repeat_enabled = controls.as_ref().and_then(|controls| controls.IsRepeatEnabled().ok()).unwrap_or(false);
    let shuffle_active = playback.IsShuffleActive().and_then(|value| value.Value()).unwrap_or(false);
    let repeat_mode = match playback.AutoRepeatMode().and_then(|value| value.Value()) {
        Ok(MediaPlaybackAutoRepeatMode::Track) => RepeatMode::Track,
        Ok(MediaPlaybackAutoRepeatMode::List) => RepeatMode::List,
        _ => RepeatMode::Off,
    };
    let timeline = session.GetTimelineProperties().ok().and_then(|timeline| {
        Some((
            timeline.StartTime().ok()?.Duration,
            timeline.EndTime().ok()?.Duration,
            timeline.Position().ok()?.Duration,
            timeline.LastUpdatedTime().ok()?.UniversalTime,
        ))
    });
    update_info(shared, |info| {
        info.available = true;
        info.is_playing = is_playing;
        info.can_seek = can_seek;
        info.shuffle_enabled = shuffle_enabled;
        info.shuffle_active = shuffle_active;
        info.repeat_enabled = repeat_enabled;
        info.repeat_mode = repeat_mode;
        if let Some((start, end, position, updated_at)) = timeline {
            info.start_ticks = start;
            info.duration_ticks = (end - start).max(0);
            info.position_ticks = (position - start).max(0);
            info.position_updated_at_ticks = updated_at;
        }
    });
}

fn update_info<R: Default>(shared: &Arc<Shared>, change: impl FnOnce(&mut MediaInfo) -> R) -> R {
    let result = shared.info.lock().map(|mut info| change(&mut info)).unwrap_or_default();
    (shared.notify_changed)();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: i64 = 10_000_000;

    #[test]
    fn formats_times_as_minutes_and_seconds() {
        assert_eq!(format_seconds(0), "0:00");
        assert_eq!(format_seconds(65 * SECOND), "1:05");
        assert_eq!(format_seconds(3661 * SECOND), "1:01:01");
    }

    #[test]
    fn negative_times_show_as_zero() {
        assert_eq!(format_seconds(-5 * SECOND), "0:00");
    }

    fn info(is_playing: bool, position_seconds: i64) -> MediaInfo {
        MediaInfo {
            is_playing,
            duration_ticks: 100 * SECOND,
            position_ticks: position_seconds * SECOND,
            position_updated_at_ticks: now_ticks(),
            ..MediaInfo::default()
        }
    }

    #[test]
    fn a_paused_track_stays_where_it_stopped() {
        assert_eq!(info(false, 50).position_ticks_now(), 50 * SECOND);
    }

    #[test]
    fn a_playing_track_moves_forward_from_its_last_report() {
        let mut playing = info(true, 50);
        playing.position_updated_at_ticks = now_ticks() - 3 * SECOND;
        let position = playing.position_ticks_now();
        assert!((53 * SECOND..54 * SECOND).contains(&position), "got {position}");
    }

    #[test]
    fn the_position_never_leaves_the_track() {
        let mut past_the_end = info(false, 500);
        assert_eq!(past_the_end.position_ticks_now(), 100 * SECOND);
        past_the_end.position_ticks = -5 * SECOND;
        assert_eq!(past_the_end.position_ticks_now(), 0);
    }
}
