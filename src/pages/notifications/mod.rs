//! The notifications page: each new Windows notification shows in the collapsed pill for four seconds, with who it is
//! from and its first words. Pointing at the notch while one shows opens this page over the page you were on, with
//! the stack of recent notifications, each with Open and Dismiss. In private mode the text of a message is left out.
//!
//! The notch shows a notification a few seconds after Windows shows its own banner (see `listener`), and the page
//! can only show what Windows lets a program that is not a packaged app read. It adds to the Windows notification
//! center and does not replace it.

mod items;
mod listener;

use std::{sync::Arc, time::Duration};

use self::{
    items::{Item, Stack, age_text, app_color, app_letter, card_body, compact_parts},
    listener::{Access, Shared},
};
use crate::{
    activity::{FLASH_TIMER, Flash, Presence},
    gfx::{Align, Canvas, Color, FontStyle, Point, Rect, SEGOE_UI, Size, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, TimerId},
    platform,
    ui::{Icon, PillButton},
};

const CONTENT_WIDTH: f32 = 408.0;
const RESTING_WIDTH: f32 = 150.0;
const BIG_HEIGHT: f32 = 104.0;
const SMALL_HEIGHT: f32 = 36.0;
const CARD_GAP: f32 = 10.0;
const CARD_RADIUS: f32 = 12.0;
const CARD_PADDING: f32 = 10.0;
const EMPTY_HEIGHT: f32 = 60.0;
/// Cards on the page: one large and the rest small.
const MAX_CARDS: usize = 3;
const BIG_TILE: f32 = 30.0;
const SMALL_TILE: f32 = 24.0;
const COMPACT_TILE: f32 = 18.0;
const BUTTON_HEIGHT: f32 = 24.0;
const OPEN_WIDTH: f32 = 56.0;
const DISMISS_WIDTH: f32 = 72.0;
const ALERT_MILLISECONDS: u32 = 4000;
const AGE_REFRESH_SECONDS: u64 = 30;
const COMPACT_TITLE_LIMIT: f32 = 100.0;
const COMPACT_BODY_LIMIT: f32 = 150.0;

const SHOW_SETTING: &str = "notifications.enabled";
const TEXT_SETTING: &str = "notifications.show_text";
const AGE_TIMER: TimerId = TimerId(1);

const TITLE: FontStyle = FontStyle::new(SEGOE_UI, 13.0, Weight::SemiBold, Align::Leading);
const COMPACT_TITLE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::SemiBold, Align::Leading);
const COMPACT_BODY: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading);
const AGE: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Trailing);
const BODY: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Leading).wrapped();
const TILE_LETTER: FontStyle = FontStyle::new(SEGOE_UI, 11.0, Weight::SemiBold, Align::Center);
const EMPTY: FontStyle = FontStyle::new(SEGOE_UI, 12.0, Weight::Regular, Align::Center).wrapped();

const CARD: Color = Color::rgb(0.078, 0.078, 0.094);
const GRAY: Color = Color::rgb(0.604, 0.604, 0.651);
const LIGHT: Color = Color::rgb(0.839, 0.839, 0.871);
const ACCENT: Color = Color::rgb(0.302, 0.671, 0.969);

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Open,
    Dismiss,
    /// The small card at this position in the stack.
    Card(usize),
}

/// The height of the page's content for this many notifications.
fn content_height(count: usize) -> f32 {
    match count {
        0 => EMPTY_HEIGHT,
        1 => BIG_HEIGHT,
        _ => {
            let small = (count.min(MAX_CARDS) - 1) as f32;
            BIG_HEIGHT + small * (SMALL_HEIGHT + CARD_GAP)
        }
    }
}

/// The card at `position` (0 is the large one).
fn card_rect(position: usize, bounds: Rect, scale: f32) -> Rect {
    let top =
        if position == 0 { 0.0 } else { BIG_HEIGHT + CARD_GAP + (position - 1) as f32 * (SMALL_HEIGHT + CARD_GAP) };
    let height = if position == 0 { BIG_HEIGHT } else { SMALL_HEIGHT };
    Rect::new(bounds.left, bounds.top + top * scale, bounds.right, bounds.top + (top + height) * scale)
}

