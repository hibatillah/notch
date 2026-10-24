use super::{MediaPage, controls, layout::Layout, lyric_panel, seek, styles};
use crate::{
    gfx::{Canvas, Color},
    page::Frame,
};

const CORNER_ICON_RADIUS: f32 = 4.0;
const CORNER_ICON_OPACITY: f32 = 0.9;

/// The open media player: artwork, title and artist, progress, controls and, when enabled, the lyric panel.
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
    canvas.text(&styles::ARTIST, &info.artist, layout.artist(), Color::gray(0.65).faded(opacity));

    if let Some(icon) = page.app_icon.brush() {
        canvas.image(icon, layout.corner_icon(), CORNER_ICON_RADIUS * layout.scale, opacity * CORNER_ICON_OPACITY);
    }

    seek::draw(page, canvas, &layout, opacity);
    controls::draw(page, canvas, &layout, opacity);
    lyric_panel::draw(page, canvas, &layout, opacity);
}
