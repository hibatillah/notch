use windows::{
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode,
    },
    Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
    core::Result,
};

use crate::gfx::ImageData;

/// Side length in pixels of the decoded album art; the page scales it down when drawing.
pub const ARTWORK_PIXELS: u32 = 192;

/// The size to decode an image of `width` x `height` at so its shorter side is `side` and its shape is kept.
fn cover_size(width: u32, height: u32, side: u32) -> (u32, u32) {
    let shorter = width.min(height).max(1);
    let scaled =
        |length: u32| ((u64::from(length) * u64::from(side) + u64::from(shorter) / 2) / u64::from(shorter)) as u32;
    (scaled(width).max(side), scaled(height).max(side))
}

/// Decodes the session's album art thumbnail to premultiplied BGRA at `ARTWORK_PIXELS`. An image that is not square
/// keeps its shape and loses its edges, so the part in the middle fills the square (like `object-fit: cover`).
pub fn read(properties: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> Result<ImageData> {
    let stream = properties.Thumbnail()?.OpenReadAsync()?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let (scaled_width, scaled_height) = cover_size(decoder.PixelWidth()?, decoder.PixelHeight()?, ARTWORK_PIXELS);
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(scaled_width)?;
    transform.SetScaledHeight(scaled_height)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )?
        .join()?
        .DetachPixelData()?;
    Ok(ImageData::new(scaled_width, scaled_height, pixels.to_vec()).centered_square())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_image_is_scaled_until_its_height_fits_the_square() {
        assert_eq!(cover_size(1280, 720, 192), (341, 192));
    }

    #[test]
    fn a_tall_image_is_scaled_until_its_width_fits_the_square() {
        assert_eq!(cover_size(600, 900, 192), (192, 288));
    }

    #[test]
    fn a_square_image_is_scaled_to_the_square() {
        assert_eq!(cover_size(640, 640, 192), (192, 192));
        assert_eq!(cover_size(100, 100, 192), (192, 192));
    }
}
