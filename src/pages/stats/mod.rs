//! Opt-in system stats; sample every two seconds only while visible.

mod format;
mod sampler;

use std::collections::VecDeque;

use self::{
    format::{disk_text, memory_pair_text, percent_text, rate_text},
    sampler::{Sample, Sampler},
};
use crate::{
    gfx::{Align, Canvas, Color, FontStyle, Rect, Size, UI_FONT, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, Priority, TimerId},
    ui::Icon,
};

const EXPANDED_SIZE: Size = Size::new(508.0, 86.0);
const RESTING_WIDTH: f32 = 150.0;
const TILE_COUNT: usize = 4;
const TILE_GAP: f32 = 8.0;
const TILE_RADIUS: f32 = 12.0;
const TILE_PADDING_X: f32 = 12.0;
const TILE_PADDING_Y: f32 = 7.0;
const GRAPHIC_TOP: f32 = 47.0;
const GRAPHIC_HEIGHT: f32 = 16.0;
const SUB_TOP: f32 = 66.0;
const BAR_HEIGHT: f32 = 5.0;
const HISTORY_LENGTH: usize = 30;

const SAMPLE_TIMER: TimerId = TimerId(0);
const FIRST_SAMPLE_MILLISECONDS: u32 = 600;
const SAMPLE_INTERVAL_MILLISECONDS: u32 = 2000;

const MONO: &str = "Consolas";
const LABEL: FontStyle = FontStyle::new(UI_FONT, 11.0, Weight::Regular, Align::Leading);
const VALUE: FontStyle = FontStyle::new(MONO, 20.0, Weight::Medium, Align::Leading);
const RATE: FontStyle = FontStyle::new(MONO, 14.0, Weight::Medium, Align::Leading);

const TILE: Color = Color::rgb(0.078, 0.078, 0.094);
const TRACK: Color = Color::rgb(0.149, 0.149, 0.176);
const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const CPU_BLUE: Color = Color::rgb(0.302, 0.671, 0.969);
const MEMORY_PURPLE: Color = Color::rgb(0.745, 0.294, 0.859);
const DISK_GREEN: Color = Color::rgb(0.251, 0.753, 0.341);

pub struct StatsPage {
    sampler: Sampler,
    latest: Option<Sample>,
    cpu_history: VecDeque<f32>,
}

impl StatsPage {
    pub fn new() -> Self {
        Self { sampler: Sampler::new(), latest: None, cpu_history: VecDeque::with_capacity(HISTORY_LENGTH) }
    }

    #[cfg(test)]
    pub fn sample() -> Self {
        let mut page = Self::new();
        let reading = page.sampler.sample();
        for index in 0..HISTORY_LENGTH {
            let wave = 20.0 + 18.0 * ((index as f32) * 0.6).sin() + (index % 5) as f32 * 3.0;
            page.record(Sample {
                cpu_percent: Some(wave),
                memory_percent: 51.0,
                memory_used_bytes: 8_697_308_160,
                memory_total_bytes: 17_179_869_184,
                download_bytes_per_second: Some(2_400_000.0),
                upload_bytes_per_second: Some(120_000.0),
                disk_percent: 61.0,
                disk_free_bytes: 200_000_000_000,
                ..reading
            });
        }
        page
    }

    fn record(&mut self, sample: Sample) {
        if let Some(cpu) = sample.cpu_percent {
            if self.cpu_history.len() == HISTORY_LENGTH {
                self.cpu_history.pop_front();
            }
            self.cpu_history.push_back(cpu);
        }
        self.latest = Some(sample);
    }
}

fn tile_rects(bounds: Rect, scale: f32) -> [Rect; TILE_COUNT] {
    let gap = TILE_GAP * scale;
    let width = (bounds.width() - gap * (TILE_COUNT - 1) as f32) / TILE_COUNT as f32;
    std::array::from_fn(|index| {
        let left = bounds.left + index as f32 * (width + gap);
        Rect::new(left, bounds.top, left + width, bounds.bottom)
    })
}

