use crate::gfx::{Canvas, Color, Point, Rect};

/// How a dash lies on the outline, which decides its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
    /// On a corner: a small square.
    Corner,
}

/// The point at `distance` along the outline of a rounded rectangle, walking clockwise from the start of the top
/// edge, and how the outline runs there. `distance` wraps around the full length.
fn point_at(rect: Rect, radius: f32, distance: f32) -> (Point, Direction) {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let (straight_width, straight_height) = (rect.width() - 2.0 * radius, rect.height() - 2.0 * radius);
    let quarter = std::f32::consts::FRAC_PI_2 * radius;
    let length = 2.0 * (straight_width + straight_height) + 4.0 * quarter;
    let mut remaining = distance.rem_euclid(length);

    // Top edge, top-right corner, right edge, bottom-right corner, bottom edge, bottom-left corner, left edge,
    // top-left corner.
    let sides = [
        (straight_width, Some((Point::new(rect.left + radius, rect.top), 1.0, 0.0, Direction::Horizontal))),
        (quarter, None),
        (straight_height, Some((Point::new(rect.right, rect.top + radius), 0.0, 1.0, Direction::Vertical))),
        (quarter, None),
        (straight_width, Some((Point::new(rect.right - radius, rect.bottom), -1.0, 0.0, Direction::Horizontal))),
        (quarter, None),
        (straight_height, Some((Point::new(rect.left, rect.bottom - radius), 0.0, -1.0, Direction::Vertical))),
        (quarter, None),
    ];
    let corners = [
        Point::new(rect.right - radius, rect.top + radius),
        Point::new(rect.right - radius, rect.bottom - radius),
        Point::new(rect.left + radius, rect.bottom - radius),
        Point::new(rect.left + radius, rect.top + radius),
    ];
    let mut corner_index = 0;
    for (side_length, straight) in sides {
        if remaining <= side_length {
            return match straight {
                Some((start, step_x, step_y, direction)) => {
                    (Point::new(start.x + step_x * remaining, start.y + step_y * remaining), direction)
                }
                None => {
                    // Corners start pointing along the edge that came before them.
                    let angle = -std::f32::consts::FRAC_PI_2
                        + corner_index as f32 * std::f32::consts::FRAC_PI_2
                        + if radius > 0.0 { remaining / radius } else { 0.0 };
                    let center = corners[corner_index];
                    (Point::new(center.x + radius * angle.cos(), center.y + radius * angle.sin()), Direction::Corner)
                }
            };
        }
        remaining -= side_length;
        if straight.is_none() {
            corner_index += 1;
        }
    }
    (Point::new(rect.left + radius, rect.top), Direction::Horizontal)
}

/// One dash of the outline.
#[derive(Clone, Debug, PartialEq)]
enum Dash {
    /// A dash on a straight edge: one rounded bar.
    Straight(Rect),
    /// A dash that touches a rounded corner: round dots along it, so it bends with the corner.
    Curved(Vec<Point>),
}

/// The dashes that make up a dashed outline, evenly spread so the pattern closes without a seam.
fn dashes(rect: Rect, radius: f32, thickness: f32, dash: f32, gap: f32) -> Vec<Dash> {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let length =
        2.0 * (rect.width() - 2.0 * radius + rect.height() - 2.0 * radius) + 2.0 * std::f32::consts::PI * radius;
    let period = dash + gap;
    let count = (length / period).round().max(1.0) as usize;
    let step = length / count as f32;
    let dot_spacing = (thickness * 0.5).max(0.25);
    (0..count)
        .map(|index| {
            let start = index as f32 * step + (step - dash) / 2.0;
            let (_, first_direction) = point_at(rect, radius, start);
            let (middle, middle_direction) = point_at(rect, radius, start + dash / 2.0);
            let (_, last_direction) = point_at(rect, radius, start + dash);
            let is_straight = first_direction == middle_direction
                && middle_direction == last_direction
                && first_direction != Direction::Corner;
            if is_straight {
                let (half_width, half_height) = match middle_direction {
                    Direction::Horizontal => (dash / 2.0, thickness / 2.0),
                    _ => (thickness / 2.0, dash / 2.0),
                };
                return Dash::Straight(Rect::around(middle, half_width, half_height));
            }
            let dots = (dash / dot_spacing).ceil().max(1.0) as usize;
            Dash::Curved(
                (0..=dots).map(|dot| point_at(rect, radius, start + dash * dot as f32 / dots as f32).0).collect(),
            )
        })
        .collect()
}

