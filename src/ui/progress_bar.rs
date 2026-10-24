use crate::gfx::{Canvas, Color, Point, Rect, lerp};

const THICKNESS: f32 = 4.0;
const THUMB_RADIUS: f32 = 4.5;
/// The thumb grows while it is being dragged.
const DRAGGED_THUMB_RADIUS: f32 = 6.5;
const TRACK_ALPHA: f32 = 0.22;
const HIT_HALF_HEIGHT: f32 = 14.0;
const HIT_SLACK: f32 = 8.0;

/// A horizontal progress bar that can also be scrubbed. Coordinates are device pixels.
pub struct ProgressBar {
    pub left: f32,
    pub right: f32,
    pub center_y: f32,
    pub scale: f32,
}

impl ProgressBar {
    pub fn draw(&self, canvas: &Canvas, fraction: f32, is_dragged: bool, opacity: f32) {
        let half_thickness = THICKNESS * self.scale / 2.0;
        let fill_right = lerp(self.left, self.right, fraction.clamp(0.0, 1.0));
        let track = Rect::new(self.left, self.center_y - half_thickness, self.right, self.center_y + half_thickness);
        canvas.fill_rounded_rect(track, half_thickness, Color::WHITE.with_alpha(TRACK_ALPHA * opacity));
        canvas.fill_rounded_rect(Rect { right: fill_right, ..track }, half_thickness, Color::WHITE.with_alpha(opacity));
        let thumb_radius = if is_dragged { DRAGGED_THUMB_RADIUS } else { THUMB_RADIUS } * self.scale;
        canvas.fill_circle(Point::new(fill_right, self.center_y), thumb_radius, Color::WHITE.with_alpha(opacity));
    }

    /// Maps an x coordinate to a position along the bar, from 0 to 1.
    pub fn fraction_at(&self, x: f32) -> f32 {
        ((x - self.left) / (self.right - self.left)).clamp(0.0, 1.0)
    }

    /// Whether `point` is close enough to the bar to grab it.
    pub fn is_hit(&self, point: Point) -> bool {
        let slack = HIT_SLACK * self.scale;
        point.x >= self.left - slack
            && point.x <= self.right + slack
            && (point.y - self.center_y).abs() <= HIT_HALF_HEIGHT * self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> ProgressBar {
        ProgressBar { left: 100.0, right: 300.0, center_y: 50.0, scale: 1.0 }
    }

    #[test]
    fn fraction_follows_the_pointer_along_the_bar() {
        assert_eq!(bar().fraction_at(100.0), 0.0);
        assert_eq!(bar().fraction_at(200.0), 0.5);
        assert_eq!(bar().fraction_at(300.0), 1.0);
    }

    #[test]
    fn fraction_is_clamped_outside_the_bar() {
        assert_eq!(bar().fraction_at(0.0), 0.0);
        assert_eq!(bar().fraction_at(999.0), 1.0);
    }

    #[test]
    fn the_bar_can_be_grabbed_a_little_beyond_its_ends_and_thickness() {
        assert!(bar().is_hit(Point::new(200.0, 50.0)));
        assert!(bar().is_hit(Point::new(95.0, 60.0)));
        assert!(!bar().is_hit(Point::new(200.0, 80.0)));
        assert!(!bar().is_hit(Point::new(50.0, 50.0)));
    }
}
