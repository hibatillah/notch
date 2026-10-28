use super::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Segment {
    MoveTo(Point),
    LineTo(Point),
    CubicTo(Point, Point, Point),
    Close,
}

const CURVE_STEPS: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub points: Vec<Point>,
    pub is_closed: bool,
}

/// A sequence of lines and curves in device pixels, turned into pixels by `Mask`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub(super) segments: Vec<Segment>,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, point: Point) -> Self {
        self.segments.push(Segment::MoveTo(point));
        self
    }

    pub fn line_to(mut self, point: Point) -> Self {
        self.segments.push(Segment::LineTo(point));
        self
    }

    pub fn curve_to(mut self, first_control: Point, second_control: Point, end: Point) -> Self {
        self.segments.push(Segment::CubicTo(first_control, second_control, end));
        self
    }

    pub fn append(&mut self, other: Path) {
        self.segments.extend(other.segments);
    }

    pub fn close(mut self) -> Self {
        self.segments.push(Segment::Close);
        self
    }

    /// A closed figure through `points` with each corner rounded by `radius` (less on short edges).
    pub fn rounded_polygon(points: &[Point], radius: f32) -> Self {
        let mut path = Self::new();
        let count = points.len();
        for (index, &corner) in points.iter().enumerate() {
            let before = toward(corner, points[(index + count - 1) % count], radius);
            let after = toward(corner, points[(index + 1) % count], radius);
            path = if index == 0 { path.move_to(before) } else { path.line_to(before) };
            // A quadratic curve with the corner as its control point, written as the equivalent cubic.
            let first_control =
                Point::new(before.x + (corner.x - before.x) * 2.0 / 3.0, before.y + (corner.y - before.y) * 2.0 / 3.0);
            let second_control =
                Point::new(after.x + (corner.x - after.x) * 2.0 / 3.0, after.y + (corner.y - after.y) * 2.0 / 3.0);
            path = path.curve_to(first_control, second_control, after);
        }
        path.close()
    }

    pub fn polyline(points: &[Point]) -> Self {
        let mut path = Self::new();
        for (index, &point) in points.iter().enumerate() {
            path = if index == 0 { path.move_to(point) } else { path.line_to(point) };
        }
        path
    }

    pub fn flatten(&self) -> Vec<Figure> {
        let mut figures: Vec<Figure> = Vec::new();
        let mut current: Option<Figure> = None;
        for segment in &self.segments {
            match *segment {
                Segment::MoveTo(point) => {
                    figures.extend(current.take());
                    current = Some(Figure { points: vec![point], is_closed: false });
                }
                Segment::LineTo(point) => {
                    if let Some(figure) = current.as_mut() {
                        figure.points.push(point);
                    }
                }
                Segment::CubicTo(first, second, end) => {
                    if let Some(figure) = current.as_mut() {
                        let start = *figure.points.last().unwrap_or(&first);
                        for step in 1..=CURVE_STEPS {
                            figure.points.push(cubic_point(
                                start,
                                first,
                                second,
                                end,
                                step as f32 / CURVE_STEPS as f32,
                            ));
                        }
                    }
                }
                Segment::Close => {
                    if let Some(mut figure) = current.take() {
                        figure.is_closed = true;
                        figures.push(figure);
                    }
                }
            }
        }
        figures.extend(current);
        figures
    }

    #[cfg(test)]
    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        self.segments.iter().flat_map(|segment| match *segment {
            Segment::MoveTo(point) | Segment::LineTo(point) => vec![point],
            Segment::CubicTo(first, second, end) => vec![first, second, end],
            Segment::Close => Vec::new(),
        })
    }
}

fn cubic_point(start: Point, first: Point, second: Point, end: Point, amount: f32) -> Point {
    let remaining = 1.0 - amount;
    let weights =
        [remaining.powi(3), 3.0 * remaining.powi(2) * amount, 3.0 * remaining * amount.powi(2), amount.powi(3)];
    let points = [start, first, second, end];
    Point::new(
        points.iter().zip(weights).map(|(point, weight)| point.x * weight).sum(),
        points.iter().zip(weights).map(|(point, weight)| point.y * weight).sum(),
    )
}

/// The point `distance` away from `from` along the edge to `to`, but never past the edge's midpoint.
fn toward(from: Point, to: Point, distance: f32) -> Point {
    let (delta_x, delta_y) = (to.x - from.x, to.y - from.y);
    let length = delta_x.hypot(delta_y);
    if length == 0.0 {
        return from;
    }
    let step = distance.min(length / 2.0) / length;
    Point::new(from.x + delta_x * step, from.y + delta_y * step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_stays_inside_the_polygon_bounds() {
        let square = [Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)];
        for point in Path::rounded_polygon(&square, 3.0).points() {
            assert!((0.0..=10.0).contains(&point.x) && (0.0..=10.0).contains(&point.y));
        }
    }

    #[test]
    fn a_corner_is_never_rounded_past_half_of_a_short_edge() {
        let edge_end = toward(Point::new(0.0, 0.0), Point::new(4.0, 0.0), 100.0);
        assert_eq!(edge_end, Point::new(2.0, 0.0));
    }

    #[test]
    fn a_rounded_polygon_is_closed_and_a_polyline_is_not() {
        let points = [Point::new(0.0, 0.0), Point::new(4.0, 0.0), Point::new(4.0, 4.0)];
        assert_eq!(Path::rounded_polygon(&points, 1.0).segments.last(), Some(&Segment::Close));
        assert_ne!(Path::polyline(&points).segments.last(), Some(&Segment::Close));
    }

    #[test]
    fn flattening_replaces_a_curve_with_short_lines_ending_at_its_end() {
        let curve = Path::new().move_to(Point::new(0.0, 0.0)).curve_to(
            Point::new(0.0, 10.0),
            Point::new(10.0, 10.0),
            Point::new(10.0, 0.0),
        );
        let figures = curve.flatten();
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].points.len(), 1 + CURVE_STEPS);
        assert_eq!(figures[0].points.last(), Some(&Point::new(10.0, 0.0)));
        assert!(!figures[0].is_closed);
    }

    #[test]
    fn each_move_starts_a_new_figure_and_close_closes_it() {
        let path = Path::new()
            .move_to(Point::new(0.0, 0.0))
            .line_to(Point::new(4.0, 0.0))
            .line_to(Point::new(4.0, 4.0))
            .close()
            .move_to(Point::new(8.0, 8.0))
            .line_to(Point::new(9.0, 9.0));
        let figures = path.flatten();
        assert_eq!(figures.len(), 2);
        assert!(figures[0].is_closed && !figures[1].is_closed);
    }

    #[test]
    fn points_include_curve_control_points() {
        let path = Path::new().move_to(Point::new(0.0, 0.0)).curve_to(
            Point::new(1.0, 5.0),
            Point::new(2.0, 5.0),
            Point::new(3.0, 0.0),
        );
        assert_eq!(path.points().count(), 4);
    }
}