fn button_rects(bounds: Rect, scale: f32) -> (Rect, Rect) {
    let card = card_rect(0, bounds, scale);
    let top = card.bottom - (CARD_PADDING + BUTTON_HEIGHT) * scale;
    let left = card.left + (CARD_PADDING + BIG_TILE + 10.0) * scale;
    let open = Rect::new(left, top, left + OPEN_WIDTH * scale, top + BUTTON_HEIGHT * scale);
    let dismiss = Rect::new(
        open.right + 6.0 * scale,
        top,
        open.right + (6.0 + DISMISS_WIDTH) * scale,
        top + BUTTON_HEIGHT * scale,
    );
    (open, dismiss)
}

fn target_at(point: Point, bounds: Rect, scale: f32, count: usize) -> Option<Target> {
    if count == 0 {
        return None;
    }
    let (open, dismiss) = button_rects(bounds, scale);
    if open.contains(point) {
        return Some(Target::Open);
    }
    if dismiss.contains(point) {
        return Some(Target::Dismiss);
    }
    (1..count.min(MAX_CARDS)).find(|&position| card_rect(position, bounds, scale).contains(point)).map(Target::Card)
}

pub struct NotificationsPage {
    stack: Stack,
    shared: Option<Arc<Shared>>,
    is_enabled: bool,
    shows_text: bool,
    access: Access,
    alert: Flash,
    alert_item: Option<Item>,
    alert_width: f32,
    hovered: Option<Target>,
}

impl NotificationsPage {
    pub fn new() -> Self {
        Self {
            stack: Stack::default(),
            shared: None,
            is_enabled: true,
            shows_text: true,
            access: Access::Unknown,
            alert: Flash::default(),
            alert_item: None,
            alert_width: 0.0,
            hovered: None,
        }
    }

    /// A page with example notifications, for rendering the page off-screen.
    #[cfg(test)]
    pub fn sample(fonts: &crate::gfx::Fonts, with_alert: bool) -> Self {
        let mut page = Self::new();
        let item = |id: u32, app: &str, title: &str, body: &str, minutes: u64| Item {
            id,
            app: app.to_owned(),
            app_id: String::new(),
            title: title.to_owned(),
            body: body.to_owned(),
            age: Duration::from_secs(minutes * 60),
        };
        page.stack.push(item(3, "GitHub", "CI passed on main", "", 9));
        page.stack.push(item(2, "Mail", "Invoice for October", "", 4));
        page.stack.push(item(
            1,
            "Discord",
            "Rafi",
            "Deploy is green. Can you check the hover state on the 1440p monitor before we tag it?",
            0,
        ));
        if with_alert {
            let newest = page.stack.items()[0].clone();
            let (title, text) = compact_parts(&newest, true);
            let title_width = fonts.measure(&COMPACT_TITLE, &title).min(COMPACT_TITLE_LIMIT).ceil();
            let body_width = fonts.measure(&COMPACT_BODY, &text).min(COMPACT_BODY_LIMIT).ceil();
            page.alert_width = 10.0 + COMPACT_TILE + 8.0 + title_width + 8.0 + body_width + 10.0;
            page.alert_item = Some(newest);
            page.alert = Flash::visible();
        }
        page
    }

    fn show_alert(&mut self, item: &Item, context: &mut Context) {
        let (title, text) = compact_parts(item, self.shows_text);
        let fonts = context.fonts();
        let title_width = fonts.measure(&COMPACT_TITLE, &title).min(COMPACT_TITLE_LIMIT).ceil();
        let body_width = fonts.measure(&COMPACT_BODY, &text).min(COMPACT_BODY_LIMIT).ceil();
        let gap = if text.is_empty() { 0.0 } else { 8.0 };
        self.alert_width = 10.0 + COMPACT_TILE + 8.0 + title_width + gap + body_width + 10.0;
        self.alert_item = Some(item.clone());
        self.alert.show(context, ALERT_MILLISECONDS);
    }

    fn dismiss(&mut self, id: u32) {
        if self.stack.dismiss(id).is_some() {
            listener::remove(id);
        }
    }

    fn draw_tile(&self, canvas: &Canvas, item: &Item, area: Rect, radius: f32, opacity: f32) {
        canvas.fill_rounded_rect(area, radius, app_color(&item.app).faded(opacity));
        canvas.text(&TILE_LETTER, &app_letter(&item.app), area, Color::WHITE.faded(opacity));
    }
}

