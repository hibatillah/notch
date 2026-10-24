use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    rc::Rc,
};

use windows::Win32::Graphics::Direct2D::{
    Common::{D2D_RECT_F, D2D1_COLOR_F},
    D2D1_ANTIALIAS_MODE_ALIASED, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT, ID2D1DCRenderTarget,
    ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows_numerics::{Matrix3x2, Vector2};

use super::{
    color::Color,
    fonts::{FontStyle, Fonts},
    geometry::{Point, Rect},
    image::{self, ImageBrush, ImageCache, ImageData},
    raster::Mask,
};

/// Text below this opacity is invisible, so drawing it is skipped.
const MIN_VISIBLE_ALPHA: f32 = 0.01;

/// What a page draws with. All coordinates are device pixels; multiply device-independent sizes by `scale`.
pub struct Canvas<'a> {
    target: &'a ID2D1DCRenderTarget,
    brush: &'a ID2D1SolidColorBrush,
    images: &'a ImageCache,
    fonts: &'a Fonts,
    scale: f32,
}

#[derive(Clone, Copy, Hash)]
enum FlareSide {
    Left,
    Right,
}

impl<'a> Canvas<'a> {
    pub(super) fn new(
        target: &'a ID2D1DCRenderTarget,
        brush: &'a ID2D1SolidColorBrush,
        images: &'a ImageCache,
        fonts: &'a Fonts,
        scale: f32,
    ) -> Self {
        Self { target, brush, images, fonts, scale }
    }

