use super::icons::Icon;
use crate::gfx::{Canvas, Color, Point, Rect};

/// Half the side of the square that holds the icon and answers clicks.
const HIT_HALF: f32 = 17.0;
/// Half the side of the faint background shown under the pointer.
const WASH_HALF: f32 = 15.0;
const WASH_RADIUS: f32 = 8.0;
const WASH_ALPHA: f32 = 0.12;

/// An icon button with a ghost-style hover: no background until the pointer is over it, then a faint rounded wash.
pub struct IconButton {
    pub icon: Icon,
    pub center: Point,
    pub color: Color,
    pub is_hovered: bool,
}

impl IconButton {
    pub fn draw(&self, canvas: &Canvas, opacity: f32) {
        let scale = canvas.scale();
        if self.is_hovered {
            let wash = Rect::around(self.center, WASH_HALF * scale, WASH_HALF * scale);
            canvas.fill_rounded_rect(wash, WASH_RADIUS * scale, Color::WHITE.with_alpha(WASH_ALPHA * opacity));
        }
        self.icon.draw(canvas, self.center, self.icon.size() * scale, self.color, opacity);
    }

    /// Whether `point` is on a button centered at `center`.
    pub fn is_hit(center: Point, scale: f32, point: Point) -> bool {
        Rect::around(center, HIT_HALF * scale, HIT_HALF * scale).contains(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_answers_clicks_in_a_square_around_its_center() {
        let center = Point::new(100.0, 100.0);
        assert!(IconButton::is_hit(center, 1.0, Point::new(100.0, 100.0)));
        assert!(IconButton::is_hit(center, 1.0, Point::new(116.0, 84.0)));
        assert!(!IconButton::is_hit(center, 1.0, Point::new(125.0, 100.0)));
    }

    #[test]
    fn the_hit_area_grows_with_the_display_scale() {
        let center = Point::new(100.0, 100.0);
        assert!(!IconButton::is_hit(center, 1.0, Point::new(130.0, 100.0)));
        assert!(IconButton::is_hit(center, 2.0, Point::new(130.0, 100.0)));
    }

    #[test]
    fn every_icon_fits_inside_the_hit_area() {
        assert!(Icon::Play.size() / 2.0 <= HIT_HALF);
    }
}
