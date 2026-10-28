/// Straight (non-premultiplied) RGBA color with each channel in 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color::gray(0.0);
    pub const WHITE: Color = Color::gray(1.0);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn gray(level: f32) -> Self {
        Self::rgb(level, level, level)
    }

    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn faded(self, factor: f32) -> Self {
        Self { a: self.a * factor, ..self }
    }

    pub fn rgb_bytes(self) -> [u8; 3] {
        [self.r, self.g, self.b].map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_has_equal_channels_and_full_alpha() {
        assert_eq!(Color::gray(0.5), Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 });
    }

    #[test]
    fn with_alpha_replaces_only_alpha() {
        assert_eq!(Color::rgb(0.1, 0.2, 0.3).with_alpha(0.4), Color { r: 0.1, g: 0.2, b: 0.3, a: 0.4 });
    }

    #[test]
    fn faded_multiplies_alpha() {
        assert_eq!(Color::WHITE.with_alpha(0.5).faded(0.5).a, 0.25);
    }
}
