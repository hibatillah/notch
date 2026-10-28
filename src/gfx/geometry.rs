use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

pub fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn around(center: Point, half_width: f32, half_height: f32) -> Self {
        Self::new(center.x - half_width, center.y - half_height, center.x + half_width, center.y + half_height)
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn center_x(&self) -> f32 {
        (self.left + self.right) / 2.0
    }

    pub fn center_y(&self) -> f32 {
        (self.top + self.bottom) / 2.0
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.left && point.x <= self.right && point.y >= self.top && point.y <= self.bottom
    }
}

impl From<Rect> for D2D_RECT_F {
    fn from(rect: Rect) -> Self {
        D2D_RECT_F { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_blends_between_two_values() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 0.25), 2.5);
        assert_eq!(lerp(4.0, 8.0, 1.0), 8.0);
    }

    #[test]
    fn a_rect_reports_its_size_and_center() {
        let rect = Rect::new(10.0, 20.0, 50.0, 40.0);
        assert_eq!((rect.width(), rect.height()), (40.0, 20.0));
        assert_eq!((rect.center_x(), rect.center_y()), (30.0, 30.0));
    }

    #[test]
    fn a_rect_around_a_point_is_centered_on_it() {
        let rect = Rect::around(Point::new(100.0, 50.0), 20.0, 10.0);
        assert_eq!(rect, Rect::new(80.0, 40.0, 120.0, 60.0));
    }

    #[test]
    fn contains_includes_the_edges() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains(Point::new(0.0, 0.0)));
        assert!(rect.contains(Point::new(10.0, 10.0)));
        assert!(!rect.contains(Point::new(10.1, 5.0)));
        assert!(!rect.contains(Point::new(5.0, -0.1)));
    }
}
