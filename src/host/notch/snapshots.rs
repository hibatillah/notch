//! Renders the notch off-screen, page by page and state by state, into image files, so its look can be reviewed against
//! the design without a screen (the screen may be locked) or a running notch. Nothing here runs with `cargo test`;
//! run it with
//!
//! ```text
//! cargo test snapshots -- --ignored --nocapture
//! ```
//!
//! The images are written to `target/snapshots`, drawn over the same orange gradient as the design boards.

use std::{fs, path::PathBuf};

use windows::{
    Win32::{
        Foundation::{HWND, POINT},
        UI::WindowsAndMessaging::{CreateWindowExW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP},
    },
    core::w,
};

use super::*;
use crate::{
    gfx::{Fonts, Surface},
    pages::{SampleOptions, samples},
    ui::{self, Icon},
};

const SCALE: f32 = 1.25;
/// How wide a slice of the screen the images show, in device-independent pixels.
const IMAGE_WIDTH: f32 = 700.0;

fn output_directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("snapshots");
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn test_window() -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("notch snapshot"),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            None,
            None,
        )
        .unwrap()
    }
}

/// The design boards' background, from left to right.
fn background(fraction: f32) -> [f32; 3] {
    let stops = [(0.0, [214.0, 164.0, 230.0]), (0.6, [240.0, 138.0, 75.0]), (1.0, [239.0, 106.0, 42.0])];
    let (low, high) = if fraction < stops[1].0 { (stops[0], stops[1]) } else { (stops[1], stops[2]) };
    let amount = ((fraction - low.0) / (high.0 - low.0)).clamp(0.0, 1.0);
    std::array::from_fn(|channel| low.1[channel] + (high.1[channel] - low.1[channel]) * amount)
}

/// Writes premultiplied BGRA pixels, cropped to the middle `width` columns and the top `height` rows and laid over the
/// background, as a 24-bit bitmap.
fn write_image(name: &str, pixels: &[u8], surface_width: usize, width: usize, height: usize) {
    let left = (surface_width - width) / 2;
    let row_bytes = (width * 3).div_ceil(4) * 4;
    let mut file = Vec::with_capacity(54 + row_bytes * height);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&((54 + row_bytes * height) as u32).to_le_bytes());
    file.extend_from_slice(&[0, 0, 0, 0]);
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&(width as i32).to_le_bytes());
    file.extend_from_slice(&(height as i32).to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&24u16.to_le_bytes());
    file.extend_from_slice(&[0; 24]);
    for row in (0..height).rev() {
        let start = file.len();
        for column in 0..width {
            let source = (row * surface_width + left + column) * 4;
            let alpha = f32::from(pixels[source + 3]) / 255.0;
            let behind = background(column as f32 / width as f32);
            // BGR order; the source is premultiplied, so what shows through is scaled by the missing alpha.
            for channel in [0usize, 1, 2] {
                let covered = f32::from(pixels[source + channel]);
                file.push((covered + behind[2 - channel] * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8);
            }
        }
        file.resize(start + row_bytes, 0);
    }
    fs::write(output_directory().join(format!("{name}.bmp")), file).unwrap();
}

