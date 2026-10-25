use super::{MediaPage, styles, wave};
use crate::{
    gfx::{Canvas, Color, Rect},
    page::{Frame, RESTING_HEIGHT},
};

/// Left and right padding in the collapsed pill.
const PADDING: f32 = 8.0;
const ICON_SIZE: f32 = 18.0;
const ICON_RADIUS: f32 = 4.0;
const GAP: f32 = 8.0;
/// The space between the art, the title and the wave while music plays with the lyrics off.
const WAVE_VIEW_GAP: f32 = 12.0;
/// The smallest collapsed pill with the art, the title and the wave.
const WAVE_VIEW_MIN_WIDTH: f32 = 96.0;
const TITLE_LIMIT: f32 = 140.0;
const ARTIST_LIMIT: f32 = 80.0;
const LYRIC_SLACK: f32 = 8.0;
/// Smallest collapsed pill while lyrics are on, so a short line or a rest in the music does not shrink it.
const MIN_LYRIC_WIDTH: f32 = 240.0;
/// Shown in place of a lyric line while the song has no words.
const NOTE: &str = "\u{266A}";
/// Largest collapsed pill, leaving room for the spring's small overshoot inside the window.
const MAX_LYRIC_WIDTH: f32 = 520.0;
/// How far a lyric line drifts while it changes.
const LYRIC_TRAVEL: f32 = 6.0;

/// What the collapsed pill shows for the music.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The art, the title and the wave: music plays with the lyrics off.
    Wave,
    /// The art, the title and the artist: for a moment after the track changes.
    TrackChange,
    /// The art and the line being sung.
    Lyrics,
}

/// Width of the collapsed pill for `view`, in device-independent pixels.
pub fn width(view: View, title_width: f32, artist_width: f32, lyric_width: f32) -> f32 {
    match view {
        View::Wave => {
            (PADDING + ICON_SIZE + WAVE_VIEW_GAP + title_width.min(TITLE_LIMIT) + WAVE_VIEW_GAP + wave::WIDTH + PADDING)
                .max(WAVE_VIEW_MIN_WIDTH)
        }
        View::TrackChange => {
            let artist = if artist_width > 0.0 { GAP + artist_width.min(ARTIST_LIMIT) } else { 0.0 };
            (PADDING + ICON_SIZE + GAP + title_width.min(TITLE_LIMIT) + artist + PADDING).max(150.0)
        }
        View::Lyrics => {
            (PADDING + ICON_SIZE + GAP + lyric_width + LYRIC_SLACK + PADDING).clamp(MIN_LYRIC_WIDTH, MAX_LYRIC_WIDTH)
        }
    }
}

/// The art of the track, or the player's icon when there is no art, or a note when there is neither.
fn draw_art(page: &MediaPage, canvas: &Canvas, area: Rect, opacity: f32) {
    let radius = ICON_RADIUS * canvas.scale();
    match page.artwork.brush().or(page.app_icon.brush()) {
        Some(image) => canvas.image(image, area, radius, opacity),
        None => canvas.text(&styles::RESTING_LABEL, "\u{266A}", area, Color::WHITE.faded(opacity)),
    }
}

