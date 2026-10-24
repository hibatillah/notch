//! What the microphone and camera activities share: a message when the device turns on or off, then a coloured dot
//! for as long as it is in use.

use crate::{
    activity::{Flash, Presence},
    gfx::{Canvas, Color, Point, Rect},
    page::{Context, TimerId},
    ui::{self, Icon},
};

const SHOW_MILLISECONDS: u32 = 2500;
const DOT_RADIUS: f32 = 4.0;
/// Width of a dot's slot in the pill: the dot and the space before the next badge.
const DOT_SLOT_WIDTH: f32 = 14.0;

/// What differs between devices.
pub struct Device {
    pub name: &'static str,
    pub icon: Icon,
    pub color: Color,
}

pub struct Privacy {
    device: Device,
    is_active: bool,
    flash: Flash,
    message: String,
    message_width: f32,
}

impl Privacy {
    pub fn new(device: Device) -> Self {
        Self { device, is_active: false, flash: Flash::default(), message: String::new(), message_width: 0.0 }
    }

    /// Takes the state found at start without announcing it; a device already in use just shows its dot.
    pub fn initialize(&mut self, is_active: bool) {
        self.is_active = is_active;
    }

    /// Announces a change of state; the same state again does nothing.
    pub fn update(&mut self, is_active: bool, context: &mut Context) {
        if is_active == self.is_active {
            return;
        }
        self.is_active = is_active;
        self.message = message_for(self.device.name, is_active);
        self.message_width = ui::message_width(context.fonts(), &self.message, None);
        self.flash.show(context, SHOW_MILLISECONDS);
    }

    pub fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        self.flash.on_timer(timer, context);
    }

    /// The message while it is fresh, then the dot while the device stays in use.
    pub fn presence(&self) -> Option<Presence> {
        if self.flash.is_visible() {
            Some(Presence::Transient { width: self.message_width })
        } else if self.is_active {
            Some(Presence::Badge { width: DOT_SLOT_WIDTH })
        } else {
            None
        }
    }

    pub fn draw(&self, canvas: &Canvas, area: Rect, opacity: f32) {
        if self.flash.is_visible() {
            let tint = if self.is_active { self.device.color } else { Color::gray(0.7) };
            ui::draw_message(canvas, area, opacity, self.device.icon, tint, &self.message, None);
        } else {
            let radius = DOT_RADIUS * canvas.scale();
            let center = Point::new(area.left + radius, area.center_y());
            canvas.fill_circle(center, radius, self.device.color.faded(opacity));
        }
    }
}

fn message_for(name: &str, is_active: bool) -> String {
    format!("{name} {}", if is_active { "on" } else { "off" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn microphone() -> Privacy {
        Privacy::new(Device { name: "Microphone", icon: Icon::Microphone, color: Color::rgb(1.0, 0.62, 0.04) })
    }

    #[test]
    fn messages_name_the_device_and_its_state() {
        assert_eq!(message_for("Microphone", true), "Microphone on");
        assert_eq!(message_for("Camera", false), "Camera off");
    }

    #[test]
    fn a_device_in_use_at_start_shows_only_its_dot() {
        let mut privacy = microphone();
        privacy.initialize(true);
        assert_eq!(privacy.presence(), Some(Presence::Badge { width: DOT_SLOT_WIDTH }));
    }

    #[test]
    fn a_device_not_in_use_shows_nothing() {
        assert_eq!(microphone().presence(), None);
    }
}
