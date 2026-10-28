//! Rasterize small shapes on the CPU to avoid the initial 10 MB cost of Direct2D geometry.

use super::{geometry::Point, image::ImageData, path::Figure};

pub struct Mask {
    width: u32,
    height: u32,
    coverage: Vec<f32>,
}

impl Mask {
    /// Signed distance in pixels: negative inside, positive outside, half coverage on the edge.
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

    pub fn stroke(width: u32, height: u32, figures: &[Figure], half_width: f32) -> Self {
        Self::from_distance(width, height, |point| edge_distance(figures, point, false) - half_width)
    }

    #[cfg(test)]
    fn coverage_at(&self, x: u32, y: u32) -> f32 {
        self.coverage[(y * self.width + x) as usize]
    }

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

fn edge_distance(figures: &[Figure], point: Point, closes_open_figures: bool) -> f32 {
    edges(figures, closes_open_figures)
        .map(|(start, end)| distance_to_segment(point, start, end))
        .fold(f32::MAX, f32::min)
}

fn signed_distance(figures: &[Figure], point: Point) -> f32 {
    let distance = edge_distance(figures, point, true);
    if is_inside(figures, point) { -distance } else { distance }
}

/// Fills close open figures; strokes must leave them open.
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
        assert!(mask.coverage_at(16, 4) > 0.5);
        assert_eq!(mask.coverage_at(18, 4), 0.0);
    }

    #[test]
    fn a_tinted_mask_is_premultiplied() {
        let mask = Mask::from_distance(1, 1, |_| 0.0);
        let image = mask.tinted(1.0, 0.0, 0.0);
        assert_eq!(image.bgra_premultiplied, vec![0, 0, 128, 128]);
    }
}
