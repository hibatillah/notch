//! The two shapes a short-lived message takes in the collapsed pill: an icon with a level bar and a value (volume,
//! brightness), and an icon with a message and an optional value (charger, microphone, copied).

use super::Icon;
use crate::gfx::{Align, Canvas, Color, FontStyle, Fonts, Rect, SEGOE_UI, Weight};

/// Width of a level message, in device-independent pixels.
pub const LEVEL_WIDTH: f32 = 176.0;

const PADDING: f32 = 14.0;
const ICON_SIZE: f32 = 18.0;
const GAP: f32 = 10.0;
/// Room for the longest value shown next to a level bar, "Muted".
const LEVEL_VALUE_WIDTH: f32 = 40.0;
const BAR_THICKNESS: f32 = 4.0;
const TRACK_ALPHA: f32 = 0.22;
const MESSAGE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
const VALUE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::Regular, Align::Trailing);

/// Width of a message with an optional value, in device-independent pixels.
pub fn message_width(fonts: &Fonts, message: &str, value: Option<&str>) -> f32 {
    let value_span = value.map_or(0.0, |value| GAP + fonts.measure(&VALUE, value).ceil());
    PADDING + ICON_SIZE + GAP + fonts.measure(&MESSAGE, message).ceil() + value_span + PADDING
}

/// Icon, message and value, vertically centered in `area`.
pub fn draw_message(
    canvas: &Canvas,
    area: Rect,
    opacity: f32,
    icon: Icon,
    tint: Color,
    message: &str,
    value: Option<&str>,
) {
    let scale = canvas.scale();
    draw_icon(canvas, area, opacity, icon, tint);
    let text_left = area.left + (PADDING + ICON_SIZE + GAP) * scale;
    let text_right = area.right - PADDING * scale;
    canvas.text(
        &MESSAGE,
        message,
        Rect::new(text_left, area.top, text_right, area.bottom),
        Color::WHITE.faded(opacity),
    );
    if let Some(value) = value {
        canvas.text(
            &VALUE,
            value,
            Rect::new(text_left, area.top, text_right, area.bottom),
            Color::gray(0.7).faded(opacity),
        );
    }
}

/// Icon, a bar filled to `fraction` and the value, vertically centered in `area`.
pub fn draw_level(canvas: &Canvas, area: Rect, opacity: f32, icon: Icon, fraction: f32, value: &str) {
    let scale = canvas.scale();
    draw_icon(canvas, area, opacity, icon, Color::WHITE);
    let bar_left = area.left + (PADDING + ICON_SIZE + GAP) * scale;
    let value_left = area.right - (PADDING + LEVEL_VALUE_WIDTH) * scale;
    let bar_right = value_left - GAP * scale;
    let half_thickness = BAR_THICKNESS * scale / 2.0;
    let center_y = area.center_y();
    let track = Rect::new(bar_left, center_y - half_thickness, bar_right, center_y + half_thickness);
    canvas.fill_rounded_rect(track, half_thickness, Color::WHITE.with_alpha(TRACK_ALPHA * opacity));
    let fill_right = bar_left + (bar_right - bar_left) * fraction.clamp(0.0, 1.0);
    if fill_right - bar_left > half_thickness {
        canvas.fill_rounded_rect(Rect { right: fill_right, ..track }, half_thickness, Color::WHITE.with_alpha(opacity));
    }
    let value_area = Rect::new(value_left, area.top, area.right - PADDING * scale, area.bottom);
    canvas.text(&VALUE, value, value_area, Color::WHITE.faded(opacity));
}

fn draw_icon(canvas: &Canvas, area: Rect, opacity: f32, icon: Icon, tint: Color) {
    let scale = canvas.scale();
    let center = crate::gfx::Point::new(area.left + (PADDING + ICON_SIZE / 2.0) * scale, area.center_y());
    icon.draw(canvas, center, ICON_SIZE * scale, tint, opacity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_longer_message_makes_a_wider_pill() {
        let fonts = Fonts::new(1.0).unwrap();
        assert!(message_width(&fonts, "Microphone on", None) > message_width(&fonts, "Copied", None));
    }

    #[test]
    fn a_value_adds_width_to_a_message() {
        let fonts = Fonts::new(1.0).unwrap();
        assert!(message_width(&fonts, "Charging", Some("56%")) > message_width(&fonts, "Charging", None));
    }

    #[test]
    fn a_message_without_text_still_has_room_for_its_icon() {
        let fonts = Fonts::new(1.0).unwrap();
        assert!(message_width(&fonts, "", None) >= 2.0 * PADDING + ICON_SIZE);
    }
}
