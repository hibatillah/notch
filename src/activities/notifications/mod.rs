//! Hover keeps banners visible; notifications.show_text=off hides message text.
//! Unpackaged apps cannot read notification buttons, so banners cannot open their source.

mod items;
mod listener;

use std::sync::Arc;

use self::{
    items::{Item, app_color, app_letter, body_text, heading},
    listener::{Access, Shared},
};
use crate::{
    activity::{Activity, FLASH_TIMER, Flash, Presence},
    gfx::{Align, Canvas, Color, FontStyle, Fonts, ImageData, Rect, UI_FONT, Weight, clamp_lines},
    log,
    page::{Context, TimerId},
};

const SHOW_MILLISECONDS: u32 = 4000;
const SHOW_TEXT_SETTING: &str = "notifications.show_text";

const WIDTH: f32 = 330.0;
const PADDING: f32 = 14.0;
const TOP_PADDING: f32 = 10.0;
const BOTTOM_PADDING: f32 = 12.0;
const TILE_SIZE: f32 = 30.0;
const TILE_RADIUS: f32 = 8.0;
const TILE_GAP: f32 = 12.0;
const HEADING_HEIGHT: f32 = 18.0;
const BODY_LINE_HEIGHT: f32 = 15.0;
const BODY_LINES: usize = 2;
const MIN_HEIGHT: f32 = 48.0;
const TEXT_WIDTH: f32 = WIDTH - 2.0 * PADDING - TILE_SIZE - TILE_GAP;

const HEADING: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Leading);
const BODY: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Regular, Align::Leading);
const TILE_LETTER: FontStyle = FontStyle::new(UI_FONT, 12.0, Weight::Medium, Align::Center);

const BODY_COLOR: Color = Color::rgb(0.68, 0.68, 0.72);

fn banner_height(body_lines: usize) -> f32 {
    (TOP_PADDING + HEADING_HEIGHT + body_lines as f32 * BODY_LINE_HEIGHT + BOTTOM_PADDING).max(MIN_HEIGHT)
}

struct Words {
    heading: String,
    body: Vec<String>,
}

fn words(item: &Item, shows_text: bool, fonts: &Fonts) -> Words {
    let heading =
        clamp_lines(&heading(item), 1, TEXT_WIDTH, |text| fonts.measure(&HEADING, text)).pop().unwrap_or_default();
    let body = clamp_lines(&body_text(item, shows_text), BODY_LINES, TEXT_WIDTH, |text| fonts.measure(&BODY, text));
    Words { heading, body }
}

pub struct NotificationsActivity {
    shared: Option<Arc<Shared>>,
    shows_text: bool,
    flash: Flash,
    item: Option<Item>,
    words: Option<Words>,
    is_hovered: bool,
}

impl NotificationsActivity {
    pub fn new() -> Self {
        Self { shared: None, shows_text: true, flash: Flash::default(), item: None, words: None, is_hovered: false }
    }

    #[cfg(test)]
    pub fn sample(fonts: &Fonts) -> Self {
        let mut activity = Self::new();
        let item = Item {
            id: 1,
            app: "Discord".to_owned(),
            app_id: String::new(),
            icon: None,
            title: "Rafi".to_owned(),
            body: "Deploy is green. Can you check the hover state on the 1440p monitor before we tag it?".to_owned(),
        };
        activity.words = Some(words(&item, true, fonts));
        activity.item = Some(item);
        activity.flash = Flash::visible();
        activity
    }

    fn show(&mut self, item: Item, context: &mut Context) {
        self.words = Some(words(&item, self.shows_text, context.fonts()));
        self.item = Some(item);
        self.flash.show(context, SHOW_MILLISECONDS);
        if self.is_hovered {
            context.kill_timer(FLASH_TIMER);
        }
    }

    fn draw_tile(&self, canvas: &Canvas, item: &Item, tile: Rect, scale: f32, opacity: f32) {
        let logo = item.icon.as_deref().and_then(|icon| {
            let key = ("notification-logo", item.app_id.as_str());
            canvas.cached_image(key, || ImageData::new(icon.width, icon.height, icon.bgra_premultiplied.clone()))
        });
        match logo {
            Some(brush) => canvas.image(&brush, tile, TILE_RADIUS * scale, opacity),
            None => {
                canvas.fill_rounded_rect(tile, TILE_RADIUS * scale, app_color(&item.app).faded(opacity));
                canvas.text(&TILE_LETTER, &app_letter(&item.app), tile, Color::WHITE.faded(opacity));
            }
        }
    }
}

