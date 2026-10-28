//! The running timer uses a collapsed presence and announces phase endings with sound.

mod layout;
mod session;
mod view;

use std::time::{SystemTime, UNIX_EPOCH};

use windows::Win32::{System::Diagnostics::Debug::MessageBeep, UI::WindowsAndMessaging::MB_ICONASTERISK};

use self::{
    layout::{EXPANDED_SIZE, Layout},
    session::{Phase, State, Timer, next_wake},
    view::{Action, Compact, Tone},
};
use crate::{
    activity::{FLASH_TIMER, Flash, Presence},
    gfx::{Align, Canvas, Color, FontStyle, Fonts, Rect, UI_FONT, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    ui::{self, Icon, PillButton, Ring},
};

const FOCUS_ORANGE: Color = Color::rgb(1.0, 0.57, 0.17);
const BREAK_GREEN: Color = Color::rgb(0.19, 0.82, 0.35);
const PAUSED_GRAY: Color = Color::gray(0.6);

const RESTING_WIDTH: f32 = 104.0;
const RING_DIAMETER: f32 = 14.0;
const RING_GAP: f32 = 6.0;
const BADGE_TRAILING: f32 = 6.0;
const ALERT_MILLISECONDS: u32 = 5000;

const TITLE: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Leading);
const DETAIL: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Regular, Align::Leading);
const TIME: FontStyle = FontStyle::new("Consolas", 40.0, Weight::Medium, Align::Leading);
const BADGE: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Medium, Align::Leading);

/// Timer zero belongs to Flash; the session uses a separate timer.
const TICK_TIMER: TimerId = TimerId(1);

pub struct TimerPage {
    timer: Timer,
    is_open: bool,
    hovered_button: Option<usize>,
    alert: Flash,
    alert_title: String,
    alert_value: String,
    alert_width: f32,
    alert_tint: Color,
    compact: Option<Compact>,
    compact_width: f32,
}

impl TimerPage {
    pub fn new() -> Self {
        Self {
            timer: Timer::new(),
            is_open: false,
            hovered_button: None,
            alert: Flash::default(),
            alert_title: String::new(),
            alert_value: String::new(),
            alert_width: 0.0,
            alert_tint: Color::WHITE,
            compact: None,
            compact_width: 0.0,
        }
    }

    fn refresh_compact(&mut self, fonts: &Fonts) {
        self.compact = view::compact(&self.timer, now_ms());
        if let Some(compact) = &self.compact {
            let text_width = fonts.measure(&BADGE, &view::widest_form(&compact.text)).ceil();
            self.compact_width = RING_DIAMETER + RING_GAP + text_width + BADGE_TRAILING;
        }
    }

    fn schedule(&self, context: &mut Context) {
        let remaining = match self.timer.state {
            State::Running { .. } => self.timer.remaining(now_ms()),
            _ => None,
        };
        match remaining {
            Some(remaining) => {
                let milliseconds = next_wake(remaining, self.is_open).min(u64::from(u32::MAX));
                context.set_timer(TICK_TIMER, milliseconds as u32);
            }
            None => context.kill_timer(TICK_TIMER),
        }
    }

    #[cfg(test)]
    pub fn sample(fonts: &Fonts) -> Self {
        let mut page = Self::new();
        let now = now_ms();
        page.timer.state =
            State::Running { phase: Phase::Focus, total: 25 * 60_000, ends_at: now + 16 * 60_000 + 43_000 };
        page.timer.completed = 1;
        page.refresh_compact(fonts);
        page
    }

    fn changed(&mut self, context: &mut Context) {
        self.timer.save(context.settings);
        self.refresh_compact(context.fonts());
        self.schedule(context);
        context.request_repaint();
    }

    fn announce(&mut self, context: &mut Context) {
        let Some((title, value)) = view::alert(&self.timer) else { return };
        self.alert_tint = if self.timer.phase() == Some(Phase::Focus) { BREAK_GREEN } else { FOCUS_ORANGE };
        self.alert_width = ui::message_width(context.fonts(), &title, Some(&value));
        self.alert_title = title;
        self.alert_value = value;
        self.alert.show(context, ALERT_MILLISECONDS);
        let _ = unsafe { MessageBeep(MB_ICONASTERISK) };
    }

    fn perform(&mut self, action: Action, context: &mut Context) {
        let now = now_ms();
        match action {
            Action::Start => self.timer.start_focus(now),
            Action::Shorter => self.timer.adjust_focus_length(-1),
            Action::Longer => self.timer.adjust_focus_length(1),
            Action::Pause => self.timer.pause(now),
            Action::Resume => self.timer.resume(now),
            Action::Extend => self.timer.extend(),
            Action::Stop => self.timer.stop(),
            Action::StartNext => self.timer.start_next(now),
            Action::Skip => self.timer.skip(),
        }
        self.alert.on_timer(FLASH_TIMER, context);
        self.hovered_button = None;
        self.changed(context);
    }

