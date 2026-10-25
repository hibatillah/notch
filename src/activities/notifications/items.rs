//! A notification as Windows gave it, and how it is worded in the banner. Pure, so it is tested without a window.

use std::sync::Arc;

use crate::gfx::{Color, ImageData};

/// What a notification says in private mode, in place of its text.
pub const PRIVATE_TEXT: &str = "New message";

/// One notification, as Windows gave it.
#[derive(Clone)]
pub struct Item {
    pub id: u32,
    pub app: String,
    /// Identifies the app, so it can be opened.
    pub app_id: String,
    /// The app's logo, when Windows has one; apps with the same id share it.
    pub icon: Option<Arc<ImageData>>,
    pub title: String,
    pub body: String,
}

const PALETTE: [Color; 8] = [
    Color::rgb(0.345, 0.396, 0.949),
    Color::rgb(0.169, 0.541, 0.243),
    Color::rgb(0.788, 0.165, 0.165),
    Color::rgb(0.110, 0.494, 0.839),
    Color::rgb(0.910, 0.349, 0.047),
    Color::rgb(0.404, 0.267, 0.851),
    Color::rgb(0.047, 0.522, 0.600),
    Color::rgb(0.761, 0.145, 0.361),
];

/// The color of an app's tile: the same app always gets the same one.
pub fn app_color(app: &str) -> Color {
    let hash = app.bytes().fold(5381u32, |hash, byte| hash.wrapping_mul(33).wrapping_add(u32::from(byte)));
    PALETTE[hash as usize % PALETTE.len()]
}

/// The letter on an app's tile.
pub fn app_letter(app: &str) -> String {
    app.chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect())
        .unwrap_or_else(|| "?".to_owned())
}

/// The line above the message: who it is from, and its app when it has a title of its own.
pub fn heading(item: &Item) -> String {
    if item.title.is_empty() { item.app.clone() } else { format!("{} \u{00b7} {}", item.title, item.app) }
}

/// The message on one line with single spaces, or the private-mode words.
pub fn body_text(item: &Item, shows_text: bool) -> String {
    if !shows_text { PRIVATE_TEXT.to_owned() } else { item.body.split_whitespace().collect::<Vec<_>>().join(" ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, title: &str, body: &str) -> Item {
        Item {
            id,
            app: "Discord".to_owned(),
            app_id: String::new(),
            icon: None,
            title: title.to_owned(),
            body: body.to_owned(),
        }
    }

    #[test]
    fn an_app_always_gets_the_same_color_and_a_letter() {
        assert_eq!(app_color("Discord"), app_color("Discord"));
        assert_eq!(app_letter("discord"), "D");
        assert_eq!(app_letter("  \u{1f600} mail"), "M");
        assert_eq!(app_letter(""), "?");
    }

    #[test]
    fn the_heading_names_the_sender_and_the_app() {
        assert_eq!(heading(&item(1, "Rafi", "hi")), "Rafi \u{00b7} Discord");
        assert_eq!(heading(&item(1, "", "hi")), "Discord");
    }

    #[test]
    fn private_mode_leaves_the_text_out() {
        let message = item(1, "Rafi", "Deploy is green.");
        assert_eq!(body_text(&message, false), PRIVATE_TEXT);
        assert_eq!(body_text(&message, true), "Deploy is green.");
    }

    #[test]
    fn a_long_message_is_written_on_one_line_with_single_spaces() {
        assert_eq!(body_text(&item(1, "a", "one   two\nthree"), true), "one two three");
    }
}