pub fn draw(page: &MediaPage, canvas: &Canvas, frame: &Frame, opacity: f32) {
    let scale = frame.scale;
    let bounds = frame.bounds;
    let row_top = bounds.top;
    let row_bottom = bounds.top + RESTING_HEIGHT * scale;
    let center_y = (row_top + row_bottom) / 2.0;
    let icon_size = ICON_SIZE * scale;
    let left = bounds.left + PADDING * scale;
    let icon_area = Rect::new(left, center_y - icon_size / 2.0, left + icon_size, center_y + icon_size / 2.0);
    let view = page.resting_view();
    let info = &page.info;

    draw_art(page, canvas, icon_area, opacity);
    match view {
        View::Wave => {
            let right = bounds.right - PADDING * scale;
            let half_height = wave::HEIGHT * scale / 2.0;
            let area = Rect::new(right - wave::WIDTH * scale, center_y - half_height, right, center_y + half_height);
            wave::draw(canvas, area, scale, wave::now_seconds(), opacity);
            let title_left = icon_area.right + WAVE_VIEW_GAP * scale;
            let title_area = Rect::new(title_left, row_top, area.left - WAVE_VIEW_GAP * scale, row_bottom);
            canvas.text(&styles::COMPACT_TITLE, &info.title, title_area, Color::WHITE.faded(opacity));
        }
        View::TrackChange => {
            let title_width = page.title_width.min(TITLE_LIMIT) * scale;
            let title_left = icon_area.right + GAP * scale;
            let title_area = Rect::new(title_left, row_top, title_left + title_width + 2.0 * scale, row_bottom);
            canvas.text(&styles::COMPACT_TITLE, &info.title, title_area, Color::WHITE.faded(opacity));
            if !info.artist.is_empty() {
                let artist_left = title_area.right + GAP * scale;
                let artist_width = page.artist_width.min(ARTIST_LIMIT) * scale;
                let artist_area = Rect::new(artist_left, row_top, artist_left + artist_width + 2.0 * scale, row_bottom);
                canvas.text(&styles::COMPACT_ARTIST, &info.artist, artist_area, Color::gray(0.6).faded(opacity));
            }
        }
        View::Lyrics => {
            let lyric_left = icon_area.right + GAP * scale;
            let lyric_right = bounds.right - PADDING * scale;
            let transition = page.follower.transition().clamp(0.0, 1.0);
            let travel = LYRIC_TRAVEL * scale;
            let band = |offset: f32| Rect::new(lyric_left, row_top + offset, lyric_right, row_bottom + offset);
            // The old line drifts up and fades while the new one rises into place.
            let previous = Color::WHITE.faded(opacity * (1.0 - transition));
            let current = Color::WHITE.faded(opacity * transition);
            let shown = |text: &str| if text.is_empty() { NOTE.to_owned() } else { text.to_owned() };
            canvas.text(&styles::RESTING_LYRIC, &shown(page.follower.previous()), band(-transition * travel), previous);
            canvas.text(
                &styles::RESTING_LYRIC,
                &shown(page.follower.current()),
                band((1.0 - transition) * travel),
                current,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wave_pill_grows_with_the_title_up_to_a_limit_and_is_never_tiny() {
        assert_eq!(width(View::Wave, 0.0, 0.0, 0.0), WAVE_VIEW_MIN_WIDTH);
        assert!(width(View::Wave, 100.0, 0.0, 0.0) > width(View::Wave, 60.0, 0.0, 0.0));
        assert_eq!(width(View::Wave, 5000.0, 0.0, 0.0), width(View::Wave, TITLE_LIMIT, 0.0, 0.0));
    }

    #[test]
    fn a_track_change_pill_grows_with_the_title_and_the_artist() {
        assert!(width(View::TrackChange, 100.0, 60.0, 0.0) > width(View::TrackChange, 100.0, 0.0, 0.0));
        assert!(width(View::TrackChange, 120.0, 0.0, 0.0) > width(View::TrackChange, 60.0, 0.0, 0.0));
    }

    #[test]
    fn a_track_change_pill_stays_within_its_limits() {
        assert_eq!(width(View::TrackChange, 0.0, 0.0, 0.0), 150.0);
        assert_eq!(width(View::TrackChange, 5000.0, 5000.0, 0.0), 270.0);
    }

    #[test]
    fn the_lyrics_pill_grows_with_the_line_up_to_a_maximum() {
        assert!(width(View::Lyrics, 0.0, 0.0, 300.0) > width(View::Lyrics, 0.0, 0.0, 150.0));
        assert_eq!(width(View::Lyrics, 0.0, 0.0, 5000.0), MAX_LYRIC_WIDTH);
        assert_eq!(width(View::Lyrics, 0.0, 0.0, 1.0), MIN_LYRIC_WIDTH);
    }
}
