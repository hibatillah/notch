use std::{cell::RefCell, collections::HashMap};

use windows::{
    Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
        DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        DWRITE_WORD_WRAPPING_NO_WRAP, DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory,
        IDWriteTextFormat,
    },
    core::{HSTRING, Result, w},
};

pub const SEGOE_UI: &str = "Segoe UI";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    SemiBold,
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
    scale: f32,
    cache: RefCell<HashMap<StyleKey, IDWriteTextFormat>>,
}

impl Fonts {
    pub fn new(scale: f32) -> Result<Self> {
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        Ok(Self { factory, scale, cache: RefCell::new(HashMap::new()) })
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

    /// Width of `text` in device-independent pixels when drawn with `style` on one line.
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
            Weight::SemiBold => DWRITE_FONT_WEIGHT_SEMI_BOLD,
        };
        let alignment: DWRITE_TEXT_ALIGNMENT = match style.align {
            Align::Leading => DWRITE_TEXT_ALIGNMENT_LEADING,
            Align::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            Align::Trailing => DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        unsafe {
            let format = self.factory.CreateTextFormat(
                &HSTRING::from(style.family),
                None,
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