    fn accent(&self) -> Color {
        match self.timer.state {
            State::Running { phase: Phase::Focus, .. } | State::Paused { phase: Phase::Focus, .. } | State::Idle => {
                FOCUS_ORANGE
            }
            State::Finished { .. } => match self.timer.next_phase() {
                Some(Phase::Focus) => FOCUS_ORANGE,
                Some(Phase::ShortBreak | Phase::LongBreak) | None => BREAK_GREEN,
            },
            State::Running { .. } | State::Paused { .. } => BREAK_GREEN,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn tone_color(tone: Tone) -> Color {
    match tone {
        Tone::Focus => FOCUS_ORANGE,
        Tone::Break => BREAK_GREEN,
        Tone::Paused => PAUSED_GRAY,
    }
}

impl Page for TimerPage {
    fn id(&self) -> &'static str {
        "timer"
    }

    /// The timer has nothing to say in the collapsed pill as a page; its compact view is a `presence`.
    fn name(&self) -> &'static str {
        "Focus timer"
    }

    fn icon(&self) -> Icon {
        Icon::Stopwatch
    }

    fn priority(&self, mode: Mode) -> Option<Priority> {
        (mode == Mode::Expanded).then_some(Priority::BACKGROUND)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: RESTING_WIDTH, expanded: EXPANDED_SIZE }
    }

    fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let layout = Layout { scale: frame.scale, bounds: frame.bounds };
        let view = view::view(&self.timer, now_ms());
        let has_time = view.time.is_some();
        let title = layout.title(has_time, view.detail.is_some());
        canvas.text(&TITLE, &view.title, title, Color::WHITE.faded(opacity));
        if let Some(detail) = &view.detail {
            canvas.text(&DETAIL, detail, layout.detail(has_time), Color::gray(0.6).faded(opacity));
        }
        if let Some(time) = &view.time {
            canvas.text(&TIME, time, layout.time(), Color::WHITE.faded(opacity));
        }
        let accent = self.accent();
        for (index, (spec, area)) in view.buttons.iter().zip(layout.buttons(view.buttons.len())).enumerate() {
            let button = PillButton {
                label: &spec.label,
                area,
                is_primary: spec.is_primary,
                is_destructive: spec.is_destructive,
                is_hovered: self.hovered_button == Some(index),
                accent,
            };
            button.draw(canvas, opacity);
        }
    }

    fn presence(&self) -> Option<Presence> {
        if self.alert.is_visible() {
            Some(Presence::Transient { width: self.alert_width })
        } else {
            self.compact.as_ref().map(|_| Presence::Badge { width: self.compact_width })
        }
    }

    fn header_status_width(&self) -> Option<f32> {
        self.compact.as_ref().map(|_| RING_DIAMETER)
    }

    fn header_status_rank(&self) -> u8 {
        1
    }

    fn draw_header_status(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        let Some(compact) = &self.compact else { return };
        let center = crate::gfx::Point::new(area.center_x(), area.center_y());
        Ring { center, diameter: RING_DIAMETER * scale }.draw(
            canvas,
            compact.fraction,
            tone_color(compact.tone),
            opacity,
        );
    }

    fn draw_presence(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        if self.alert.is_visible() {
            ui::draw_message(
                canvas,
                area,
                opacity,
                Icon::Bell,
                self.alert_tint,
                &self.alert_title,
                Some(&self.alert_value),
            );
            return;
        }
        let Some(compact) = &self.compact else { return };
        let color = tone_color(compact.tone);
        let diameter = RING_DIAMETER * scale;
        let center = crate::gfx::Point::new(area.left + diameter / 2.0, area.center_y());
        Ring { center, diameter }.draw(canvas, compact.fraction, color, opacity);
        let text_area = Rect::new(area.left + (RING_DIAMETER + RING_GAP) * scale, area.top, area.right, area.bottom);
        canvas.text(&BADGE, &compact.text, text_area, color.faded(opacity));
    }

    fn on_start(&mut self, context: &mut Context) {
        self.timer = Timer::load(context.settings, now_ms());
        self.refresh_compact(context.fonts());
        self.schedule(context);
    }

    fn on_hover_changed(&mut self, is_hovering: bool, context: &mut Context) {
        self.is_open = is_hovering;
        if !is_hovering {
            self.hovered_button = None;
        }
        self.schedule(context);
        context.request_repaint();
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let layout = Layout { scale: frame.scale, bounds: frame.bounds };
        let view = view::view(&self.timer, now_ms());
        let count = view.buttons.len();
        match event {
            PointerEvent::Moved(point) => {
                let hovered = layout.button_at(count, point);
                if hovered != self.hovered_button {
                    self.hovered_button = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Pressed(point) if frame.is_open() => {
                if let Some(index) = layout.button_at(count, point) {
                    self.perform(view.buttons[index].action, context);
                }
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                let was_hovered = self.hovered_button.take().is_some();
                if was_hovered {
                    context.request_repaint();
                }
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if self.alert.on_timer(timer, context) || timer != TICK_TIMER {
            return;
        }
        if self.timer.tick(now_ms()) {
            self.announce(context);
            self.changed(context);
        } else {
            self.refresh_compact(context.fonts());
            self.schedule(context);
            context.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timer_is_a_page_to_open_but_never_the_collapsed_page() {
        let page = TimerPage::new();
        assert_eq!(page.priority(Mode::Resting), None);
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::BACKGROUND));
    }

    #[test]
    fn an_idle_timer_shows_nothing_in_the_collapsed_pill() {
        assert_eq!(TimerPage::new().presence(), None);
    }
}
