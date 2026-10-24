use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, POINT, RECT, SIZE},
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                D2D1_ANTIALIAS_MODE_ALIASED, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
                D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE, D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
                ID2D1SolidColorBrush,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, CreateCompatibleDC,
                CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC, HGDIOBJ, SelectObject,
            },
        },
        UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow},
    },
    core::Result,
};

use super::{
    canvas::Canvas,
    fonts::Fonts,
    image::{self, ImageBrush, ImageCache, ImageData},
};

/// Off-screen premultiplied-alpha bitmap that Direct2D draws into and a layered window displays.
pub struct Surface {
    pub width: i32,
    pub height: i32,
    memory_context: HDC,
    bitmap: HBITMAP,
    /// The bitmap's pixels, premultiplied BGRA, `width * height * 4` bytes.
    #[cfg(test)]
    pixels: *const u8,
    previous_object: HGDIOBJ,
    target: ID2D1DCRenderTarget,
    brush: ID2D1SolidColorBrush,
    image_cache: ImageCache,
}

impl Surface {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                // 96 dpi keeps one drawing unit equal to one pixel; callers apply display scaling themselves.
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory.CreateDCRenderTarget(&properties)?;
            let brush = target.CreateSolidColorBrush(&D2D1_COLOR_F { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }, None)?;

            let memory_context = CreateCompatibleDC(None);
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixel_pointer = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(Some(memory_context), &bitmap_info, DIB_RGB_COLORS, &mut pixel_pointer, None, 0)?;
            let previous_object = SelectObject(memory_context, bitmap.into());

            Ok(Self {
                width,
                height,
                memory_context,
                bitmap,
                #[cfg(test)]
                pixels: pixel_pointer.cast::<u8>(),
                previous_object,
                target,
                brush,
                image_cache: ImageCache::default(),
            })
        }
    }

    /// The pixels as last drawn, premultiplied BGRA; only used to render the notch off-screen for review.
    #[cfg(test)]
    pub fn read_pixels(&self) -> &[u8] {
        // SAFETY: the pixels belong to the DIB section, which lives as long as the surface.
        unsafe { std::slice::from_raw_parts(self.pixels, self.width as usize * self.height as usize * 4) }
    }

    /// Uploads decoded pixels as a brush that can fill any shape, such as a rounded square.
    pub fn create_image_brush(&self, image: &ImageData) -> Result<ImageBrush> {
        image::upload(&self.target, image)
    }

    /// Redraws only `region` of the bitmap, then shows just that region as the layered window.
    /// The window shrinks and grows with the pill, so the system composes far fewer pixels per frame.
    pub fn render_and_present(
        &self,
        window: HWND,
        screen_position: POINT,
        region: RECT,
        fonts: &Fonts,
        scale: f32,
        draw: impl FnOnce(&Canvas),
    ) -> Result<()> {
        unsafe {
            let bounds = RECT { left: 0, top: 0, right: self.width, bottom: self.height };
            self.target.BindDC(self.memory_context, &bounds)?;
            self.target.BeginDraw();
            self.target.PushAxisAlignedClip(
                &D2D_RECT_F {
                    left: region.left as f32,
                    top: region.top as f32,
                    right: region.right as f32,
                    bottom: region.bottom as f32,
                },
                D2D1_ANTIALIAS_MODE_ALIASED,
            );
            self.target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            draw(&Canvas::new(&self.target, &self.brush, &self.image_cache, fonts, scale));
            self.target.PopAxisAlignedClip();
            self.target.EndDraw(None, None)?;

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                window,
                None,
                Some(&POINT { x: screen_position.x + region.left, y: screen_position.y + region.top }),
                Some(&SIZE { cx: region.right - region.left, cy: region.bottom - region.top }),
                Some(self.memory_context),
                Some(&POINT { x: region.left, y: region.top }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.memory_context, self.previous_object);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.memory_context);
        }
    }
}
