use super::{MediaPage, styles};
use crate::{
    gfx::{Canvas, Color, Rect},
    page::{Frame, RESTING_HEIGHT},
};

const PADDING: f32 = 14.0;
const ICON_SIZE: f32 = 16.0;
const ICON_RADIUS: f32 = 3.0;
const ICON_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 14.0;
const ARTIST_GAP: f32 = 8.0;
/// With a lyric line beside it, the title gets little room.
const TITLE_LIMIT_WITH_LYRICS: f32 = 110.0;
/// Without a lyric line the pill has room for a longer title followed by the artist.
const TITLE_LIMIT_WITH_ARTIST: f32 = 220.0;
const ARTIST_LIMIT: f32 = 160.0;
const LYRIC_WIDTH_SLACK: f32 = 8.0;
/// Largest collapsed pill, leaving room for the spring's small overshoot inside the window.
const MAX_WIDTH: f32 = 1100.0;
/// How far a lyric line drifts while it changes.
const LYRIC_TRAVEL: f32 = 6.0;

/// Width of the collapsed pill: the title and artist, or the title plus the current lyric line.
/// `lyric_width` is `Some` while lyrics are shown, even when the current line is empty.
pub fn width(title_width: f32, artist_width: f32, lyric_width: Option<f32>) -> f32 {
    let chrome = PADDING + ICON_SIZE + ICON_GAP;
    match lyric_width {
        Some(lyric_width) if lyric_width > 0.0 => {
            let title_group = chrome + title_width.min(TITLE_LIMIT_WITH_LYRICS);
            (title_group + SECTION_GAP + lyric_width + LYRIC_WIDTH_SLACK + PADDING).clamp(260.0, MAX_WIDTH)
        }
        // An empty line (an instrumental gap) leaves only the title.
        Some(_) => (chrome + title_width.min(TITLE_LIMIT_WITH_LYRICS) + PADDING).clamp(150.0, 300.0),
        None => {
            let artist_span = if artist_width > 0.0 { ARTIST_GAP + artist_width.min(ARTIST_LIMIT) } else { 0.0 };
            (chrome + title_width.min(TITLE_LIMIT_WITH_ARTIST) + artist_span + PADDING).clamp(150.0, 520.0)
        }
    }
}

pub fn draw(page: &MediaPage, canvas: &Canvas, frame: &Frame, opacity: f32) {
    let scale = frame.scale;
    let bounds = frame.bounds;
    let row_top = bounds.top;
    let row_bottom = bounds.top + RESTING_HEIGHT * scale;
    let center_y = (row_top + row_bottom) / 2.0;
    let info = &page.info;

    let lyric_mode = page.is_lyric_mode();
    let title_limit = if lyric_mode { TITLE_LIMIT_WITH_LYRICS } else { TITLE_LIMIT_WITH_ARTIST };
    let title_width = page.title_width.min(title_limit) * scale;
    let show_artist = !lyric_mode && !info.artist.is_empty();
    let artist_width = if show_artist { page.artist_width.min(ARTIST_LIMIT) * scale } else { 0.0 };
    let artist_span = if show_artist { ARTIST_GAP * scale + artist_width } else { 0.0 };
    let icon_size = ICON_SIZE * scale;
    let group_width = icon_size + ICON_GAP * scale + title_width + artist_span;
    // With lyrics the group sits at the left edge; without them it is centered.
    let group_left =
        if lyric_mode { bounds.left + PADDING * scale } else { (bounds.left + bounds.right - group_width) / 2.0 };

    let icon_area =
        Rect::new(group_left, center_y - icon_size / 2.0, group_left + icon_size, center_y + icon_size / 2.0);
    match page.app_icon.brush() {
        Some(icon) => canvas.image(icon, icon_area, ICON_RADIUS * scale, opacity),
        None => canvas.text(&styles::RESTING_LABEL, "\u{266A}", icon_area, Color::WHITE.faded(opacity)),
    }

    let title_left = group_left + icon_size + ICON_GAP * scale;
    let title_right = title_left + title_width + 2.0 * scale;
    let title_color = Color::gray(if lyric_mode { 0.7 } else { 1.0 });
    canvas.text(
        &styles::RESTING_TITLE,
        &info.title,
        Rect::new(title_left, row_top, title_right, row_bottom),
        title_color.faded(opacity),
    );
    if show_artist {
        let artist_left = title_right + ARTIST_GAP * scale;
        let artist = Rect::new(artist_left, row_top, artist_left + artist_width + 2.0 * scale, row_bottom);
        canvas.text(&styles::RESTING_TITLE, &info.artist, artist, Color::gray(0.55).faded(opacity));
    }

    if lyric_mode {
        let lyric_left = title_right + SECTION_GAP * scale;
        let lyric_right = bounds.right - PADDING * scale;
        let transition = page.follower.transition().clamp(0.0, 1.0);
        let travel = LYRIC_TRAVEL * scale;
        let band = |offset: f32| Rect::new(lyric_left, row_top + offset, lyric_right, row_bottom + offset);
        // The old line drifts up and fades while the new one rises into place.
        let previous = Color::WHITE.faded(opacity * (1.0 - transition));
        let current = Color::WHITE.faded(opacity * transition);
        canvas.text(&styles::RESTING_LYRIC, page.follower.previous(), band(-transition * travel), previous);
        canvas.text(&styles::RESTING_LYRIC, page.follower.current(), band((1.0 - transition) * travel), current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn showing_a_lyric_line_makes_the_pill_wider_than_the_title_alone() {
        let with_lyric = width(80.0, 60.0, Some(150.0));
        let without = width(80.0, 0.0, None);
        assert!(with_lyric > without);
    }

    #[test]
    fn the_pill_grows_with_the_lyric_line() {
        assert!(width(80.0, 0.0, Some(300.0)) > width(80.0, 0.0, Some(150.0)));
    }

    #[test]
    fn a_very_long_lyric_line_stops_at_the_maximum() {
        assert_eq!(width(80.0, 0.0, Some(5000.0)), MAX_WIDTH);
    }

    #[test]
    fn title_and_artist_make_the_pill_wider_than_the_title_alone() {
        assert!(width(100.0, 80.0, None) > width(100.0, 0.0, None));
    }

    #[test]
    fn the_pill_never_shrinks_below_its_minimum() {
        assert_eq!(width(0.0, 0.0, None), 150.0);
    }

    #[test]
    fn an_empty_lyric_line_leaves_just_the_title() {
        assert_eq!(width(80.0, 60.0, Some(0.0)), width(80.0, 0.0, Some(0.0)));
    }
}
