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

/// Decodes the session's album art thumbnail to premultiplied BGRA at `ARTWORK_PIXELS`.
pub fn read(properties: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> Result<ImageData> {
    let stream = properties.Thumbnail()?.OpenReadAsync()?.join()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(ARTWORK_PIXELS)?;
    transform.SetScaledHeight(ARTWORK_PIXELS)?;
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
    Ok(ImageData::new(ARTWORK_PIXELS, ARTWORK_PIXELS, pixels.to_vec()))
}