impl Activity for NotificationsActivity {
    fn id(&self) -> &'static str {
        "notifications"
    }

    fn label(&self) -> &'static str {
        "Notifications"
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn presence(&self) -> Option<Presence> {
        let words = self.words.as_ref()?;
        self.flash.is_visible().then_some(Presence::Banner { width: WIDTH, height: banner_height(words.body.len()) })
    }

    fn draw(&self, canvas: &Canvas, area: Rect, scale: f32, opacity: f32) {
        let (Some(item), Some(words)) = (&self.item, &self.words) else { return };
        let tile = Rect::new(
            area.left + PADDING * scale,
            area.top + TOP_PADDING * scale,
            area.left + (PADDING + TILE_SIZE) * scale,
            area.top + (TOP_PADDING + TILE_SIZE) * scale,
        );
        self.draw_tile(canvas, item, tile, scale, opacity);
        let left = tile.right + TILE_GAP * scale;
        let right = area.right - PADDING * scale;
        let top = area.top + TOP_PADDING * scale;
        let heading = Rect::new(left, top, right, top + HEADING_HEIGHT * scale);
        canvas.text(&HEADING, &words.heading, heading, Color::WHITE.faded(opacity));
        for (line, text) in words.body.iter().enumerate() {
            let line_top = heading.bottom + line as f32 * BODY_LINE_HEIGHT * scale;
            let rect = Rect::new(left, line_top, right, line_top + BODY_LINE_HEIGHT * scale);
            canvas.text(&BODY, text, rect, BODY_COLOR.faded(opacity));
        }
    }

    fn on_start(&mut self, context: &mut Context) {
        self.shows_text = context.settings.get_bool(SHOW_TEXT_SETTING, true);
        self.shared = Some(listener::start(context.notifier()));
    }

    fn on_timer(&mut self, timer: TimerId, context: &mut Context) {
        if timer == FLASH_TIMER {
            self.flash.on_timer(timer, context);
        }
    }

    fn on_hover_changed(&mut self, is_hovering: bool, context: &mut Context) {
        self.is_hovered = is_hovering;
        if is_hovering {
            context.kill_timer(FLASH_TIMER);
        } else if self.flash.is_visible() {
            self.flash.on_timer(FLASH_TIMER, context);
        }
    }

    fn on_notify(&mut self, context: &mut Context) {
        let Some(shared) = self.shared.clone() else { return };
        if shared.access() == Access::Denied {
            log::debug("notifications: Windows does not let the notch read notifications");
        }
        if let Some(newest) = shared.take_arrived().pop() {
            self.show(newest, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_grows_with_the_lines_of_the_message() {
        assert_eq!(banner_height(0), MIN_HEIGHT);
        assert!(banner_height(1) < banner_height(2));
        assert_eq!(banner_height(2), TOP_PADDING + HEADING_HEIGHT + 2.0 * BODY_LINE_HEIGHT + BOTTOM_PADDING);
    }

    #[test]
    fn the_banner_is_taller_than_the_idle_pill() {
        assert!(banner_height(0) > crate::page::RESTING_HEIGHT * 1.4);
    }

    #[test]
    fn a_long_message_takes_two_lines_and_a_long_heading_one() {
        let fonts = Fonts::new(1.0).unwrap();
        let item = Item {
            id: 1,
            app: "Discord".to_owned(),
            app_id: String::new(),
            icon: None,
            title: "A very long conversation name that goes on and on and on".to_owned(),
            body: "Deploy is green. Can you check the hover state on the 1440p monitor before we tag it? ".repeat(3),
        };
        let words = words(&item, true, &fonts);
        assert!(words.heading.ends_with('\u{2026}'));
        assert_eq!(words.body.len(), BODY_LINES);
        assert!(words.body[BODY_LINES - 1].ends_with('\u{2026}'));
    }

    #[test]
    fn private_mode_shows_only_the_private_words() {
        let fonts = Fonts::new(1.0).unwrap();
        let item = Item {
            id: 1,
            app: "Mail".to_owned(),
            app_id: String::new(),
            icon: None,
            title: "Invoice".to_owned(),
            body: "secret".to_owned(),
        };
        assert_eq!(words(&item, false, &fonts).body, vec![items::PRIVATE_TEXT.to_owned()]);
    }

    #[test]
    fn nothing_shows_until_a_notification_arrives() {
        assert_eq!(NotificationsActivity::new().presence(), None);
    }

    #[test]
    fn the_banner_is_the_same_size_whether_or_not_the_pointer_is_on_it() {
        let fonts = Fonts::new(1.0).unwrap();
        let mut activity = NotificationsActivity::sample(&fonts);
        let calm = activity.presence();
        activity.is_hovered = true;
        assert_eq!(activity.presence(), calm);
    }
}
