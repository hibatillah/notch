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
    Previous,
    Play,
    Pause,
    Next,
    Speaker,
    SpeakerMuted,
    Sun,
    Bolt,
    Battery,
    BatteryLow,
    Microphone,
    Camera,
    Copy,
    Bell,
    Sliders,
    Clock,
    Music,
    Folder,
    Calendar,
    Stopwatch,
    Chart,
    Bluetooth,
    Close,
    CapsLock,
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
            Icon::Speaker | Icon::SpeakerMuted => {
                let body =
                    line(&[(3.5, 9.5), (7.5, 9.5), (12.5, 5.5), (12.5, 18.5), (7.5, 14.5), (3.5, 14.5), (3.5, 9.5)]);
                let marks = if self == Icon::Speaker {
                    join(
                        Path::new().move_to(at(15.5, 9.5)).curve_to(at(17.0, 10.8), at(17.0, 13.2), at(15.5, 14.5)),
                        Path::new().move_to(at(18.0, 7.0)).curve_to(at(21.2, 9.8), at(21.2, 14.2), at(18.0, 17.0)),
                    )
                } else {
                    join(line(&[(16.0, 9.5), (21.0, 14.5)]), line(&[(21.0, 9.5), (16.0, 14.5)]))
                };
                (join(body, marks), Paint::Line)
            }
            Icon::Sun => {
                let mut sun = circle(at(12.0, 12.0), 4.0 * unit);
                for step in 0..8 {
                    let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                    let (sine, cosine) = angle.sin_cos();
                    sun.append(line(&[
                        (12.0 + 7.0 * cosine, 12.0 + 7.0 * sine),
                        (12.0 + 9.5 * cosine, 12.0 + 9.5 * sine),
                    ]));
                }
                (sun, Paint::Line)
            }
            Icon::Bolt => (
                solid(&[(13.5, 3.0), (6.0, 13.5), (11.0, 13.5), (10.0, 21.0), (18.0, 10.0), (13.0, 10.0)], 1.0),
                Paint::Fill,
            ),
            Icon::Battery | Icon::BatteryLow => {
                let body = solid(&[(3.0, 7.0), (19.0, 7.0), (19.0, 17.0), (3.0, 17.0)], 2.0);
                let terminal = line(&[(21.5, 10.5), (21.5, 13.5)]);
                let level = if self == Icon::Battery {
                    join(line(&[(6.5, 10.5), (6.5, 13.5)]), line(&[(10.0, 10.5), (10.0, 13.5)]))
                } else {
                    line(&[(6.5, 10.5), (6.5, 13.5)])
                };
                (join(join(body, terminal), level), Paint::Line)
            }
            Icon::Microphone => {
                let capsule = solid(&[(9.0, 3.0), (15.0, 3.0), (15.0, 12.0), (9.0, 12.0)], 3.0);
                let cradle = Path::new().move_to(at(5.5, 11.0)).curve_to(at(5.5, 17.0), at(18.5, 17.0), at(18.5, 11.0));
                let stand = join(line(&[(12.0, 16.5), (12.0, 21.0)]), line(&[(8.5, 21.0), (15.5, 21.0)]));
                (join(join(capsule, cradle), stand), Paint::Line)
            }
            Icon::Camera => {
                let body = solid(&[(3.0, 7.0), (15.0, 7.0), (15.0, 17.0), (3.0, 17.0)], 2.0);
                let lens = line(&[(15.5, 10.5), (21.0, 7.5), (21.0, 16.5), (15.5, 13.5)]);
                (join(body, lens), Paint::Line)
            }
            Icon::Bell => {
                let dome = Path::new()
                    .move_to(at(5.0, 17.0))
                    .line_to(at(7.0, 15.0))
                    .line_to(at(7.0, 10.5))
                    .curve_to(at(7.0, 7.5), at(9.2, 5.5), at(12.0, 5.5))
                    .curve_to(at(14.8, 5.5), at(17.0, 7.5), at(17.0, 10.5))
                    .line_to(at(17.0, 15.0))
                    .line_to(at(19.0, 17.0))
                    .line_to(at(5.0, 17.0));
                let clapper = Path::new()
                    .move_to(at(10.2, 19.5))
                    .curve_to(at(10.6, 20.5), at(11.2, 21.0), at(12.0, 21.0))
                    .curve_to(at(12.8, 21.0), at(13.4, 20.5), at(13.8, 19.5));
                (join(join(dome, clapper), line(&[(12.0, 3.5), (12.0, 5.5)])), Paint::Line)
            }
            // Lucide "sliders-horizontal" (ISC license, https://lucide.dev).
            Icon::Sliders => {
                let bars = [
                    [(3.0, 5.0), (10.0, 5.0)],
                    [(14.0, 5.0), (21.0, 5.0)],
                    [(3.0, 12.0), (8.0, 12.0)],
                    [(12.0, 12.0), (21.0, 12.0)],
                    [(3.0, 19.0), (12.0, 19.0)],
                    [(16.0, 19.0), (21.0, 19.0)],
                    [(14.0, 3.0), (14.0, 7.0)],
                    [(8.0, 10.0), (8.0, 14.0)],
                    [(16.0, 17.0), (16.0, 21.0)],
                ];
                let mut sliders = Path::new();
                for bar in bars {
                    sliders.append(line(&bar));
                }
                (sliders, Paint::Line)
            }
            Icon::Clock => {
                let face = circle(at(12.0, 12.0), 9.0 * unit);
                (join(face, line(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)])), Paint::Line)
            }
            Icon::Music => {
                let stem = line(&[(9.5, 17.5), (9.5, 5.5), (18.0, 3.5), (18.0, 15.5)]);
                let heads = join(circle(at(7.0, 17.5), 2.5 * unit), circle(at(15.5, 15.5), 2.5 * unit));
                (join(stem, heads), Paint::Line)
            }
            Icon::Folder => (
                solid(&[(3.0, 6.0), (9.0, 6.0), (11.0, 9.0), (21.0, 9.0), (21.0, 19.0), (3.0, 19.0)], 2.0),
                Paint::Line,
            ),
            Icon::Calendar => {
                let body = solid(&[(4.0, 6.0), (20.0, 6.0), (20.0, 20.0), (4.0, 20.0)], 2.0);
                let rings = join(line(&[(8.5, 3.5), (8.5, 7.5)]), line(&[(15.5, 3.5), (15.5, 7.5)]));
                (join(join(body, rings), line(&[(4.0, 10.5), (20.0, 10.5)])), Paint::Line)
            }
            Icon::Stopwatch => {
                let face = circle(at(12.0, 13.5), 7.0 * unit);
                let hand = line(&[(12.0, 13.5), (14.5, 10.5)]);
                let top = join(line(&[(10.0, 3.5), (14.0, 3.5)]), line(&[(12.0, 3.5), (12.0, 6.5)]));
                (join(join(face, hand), top), Paint::Line)
            }
            Icon::Bluetooth => {
                (line(&[(7.0, 8.0), (17.0, 16.0), (12.0, 20.5), (12.0, 3.5), (17.0, 8.0), (7.0, 16.0)]), Paint::Line)
            }
            Icon::Chart => {
                let bars = join(line(&[(6.0, 20.0), (6.0, 12.0)]), line(&[(12.0, 20.0), (12.0, 5.0)]));
                (join(bars, line(&[(18.0, 20.0), (18.0, 9.0)])), Paint::Line)
            }
            // Lucide "copy" (ISC license, https://lucide.dev): a sheet with another behind it.
            Icon::Copy => {
                let front = solid(&[(8.0, 8.0), (22.0, 8.0), (22.0, 22.0), (8.0, 22.0)], 2.0);
                let back = Path::new()
                    .move_to(at(4.0, 16.0))
                    .curve_to(at(2.9, 16.0), at(2.0, 15.1), at(2.0, 14.0))
                    .line_to(at(2.0, 4.0))
                    .curve_to(at(2.0, 2.9), at(2.9, 2.0), at(4.0, 2.0))
                    .line_to(at(14.0, 2.0))
                    .curve_to(at(15.1, 2.0), at(16.0, 2.9), at(16.0, 4.0));
                (join(front, back), Paint::Line)
            }
            // Lucide "x" (ISC license, https://lucide.dev).
            Icon::Close => (join(line(&[(6.0, 6.0), (18.0, 18.0)]), line(&[(18.0, 6.0), (6.0, 18.0)])), Paint::Line),
            Icon::CapsLock => {
                let arrow = solid(
                    &[(12.0, 3.0), (20.5, 12.0), (15.5, 12.0), (15.5, 16.5), (8.5, 16.5), (8.5, 12.0), (3.5, 12.0)],
                    1.5,
                );
                (join(arrow, line(&[(8.5, 20.5), (15.5, 20.5)])), Paint::Line)
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

/// A closed circle of four curves.
fn circle(center: Point, radius: f32) -> Path {
    const KAPPA: f32 = 0.552_284_8;
    let (x, y, reach) = (center.x, center.y, radius * KAPPA);
    Path::new()
        .move_to(Point::new(x + radius, y))
        .curve_to(Point::new(x + radius, y + reach), Point::new(x + reach, y + radius), Point::new(x, y + radius))
        .curve_to(Point::new(x - reach, y + radius), Point::new(x - radius, y + reach), Point::new(x - radius, y))
        .curve_to(Point::new(x - radius, y - reach), Point::new(x - reach, y - radius), Point::new(x, y - radius))
        .curve_to(Point::new(x + reach, y - radius), Point::new(x + radius, y - reach), Point::new(x + radius, y))
        .close()
}

/// Puts the figures of `second` after those of `first`.
fn join(mut first: Path, second: Path) -> Path {
    first.append(second);
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Icon; 25] = [
        Icon::Lyrics,
        Icon::Previous,
        Icon::Play,
        Icon::Pause,
        Icon::Next,
        Icon::Speaker,
        Icon::SpeakerMuted,
        Icon::Sun,
        Icon::Bolt,
        Icon::Battery,
        Icon::BatteryLow,
        Icon::Microphone,
        Icon::Camera,
        Icon::Copy,
        Icon::Bell,
        Icon::Sliders,
        Icon::Clock,
        Icon::Music,
        Icon::Folder,
        Icon::Calendar,
        Icon::Stopwatch,
        Icon::Chart,
        Icon::Bluetooth,
        Icon::Close,
        Icon::CapsLock,
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
        for icon in [Icon::Play, Icon::Pause, Icon::Previous, Icon::Next, Icon::Bolt] {
            assert_eq!(paint(icon), Paint::Fill);
        }
        for icon in ALL
            .into_iter()
            .filter(|icon| ![Icon::Play, Icon::Pause, Icon::Previous, Icon::Next, Icon::Bolt].contains(icon))
        {
            assert_eq!(paint(icon), Paint::Line);
        }
    }

    #[test]
    fn muting_the_speaker_replaces_its_waves_with_a_cross() {
        let count = |icon: Icon| icon.path(Point::default(), 24.0).0.points().count();
        assert_ne!(count(Icon::Speaker), count(Icon::SpeakerMuted));
    }

    #[test]
    fn a_low_battery_shows_fewer_bars_than_a_battery() {
        let count = |icon: Icon| icon.path(Point::default(), 24.0).0.points().count();
        assert!(count(Icon::BatteryLow) < count(Icon::Battery));
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