fn inside(tile: Rect, scale: f32, left: f32, top: f32, bottom: f32) -> Rect {
    Rect::new(
        tile.left + left * scale,
        tile.top + top * scale,
        tile.right - TILE_PADDING_X * scale,
        tile.top + bottom * scale,
    )
}

fn draw_bar(canvas: &Canvas, tile: Rect, scale: f32, percent: f32, color: Color, opacity: f32) {
    let area = inside(tile, scale, TILE_PADDING_X, 0.0, 0.0);
    let center_y = tile.top + (GRAPHIC_TOP + GRAPHIC_HEIGHT / 2.0) * scale;
    let half = BAR_HEIGHT * scale / 2.0;
    let track = Rect::new(area.left, center_y - half, area.right, center_y + half);
    canvas.fill_rounded_rect(track, half, TRACK.faded(opacity));
    let fill_right = track.left + track.width() * (percent / 100.0).clamp(0.0, 1.0);
    if fill_right - track.left > half {
        canvas.fill_rounded_rect(Rect { right: fill_right, ..track }, half, color.faded(opacity));
    }
}

fn draw_history(canvas: &Canvas, tile: Rect, scale: f32, history: &VecDeque<f32>, opacity: f32) {
    let area = inside(tile, scale, TILE_PADDING_X, 0.0, 0.0);
    let bottom = tile.top + (GRAPHIC_TOP + GRAPHIC_HEIGHT) * scale;
    let slot = (area.right - area.left) / HISTORY_LENGTH as f32;
    let first_slot = HISTORY_LENGTH - history.len();
    for (position, value) in history.iter().enumerate() {
        let left = area.left + (first_slot + position) as f32 * slot;
        let height = (value / 100.0).clamp(0.0, 1.0) * GRAPHIC_HEIGHT * scale;
        let rect = Rect::new(left + slot * 0.15, bottom - height.max(1.5 * scale), left + slot * 0.85, bottom);
        canvas.fill_rounded_rect(rect, slot * 0.3, CPU_BLUE.faded(opacity * 0.85));
    }
}

fn text(canvas: &Canvas, style: &FontStyle, content: &str, area: Rect, color: Color, opacity: f32) {
    canvas.text(style, content, area, color.faded(opacity));
}

