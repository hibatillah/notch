use crate::gfx::{Canvas, Color, ImageData, Mask, Path, Point, Rect};

/// Icons are drawn on a 24-unit grid and scaled to the size asked for.
const GRID: f32 = 24.0;
const LINE_WIDTH: f32 = 2.0;
const CORNER_RADIUS: f32 = 1.8;
/// Play and pause read as the main buttons, so they are drawn a little larger.
const SIZE: f32 = 18.0;
const LARGE_SIZE: f32 = 22.0;
/// Empty pixels around an icon inside its image, so soft edges are never cut off.
const MARGIN: u32 = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Lyrics,
    Shuffle,
    Previous,
    Play,
    Pause,
    Next,
    Repeat,
    RepeatOne,
}

/// How a path is painted: solid shapes are filled, line icons are stroked with round ends.
#[derive(Debug, PartialEq)]
enum Paint {
    Fill,
    Line,
}

impl Icon {
    /// Edge length in device-independent pixels.
    pub fn size(self) -> f32 {
        match self {
            Icon::Play | Icon::Pause => LARGE_SIZE,
            _ => SIZE,
        }
    }

    /// Draws the icon centered on `center`, `size` device pixels wide, in `color` faded by `opacity`.
    /// Each icon is drawn once into a small image and reused, so drawing it again costs almost nothing.
    pub fn draw(self, canvas: &Canvas, center: Point, size: f32, color: Color, opacity: f32) {
        let side = size.ceil() as u32 + 2 * MARGIN;
        let key = ("icon", self as u8, side, color.rgb_bytes());
        let Some(brush) = canvas.cached_image(key, || self.render(size, side, color)) else { return };
        // Whole pixels keep the image crisp.
        let left = (center.x - side as f32 / 2.0).round();
        let top = (center.y - side as f32 / 2.0).round();
        let rect = Rect::new(left, top, left + side as f32, top + side as f32);
        canvas.image(&brush, rect, 0.0, opacity * color.a);
    }

    /// The icon as a `side` pixel square image, in `color`.
    fn render(self, size: f32, side: u32, color: Color) -> ImageData {
        let middle = side as f32 / 2.0;
        let (path, paint) = self.path(Point::new(middle, middle), size);
        let figures = path.flatten();
        let mask = match paint {
            Paint::Fill => Mask::fill(side, side, &figures),
            Paint::Line => Mask::stroke(side, side, &figures, LINE_WIDTH * size / GRID / 2.0),
        };
        mask.tinted(color.r, color.g, color.b)
    }