    /// Display scale: device pixels per device-independent pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn fill_rounded_rect(&self, rect: Rect, radius: f32, color: Color) {
        self.use_color(color);
        unsafe {
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT { rect: rect.into(), radiusX: radius, radiusY: radius },
                self.brush,
            );
        }
    }

    /// An image made by `build`, uploaded once and reused whenever the same `key` is asked for again.
    /// Everything that changes how the image looks must be part of the key.
    pub fn cached_image(&self, key: impl Hash, build: impl FnOnce() -> ImageData) -> Option<Rc<ImageBrush>> {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        self.images.get_or_insert(hasher.finish(), || image::upload(self.target, &build()).ok())
    }

    /// Fills the two concave fillets that join a shape's vertical sides to the top edge of the screen, like the
    /// outward-curving corners of a MacBook notch. `flare` is each fillet's radius; `bounds` is the shape they join.
    ///
    /// They are small cached images rather than Direct2D geometry, which costs about 10 MB of memory when first used.
    pub fn fill_top_flares(&self, bounds: Rect, flare: f32, color: Color) {
        let size = flare.round().max(1.0) as u32;
        let rgb = color.rgb_bytes();
        for side in [FlareSide::Left, FlareSide::Right] {
            let Some(brush) = self.cached_image(("flare", side, size, rgb), || flare_image(size, side, color)) else {
                continue;
            };
            // Each image has one extra column that overlaps the shape, so no seam shows between them.
            let left = match side {
                FlareSide::Left => bounds.left.round() - size as f32,
                FlareSide::Right => bounds.right.round() - 1.0,
            };
            let rect = Rect::new(left, bounds.top, left + size as f32 + 1.0, bounds.top + size as f32);
            self.image(&brush, rect, 0.0, color.a);
        }
    }

    pub fn fill_circle(&self, center: Point, radius: f32, color: Color) {
        self.use_color(color);
        unsafe {
            self.target.FillEllipse(
                &D2D1_ELLIPSE { point: Vector2 { X: center.x, Y: center.y }, radiusX: radius, radiusY: radius },
                self.brush,
            );
        }
    }

    /// Draws one run of text inside `rect`, aligned and trimmed according to `style`.
    pub fn text(&self, style: &FontStyle, text: &str, rect: Rect, color: Color) {
        if color.a <= MIN_VISIBLE_ALPHA || text.is_empty() {
            return;
        }
        let Some(format) = self.fonts.format(style) else { return };
        let wide_text: Vec<u16> = text.encode_utf16().collect();
        self.use_color(color);
        let bounds: D2D_RECT_F = rect.into();
        unsafe {
            self.target.DrawText(
                &wide_text,
                &format,
                &bounds,
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }

    /// Fills a rounded rectangle with an uploaded image, scaled so the whole image covers `rect`.
    pub fn image(&self, image: &ImageBrush, rect: Rect, radius: f32, opacity: f32) {
        let pixel_scale = rect.width() / image.width as f32;
        let brush = image.brush();
        unsafe {
            brush.SetTransform(&Matrix3x2 {
                M11: pixel_scale,
                M12: 0.0,
                M21: 0.0,
                M22: pixel_scale,
                M31: rect.left,
                M32: rect.top,
            });
            brush.SetOpacity(opacity);
            self.target.FillRoundedRectangle(
                &D2D1_ROUNDED_RECT { rect: rect.into(), radiusX: radius, radiusY: radius },
                brush,
            );
        }
    }

    /// Runs `draw` with everything outside `rect` cut away.
    pub fn clipped(&self, rect: Rect, draw: impl FnOnce()) {
        unsafe { self.target.PushAxisAlignedClip(&rect.into(), D2D1_ANTIALIAS_MODE_ALIASED) };
        draw();
        unsafe { self.target.PopAxisAlignedClip() };
    }

    /// Runs `draw` with the drawing scaled by `factor` around `origin`; text stays sharp at any factor.
    pub fn scaled_about(&self, origin: Point, factor: f32, draw: impl FnOnce()) {
        unsafe {
            self.target.SetTransform(&Matrix3x2 {
                M11: factor,
                M12: 0.0,
                M21: 0.0,
                M22: factor,
                M31: origin.x * (1.0 - factor),
                M32: origin.y * (1.0 - factor),
            });
        }
        draw();
        unsafe { self.target.SetTransform(&Matrix3x2 { M11: 1.0, M12: 0.0, M21: 0.0, M22: 1.0, M31: 0.0, M32: 0.0 }) };
    }

    fn use_color(&self, color: Color) {
        unsafe { self.brush.SetColor(&D2D1_COLOR_F { r: color.r, g: color.g, b: color.b, a: color.a }) };
    }
}

/// One flare as an image `size` pixels square plus one column that overlaps the shape it joins.
/// It is solid where it lies outside a circle of radius `size` centered on the shape's top edge, one flare away.
fn flare_image(size: u32, side: FlareSide, color: Color) -> ImageData {
    let (width, radius) = (size + 1, size as f32);
    let mask = Mask::from_distance(width, size, |point| {
        let x = match side {
            FlareSide::Left => point.x,
            FlareSide::Right => width as f32 - point.x,
        };
        // Negative inside the solid part, which is the outside of the circle.
        radius - x.hypot(point.y - radius)
    });
    mask.tinted(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(image: &ImageData, x: u32, y: u32) -> u8 {
        image.bgra_premultiplied[((y * image.width + x) * 4 + 3) as usize]
    }

    #[test]
    fn a_left_flare_is_solid_at_the_top_edge_and_empty_where_the_curve_meets_the_side() {
        let image = flare_image(10, FlareSide::Left, Color::BLACK);
        assert_eq!((image.width, image.height), (11, 10));
        // Along the top the flare is solid, tapering to nothing at the bottom of the shape's side.
        assert_eq!(coverage(&image, 9, 0), 255);
        assert_eq!(coverage(&image, 0, 9), 0);
        // The overlap column is solid all the way down.
        assert_eq!(coverage(&image, 10, 9), 255);
    }

    #[test]
    fn the_right_flare_mirrors_the_left_one() {
        let left = flare_image(10, FlareSide::Left, Color::BLACK);
        let right = flare_image(10, FlareSide::Right, Color::BLACK);
        for y in 0..10 {
            for x in 0..11 {
                assert_eq!(coverage(&left, x, y), coverage(&right, 10 - x, y), "pixel {x},{y}");
            }
        }
    }
}