/// A notch with every page, filled in as `options` says.
fn rig(options: SampleOptions) -> Notch {
    let fonts = Fonts::new(SCALE).unwrap();
    let directory = std::env::temp_dir().join(format!("notch-snapshots-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let pages = samples(&fonts, &directory, options);
    let count = pages.len();
    let width = (WINDOW_WIDTH * SCALE).round() as i32;
    let height = (WINDOW_HEIGHT * SCALE).round() as i32;
    let mut notch = Notch::new(NotchParts {
        window: test_window(),
        scale: SCALE,
        screen_position: POINT { x: 0, y: 0 },
        surface: Surface::new(width, height).unwrap(),
        fonts: Fonts::new(SCALE).unwrap(),
        settings: Settings::at(None),
        pages,
        activities: Vec::new(),
        ticks_per_second: 10_000_000,
    });
    notch.page_list = PageList::new(count);
    notch
}

/// Jumps every spring to where it is heading, so the picture shows the finished layout and not a frame of the motion.
fn settle(notch: &mut Notch) {
    for _ in 0..3 {
        notch.refresh_layout();
        for spring in notch.layout_springs() {
            spring.snap_to_target();
        }
        notch.expansion.snap_to_target();
    }
}

/// Wipes the whole surface, so what an earlier, larger picture left outside the next one's region does not show.
fn clear(notch: &Notch) {
    let surface = &notch.surface;
    let region = RECT { left: 0, top: 0, right: surface.width, bottom: surface.height };
    surface.render_and_present(notch.window, notch.screen_position, region, &notch.fonts, SCALE, |_| {}).unwrap();
}

fn save(notch: &Notch, name: &str, height_dip: f32) {
    clear(notch);
    notch.paint();
    let surface = &notch.surface;
    let height = ((height_dip * SCALE) as usize).min(surface.height as usize);
    write_image(name, surface.read_pixels(), surface.width as usize, (IMAGE_WIDTH * SCALE) as usize, height);
}

fn open(notch: &mut Notch, page_id: &str) {
    let index = notch.pages.iter().position(|page| page.id() == page_id).unwrap();
    notch.selected_page = Some(index);
    notch.is_hovering = true;
    notch.expansion.target = 1.0;
    settle(notch);
}

fn close(notch: &mut Notch) {
    notch.is_hovering = false;
    notch.expansion.target = 0.0;
    settle(notch);
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshots_of_every_open_page() {
    let options = SampleOptions { media: Some((true, false)), timer: true, calendar: true, notification_alert: false };
    let mut notch = rig(options);
    for id in ["clock", "media", "notifications", "shelf", "calendar", "timer", "stats"] {
        open(&mut notch, id);
        save(&notch, &format!("open-{id}"), 260.0);
    }
    // Half way between two pages, as in the middle of a swipe.
    open(&mut notch, "media");
    notch.track.position = 0.45;
    save(&notch, "open-swipe", 260.0);
    // The media player without lyrics is narrower and taller.
    let mut plain = rig(SampleOptions { media: Some((false, false)), ..options });
    open(&mut plain, "media");
    save(&plain, "open-media-plain", 260.0);
    // The page editor.
    open(&mut notch, "timer");
    notch.page_list.set_enabled(6, false);
    notch.editor = Some(PageEditor::default());
    settle(&mut notch);
    save(&notch, "open-edit-pages", 260.0);
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshots_of_the_collapsed_pill() {
    let scenarios: [(&str, SampleOptions); 8] = [
        ("resting-clock", SampleOptions::default()),
        ("resting-clock-with-timer", SampleOptions { timer: true, ..SampleOptions::default() }),
        ("resting-clock-with-calendar", SampleOptions { calendar: true, ..SampleOptions::default() }),
        ("resting-media-bars", SampleOptions { media: Some((false, false)), ..SampleOptions::default() }),
        ("resting-media-lyrics", SampleOptions { media: Some((true, false)), ..SampleOptions::default() }),
        ("resting-media-title", SampleOptions { media: Some((false, true)), ..SampleOptions::default() }),
        (
            "resting-media-with-timer",
            SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() },
        ),
        ("resting-notification", SampleOptions { notification_alert: true, ..SampleOptions::default() }),
    ];
    for (name, options) in scenarios {
        let mut notch = rig(options);
        close(&mut notch);
        save(&notch, name, 40.0);
    }
}

/// Draws a short-lived message the way the notch does: in a pill 28 px tall.
fn pill(notch: &Notch, name: &str, width: f32, content: impl FnOnce(&Canvas, Rect)) {
    let surface = &notch.surface;
    let scale = SCALE;
    let region = RECT { left: 0, top: 0, right: surface.width, bottom: (40.0 * scale) as i32 };
    clear(notch);
    let result =
        surface.render_and_present(notch.window, notch.screen_position, region, &notch.fonts, scale, |canvas| {
            let left = (surface.width as f32 - width * scale) / 2.0;
            let bounds = Rect::new(left, 0.0, left + width * scale, RESTING_HEIGHT * scale);
            canvas.fill_rounded_rect(
                Rect::new(bounds.left, -14.0 * scale, bounds.right, bounds.bottom),
                14.0 * scale,
                Color::BLACK,
            );
            canvas.fill_top_flares(bounds, RESTING_FLARE * scale, Color::BLACK);
            content(canvas, bounds);
        });
    result.unwrap();
    write_image(
        name,
        surface.read_pixels(),
        surface.width as usize,
        (IMAGE_WIDTH * SCALE) as usize,
        (40.0 * scale) as usize,
    );
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshots_of_the_short_lived_messages() {
    let notch = rig(SampleOptions::default());
    let fonts = &notch.fonts;
    let green = Color::rgb(0.19, 0.82, 0.35);
    let orange = Color::rgb(1.0, 0.62, 0.04);
    let blue = Color::rgb(0.302, 0.671, 0.969);
    let level_width = ui::LEVEL_WIDTH;
    pill(&notch, "hud-volume", level_width, |canvas, area| {
        ui::draw_level(canvas, area, 1.0, Icon::Speaker, 0.42, "42")
    });
    let muted_width = ui::message_width(fonts, "Muted", None);
    pill(&notch, "hud-muted", muted_width, |canvas, area| {
        ui::draw_message(canvas, area, 1.0, Icon::SpeakerMuted, Color::WHITE, "Muted", None)
    });
    pill(&notch, "hud-brightness", level_width, |canvas, area| ui::draw_level(canvas, area, 1.0, Icon::Sun, 0.7, "70"));
    let messages: [(&str, Icon, Color, &str, Option<&str>); 8] = [
        ("hud-charging", Icon::Bolt, green, "Charging", Some("56%")),
        ("hud-on-battery", Icon::Battery, Color::WHITE, "On battery", Some("56%")),
        ("hud-low-battery", Icon::BatteryLow, Color::rgb(1.0, 0.27, 0.23), "Low battery", Some("20%")),
        ("hud-bluetooth", Icon::Bluetooth, blue, "DAXA SL1 connected", Some("80%")),
        ("hud-microphone", Icon::Microphone, orange, "Microphone on", None),
        ("hud-camera", Icon::Camera, green, "Camera on", None),
        ("hud-copied", Icon::Clipboard, Color::WHITE, "Copied", None),
        ("hud-timer", Icon::Bell, green, "Focus session done", Some("25 min")),
    ];
    for (name, icon, color, message, value) in messages {
        let width = ui::message_width(fonts, message, value);
        pill(&notch, name, width, |canvas, area| ui::draw_message(canvas, area, 1.0, icon, color, message, value));
    }
}
