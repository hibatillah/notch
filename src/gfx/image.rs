use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use windows::{
    Win32::Graphics::{
        Direct2D::{
            Common::{D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT},
            D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_BITMAP_PROPERTIES, ID2D1BitmapBrush, ID2D1DCRenderTarget,
        },
        Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    },
    core::Result,
};

static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

/// Decoded image pixels in premultiplied BGRA, ready to upload. Every image gets a unique `version`.
pub struct ImageData {
    pub version: u64,
    pub width: u32,
    pub height: u32,
    pub bgra_premultiplied: Vec<u8>,
}

impl ImageData {
    pub fn new(width: u32, height: u32, bgra_premultiplied: Vec<u8>) -> Self {
        Self { version: NEXT_VERSION.fetch_add(1, Ordering::Relaxed), width, height, bgra_premultiplied }
    }

    /// A copy shrunk to `target_width` by averaging every source pixel under each new pixel.
    /// Drawing a large image into a small area directly skips pixels and looks jagged; shrinking first keeps it smooth.
    pub fn downscaled(&self, target_width: u32) -> ImageData {
        let target_width = target_width.max(1);
        if target_width >= self.width {
            return ImageData::new(self.width, self.height, self.bgra_premultiplied.clone());
        }
        let ratio_x = self.width as f32 / target_width as f32;
        let target_height = ((self.height as f32 / ratio_x).round() as u32).max(1);
        let ratio_y = self.height as f32 / target_height as f32;

        let mut pixels = vec![0u8; (target_width * target_height * 4) as usize];
        for target_y in 0..target_height {
            let (top, bottom) = (target_y as f32 * ratio_y, (target_y + 1) as f32 * ratio_y);
            for target_x in 0..target_width {
                let (left, right) = (target_x as f32 * ratio_x, (target_x + 1) as f32 * ratio_x);
                let mut sum = [0.0f32; 4];
                let mut total_weight = 0.0f32;
                for source_y in top.floor() as u32..(bottom.ceil() as u32).min(self.height) {
                    let weight_y = (bottom.min((source_y + 1) as f32) - top.max(source_y as f32)).max(0.0);
                    for source_x in left.floor() as u32..(right.ceil() as u32).min(self.width) {
                        let weight = weight_y * (right.min((source_x + 1) as f32) - left.max(source_x as f32)).max(0.0);
                        let index = ((source_y * self.width + source_x) * 4) as usize;
                        for (channel, total) in sum.iter_mut().enumerate() {
                            *total += f32::from(self.bgra_premultiplied[index + channel]) * weight;
                        }
                        total_weight += weight;
                    }
                }
                let index = ((target_y * target_width + target_x) * 4) as usize;
                for (channel, total) in sum.iter().enumerate() {
                    pixels[index + channel] = (total / total_weight.max(f32::EPSILON)).round() as u8;
                }
            }
        }
        ImageData::new(target_width, target_height, pixels)
    }
}

/// An image uploaded to the renderer, drawable into any rounded rectangle with `Canvas::image`.
pub struct ImageBrush {
    brush: ID2D1BitmapBrush,
    pub width: u32,
}

impl ImageBrush {
    pub(super) fn new(brush: ID2D1BitmapBrush, width: u32) -> Self {
        Self { brush, width }
    }

    pub(super) fn brush(&self) -> &ID2D1BitmapBrush {
        &self.brush
    }
}

/// Uploads decoded pixels as a brush that can fill any shape, such as a rounded square.
pub(super) fn upload(target: &ID2D1DCRenderTarget, image: &ImageData) -> Result<ImageBrush> {
    unsafe {
        let properties = D2D1_BITMAP_PROPERTIES {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
        };
        let bitmap = target.CreateBitmap(
            D2D_SIZE_U { width: image.width, height: image.height },
            Some(image.bgra_premultiplied.as_ptr().cast()),
            image.width * 4,
            &properties,
        )?;
        let brush = target.CreateBitmapBrush(&bitmap, None, None)?;
        brush.SetInterpolationMode(D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
        Ok(ImageBrush::new(brush, image.width))
    }
}

/// Uploaded images that are drawn again and again, such as icons, found by a key that stands for how they look.
#[derive(Default)]
pub struct ImageCache {
    entries: RefCell<HashMap<u64, Rc<ImageBrush>>>,
}

impl ImageCache {
    /// The image stored under `key`, uploading the one `build` makes the first time it is asked for.
    pub(super) fn get_or_insert(&self, key: u64, build: impl FnOnce() -> Option<ImageBrush>) -> Option<Rc<ImageBrush>> {
        if let Some(brush) = self.entries.borrow().get(&key) {
            return Some(Rc::clone(brush));
        }
        let brush = Rc::new(build()?);
        self.entries.borrow_mut().insert(key, Rc::clone(&brush));
        Some(brush)
    }
}

/// Keeps an uploaded copy of an image that changes over time, re-uploading only when the image is replaced.
#[derive(Default)]
pub struct ImageSlot {
    version: u64,
    brush: Option<ImageBrush>,
}

impl ImageSlot {
    pub fn brush(&self) -> Option<&ImageBrush> {
        self.brush.as_ref()
    }

    /// Makes the slot match `source`; `upload` runs only when the image differs from the one already uploaded.
    pub fn sync(&mut self, source: Option<&ImageData>, upload: impl FnOnce(&ImageData) -> Option<ImageBrush>) {
        let version = source.map_or(0, |image| image.version);
        if version == self.version {
            return;
        }
        self.version = version;
        self.brush = source.and_then(upload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> ImageData {
        ImageData::new(1, 1, vec![0; 4])
    }

    #[test]
    fn shrinking_averages_the_pixels_under_each_new_pixel() {
        // A 2x2 image of black and white pixels becomes one mid-gray pixel.
        let pixels = vec![0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255];
        let shrunk = ImageData::new(2, 2, pixels).downscaled(1);
        assert_eq!((shrunk.width, shrunk.height), (1, 1));
        assert_eq!(shrunk.bgra_premultiplied, vec![128, 128, 128, 255]);
    }

    #[test]
    fn shrinking_keeps_a_flat_color_unchanged() {
        let flat: Vec<u8> = [10, 20, 30, 255].repeat(36 * 36);
        let shrunk = ImageData::new(36, 36, flat).downscaled(10);
        assert_eq!((shrunk.width, shrunk.height), (10, 10));
        assert!(shrunk.bgra_premultiplied.chunks_exact(4).all(|pixel| pixel == [10, 20, 30, 255]));
    }

    #[test]
    fn an_image_already_small_enough_keeps_its_size() {
        assert_eq!(image().downscaled(8).width, 1);
    }

    #[test]
    fn every_image_gets_its_own_version() {
        assert_ne!(image().version, image().version);
    }

    #[test]
    fn a_slot_uploads_only_when_the_image_changes() {
        let (first, second) = (image(), image());
        let mut slot = ImageSlot::default();
        let mut uploads = 0;

        slot.sync(Some(&first), |_| {
            uploads += 1;
            None
        });
        slot.sync(Some(&first), |_| {
            uploads += 1;
            None
        });
        assert_eq!(uploads, 1, "the same image must not be uploaded twice");

        slot.sync(Some(&second), |_| {
            uploads += 1;
            None
        });
        assert_eq!(uploads, 2);

        slot.sync(None, |_| {
            uploads += 1;
            None
        });
        assert_eq!(uploads, 2, "clearing needs no upload");
        assert!(slot.brush().is_none());
    }
}
