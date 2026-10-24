//! Anti-aliased shapes drawn on the CPU into small images, so icons and corner fillets need no Direct2D geometry
//! (which costs about 10 MB of memory the first time it is used).

use super::{geometry::Point, image::ImageData, path::Figure};

/// How much of each pixel a shape covers, from 0 to 1.
pub struct Mask {
    width: u32,
    height: u32,
    coverage: Vec<f32>,
}

impl Mask {
    /// Builds a mask from a signed distance to a shape's edge: negative inside, positive outside, in pixels.
    /// A pixel whose center is on the edge is half covered, which gives smooth edges.
    pub fn from_distance(width: u32, height: u32, distance: impl Fn(Point) -> f32) -> Self {
        let mut coverage = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                let center = Point::new(x as f32 + 0.5, y as f32 + 0.5);
                coverage.push((0.5 - distance(center)).clamp(0.0, 1.0));
            }
        }
        Self { width, height, coverage }
    }

    /// The inside of `figures`; open figures count as closed.
    pub fn fill(width: u32, height: u32, figures: &[Figure]) -> Self {
        Self::from_distance(width, height, |point| signed_distance(figures, point))
    }

    /// A line of `half_width` on each side along `figures`, with rounded ends and corners.
    pub fn stroke(width: u32, height: u32, figures: &[Figure], half_width: f32) -> Self {
        Self::from_distance(width, height, |point| edge_distance(figures, point, false) - half_width)
    }

    #[cfg(test)]
    fn coverage_at(&self, x: u32, y: u32) -> f32 {
        self.coverage[(y * self.width + x) as usize]
    }

    /// The mask as an image of one color, in the premultiplied BGRA order Direct2D expects.
    pub fn tinted(&self, red: f32, green: f32, blue: f32) -> ImageData {
        let mut pixels = Vec::with_capacity(self.coverage.len() * 4);
        for &coverage in &self.coverage {
            for channel in [blue, green, red, 1.0] {
                pixels.push((channel * coverage * 255.0).round() as u8);
            }
        }
        ImageData::new(self.width, self.height, pixels)
    }
}

/// Distance from `point` to the nearest edge of any figure; see `edges` for `closes_open_figures`.
fn edge_distance(figures: &[Figure], point: Point, closes_open_figures: bool) -> f32 {
    edges(figures, closes_open_figures)
        .map(|(start, end)| distance_to_segment(point, start, end))
        .fold(f32::MAX, f32::min)
}

/// Distance to the nearest edge, negative when `point` is inside the figures.
fn signed_distance(figures: &[Figure], point: Point) -> f32 {
    let distance = edge_distance(figures, point, true);
    if is_inside(figures, point) { -distance } else { distance }
}

/// Every edge of every figure. The edge back to the start is included for closed figures, and for open ones when
/// `closes_open_figures` is set, which a fill needs and a stroked line must not have.
fn edges(figures: &[Figure], closes_open_figures: bool) -> impl Iterator<Item = (Point, Point)> + '_ {
    figures.iter().flat_map(move |figure| {
        let count = figure.points.len();
        let closes = figure.is_closed || closes_open_figures;
        let edge_count = if closes { count } else { count.saturating_sub(1) };
        (0..edge_count).map(move |index| {
            let end_index = if closes { (index + 1) % count } else { index + 1 };
            (figure.points[index], figure.points[end_index])
        })
    })
}

/// Even-odd rule: a point is inside when a ray to the right crosses an odd number of edges.
fn is_inside(figures: &[Figure], point: Point) -> bool {
    let mut crossings = 0;
    for (start, end) in edges(figures, true) {
        let straddles = (start.y > point.y) != (end.y > point.y);
        if straddles {
            let crossing_x = start.x + (point.y - start.y) / (end.y - start.y) * (end.x - start.x);
            if crossing_x > point.x {
                crossings += 1;
            }
        }
    }
    crossings % 2 == 1
}

fn distance_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let (edge_x, edge_y) = (end.x - start.x, end.y - start.y);
    let length_squared = edge_x * edge_x + edge_y * edge_y;
    let along = if length_squared == 0.0 {
        0.0
    } else {
        (((point.x - start.x) * edge_x + (point.y - start.y) * edge_y) / length_squared).clamp(0.0, 1.0)
    };
    (point.x - (start.x + edge_x * along)).hypot(point.y - (start.y + edge_y * along))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(left: f32, top: f32, right: f32, bottom: f32) -> Figure {
        let points =
            vec![Point::new(left, top), Point::new(right, top), Point::new(right, bottom), Point::new(left, bottom)];
        Figure { points, is_closed: true }
    }

    #[test]
    fn a_filled_square_covers_its_inside_and_nothing_outside() {
        let mask = Mask::fill(10, 10, &[square(2.0, 2.0, 8.0, 8.0)]);
        assert_eq!(mask.coverage_at(5, 5), 1.0);
        assert_eq!(mask.coverage_at(0, 0), 0.0);
        assert_eq!(mask.coverage_at(9, 5), 0.0);
    }

    #[test]
    fn an_edge_through_a_pixel_covers_it_partly() {
        // An edge at x = 5.25 leaves the quarter of pixel 5 that lies left of it covered.
        let mask = Mask::fill(10, 10, &[square(0.0, 0.0, 5.25, 10.0)]);
        let coverage = mask.coverage_at(5, 5);
        assert!((coverage - 0.25).abs() < 0.01, "{coverage}");
    }

    #[test]
    fn a_hole_inside_a_figure_stays_empty() {
        let mask = Mask::fill(12, 12, &[square(0.0, 0.0, 12.0, 12.0), square(4.0, 4.0, 8.0, 8.0)]);
        assert_eq!(mask.coverage_at(1, 1), 1.0);
        assert_eq!(mask.coverage_at(6, 6), 0.0);
    }

    #[test]
    fn a_stroked_line_is_as_wide_as_asked_with_round_ends() {
        let line = Figure { points: vec![Point::new(4.0, 5.0), Point::new(16.0, 5.0)], is_closed: false };
        let mask = Mask::stroke(20, 10, &[line], 1.0);
        assert_eq!(mask.coverage_at(10, 4), 1.0);
        assert_eq!(mask.coverage_at(10, 5), 1.0);
        assert_eq!(mask.coverage_at(10, 2), 0.0);
        // The round end reaches one half-width past the last point, and no further.
        assert!(mask.coverage_at(16, 4) > 0.5);
        assert_eq!(mask.coverage_at(18, 4), 0.0);
    }

    #[test]
    fn a_tinted_mask_is_premultiplied() {
        let mask = Mask::from_distance(1, 1, |_| 0.0);
        let image = mask.tinted(1.0, 0.0, 0.0);
        // Half covered: blue 0, green 0, red 128, alpha 128.
        assert_eq!(image.bgra_premultiplied, vec![0, 0, 128, 128]);
    }
}
