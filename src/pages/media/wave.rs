//! Animated playback indicator used when lyrics are hidden.

use windows::Win32::System::SystemInformation::GetTickCount64;

use crate::gfx::{Canvas, Color, Rect};

pub const WIDTH: f32 = 16.0;
pub const HEIGHT: f32 = 12.0;
const BARS: usize = 5;
const BAR_WIDTH: f32 = 2.0;
const LOWEST: f32 = 0.25;
const CYCLES_PER_SECOND: f32 = 1.0;
const PHASE_STEP: f32 = 1.1;

pub fn now_seconds() -> f32 {
    (unsafe { GetTickCount64() } % 3_600_000) as f32 / 1000.0
}

fn bar_fractions(seconds: f32) -> [f32; BARS] {
    std::array::from_fn(|index| {
        let angle = seconds * CYCLES_PER_SECOND * std::f32::consts::TAU + index as f32 * PHASE_STEP;
        LOWEST + (1.0 - LOWEST) * (angle.sin() + 1.0) / 2.0
    })
}

pub fn draw(canvas: &Canvas, area: Rect, scale: f32, seconds: f32, opacity: f32) {
    let gap = (WIDTH - BARS as f32 * BAR_WIDTH) / (BARS - 1) as f32;
    for (index, fraction) in bar_fractions(seconds).into_iter().enumerate() {
        let left = area.left + index as f32 * (BAR_WIDTH + gap) * scale;
        let half_height = fraction * HEIGHT * scale / 2.0;
        let bar =
            Rect::new(left, area.center_y() - half_height, left + BAR_WIDTH * scale, area.center_y() + half_height);
        canvas.fill_rounded_rect(bar, BAR_WIDTH * scale / 2.0, Color::WHITE.faded(opacity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bar_stays_between_its_lowest_and_full_height() {
        for step in 0..200 {
            for fraction in bar_fractions(step as f32 * 0.05) {
                assert!((LOWEST - 1e-4..=1.0 + 1e-4).contains(&fraction), "{fraction}");
            }
        }
    }

    #[test]
    fn the_bars_move_over_time_and_are_not_all_the_same_height() {
        let (now, later) = (bar_fractions(0.0), bar_fractions(0.2));
        assert_ne!(now, later);
        assert!(now.iter().any(|height| (height - now[0]).abs() > 0.05));
    }

    #[test]
    fn the_wave_repeats_after_a_cycle() {
        let (start, after) = (bar_fractions(0.3), bar_fractions(0.3 + 1.0 / CYCLES_PER_SECOND));
        for (a, b) in start.iter().zip(after.iter()) {
            assert!((a - b).abs() < 1e-3);
        }
    }
}