/// Draws a dashed outline just inside `rect`. Sizes are device pixels.
pub fn draw_dashed_outline(
    canvas: &Canvas,
    rect: Rect,
    radius: f32,
    thickness: f32,
    dash: f32,
    gap: f32,
    color: Color,
) {
    let inset = thickness / 2.0;
    let inner = Rect::new(rect.left + inset, rect.top + inset, rect.right - inset, rect.bottom - inset);
    for piece in dashes(inner, (radius - inset).max(0.0), thickness, dash, gap) {
        match piece {
            Dash::Straight(bar) => canvas.fill_rounded_rect(bar, thickness / 2.0, color),
            Dash::Curved(dots) => dots.into_iter().for_each(|dot| canvas.fill_circle(dot, thickness / 2.0, color)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: Rect = Rect::new(0.0, 0.0, 100.0, 60.0);

    fn on_the_outline(point: Point) -> bool {
        let eps = 0.01;
        let on_vertical =
            (point.x.abs() < eps || (point.x - 100.0).abs() < eps) && (-eps..=60.0 + eps).contains(&point.y);
        let on_horizontal =
            (point.y.abs() < eps || (point.y - 60.0).abs() < eps) && (-eps..=100.0 + eps).contains(&point.x);
        on_vertical || on_horizontal
    }

    #[test]
    fn walking_without_corners_stays_on_the_edges() {
        for distance in [0.0, 30.0, 100.0, 130.0, 200.0, 230.0, 300.0, 319.0] {
            let (point, _) = point_at(RECT, 0.0, distance);
            assert!(on_the_outline(point), "{distance}: {point:?}");
        }
    }

    #[test]
    fn the_walk_starts_at_the_top_left_and_goes_clockwise() {
        let (start, direction) = point_at(RECT, 10.0, 0.0);
        assert_eq!((start.x, start.y, direction), (10.0, 0.0, Direction::Horizontal));
        let (on_right, direction) = point_at(RECT, 10.0, 80.0 + 15.708 + 20.0);
        assert!((on_right.x - 100.0).abs() < 0.01 && direction == Direction::Vertical);
    }

    #[test]
    fn a_rounded_corner_stays_within_a_radius_of_its_center() {
        let radius = 10.0;
        let (middle, direction) = point_at(RECT, radius, 80.0 + std::f32::consts::FRAC_PI_2 * radius / 2.0);
        assert_eq!(direction, Direction::Corner);
        let center = Point::new(90.0, 10.0);
        assert!(((middle.x - center.x).hypot(middle.y - center.y) - radius).abs() < 0.01);
    }

    #[test]
    fn the_distance_wraps_around_the_outline() {
        let (first, _) = point_at(RECT, 0.0, 10.0);
        let (again, _) = point_at(RECT, 0.0, 10.0 + 320.0);
        assert!((first.x - again.x).abs() < 0.01 && (first.y - again.y).abs() < 0.01);
    }

    #[test]
    fn dashes_are_spread_evenly_around_the_whole_outline() {
        let pieces = dashes(RECT, 8.0, 1.5, 6.0, 4.0);
        let length = 2.0 * (84.0 + 44.0) + 2.0 * std::f32::consts::PI * 8.0;
        assert_eq!(pieces.len(), (length / 10.0).round() as usize);
        for piece in pieces {
            match piece {
                Dash::Straight(bar) => assert!(bar.left >= -3.1 && bar.right <= 103.1),
                Dash::Curved(dots) => assert!(dots.iter().all(|dot| (-0.1..=100.1).contains(&dot.x))),
            }
        }
    }

    #[test]
    fn a_dash_on_a_corner_bends_with_it() {
        let pieces = dashes(RECT, 14.0, 1.5, 12.0, 5.0);
        let curved: Vec<_> = pieces
            .iter()
            .filter_map(|piece| match piece {
                Dash::Curved(dots) => Some(dots),
                Dash::Straight(_) => None,
            })
            .collect();
        assert!(curved.len() >= 4, "every corner keeps a dash");
        assert!(curved.iter().all(|dots| dots.len() > 2));
    }

    #[test]
    fn a_dash_on_a_straight_edge_is_one_bar() {
        let pieces = dashes(RECT, 8.0, 1.5, 6.0, 4.0);
        assert!(pieces.iter().any(|piece| matches!(piece, Dash::Straight(_))));
    }

    #[test]
    fn a_degenerate_rectangle_does_not_panic() {
        assert!(!dashes(Rect::new(0.0, 0.0, 0.0, 0.0), 5.0, 1.0, 4.0, 3.0).is_empty());
    }
}