impl Page for NotificationsPage {
    fn id(&self) -> &'static str {
        "notifications"
    }

    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn icon(&self) -> Icon {
        Icon::Bell
    }

    /// Collapsed it has nothing to show as a page. Open it is an ordinary page, but while a notification is showing in
    /// the collapsed pill it comes first, so pointing at the notch shows the stack.
    fn priority(&self, mode: Mode) -> Option<Priority> {
        match mode {
            Mode::Resting => None,
            Mode::Expanded if self.alert.is_visible() => Some(Priority::URGENT),
            Mode::Expanded => Some(Priority::BACKGROUND),
        }
    }

    fn sizes(&self) -> PageSizes {
        PageSizes {
            resting_width: RESTING_WIDTH,
            expanded: Size::new(CONTENT_WIDTH, content_height(self.stack.items().len())),
        }
    }

    fn draw_resting(&self, _canvas: &Canvas, _frame: &Frame, _opacity: f32) {}

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let (bounds, scale) = (frame.bounds, frame.scale);
        let items = self.stack.items();
        if items.is_empty() {
            let message = match (self.is_enabled, self.access) {
                (false, _) => "Notifications are switched off. Use the tray menu to turn them on.",
                (true, Access::Denied) => {
                    "Windows does not let the notch read notifications. Allow it in Settings, Privacy & security, Notifications."
                }
                _ => "No notifications",
            };
            let area = Rect::new(bounds.left + 16.0 * scale, bounds.top, bounds.right - 16.0 * scale, bounds.bottom);
            canvas.text(&EMPTY, message, area, GRAY.faded(opacity));
            return;
        }
        for (position, item) in items.iter().take(MAX_CARDS).enumerate() {
            let card = card_rect(position, bounds, scale);
            canvas.fill_rounded_rect(card, CARD_RADIUS * scale, CARD.faded(opacity));
            let is_hovered = self.hovered == Some(Target::Card(position));
            if is_hovered {
                canvas.fill_rounded_rect(card, CARD_RADIUS * scale, Color::WHITE.with_alpha(0.05 * opacity));
            }
            if position == 0 {
                let tile = Rect::new(
                    card.left + CARD_PADDING * scale,
                    card.top + CARD_PADDING * scale,
                    card.left + (CARD_PADDING + BIG_TILE) * scale,
                    card.top + (CARD_PADDING + BIG_TILE) * scale,
                );
                self.draw_tile(canvas, item, tile, 7.0 * scale, opacity);
                let text_left = tile.right + 10.0 * scale;
                let right = card.right - CARD_PADDING * scale;
                let heading = if item.title.is_empty() {
                    item.app.clone()
                } else {
                    format!("{} \u{00b7} {}", item.title, item.app)
                };
                canvas.text(
                    &TITLE,
                    &heading,
                    Rect::new(
                        text_left,
                        card.top + CARD_PADDING * scale,
                        right - 44.0 * scale,
                        card.top + (CARD_PADDING + 18.0) * scale,
                    ),
                    Color::WHITE.faded(opacity),
                );
                canvas.text(
                    &AGE,
                    &age_text(item.age),
                    Rect::new(
                        right - 44.0 * scale,
                        card.top + CARD_PADDING * scale,
                        right,
                        card.top + (CARD_PADDING + 18.0) * scale,
                    ),
                    GRAY.faded(opacity),
                );
                let body = Rect::new(
                    text_left,
                    card.top + (CARD_PADDING + 20.0) * scale,
                    right,
                    card.top + (CARD_PADDING + 20.0 + 36.0) * scale,
                );
                canvas.text(&BODY, &card_body(item, self.shows_text), body, LIGHT.faded(opacity));
                let (open, dismiss) = button_rects(bounds, scale);
                for (label, area, target) in [("Open", open, Target::Open), ("Dismiss", dismiss, Target::Dismiss)] {
                    let button = PillButton {
                        label,
                        area,
                        is_primary: false,
                        is_hovered: self.hovered == Some(target),
                        accent: ACCENT,
                    };
                    button.draw(canvas, opacity);
                }
            } else {
                let tile = Rect::new(
                    card.left + CARD_PADDING * scale,
                    card.center_y() - SMALL_TILE * scale / 2.0,
                    card.left + (CARD_PADDING + SMALL_TILE) * scale,
                    card.center_y() + SMALL_TILE * scale / 2.0,
                );
                self.draw_tile(canvas, item, tile, 6.0 * scale, opacity);
                let text_left = tile.right + 10.0 * scale;
                let right = card.right - CARD_PADDING * scale;
                let heading = if item.title.is_empty() {
                    item.app.clone()
                } else {
                    format!("{} \u{00b7} {}", item.app, item.title)
                };
                canvas.text(
                    &COMPACT_TITLE,
                    &heading,
                    Rect::new(text_left, card.top, right - 44.0 * scale, card.bottom),
                    Color::WHITE.faded(opacity),
                );
                canvas.text(
                    &AGE,
                    &age_text(item.age),
                    Rect::new(right - 44.0 * scale, card.top, right, card.bottom),
                    GRAY.faded(opacity),
                );
            }
        }
    }

    fn presence(&self) -> Option<Presence> {
        (self.alert.is_visible() && self.is_enabled).then_some(Presence::Transient { width: self.alert_width })
    }

    fn draw_presence(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        let Some(item) = &self.alert_item else { return };
        let (title, text) = compact_parts(item, self.shows_text);
        let tile = Rect::new(
            area.left + 10.0 * scale,
            area.center_y() - COMPACT_TILE * scale / 2.0,
            area.left + (10.0 + COMPACT_TILE) * scale,
            area.center_y() + COMPACT_TILE * scale / 2.0,
        );
        canvas.fill_rounded_rect(tile, 5.0 * scale, app_color(&item.app).faded(opacity));
        canvas.text(
            &FontStyle::new(SEGOE_UI, 9.0, Weight::SemiBold, Align::Center),
            &app_letter(&item.app),
            tile,
            Color::WHITE.faded(opacity),
        );
        let title_width = canvas.measure(&COMPACT_TITLE, &title).min(COMPACT_TITLE_LIMIT).ceil() * scale;
        let title_left = tile.right + 8.0 * scale;
        canvas.text(
            &COMPACT_TITLE,
            &title,
            Rect::new(title_left, area.top, title_left + title_width + 2.0 * scale, area.bottom),
            Color::WHITE.faded(opacity),
        );
        if !text.is_empty() {
            let body_left = title_left + title_width + 8.0 * scale;
            canvas.text(
                &COMPACT_BODY,
                &text,
                Rect::new(body_left, area.top, area.right - 10.0 * scale, area.bottom),
                GRAY.faded(opacity),
            );
        }
    }

    fn menu_items(&self) -> Vec<String> {
        let mark = |is_on: bool| if is_on { "\u{2713} " } else { "    " };
        vec![
            format!("{}Show notifications in the notch", mark(self.is_enabled)),
            format!("{}Show message text", mark(self.shows_text)),
            "Open Windows notification settings".to_owned(),
        ]
    }

    fn on_menu_item(&mut self, index: usize, context: &mut Context) {
        match index {
            0 => {
                self.is_enabled = !self.is_enabled;
                context.settings.set_bool(SHOW_SETTING, self.is_enabled);
                if self.is_enabled && self.shared.is_none() {
                    self.shared = Some(listener::start(context.notifier()));
                }
            }
            1 => {
                self.shows_text = !self.shows_text;
                context.settings.set_bool(TEXT_SETTING, self.shows_text);
            }
            _ => platform::open("ms-settings:notifications"),
        }
        context.request_repaint();
    }

    fn on_start(&mut self, context: &mut Context) {
        self.is_enabled = context.settings.get_bool(SHOW_SETTING, true);
        self.shows_text = context.settings.get_bool(TEXT_SETTING, true);
        if self.is_enabled {
            self.shared = Some(listener::start(context.notifier()));
        }
    }

    fn on_notify(&mut self, context: &mut Context) {
        let Some(shared) = self.shared.clone() else { return };
        self.access = shared.access();
        let arrived = shared.take_arrived();
        for item in &arrived {
            self.stack.push(item.clone());
        }
        if self.is_enabled
            && let Some(newest) = arrived.last()
        {
            self.show_alert(newest, context);
        }
        context.request_repaint();
    }

    fn on_shown_changed(&mut self, is_shown: bool, context: &mut Context) {
        if is_shown {
            context.set_timer(AGE_TIMER, (AGE_REFRESH_SECONDS * 1000) as u32);
        } else {
            context.kill_timer(AGE_TIMER);
            self.hovered = None;
        }
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        let (bounds, scale, count) = (frame.bounds, frame.scale, self.stack.items().len());
        match event {
            PointerEvent::Moved(point) => {
                let hovered = target_at(point, bounds, scale, count);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    context.request_repaint();
                }
            }
            PointerEvent::Pressed(point) if frame.is_open() => {
                let Some(top) = self.stack.items().first().cloned() else { return };
                match target_at(point, bounds, scale, count) {
                    Some(Target::Open) => {
                        platform::open(&format!("shell:AppsFolder\\{}", top.app_id));
                        self.dismiss(top.id);
                    }
                    Some(Target::Dismiss) => self.dismiss(top.id),
                    Some(Target::Card(position)) => self.stack.bring_to_top(position),
                    None => {}
                }
                self.hovered = None;
                context.request_repaint();
            }
            PointerEvent::Left | PointerEvent::CaptureLost => {
                let was_hovered = self.hovered.take().is_some();
                if was_hovered {
                    context.request_repaint();
                }
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if timer == FLASH_TIMER {
            self.alert.on_timer(timer, context);
        } else if timer == AGE_TIMER {
            self.stack.age_by(Duration::from_secs(AGE_REFRESH_SECONDS));
            context.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: Rect = Rect::new(0.0, 0.0, 408.0, 196.0);

    #[test]
    fn the_page_grows_with_the_stack_up_to_three_cards() {
        assert_eq!(content_height(0), EMPTY_HEIGHT);
        assert_eq!(content_height(1), BIG_HEIGHT);
        assert_eq!(content_height(2), BIG_HEIGHT + SMALL_HEIGHT + CARD_GAP);
        assert_eq!(content_height(3), content_height(9));
        assert!(content_height(3) + 56.0 <= 260.0);
    }

    #[test]
    fn the_cards_stack_downward_with_a_gap() {
        let (first, second, third) = (card_rect(0, BOUNDS, 1.0), card_rect(1, BOUNDS, 1.0), card_rect(2, BOUNDS, 1.0));
        assert_eq!(second.top - first.bottom, CARD_GAP);
        assert_eq!(third.top - second.bottom, CARD_GAP);
        assert_eq!(third.bottom, content_height(3));
    }

    #[test]
    fn the_buttons_sit_inside_the_large_card() {
        let (open, dismiss) = button_rects(BOUNDS, 1.0);
        let card = card_rect(0, BOUNDS, 1.0);
        assert!(card.contains(Point::new(open.center_x(), open.center_y())));
        assert!(card.contains(Point::new(dismiss.center_x(), dismiss.center_y())));
        assert!(dismiss.left > open.right);
    }

    #[test]
    fn a_press_finds_the_button_or_card_under_it() {
        let (open, dismiss) = button_rects(BOUNDS, 1.0);
        assert_eq!(target_at(Point::new(open.center_x(), open.center_y()), BOUNDS, 1.0, 3), Some(Target::Open));
        assert_eq!(
            target_at(Point::new(dismiss.center_x(), dismiss.center_y()), BOUNDS, 1.0, 3),
            Some(Target::Dismiss)
        );
        let second = card_rect(1, BOUNDS, 1.0);
        assert_eq!(target_at(Point::new(second.center_x(), second.center_y()), BOUNDS, 1.0, 3), Some(Target::Card(1)));
        assert_eq!(target_at(Point::new(second.center_x(), second.center_y()), BOUNDS, 1.0, 1), None);
        assert_eq!(target_at(Point::new(50.0, 20.0), BOUNDS, 1.0, 0), None);
    }

    #[test]
    fn while_a_notification_shows_the_page_comes_before_the_page_you_were_on() {
        let page = NotificationsPage::new();
        assert_eq!(page.priority(Mode::Resting), None);
        assert_eq!(page.priority(Mode::Expanded), Some(Priority::BACKGROUND));
    }

    #[test]
    fn the_tray_menu_shows_which_options_are_on() {
        let page = NotificationsPage::new();
        let items = page.menu_items();
        assert!(items[0].starts_with('\u{2713}') && items[1].starts_with('\u{2713}'));
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn nothing_shows_in_the_collapsed_pill_without_a_notification() {
        assert_eq!(NotificationsPage::new().presence(), None);
    }
}
