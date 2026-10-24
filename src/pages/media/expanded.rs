use super::{MediaPage, controls, layout::Layout, lyric_panel, styles};
use crate::{
    gfx::{Canvas, Color},
    page::Frame,
};

const DIVIDER: Color = Color::rgb(0.149, 0.149, 0.176);
const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);

/// The open media player: artwork, title, album and artist, the controls and, when enabled, the lyrics.
pub fn draw(page: &MediaPage, canvas: &Canvas, frame: &Frame, opacity: f32) {
    let layout = Layout::new(frame);
    let info = &page.info;

    let artwork = layout.artwork();
    match page.artwork.brush() {
        Some(image) => canvas.image(image, artwork, layout.artwork_radius(), opacity),
        None => {
            canvas.fill_rounded_rect(artwork, layout.artwork_radius(), Color::gray(0.16).faded(opacity));
            canvas.text(&styles::RESTING_LABEL, "\u{266A}", artwork, Color::gray(0.6).faded(opacity));
        }
    }

    let title = if info.title.is_empty() { "Unknown title" } else { &info.title };
    canvas.text(&styles::TITLE, title, layout.title(), Color::WHITE.faded(opacity));
    // The album gives way to the lyrics as the pill widens.
    let album_opacity = opacity * (1.0 - layout.lyric_blend());
    canvas.text(&styles::DETAIL, &info.album, layout.album(), GRAY.faded(album_opacity));
    canvas.text(&styles::DETAIL, &info.artist, layout.artist(), GRAY.faded(opacity));

    controls::draw(page, canvas, &layout, opacity);

    let lyric_opacity = opacity * layout.lyric_blend();
    if lyric_opacity > 0.01 {
        canvas.fill_rounded_rect(layout.divider(), 0.0, DIVIDER.faded(lyric_opacity));
        lyric_panel::draw(page, canvas, &layout, lyric_opacity);
    }
}