    /// One path per icon, so overlapping parts are painted once and fade evenly.
    fn path(self, center: Point, size: f32) -> (Path, Paint) {
        let unit = size / GRID;
        let at = |x: f32, y: f32| Point::new(center.x + (x - GRID / 2.0) * unit, center.y + (y - GRID / 2.0) * unit);
        let line = |points: &[(f32, f32)]| Path::polyline(&points.iter().map(|&(x, y)| at(x, y)).collect::<Vec<_>>());
        let solid = |points: &[(f32, f32)], radius: f32| {
            Path::rounded_polygon(&points.iter().map(|&(x, y)| at(x, y)).collect::<Vec<_>>(), radius * unit)
        };
        match self {
            Icon::Play => (solid(&[(7.0, 4.5), (19.5, 12.0), (7.0, 19.5)], CORNER_RADIUS), Paint::Fill),
            Icon::Pause => (
                join(
                    solid(&[(6.5, 5.0), (10.5, 5.0), (10.5, 19.0), (6.5, 19.0)], CORNER_RADIUS * 0.8),
                    solid(&[(13.5, 5.0), (17.5, 5.0), (17.5, 19.0), (13.5, 19.0)], CORNER_RADIUS * 0.8),
                ),
                Paint::Fill,
            ),
            Icon::Previous => (
                join(
                    solid(&[(5.0, 5.0), (7.5, 5.0), (7.5, 19.0), (5.0, 19.0)], CORNER_RADIUS * 0.6),
                    solid(&[(19.5, 5.0), (19.5, 19.0), (9.0, 12.0)], CORNER_RADIUS),
                ),
                Paint::Fill,
            ),
            Icon::Next => (
                join(
                    solid(&[(19.0, 5.0), (16.5, 5.0), (16.5, 19.0), (19.0, 19.0)], CORNER_RADIUS * 0.6),
                    solid(&[(4.5, 5.0), (4.5, 19.0), (15.0, 12.0)], CORNER_RADIUS),
                ),
                Paint::Fill,
            ),
            Icon::Shuffle => {
                let curve_down = Path::new()
                    .move_to(at(3.0, 17.0))
                    .line_to(at(6.0, 17.0))
                    .curve_to(at(9.5, 17.0), at(10.5, 7.0), at(14.0, 7.0))
                    .line_to(at(20.0, 7.0));
                let curve_up = Path::new().move_to(at(3.0, 7.0)).line_to(at(6.0, 7.0)).curve_to(
                    at(7.5, 7.0),
                    at(8.5, 7.6),
                    at(9.5, 9.0),
                );
                let tail = Path::new()
                    .move_to(at(14.5, 15.0))
                    .curve_to(at(15.5, 16.3), at(16.5, 17.0), at(18.0, 17.0))
                    .line_to(at(20.0, 17.0));
                let arrows = join(
                    line(&[(17.0, 4.0), (20.0, 7.0), (17.0, 10.0)]),
                    line(&[(17.0, 14.0), (20.0, 17.0), (17.0, 20.0)]),
                );
                (join(join(curve_down, curve_up), join(tail, arrows)), Paint::Line)
            }
            Icon::Repeat | Icon::RepeatOne => {
                let top = Path::new()
                    .move_to(at(4.0, 11.0))
                    .line_to(at(4.0, 10.0))
                    .curve_to(at(4.0, 8.3), at(5.3, 7.0), at(7.0, 7.0))
                    .line_to(at(20.0, 7.0));
                let bottom = Path::new()
                    .move_to(at(20.0, 13.0))
                    .line_to(at(20.0, 14.0))
                    .curve_to(at(20.0, 15.7), at(18.7, 17.0), at(17.0, 17.0))
                    .line_to(at(4.0, 17.0));
                let arrows = join(
                    line(&[(17.0, 4.0), (20.0, 7.0), (17.0, 10.0)]),
                    line(&[(7.0, 14.0), (4.0, 17.0), (7.0, 20.0)]),
                );
                let loops = join(join(top, bottom), arrows);
                let path = if self == Icon::RepeatOne {
                    join(loops, line(&[(10.8, 10.8), (12.5, 9.6), (12.5, 14.4)]))
                } else {
                    loops
                };
                (path, Paint::Line)
            }
            Icon::Lyrics => {
                let bubble = solid(
                    &[(3.0, 4.0), (21.0, 4.0), (21.0, 16.0), (12.0, 16.0), (7.5, 20.5), (7.5, 16.0), (3.0, 16.0)],
                    2.5,
                );
                let text = join(line(&[(7.5, 8.5), (16.5, 8.5)]), line(&[(7.5, 11.5), (13.0, 11.5)]));
                (join(bubble, text), Paint::Line)
            }
        }
    }
}

/// Puts the figures of `second` after those of `first`.
fn join(mut first: Path, second: Path) -> Path {
    first.append(second);
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Icon; 8] = [
        Icon::Lyrics,
        Icon::Shuffle,
        Icon::Previous,
        Icon::Play,
        Icon::Pause,
        Icon::Next,
        Icon::Repeat,
        Icon::RepeatOne,
    ];

    #[test]
    fn every_icon_stays_inside_its_square() {
        let (center, size) = (Point::new(100.0, 50.0), 40.0);
        for icon in ALL {
            let (path, _) = icon.path(center, size);
            for point in path.points() {
                assert!(
                    (point.x - center.x).abs() <= size / 2.0 && (point.y - center.y).abs() <= size / 2.0,
                    "{point:?}"
                );
            }
        }
    }

    #[test]
    fn solid_icons_are_filled_and_line_icons_are_stroked() {
        let paint = |icon: Icon| icon.path(Point::default(), 24.0).1;
        for icon in [Icon::Play, Icon::Pause, Icon::Previous, Icon::Next] {
            assert_eq!(paint(icon), Paint::Fill);
        }
        for icon in [Icon::Lyrics, Icon::Shuffle, Icon::Repeat, Icon::RepeatOne] {
            assert_eq!(paint(icon), Paint::Line);
        }
    }

    #[test]
    fn repeat_one_adds_a_mark_to_repeat() {
        let count = |icon: Icon| icon.path(Point::default(), 24.0).0.points().count();
        assert!(count(Icon::RepeatOne) > count(Icon::Repeat));
    }

    #[test]
    fn next_mirrors_previous() {
        let (previous, _) = Icon::Previous.path(Point::default(), 24.0);
        let (next, _) = Icon::Next.path(Point::default(), 24.0);
        let (previous_extent, next_extent) = (extent(&previous), extent(&next));
        assert!((previous_extent.0 + next_extent.1).abs() < 1.0, "{previous_extent:?} {next_extent:?}");
    }

    fn extent(path: &Path) -> (f32, f32) {
        path.points().fold((f32::MAX, f32::MIN), |(low, high), point| (low.min(point.x), high.max(point.x)))
    }
}
