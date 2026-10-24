//! Drawing toolkit shared by every page: geometry, colors, fonts, images, springs and the canvas.

mod canvas;
mod color;
mod fonts;
mod geometry;
mod image;
mod path;
mod raster;
mod spring;
mod surface;

pub use canvas::Canvas;
pub use color::Color;
pub use fonts::{Align, FontStyle, Fonts, SEGOE_UI, Weight};
pub use geometry::{Point, Rect, Size, lerp};
pub use image::{ImageBrush, ImageData, ImageSlot};
pub use path::Path;
pub use raster::Mask;
pub use spring::Spring;
pub use surface::Surface;