impl Page for StatsPage {
    fn id(&self) -> &'static str {
        "stats"
    }

    fn name(&self) -> &'static str {
        "System stats"
    }

    fn icon(&self) -> Icon {
        Icon::Chart
    }

    fn is_opt_in(&self) -> bool {
        true
    }

    fn priority(&self, mode: Mode) -> Option<Priority> {
        (mode == Mode::Expanded).then_some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let scale = frame.scale;
        let tiles = tile_rects(frame.bounds, scale);
        for tile in tiles {
            canvas.fill_rounded_rect(tile, TILE_RADIUS * scale, TILE.faded(opacity));
        }
        let Some(sample) = &self.latest else { return };
        let label_area = |tile: Rect| inside(tile, scale, TILE_PADDING_X, TILE_PADDING_Y, TILE_PADDING_Y + 14.0);
        let value_area = |tile: Rect| inside(tile, scale, TILE_PADDING_X, 19.0, 45.0);
        let sub_area = |tile: Rect| inside(tile, scale, TILE_PADDING_X, SUB_TOP, SUB_TOP + 14.0);

        text(canvas, &LABEL, "CPU", label_area(tiles[0]), GRAY, opacity);
        let cpu = sample.cpu_percent.map_or_else(|| "\u{2013}".to_owned(), percent_text);
        text(canvas, &VALUE, &cpu, value_area(tiles[0]), Color::WHITE, opacity);
        draw_history(canvas, tiles[0], scale, &self.cpu_history, opacity);
        text(canvas, &LABEL, &format!("{} threads", sample.logical_processors), sub_area(tiles[0]), GRAY, opacity);

        text(canvas, &LABEL, "Memory", label_area(tiles[1]), GRAY, opacity);
        text(canvas, &VALUE, &percent_text(sample.memory_percent), value_area(tiles[1]), Color::WHITE, opacity);
        draw_bar(canvas, tiles[1], scale, sample.memory_percent, MEMORY_PURPLE, opacity);
        let memory = memory_pair_text(sample.memory_used_bytes, sample.memory_total_bytes);
        text(canvas, &LABEL, &memory, sub_area(tiles[1]), GRAY, opacity);

        text(canvas, &LABEL, "Network", label_area(tiles[2]), GRAY, opacity);
        let rate = |value: Option<f64>| value.map_or_else(|| "\u{2013}".to_owned(), rate_text);
        let download = format!("\u{2193} {}", rate(sample.download_bytes_per_second));
        let upload = format!("\u{2191} {}", rate(sample.upload_bytes_per_second));
        text(canvas, &RATE, &download, inside(tiles[2], scale, TILE_PADDING_X, 21.0, 37.0), Color::WHITE, opacity);
        text(canvas, &RATE, &upload, inside(tiles[2], scale, TILE_PADDING_X, 37.0, 53.0), GRAY, opacity);
        text(canvas, &LABEL, sample.network_kind.label(), sub_area(tiles[2]), GRAY, opacity);

        text(canvas, &LABEL, &self.sampler.disk_label(), label_area(tiles[3]), GRAY, opacity);
        text(canvas, &VALUE, &percent_text(sample.disk_percent), value_area(tiles[3]), Color::WHITE, opacity);
        draw_bar(canvas, tiles[3], scale, sample.disk_percent, DISK_GREEN, opacity);
        text(canvas, &LABEL, &format!("{} free", disk_text(sample.disk_free_bytes)), sub_area(tiles[3]), GRAY, opacity);
    }

    fn on_shown_changed(&mut self, is_shown: bool, context: &mut Context) {
        if is_shown {
            self.sampler.reset();
            self.cpu_history.clear();
            self.latest = Some(self.sampler.sample());
            context.set_timer(SAMPLE_TIMER, FIRST_SAMPLE_MILLISECONDS);
        } else {
            context.kill_timer(SAMPLE_TIMER);
        }
        context.request_repaint();
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if timer != SAMPLE_TIMER {
            return;
        }
        let sample = self.sampler.sample();
        self.record(sample);
        context.set_timer(SAMPLE_TIMER, SAMPLE_INTERVAL_MILLISECONDS);
        context.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stats_page_is_opt_in_and_has_no_collapsed_state() {
        let page = StatsPage::new();
        assert!(page.is_opt_in());
        assert_eq!(page.priority(Mode::Resting), None);
        assert!(page.priority(Mode::Expanded).is_some());
    }

    #[test]
    fn four_tiles_share_the_width_with_gaps_between() {
        let tiles = tile_rects(Rect::new(0.0, 0.0, 508.0, 86.0), 1.0);
        assert!((tiles[0].width() - 121.0).abs() < 0.01);
        assert!((tiles[1].left - tiles[0].right - TILE_GAP).abs() < 0.01);
        assert!((tiles[3].right - 508.0).abs() < 0.01);
    }

    #[test]
    fn the_history_keeps_only_the_most_recent_readings() {
        let mut page = StatsPage::new();
        let sample = Sampler::new().sample();
        for index in 0..HISTORY_LENGTH + 5 {
            page.record(Sample { cpu_percent: Some(index as f32), ..sample });
        }
        assert_eq!(page.cpu_history.len(), HISTORY_LENGTH);
        assert_eq!(page.cpu_history.front().copied(), Some(5.0));
    }

    #[test]
    fn a_reading_without_a_cpu_figure_adds_nothing_to_the_history() {
        let mut page = StatsPage::new();
        let sample = Sampler::new().sample();
        page.record(Sample { cpu_percent: None, ..sample });
        assert!(page.cpu_history.is_empty());
        assert!(page.latest.is_some());
    }
}
