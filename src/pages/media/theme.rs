//! The color of a song, taken from its album art, for the glow behind the player while the lyrics are on.

use crate::gfx::{Color, ImageData};

const SAMPLE_STEP: u32 = 4;
const BLACK_BELOW: f32 = 0.08;
/// The color is brightened until its brightest channel is at least this, so a dark cover still gives a visible glow.
const BRIGHTEST_AT_LEAST: f32 = 0.6;
const SATURATION_WEIGHT: f32 = 4.0;

/// The color the picture is mostly made of, favoring its vivid parts. `None` for a picture that is black or empty.
pub fn color_of(image: &ImageData) -> Option<Color> {
    let (width, height) = (image.width, image.height);
    let (mut red, mut green, mut blue, mut total) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for y in (0..height).step_by(SAMPLE_STEP as usize) {
        for x in (0..width).step_by(SAMPLE_STEP as usize) {
            let index = ((y * width + x) * 4) as usize;
            let Some(pixel) = image.bgra_premultiplied.get(index..index + 4) else { continue };
            let alpha = f32::from(pixel[3]) / 255.0;
            if alpha < 0.5 {
                continue;
            }
            // Premultiplied, so dividing by the alpha gives the real color.
            let (b, g, r) = (
                f32::from(pixel[0]) / 255.0 / alpha,
                f32::from(pixel[1]) / 255.0 / alpha,
                f32::from(pixel[2]) / 255.0 / alpha,
            );
            let brightest = r.max(g).max(b);
            if brightest < BLACK_BELOW {
                continue;
            }
            let saturation = (brightest - r.min(g).min(b)) / brightest;
            let weight = 1.0 + saturation * SATURATION_WEIGHT;
            red += r * weight;
            green += g * weight;
            blue += b * weight;
            total += weight;
        }
    }
    if total <= 0.0 {
        return None;
    }
    let (red, green, blue) = (red / total, green / total, blue / total);
    let brightest = red.max(green).max(blue);
    let lift = (BRIGHTEST_AT_LEAST / brightest).max(1.0);
    Some(Color::rgb((red * lift).min(1.0), (green * lift).min(1.0), (blue * lift).min(1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(red: u8, green: u8, blue: u8) -> ImageData {
        let pixels = (0..16 * 16).flat_map(|_| [blue, green, red, 255]).collect();
        ImageData::new(16, 16, pixels)
    }

    #[test]
    fn a_red_cover_gives_a_red_color() {
        let color = color_of(&plain(200, 20, 20)).unwrap();
        assert!(color.r > 0.6 && color.g < 0.2 && color.b < 0.2, "{color:?}");
    }

    #[test]
    fn a_black_cover_gives_no_color() {
        assert!(color_of(&plain(0, 0, 0)).is_none());
        assert!(color_of(&plain(10, 10, 10)).is_none());
    }

    #[test]
    fn a_dark_cover_is_brightened_so_the_glow_shows() {
        let color = color_of(&plain(60, 10, 10)).unwrap();
        assert!((color.r - BRIGHTEST_AT_LEAST).abs() < 0.01, "{color:?}");
        assert!(color.g < color.r && color.b < color.r);
    }

    #[test]
    fn a_vivid_part_of_a_cover_counts_for_more_than_a_gray_part() {
        let mut pixels = Vec::new();
        for row in 0..16 {
            for _ in 0..16 {
                pixels.extend_from_slice(if row < 8 { &[128, 128, 128, 255] } else { &[255, 0, 0, 255] });
            }
        }
        let color = color_of(&ImageData::new(16, 16, pixels)).unwrap();
        assert!(color.b > color.r + 0.15, "{color:?}");
    }

    #[test]
    fn a_see_through_picture_gives_no_color() {
        let pixels = (0..16 * 16).flat_map(|_| [0u8, 0, 0, 0]).collect();
        assert!(color_of(&ImageData::new(16, 16, pixels)).is_none());
    }
}
