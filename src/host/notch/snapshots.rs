//! Offscreen images go to target/snapshots; run cargo test snapshots -- --ignored --nocapture.

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

fn background(fraction: f32) -> [f32; 3] {
    let stops = [(0.0, [214.0, 164.0, 230.0]), (0.6, [240.0, 138.0, 75.0]), (1.0, [239.0, 106.0, 42.0])];
    let (low, high) = if fraction < stops[1].0 { (stops[0], stops[1]) } else { (stops[1], stops[2]) };
    let amount = ((fraction - low.0) / (high.0 - low.0)).clamp(0.0, 1.0);
    std::array::from_fn(|channel| low.1[channel] + (high.1[channel] - low.1[channel]) * amount)
}

/// Composite cropped premultiplied BGRA over the background and write a 24-bit bitmap.
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

fn rig(options: SampleOptions) -> Notch {
    configured_rig(options, Settings::at(None), Vec::new())
}

fn configured_rig(options: SampleOptions, settings: Settings, activities: Vec<Box<dyn Activity>>) -> Notch {
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
        settings,
        pages,
        activities,
        ticks_per_second: 10_000_000,
    });
    notch.page_list = PageList::new(count);
    notch
}

fn settle(notch: &mut Notch) {
    for _ in 0..3 {
        notch.refresh_layout();
        for spring in notch.layout_springs() {
            spring.snap_to_target();
        }
        notch.expansion.snap_to_target();
    }
}

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
    notch.swipe = None;
    notch.holds_page_position = false;
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
fn touchpad_only_centers_after_release_and_can_be_interrupted_from_the_visible_position() {
    let mut notch = rig(SampleOptions { media: Some((true, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    notch.drag_pages(0.35);
    notch.flush();
    for _ in 0..10 {
        notch.refresh_layout();
    }
    assert!((notch.track.target - 0.35).abs() < 0.001, "input must stay fractional until the quiet period ends");
    notch.track.step(1.0 / 60.0);
    let visible_position = notch.track.position;
    notch.end_swipe();
    assert_eq!(notch.track.target, 0.0);
    assert_eq!(notch.track.position, visible_position, "release must not jump the rendered position");
    notch.drag_pages(0.1);
    assert!((notch.track.target - visible_position - 0.1).abs() < 0.001);
}

#[test]
fn touchpad_progress_interpolates_between_the_pages_real_widths() {
    let mut notch = rig(SampleOptions { media: Some((true, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    let first_width = notch.pages[notch.navigable[0]].sizes().expanded.width;
    let next_width = notch.pages[notch.navigable[1]].sizes().expanded.width;
    assert!(first_width > next_width);
    notch.drag_pages(0.25);
    for progress in [0.25, 0.5, 0.75] {
        notch.track.jump_to(progress);
        notch.refresh_layout();
        let expected = lerp(first_width, next_width, progress) + chrome::WIDTH_EXTRA;
        assert!((notch.expanded_width.target - expected).abs() < 0.01, "width must follow swipe progress {progress}");
    }
}

#[test]
fn visible_page_frames_keep_natural_widths_and_the_shorter_page_enters_early() {
    let mut notch = rig(SampleOptions { media: Some((true, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    let first_width = notch.pages[notch.navigable[0]].sizes().expanded.width;
    let next_width = notch.pages[notch.navigable[1]].sizes().expanded.width;
    notch.holds_page_position = true;
    for progress in [0.25, 0.5, 0.75] {
        notch.track.jump_to(progress);
        notch.refresh_layout();
        notch.follow_page_width();
        let viewport = notch.frame();
        let [(first, first_frame, _), (next, next_frame, _)] = notch.visible_page_frames(&viewport).unwrap();
        assert_eq!((first, next), (notch.navigable[0], notch.navigable[1]));
        assert!((first_frame.bounds.width() / SCALE - first_width).abs() < 0.01);
        assert!((next_frame.bounds.width() / SCALE - next_width).abs() < 0.01);
        assert_eq!(next_frame.expanded.width, next_width);
        assert!((next_frame.bounds.left - first_frame.bounds.right - PAGE_GAP * SCALE).abs() < 0.01);
        assert!(next_frame.bounds.left < chrome::content_rect(viewport.bounds, SCALE).right);
        assert!(
            (notch.expanded_width.position - chrome::WIDTH_EXTRA - lerp(first_width, next_width, progress)).abs()
                < 0.01
        );
    }
}

#[test]
fn explicit_page_controls_keep_mouse_and_dot_navigation_centered() {
    let mut notch = rig(SampleOptions { media: Some((true, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    let first_page = notch.selected_page.unwrap();
    notch.drag_pages(0.35);
    notch.track.snap_to_target();
    notch.flush();
    notch.go_to_page(first_page);
    notch.flush();
    assert!(!notch.holds_page_position);
    assert!(notch.track.target.abs() < 0.001);
    notch.track.snap_to_target();
    notch.drag_pages(0.35);
    notch.end_swipe();
    notch.track.snap_to_target();
    notch.scrolled(-120, false, false);
    assert!(!notch.holds_page_position);
    assert!((notch.track.target - 1.0).abs() < 0.001);
}

#[test]
fn touchpad_scroll_keeps_large_deltas_in_the_same_gesture() {
    let mut notch = rig(SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    notch.scrolled(-30, false, false);
    notch.scrolled(-120, false, false);
    assert!(notch.swipe.is_some(), "a large touchpad delta must not turn into a mouse-wheel snap");
    assert!((notch.track.target - 0.375).abs() < 0.001);
}

#[test]
fn a_tagged_touchpad_starts_smoothly_even_with_a_full_wheel_sized_delta() {
    let mut notch = rig(SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    notch.scrolled(-120, false, true);
    assert!(notch.swipe.is_some());
    assert!((notch.track.target - 0.3).abs() < 0.001);
    assert_eq!(notch.track.position, 0.0);
}

#[test]
fn touchpad_scroll_does_not_change_logical_selection_at_the_midpoint() {
    let mut notch = rig(SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    let selection = notch.selected_page;
    for delta in [0.45, 0.1, -0.1, 0.1] {
        notch.drag_pages(delta);
        notch.flush();
        assert_eq!(notch.selected_page, selection);
    }
    notch.end_swipe();
    notch.flush();
    assert_ne!(notch.selected_page, selection);
}

#[test]
fn touchpad_scroll_ignores_a_snap_timer_queued_before_the_latest_input() {
    let mut notch = rig(SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    notch.drag_pages(0.25);
    notch.timer_fired(SWIPE_END_TIMER_ID);
    assert!(notch.swipe.is_some(), "fresh input must win over a previously queued timer");
    assert_eq!(notch.track.target, 0.25);
}

#[test]
fn a_touchpad_drag_follows_smoothly_and_can_continue_across_pages() {
    let mut notch = rig(SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() });
    open(&mut notch, "media");
    let origin = notch.track.position;
    notch.drag_pages(0.25);
    assert_eq!(notch.track.position, origin, "input must not teleport the rendered page");
    assert_eq!(notch.track.target, origin + 0.25);
    notch.track.step(1.0 / 60.0);
    assert!(notch.track.position > origin && notch.track.position < notch.track.target);
    notch.end_swipe();
    assert_eq!(notch.track.target, origin);
    for _ in 0..60 {
        notch.track.step(1.0 / 120.0);
    }
    assert!(notch.track.is_settled());

    for _ in 0..30 {
        notch.drag_pages(0.125);
        notch.track.step(1.0 / 60.0);
    }
    assert_eq!(notch.track.target, origin + 3.75);
    notch.end_swipe();
    assert_eq!(notch.track.target, origin + 4.0);
    for _ in 0..90 {
        notch.track.step(1.0 / 120.0);
    }
    assert!(notch.track.is_settled());
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshots_of_every_open_page() {
    let options = SampleOptions { media: Some((true, false)), timer: true, calendar: true, ..SampleOptions::default() };
    let mut notch = rig(options);
    for id in ["media", "shelf", "calendar", "timer", "stats"] {
        open(&mut notch, id);
        save(&notch, &format!("open-{id}"), 260.0);
    }
    open(&mut notch, "media");
    notch.holds_page_position = true;
    for (name, progress) in [("swipe-long-short-25", 0.25), ("swipe-long-short-50", 0.5), ("swipe-long-short-75", 0.75)]
    {
        notch.track.jump_to(progress);
        notch.refresh_layout();
        notch.follow_page_width();
        save(&notch, name, 260.0);
    }
    notch.track.jump_to(0.5);
    notch.follow_page_width();
    save(&notch, "open-swipe", 260.0);
    notch.drag_pages(0.0);
    notch.end_swipe();
    notch.refresh_layout();
    save(&notch, "open-swipe-release", 260.0);
    let mut plain = rig(SampleOptions { media: Some((false, false)), ..options });
    open(&mut plain, "media");
    save(&plain, "open-media-plain", 260.0);
    open(&mut notch, "media");
    for (name, expansion) in [("stage-header", HEADER_STAGE), ("stage-halfway", 0.75)] {
        notch.expansion.jump_to(expansion);
        notch.refresh_layout();
        save(&notch, name, 260.0);
    }
    let mut idle = rig(SampleOptions::default());
    open(&mut idle, "media");
    save(&idle, "open-media-empty", 260.0);
    let mut dragged_over = rig(SampleOptions { file_drag: true, ..options });
    open(&mut dragged_over, "shelf");
    save(&dragged_over, "open-shelf-drag", 260.0);
    open(&mut notch, "timer");
    notch.page_list.set_enabled(4, false);
    notch.editor = Some(PageEditor::default());
    settle(&mut notch);
    save(&notch, "open-edit-pages", 260.0);
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshots_of_the_collapsed_pill() {
    let scenarios: [(&str, SampleOptions); 7] = [
        ("resting-idle", SampleOptions::default()),
        ("resting-idle-with-timer", SampleOptions { timer: true, ..SampleOptions::default() }),
        ("resting-idle-with-calendar", SampleOptions { calendar: true, ..SampleOptions::default() }),
        ("resting-media-bars", SampleOptions { media: Some((false, false)), ..SampleOptions::default() }),
        ("resting-media-lyrics", SampleOptions { media: Some((true, false)), ..SampleOptions::default() }),
        ("resting-media-title", SampleOptions { media: Some((false, true)), ..SampleOptions::default() }),
        (
            "resting-media-with-timer",
            SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() },
        ),
    ];
    for (name, options) in scenarios {
        let mut notch = rig(options);
        close(&mut notch);
        save(&notch, name, 40.0);
    }
}

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
fn snapshot_of_the_notification_banner() {
    let notch = rig(SampleOptions::default());
    banner_snapshot(&notch, "banner-notification");
}

#[test]
#[ignore = "writes images to target/snapshots; run on purpose"]
fn snapshot_of_the_hovered_notification_banner() {
    let notch = rig(SampleOptions::default());
    let body = "Warp finished the task you gave it. The summary has the changed files, the checks that ran and what is left to do before the release goes out. ".repeat(2);
    let activity = crate::activities::notifications::NotificationsActivity::sample_with_body(&notch.fonts, &body);
    banner_snapshot_of(&notch, Box::new(activity.hovered()), "banner-notification-hovered");
}

fn banner_snapshot(notch: &Notch, name: &str) {
    banner_snapshot_of(notch, crate::activities::sample_notification(&notch.fonts), name);
}

fn banner_snapshot_of(notch: &Notch, activity: Box<dyn Activity>, name: &str) {
    let Some(Presence::Banner { width, height }) = activity.presence() else { panic!("the sample has a banner") };
    let surface = &notch.surface;
    let scale = SCALE;
    let image_height = ((height + 20.0) * scale) as i32;
    let region = RECT { left: 0, top: 0, right: surface.width, bottom: image_height };
    clear(notch);
    surface
        .render_and_present(notch.window, notch.screen_position, region, &notch.fonts, scale, |canvas| {
            let left = (surface.width as f32 - width * scale) / 2.0;
            let bottom = (height + RESTING_BOTTOM_PADDING) * scale;
            let bounds = Rect::new(left, 0.0, left + width * scale, bottom);
            let radius = 18.0 * scale;
            canvas.fill_rounded_rect(
                Rect::new(bounds.left, -radius, bounds.right, bounds.bottom),
                radius,
                Color::BLACK,
            );
            canvas.fill_top_flares(bounds, RESTING_FLARE * scale, Color::BLACK);
            let area = Rect::new(bounds.left, bounds.top, bounds.right, bounds.bottom - RESTING_BOTTOM_PADDING * scale);
            activity.draw(canvas, area, scale, 1.0);
        })
        .unwrap();
    write_image(
        name,
        surface.read_pixels(),
        surface.width as usize,
        (IMAGE_WIDTH * SCALE) as usize,
        image_height as usize,
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
    let messages: [(&str, Icon, Color, &str, Option<&str>); 9] = [
        ("hud-charging", Icon::Bolt, green, "Charging", Some("56%")),
        ("hud-on-battery", Icon::Battery, Color::WHITE, "On battery", Some("56%")),
        ("hud-low-battery", Icon::BatteryLow, Color::rgb(1.0, 0.27, 0.23), "Low battery", Some("20%")),
        ("hud-bluetooth", Icon::Bluetooth, blue, "DAXA SL1 connected", Some("80%")),
        ("hud-microphone", Icon::Microphone, orange, "Microphone on", None),
        ("hud-camera", Icon::Camera, green, "Camera on", None),
        ("hud-copied", Icon::Copy, Color::WHITE, "Copied", None),
        ("hud-caps-lock", Icon::CapsLock, Color::WHITE, "Caps Lock on", None),
        ("hud-timer", Icon::Bell, green, "Focus session done", Some("25 min")),
    ];
    for (name, icon, color, message, value) in messages {
        let width = ui::message_width(fonts, message, value);
        pill(&notch, name, width, |canvas, area| ui::draw_message(canvas, area, 1.0, icon, color, message, value));
    }
}

#[cfg(test)]
mod screen_mode_tests {
    use super::*;
    use crate::host::{screen_mode::ScreenMode, visibility::Showing};

    struct SettingsFile(PathBuf);

    impl SettingsFile {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("notch-settings-{}-{name}.txt", std::process::id()));
            fs::write(&path, "").unwrap();
            Self(path)
        }

        fn load(&self) -> Settings {
            Settings::at(Some(self.0.clone()))
        }
    }

    impl Drop for SettingsFile {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_file(&self.0) {
                eprintln!("could not remove test settings {}: {error}", self.0.display());
            }
        }
    }

    fn playing(lyrics_on: bool) -> Notch {
        rig(SampleOptions { media: Some((lyrics_on, false)), ..SampleOptions::default() })
    }

    #[test]
    fn an_unchanged_screen_mode_restores_a_demoted_notch_without_activating_it() {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, HWND_NOTOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SetWindowPos, WS_EX_TOPMOST,
        };
        let mut notch = rig(SampleOptions::default());
        notch.screen_mode = ScreenMode::Maximized;
        let foreground = unsafe { GetForegroundWindow() };
        unsafe {
            SetWindowPos(notch.window, Some(HWND_NOTOPMOST), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)
                .unwrap();
        }
        notch.set_screen_mode(ScreenMode::Maximized);
        assert_ne!(unsafe { GetWindowLongW(notch.window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0, 0);
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }

    #[test]
    fn hover_restores_topmost_before_the_drop_target_hands_off_input() {
        use windows::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongW, WS_EX_TOPMOST};
        let mut notch = rig(SampleOptions::default());
        notch.hides_when_idle = true;
        assert_eq!(unsafe { GetWindowLongW(notch.window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0, 0);
        notch.set_hovering(true);
        assert_ne!(unsafe { GetWindowLongW(notch.window, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST.0, 0);
    }

    #[test]
    fn session_recovery_clears_stale_hover_and_allows_a_hidden_notch_to_open_again() {
        let mut notch = rig(SampleOptions::default());
        notch.screen_mode = ScreenMode::Maximized;
        notch.hides_when_idle = true;
        notch.is_hovering = true;
        notch.is_revealed = true;
        notch.is_dragging_files = true;
        notch.is_close_pending = true;
        notch.expansion.jump_to(1.0);
        notch.reset_session_interaction();
        assert!(!notch.is_hovering);
        assert!(!notch.is_revealed);
        assert!(!notch.is_dragging_files);
        assert!(!notch.is_close_pending);
        assert_eq!(notch.expansion.target, 0.0);
        assert!(notch.is_hidden());
        notch.set_hovering(true);
        notch.timer_fired(REVEAL_TIMER_ID);
        assert!(notch.is_revealed);
        notch.timer_fired(EXPAND_TIMER_ID);
        notch.timer_fired(STAGE_TIMER_ID);
        assert_eq!(notch.expansion.target, 1.0);
    }

    #[test]
    fn fresh_settings_use_the_requested_visibility_defaults() {
        let settings_file = SettingsFile::new("defaults");
        let notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        assert!(!notch.hides_when_idle);
        assert_eq!(notch.maximized_showing, Showing::MusicWithLyrics);
        assert_eq!(notch.fullscreen_showing, Showing::Hidden);
    }

    #[test]
    fn the_old_maximized_override_shows_even_an_empty_notch_with_hide_when_idle_enabled() {
        let settings_file = SettingsFile::new("maximized");
        let mut settings = settings_file.load();
        settings.set_bool("notch.show_over_maximized", true);
        settings.set_bool(HIDE_WHEN_IDLE_SETTING, true);
        let mut notch = configured_rig(SampleOptions::default(), settings, Vec::new());
        notch.screen_mode = ScreenMode::Maximized;
        notch.refresh_layout();
        assert!(!notch.is_hidden());
        notch.screen_mode = ScreenMode::Fullscreen;
        assert!(notch.is_suppressed(), "the maximized override must not override fullscreen hiding");
    }

    #[test]
    fn disabling_fullscreen_hiding_does_not_show_the_notch_over_maximized_windows() {
        let mut notch = rig(SampleOptions::default());
        notch.fullscreen_showing = Showing::Music;
        notch.screen_mode = ScreenMode::Maximized;
        assert!(notch.is_hidden());
    }

    #[test]
    fn saved_old_visibility_choices_migrate_at_start() {
        let settings_file = SettingsFile::new("saved-visibility");
        let mut settings = settings_file.load();
        settings.set_bool(HIDE_WHEN_IDLE_SETTING, true);
        settings.set_bool("notch.hide_for_fullscreen", false);
        settings.set_bool("notch.lyrics_over_maximized", false);
        let notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        assert!(notch.hides_when_idle);
        assert_eq!(notch.fullscreen_showing, Showing::Music);
        assert_eq!(notch.maximized_showing, Showing::ActivitiesOnly);
        assert_eq!(settings_file.load().get_string("notch.fullscreen"), Some("music"));
        assert_eq!(settings_file.load().get_string("notch.maximized"), Some("activities"));
    }

    #[test]
    fn saved_showings_are_read_and_an_unknown_word_falls_back_to_the_default() {
        let settings_file = SettingsFile::new("saved-showings");
        fs::write(&settings_file.0, "notch.maximized=music\nnotch.fullscreen=sometimes\n").unwrap();
        let notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        assert_eq!(notch.maximized_showing, Showing::Music);
        assert_eq!(notch.fullscreen_showing, Showing::Hidden);
    }

    #[test]
    fn reloading_a_showing_over_a_maximized_window_applies_at_once_and_releases_the_window() {
        let settings_file = SettingsFile::new("reload-showing");
        fs::write(&settings_file.0, "notch.maximized=hidden\n").unwrap();
        let mut notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        notch.set_screen_mode(ScreenMode::Maximized);
        assert!(notch.is_suppressed());
        fs::write(&settings_file.0, "notch.maximized=music\n").unwrap();
        notch.reload_settings();
        assert_eq!(notch.maximized_showing, Showing::Music);
        assert!(!notch.is_suppressed(), "a choice other than hidden must not leave the window hidden");
        assert!(!notch.is_hidden());
        assert_eq!(notch.resting_height.target, RESTING_PILL_HEIGHT);
        fs::write(&settings_file.0, "notch.maximized=bogus\n").unwrap();
        notch.reload_settings();
        assert_eq!(notch.maximized_showing, Showing::Music, "an unknown word keeps the current choice");
        fs::write(&settings_file.0, "notch.maximized=activities\n").unwrap();
        notch.reload_settings();
        assert!(notch.is_hidden());
        assert_eq!(notch.resting_height.target, notch.hidden_height());
    }

    #[test]
    fn a_hand_edited_old_key_migrates_on_reload() {
        let settings_file = SettingsFile::new("reload-old-key");
        let mut notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        fs::write(&settings_file.0, "notch.show_over_maximized=on\n").unwrap();
        notch.reload_settings();
        assert_eq!(notch.maximized_showing, Showing::Music);
        assert_eq!(settings_file.load().get_string("notch.maximized"), Some("music"));
    }

    #[test]
    fn only_notifications_appear_in_the_activity_menu() {
        let settings_file = SettingsFile::new("activity-menu");
        let notch = configured_rig(SampleOptions::default(), settings_file.load(), crate::activities::all());
        assert_eq!(notch.activity_menu().len(), 1);
        assert!(notch.activities.iter().all(|slot| slot.is_enabled));
        let notifications_index =
            notch.activities.iter().position(|slot| slot.activity.id() == "notifications").unwrap();
        assert_eq!(notch.activity_menu(), vec![(notifications_index, "Notifications", true)]);
    }

    #[test]
    fn notifications_can_still_be_switched_off() {
        let settings_file = SettingsFile::new("notifications");
        let mut notch = configured_rig(SampleOptions::default(), settings_file.load(), crate::activities::all());
        let notifications_index = notch.activity_menu()[0].0;
        notch.toggle_activity(notifications_index);
        assert_eq!(notch.activity_menu(), vec![(notifications_index, "Notifications", false)]);
        assert!(!settings_file.load().get_bool("activity.notifications", true));
        assert!(
            notch.activities.iter().filter(|slot| slot.activity.id() != "notifications").all(|slot| slot.is_enabled)
        );
    }

    #[test]
    fn required_activities_ignore_saved_disables_and_cannot_be_toggled_off() {
        let settings_file = SettingsFile::new("required-activities");
        let mut settings = settings_file.load();
        settings.set_bool("activity.volume", false);
        settings.set_bool("activity.notifications", false);
        let mut notch = configured_rig(SampleOptions::default(), settings, crate::activities::all());
        let volume_index = notch.activities.iter().position(|slot| slot.activity.id() == "volume").unwrap();
        let notifications_index =
            notch.activities.iter().position(|slot| slot.activity.id() == "notifications").unwrap();
        assert!(notch.activities[volume_index].is_enabled);
        assert!(!notch.activities[notifications_index].is_enabled);
        notch.toggle_activity(volume_index);
        assert!(notch.activities[volume_index].is_enabled);
    }

    #[test]
    fn reloading_settings_applies_hide_when_idle() {
        let settings_file = SettingsFile::new("reload-hide-when-idle");
        let mut notch = configured_rig(SampleOptions::default(), settings_file.load(), Vec::new());
        settle(&mut notch);
        assert!(!notch.hides_when_idle);
        fs::write(&settings_file.0, "notch.hide_when_idle=on\nnotch.hide_for_fullscreen=off\n").unwrap();
        notch.reload_settings();
        assert!(notch.hides_when_idle);
        assert_eq!(notch.fullscreen_showing, Showing::Music);
        assert!(notch.is_hidden());
        assert_eq!(notch.resting_height.target, HIDDEN_HEIGHT);
    }

    #[test]
    fn reloading_settings_keeps_the_open_page_and_hover() {
        let settings_file = SettingsFile::new("reload-open-page");
        let options = SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() };
        let mut notch = configured_rig(options, settings_file.load(), Vec::new());
        open(&mut notch, "media");
        let media = notch.selected_page.unwrap();
        assert_eq!(notch.page_at(notch.track.position), media);
        let mut reversed = notch.page_ids();
        reversed.reverse();
        fs::write(&settings_file.0, format!("navigation.order={}\nnotch.hide_when_idle=on\n", reversed.join(",")))
            .unwrap();
        notch.reload_settings();
        assert!(notch.hides_when_idle);
        assert_eq!(
            notch.page_list.order().first(),
            notch.pages.iter().position(|page| page.id() == reversed[0]).as_ref()
        );
        assert!(notch.is_hovering);
        assert_eq!(notch.expansion.target, 1.0);
        assert_eq!(notch.selected_page, Some(media));
        assert_eq!(notch.expanded_index, Some(media));
        assert_eq!(notch.page_at(notch.track.position), media, "the open page must not scroll away");
        assert!(notch.track.is_settled());
    }

    #[test]
    fn switching_off_an_earlier_page_keeps_the_open_page_in_place() {
        let settings_file = SettingsFile::new("reload-disable-earlier");
        let options = SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() };
        let mut notch = configured_rig(options, settings_file.load(), Vec::new());
        open(&mut notch, "calendar");
        let calendar = notch.selected_page.unwrap();
        let shelf = notch.pages.iter().position(|page| page.id() == "shelf").unwrap();
        let navigable_before = notch.navigable.clone();
        let shelf_row = navigable_before.iter().position(|&index| index == shelf).unwrap();
        let calendar_row = navigable_before.iter().position(|&index| index == calendar).unwrap();
        assert!(shelf_row < calendar_row, "the test needs the switched-off page before the open one");
        assert_eq!(notch.page_at(notch.track.position), calendar);
        fs::write(&settings_file.0, "navigation.disabled=shelf\n").unwrap();
        notch.reload_settings();
        assert!(!notch.page_list.is_enabled(shelf));
        assert!(notch.is_hovering);
        assert_eq!(notch.expanded_index, Some(calendar));
        assert_eq!(notch.page_at(notch.track.position), calendar);
        assert_eq!(notch.page_at(notch.track.target), calendar);
        assert!(notch.track.is_settled(), "the open notch must not scroll to the page's new row");
    }

    #[test]
    fn a_malformed_settings_file_keeps_the_current_values() {
        let settings_file = SettingsFile::new("reload-malformed");
        fs::write(
            &settings_file.0,
            "notch.hide_when_idle=on\nnotch.hide_for_fullscreen=off\nnavigation.disabled=stats\n",
        )
        .unwrap();
        let options = SampleOptions { media: Some((false, false)), timer: true, ..SampleOptions::default() };
        let mut notch = configured_rig(options, settings_file.load(), Vec::new());
        notch.reload_settings();
        let stats = notch.pages.iter().position(|page| page.id() == "stats").unwrap();
        assert!(!notch.page_list.is_enabled(stats));
        fs::write(&settings_file.0, "notch.hide_when_idle=maybe\nthis is not a setting\nnotch.hide_for_fullscreen\n")
            .unwrap();
        notch.reload_settings();
        assert!(notch.hides_when_idle);
        assert_eq!(notch.fullscreen_showing, Showing::Music);
        assert_eq!(notch.maximized_showing, Showing::MusicWithLyrics);
        assert!(!notch.page_list.is_enabled(stats));

        let every_page_off = notch.page_ids().join(",");
        fs::write(&settings_file.0, format!("navigation.disabled={every_page_off}\n")).unwrap();
        notch.reload_settings();
        assert!(!notch.page_list.is_enabled(stats), "a list with no page switched on is ignored");
        assert!((0..notch.pages.len()).any(|page| notch.page_list.is_enabled(page)));

        fs::remove_file(&settings_file.0).unwrap();
        fs::create_dir(&settings_file.0).unwrap();
        notch.reload_settings();
        assert!(notch.hides_when_idle, "an unreadable file keeps every value");
        assert_eq!(notch.fullscreen_showing, Showing::Music);
        fs::remove_dir(&settings_file.0).unwrap();
        fs::write(&settings_file.0, "").unwrap();
    }

    #[test]
    fn reloading_settings_tells_the_media_page_to_switch_lyrics() {
        let settings_file = SettingsFile::new("reload-lyrics");
        let options = SampleOptions { media: Some((true, false)), ..SampleOptions::default() };
        let mut notch = configured_rig(options, settings_file.load(), Vec::new());
        let media = notch.pages.iter().position(|page| page.id() == "media").unwrap();
        assert!(notch.pages[media].shows_live_text());
        fs::write(&settings_file.0, "media.lyrics=off\n").unwrap();
        notch.reload_settings();
        assert!(!notch.pages[media].shows_live_text());
        fs::write(&settings_file.0, "media.lyrics=on\n").unwrap();
        notch.reload_settings();
        assert!(notch.pages[media].shows_live_text());
        assert!(settings_file.load().get_bool("media.lyrics", false), "a reload never writes the file");
    }

    struct CountingActivity {
        starts: std::rc::Rc<Cell<u32>>,
        settings_changes: std::rc::Rc<Cell<u32>>,
    }

    impl Activity for CountingActivity {
        fn id(&self) -> &'static str {
            "counting"
        }

        fn label(&self) -> &'static str {
            "Counting"
        }

        fn is_optional(&self) -> bool {
            true
        }

        fn presence(&self) -> Option<Presence> {
            Some(Presence::Badge { width: 20.0 })
        }

        fn draw(&self, _canvas: &Canvas, _area: Rect, _scale: f32, _opacity: f32) {}

        fn on_start(&mut self, _context: &mut Context) {
            self.starts.set(self.starts.get() + 1);
        }

        fn on_settings_changed(&mut self, _context: &mut Context) {
            self.settings_changes.set(self.settings_changes.get() + 1);
        }
    }

    #[test]
    fn reloading_settings_hides_a_switched_off_activity_and_starts_a_switched_on_one_once() {
        let settings_file = SettingsFile::new("reload-activity");
        fs::write(&settings_file.0, "activity.counting=off\n").unwrap();
        let (starts, settings_changes) = (std::rc::Rc::new(Cell::new(0)), std::rc::Rc::new(Cell::new(0)));
        let activity = CountingActivity { starts: starts.clone(), settings_changes: settings_changes.clone() };
        let mut notch = configured_rig(SampleOptions::default(), settings_file.load(), vec![Box::new(activity)]);
        assert_eq!(starts.get(), 0);
        notch.reload_settings();
        assert_eq!(settings_changes.get(), 0, "an activity that never started is not told");

        fs::write(&settings_file.0, "activity.counting=on\n").unwrap();
        notch.reload_settings();
        notch.reload_settings();
        assert_eq!(starts.get(), 1);
        assert_eq!(settings_changes.get(), 2);
        assert_eq!(notch.shown_badges.len(), 1);

        fs::write(&settings_file.0, "activity.counting=off\n").unwrap();
        notch.reload_settings();
        assert!(notch.shown_badges.is_empty());
        assert_eq!(starts.get(), 1);
        assert_eq!(settings_changes.get(), 3, "a started activity still hears about changes");
    }

    #[test]
    fn a_notch_with_nothing_to_do_hides_over_a_maximized_window_and_shows_otherwise() {
        let mut notch = rig(SampleOptions::default());
        assert!(!notch.is_hidden());
        notch.screen_mode = ScreenMode::Maximized;
        assert!(notch.is_hidden());
        notch.maximized_showing = Showing::Music;
        assert!(!notch.is_hidden(), "the music choice keeps even the idle clock up");
    }

    #[test]
    fn music_in_the_pill_does_not_keep_the_notch_up_over_a_maximized_window() {
        let mut notch = playing(false);
        notch.refresh_layout();
        assert!(notch.has_resting_content);
        notch.screen_mode = ScreenMode::Maximized;
        assert!(notch.is_hidden());
    }

    #[test]
    fn lyrics_stay_over_a_maximized_window_only_when_the_user_asked() {
        let mut notch = playing(true);
        notch.maximized_showing = Showing::ActivitiesOnly;
        notch.refresh_layout();
        notch.screen_mode = ScreenMode::Maximized;
        assert!(notch.is_hidden());
        notch.maximized_showing = Showing::MusicWithLyrics;
        assert!(!notch.is_hidden());
    }

    #[test]
    fn maximized_lyrics_hides_music_without_lyrics() {
        let mut without_lyrics = playing(false);
        without_lyrics.maximized_showing = Showing::MusicWithLyrics;
        without_lyrics.screen_mode = ScreenMode::Maximized;
        without_lyrics.refresh_layout();
        assert!(without_lyrics.is_hidden());
        assert_eq!(without_lyrics.resting_height.target, without_lyrics.hidden_height());
        assert!(!without_lyrics.is_suppressed());
    }

    #[test]
    fn maximized_music_shows_without_lyrics() {
        let mut notch = playing(false);
        notch.maximized_showing = Showing::Music;
        notch.hides_when_idle = true;
        notch.screen_mode = ScreenMode::Maximized;
        notch.refresh_layout();
        assert!(notch.has_resting_content);
        assert!(!notch.is_hidden());
        assert!(!notch.is_suppressed());
    }

    #[test]
    fn fullscreen_activities_only_shows_a_banner_but_hides_the_idle_clock() {
        let mut notch = rig(SampleOptions::default());
        notch.fullscreen_showing = Showing::ActivitiesOnly;
        notch.set_screen_mode(ScreenMode::Fullscreen);
        assert!(!notch.is_suppressed());
        assert!(notch.is_hidden(), "the idle clock hides to the strip");
        notch.shown_banner = Some((Source::Activity(0), 330.0, 70.0));
        assert!(!notch.is_hidden());
        let mut playing_notch = playing(true);
        playing_notch.fullscreen_showing = Showing::ActivitiesOnly;
        playing_notch.screen_mode = ScreenMode::Fullscreen;
        playing_notch.refresh_layout();
        assert!(playing_notch.is_hidden(), "music and lyrics do not count as activities");
    }

    #[test]
    fn leaving_full_screen_with_default_settings_keeps_the_pill_at_full_size_without_animating() {
        let mut notch = rig(SampleOptions::default());
        settle(&mut notch);
        assert_eq!(notch.resting_height.target, RESTING_PILL_HEIGHT);
        // Settling snaps the springs but leaves the rig's start-up frame timer running.
        notch.stop_animation();
        notch.set_screen_mode(ScreenMode::Fullscreen);
        assert!(!notch.is_animating, "entering full screen starts no strip animation");
        assert!(notch.is_suppressed());
        assert!(!notch.is_hidden(), "a suppressed notch keeps its full-size layout");
        assert_eq!(notch.resting_height.target, RESTING_PILL_HEIGHT);
        notch.set_screen_mode(ScreenMode::Normal);
        assert_eq!(notch.resting_height.target, RESTING_PILL_HEIGHT);
        assert!(!notch.is_animating);
    }

    #[test]
    fn fullscreen_music_keeps_the_idle_clock_up_even_with_hide_when_idle_on() {
        let mut notch = rig(SampleOptions::default());
        notch.hides_when_idle = true;
        notch.fullscreen_showing = Showing::Music;
        notch.screen_mode = ScreenMode::Fullscreen;
        notch.refresh_layout();
        assert!(!notch.has_resting_content);
        assert!(!notch.is_hidden());
    }

    #[test]
    fn fullscreen_hidden_suppresses_the_window() {
        let mut notch = rig(SampleOptions::default());
        notch.fullscreen_showing = Showing::Hidden;
        notch.set_screen_mode(ScreenMode::Fullscreen);
        assert!(notch.is_suppressed());
        notch.shown_banner = Some((Source::Activity(0), 330.0, 70.0));
        assert!(notch.is_suppressed(), "hidden hides even an activity");
        notch.set_screen_mode(ScreenMode::Normal);
        assert!(!notch.is_suppressed());
        notch.maximized_showing = Showing::Hidden;
        notch.set_screen_mode(ScreenMode::Maximized);
        assert!(notch.is_suppressed(), "hidden over a maximized window suppresses too");
    }

    #[test]
    fn maximized_window_hiding_takes_precedence_over_hide_when_idle() {
        for lyrics_enabled in [false, true] {
            for hides_when_idle in [false, true] {
                for keeps_lyrics_over_windows in [false, true] {
                    let mut notch = playing(lyrics_enabled);
                    notch.hides_when_idle = hides_when_idle;
                    notch.maximized_showing =
                        if keeps_lyrics_over_windows { Showing::MusicWithLyrics } else { Showing::ActivitiesOnly };
                    notch.screen_mode = ScreenMode::Maximized;
                    notch.refresh_layout();
                    let should_hide = !(lyrics_enabled && keeps_lyrics_over_windows);
                    assert_eq!(
                        notch.is_hidden(),
                        should_hide,
                        "lyrics={lyrics_enabled}, hide_idle={hides_when_idle}, keep_lyrics={keeps_lyrics_over_windows}"
                    );
                    if should_hide {
                        assert_eq!(notch.resting_width.target, HIDDEN_WIDTH);
                        assert_eq!(notch.resting_height.target, notch.hidden_height());
                    }
                    notch.screen_mode = ScreenMode::Normal;
                    notch.refresh_layout();
                    assert!(!notch.is_hidden(), "music remains visible outside maximized windows");
                }
            }
        }
    }

    #[test]
    fn maximized_music_keeps_music_visible_regardless_of_lyrics_and_idle_settings() {
        for lyrics_enabled in [false, true] {
            for hides_when_idle in [false, true] {
                let mut notch = playing(lyrics_enabled);
                notch.hides_when_idle = hides_when_idle;
                notch.maximized_showing = Showing::Music;
                notch.screen_mode = ScreenMode::Maximized;
                notch.refresh_layout();
                assert!(!notch.is_hidden());
                notch.screen_mode = ScreenMode::Fullscreen;
                assert!(notch.is_suppressed());
            }
        }
    }

    #[test]
    fn hovering_a_cut_banner_makes_the_pill_taller_and_keeps_its_width() {
        let fonts = Fonts::new(SCALE).unwrap();
        let body = "Deploy is green. Can you check the hover state on the 1440p monitor before we tag it? ".repeat(3);
        let activity = crate::activities::notifications::NotificationsActivity::sample_with_body(&fonts, &body);
        let mut notch = configured_rig(SampleOptions::default(), Settings::at(None), vec![Box::new(activity)]);
        notch.activities[0].is_started = true;
        settle(&mut notch);
        let (calm_width, calm_height) = (notch.resting_width.target, notch.resting_height.target);
        notch.set_hovering(true);
        notch.flush();
        assert_eq!(notch.resting_width.target, calm_width);
        assert!(notch.resting_height.target > calm_height, "{} vs {}", notch.resting_height.target, calm_height);
    }

    #[test]
    fn a_banner_or_a_short_message_still_shows_over_a_maximized_window() {
        let mut notch = rig(SampleOptions::default());
        notch.screen_mode = ScreenMode::Maximized;
        notch.shown_banner = Some((Source::Activity(0), 330.0, 70.0));
        assert!(!notch.is_hidden());
        notch.shown_banner = None;
        notch.shown_transients = vec![(Source::Activity(0), 100.0)];
        assert!(!notch.is_hidden());
    }

    #[test]
    fn a_full_screen_window_takes_the_notch_off_screen_and_nothing_else_does() {
        let mut notch = rig(SampleOptions::default());
        notch.screen_mode = ScreenMode::Fullscreen;
        assert!(notch.is_suppressed());
        for showing in [Showing::ActivitiesOnly, Showing::MusicWithLyrics, Showing::Music] {
            notch.fullscreen_showing = showing;
            assert!(!notch.is_suppressed(), "{showing:?}");
        }
        notch.fullscreen_showing = Showing::Hidden;
        notch.screen_mode = ScreenMode::Maximized;
        assert!(!notch.is_suppressed());
        notch.screen_mode = ScreenMode::Normal;
        notch.maximized_showing = Showing::Hidden;
        assert!(!notch.is_suppressed(), "a regular window never suppresses the notch");
    }

    #[test]
    fn over_a_maximized_window_the_strip_is_thinner_and_slower_to_wake_than_with_hide_when_idle() {
        let mut notch = rig(SampleOptions::default());
        notch.hides_when_idle = true;
        let (strip, delay) = (notch.hidden_height(), notch.reveal_delay());
        notch.hides_when_idle = false;
        assert!(notch.hidden_height() < strip);
        assert!(notch.reveal_delay() > delay);
    }
}
