use std::{cell::RefCell, collections::HashMap};

use windows::{
    Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
        DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT,
        DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
        DWRITE_TEXT_METRICS, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP,
        DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFactory5, IDWriteFontCollection,
        IDWriteFontCollection1, IDWriteInMemoryFontFileLoader, IDWriteTextFormat,
    },
    core::{HSTRING, Interface, Result, w},
};

/// Bundled under the SIL Open Font License; see assets/fonts/Geist-LICENSE.txt.
pub const UI_FONT: &str = "Geist";
/// Used when the bundled font cannot be loaded (Windows older than 10 version 1803).
const FALLBACK_FONT: &str = "Segoe UI";

const BUNDLED_FONT_FILES: [&[u8]; 2] =
    [include_bytes!("../../assets/fonts/Geist-Regular.ttf"), include_bytes!("../../assets/fonts/Geist-Medium.ttf")];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Medium,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Align {
    Leading,
    Center,
    Trailing,
}

/// A text style. Pages declare their styles as `const`s; the first use builds and caches the platform font.
#[derive(Clone, Copy, Debug)]
pub struct FontStyle {
    pub family: &'static str,
    /// Size in device-independent pixels.
    pub size: f32,
    pub weight: Weight,
    pub align: Align,
    /// Wrap long text onto more lines instead of cutting it with an ellipsis.
    pub wrap: bool,
}

impl FontStyle {
    pub const fn new(family: &'static str, size: f32, weight: Weight, align: Align) -> Self {
        Self { family, size, weight, align, wrap: false }
    }

    pub const fn wrapped(self) -> Self {
        Self { wrap: true, ..self }
    }

    fn key(&self) -> StyleKey {
        (self.family, self.size.to_bits(), self.weight, self.align, self.wrap)
    }
}

type StyleKey = (&'static str, u32, Weight, Align, bool);

/// Builds text formats on demand and measures text. Single-threaded, like the rest of the drawing code.
pub struct Fonts {
    factory: IDWriteFactory,
    bundled: Option<IDWriteFontCollection1>,
    scale: f32,
    cache: RefCell<HashMap<StyleKey, IDWriteTextFormat>>,
}

impl Fonts {
    pub fn new(scale: f32) -> Result<Self> {
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let bundled = load_bundled_fonts(&factory).ok();
        Ok(Self { factory, bundled, scale, cache: RefCell::new(HashMap::new()) })
    }

    pub(super) fn format(&self, style: &FontStyle) -> Option<IDWriteTextFormat> {
        let key = style.key();
        if let Some(format) = self.cache.borrow().get(&key) {
            return Some(format.clone());
        }
        let format = self.build(style).ok()?;
        self.cache.borrow_mut().insert(key, format.clone());
        Some(format)
    }

    /// Single-line width in device-independent pixels.
    pub fn measure(&self, style: &FontStyle, text: &str) -> f32 {
        let Some(format) = self.format(style) else { return 0.0 };
        let wide_text: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let Ok(layout) = self.factory.CreateTextLayout(&wide_text, &format, 4000.0, 200.0) else { return 0.0 };
            let mut metrics = DWRITE_TEXT_METRICS::default();
            if layout.GetMetrics(&mut metrics).is_err() {
                return 0.0;
            }
            metrics.widthIncludingTrailingWhitespace / self.scale
        }
    }

    fn build(&self, style: &FontStyle) -> Result<IDWriteTextFormat> {
        let weight: DWRITE_FONT_WEIGHT = match style.weight {
            Weight::Regular => DWRITE_FONT_WEIGHT_NORMAL,
            Weight::Medium => DWRITE_FONT_WEIGHT_MEDIUM,
        };
        let alignment: DWRITE_TEXT_ALIGNMENT = match style.align {
            Align::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
            Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            Align::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        let (family, collection) = match (&self.bundled, style.family) {
            (Some(bundled), UI_FONT) => (UI_FONT, Some(bundled.cast::<IDWriteFontCollection>()?)),
            (None, UI_FONT) => (FALLBACK_FONT, None),
            (_, family) => (family, None),
        };
        unsafe {
            let format = self.factory.CreateTextFormat(
                &HSTRING::from(family),
                collection.as_ref(),
                weight,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                style.size * self.scale,
                w!("en-us"),
            )?;
            format.SetTextAlignment(alignment)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetWordWrapping(if style.wrap {
                DWRITE_WORD_WRAPPING_WRAP
            } else {
                DWRITE_WORD_WRAPPING_NO_WRAP
            })?;
            let trimming =
                DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
            let ellipsis = self.factory.CreateEllipsisTrimmingSign(&format)?;
            format.SetTrimming(&trimming, &ellipsis)?;
            Ok(format)
        }
    }
}

fn load_bundled_fonts(factory: &IDWriteFactory) -> Result<IDWriteFontCollection1> {
    let factory: IDWriteFactory5 = factory.cast()?;
    unsafe {
        let loader: IDWriteInMemoryFontFileLoader = factory.CreateInMemoryFontFileLoader()?;
        factory.RegisterFontFileLoader(&loader)?;
        let builder = factory.CreateFontSetBuilder()?;
        for data in BUNDLED_FONT_FILES {
            let file = loader.CreateInMemoryFontFileReference(
                &factory,
                data.as_ptr().cast(),
                data.len() as u32,
                None::<&windows::core::IUnknown>,
            )?;
            builder.AddFontFile(&file)?;
        }
        factory.CreateFontCollectionFromFontSet(&builder.CreateFontSet()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_font_loads_and_is_what_text_is_measured_in() {
        let fonts = Fonts::new(1.0).unwrap();
        assert!(fonts.bundled.is_some(), "Geist should load from the executable");
        let inter = FontStyle::new(UI_FONT, 14.0, Weight::Regular, Align::Leading);
        let system = FontStyle::new(FALLBACK_FONT, 14.0, Weight::Regular, Align::Leading);
        assert_ne!(fonts.measure(&inter, "Notification"), fonts.measure(&system, "Notification"));
    }

    #[test]
    fn medium_text_is_wider_than_regular_text() {
        let fonts = Fonts::new(1.0).unwrap();
        let regular = FontStyle::new(UI_FONT, 14.0, Weight::Regular, Align::Leading);
        let medium = FontStyle::new(UI_FONT, 14.0, Weight::Medium, Align::Leading);
        assert!(fonts.measure(&medium, "Notification") > fonts.measure(&regular, "Notification"));
    }
}
