use super::{MediaPage, controls, layout::Layout, lyric_panel, styles};
use crate::{
    gfx::{Canvas, Color},
    page::Frame,
};

const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);

pub fn draw(page: &MediaPage, canvas: &Canvas, frame: &Frame, opacity: f32) {
    let layout = Layout::new(frame, page.title_lines.len(), page.text_width);

    let artwork = layout.artwork();
    match page.artwork.brush() {
        Some(image) => canvas.image(image, artwork, layout.artwork_radius(), opacity),
        None => {
            canvas.fill_rounded_rect(artwork, layout.artwork_radius(), Color::gray(0.16).faded(opacity));
            canvas.text(&styles::RESTING_LABEL, "\u{266A}", artwork, Color::gray(0.6).faded(opacity));
        }
    }
    if let Some(icon) = page.app_icon.brush() {
        let (ring, ring_radius) = layout.app_badge_ring();
        canvas.fill_rounded_rect(ring, ring_radius, Color::BLACK.faded(opacity));
        canvas.image(icon, layout.app_badge(), layout.app_badge_radius(), opacity);
    }

    for (index, line) in page.title_lines.iter().enumerate() {
        canvas.text(&styles::TITLE, line, layout.title_line(index), Color::WHITE.faded(opacity));
    }
    canvas.text(&styles::DETAIL, page.displayed_detail(), layout.detail(), GRAY.faded(opacity));

    controls::draw(page, canvas, &layout, opacity);

    let lyric_opacity = opacity * layout.lyric_blend();
    if lyric_opacity > 0.01 {
        lyric_panel::draw(page, canvas, &layout, lyric_opacity);
    }
}
