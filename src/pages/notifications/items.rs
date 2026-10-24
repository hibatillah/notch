//! The notifications the page keeps and how they are worded. Pure, so it is tested without a window.

use std::time::Duration;

use crate::gfx::Color;

/// Notifications kept in the stack; older ones fall off.
pub const MAX_ITEMS: usize = 20;
/// What a notification says in private mode, in place of its text.
pub const PRIVATE_TEXT: &str = "New message";

/// One notification, as Windows gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: u32,
    pub app: String,
    /// Identifies the app, so it can be opened.
    pub app_id: String,
    pub title: String,
    pub body: String,
    /// How long ago it arrived, as of the last time the page looked.
    pub age: Duration,
}

/// The notifications on show, newest first.
#[derive(Default)]
pub struct Stack {
    items: Vec<Item>,
}

impl Stack {
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Puts a notification on top; one with the same id replaces the one already there.
    pub fn push(&mut self, item: Item) {
        self.items.retain(|existing| existing.id != item.id);
        self.items.insert(0, item);
        self.items.truncate(MAX_ITEMS);
    }

    pub fn dismiss(&mut self, id: u32) -> Option<Item> {
        let position = self.items.iter().position(|item| item.id == id)?;
        Some(self.items.remove(position))
    }

    /// Ages every notification by `elapsed`.
    pub fn age_by(&mut self, elapsed: Duration) {
        for item in &mut self.items {
            item.age += elapsed;
        }
    }

    /// Moves the notification at `position` to the top, where it is shown large.
    pub fn bring_to_top(&mut self, position: usize) {
        if position < self.items.len() {
            let item = self.items.remove(position);
            self.items.insert(0, item);
        }
    }
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

/// "now", "4 m", "2 h" or "3 d".
pub fn age_text(age: Duration) -> String {
    let seconds = age.as_secs();
    match seconds {
        0..=59 => "now".to_owned(),
        60..=3599 => format!("{} m", seconds / 60),
        3600..=86_399 => format!("{} h", seconds / 3600),
        _ => format!("{} d", seconds / 86_400),
    }
}

/// The two parts shown in the collapsed pill: who it is from and the first words; the text itself is left out in
/// private mode.
pub fn compact_parts(item: &Item, shows_text: bool) -> (String, String) {
    let title = if item.title.is_empty() { item.app.clone() } else { item.title.clone() };
    let text = if !shows_text {
        PRIVATE_TEXT.to_owned()
    } else if item.body.is_empty() {
        String::new()
    } else {
        first_line(&item.body)
    };
    (title, text)
}

/// The first line of a text, with runs of spaces made single.
pub fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The text of a large card: the message, or the private-mode words.
pub fn card_body(item: &Item, shows_text: bool) -> String {
    if !shows_text { PRIVATE_TEXT.to_owned() } else { item.body.split_whitespace().collect::<Vec<_>>().join(" ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, title: &str, body: &str) -> Item {
        Item {
            id,
            app: "Discord".to_owned(),
            app_id: "app".to_owned(),
            title: title.to_owned(),
            body: body.to_owned(),
            age: Duration::ZERO,
        }
    }

    #[test]
    fn the_newest_notification_is_on_top() {
        let mut stack = Stack::default();
        stack.push(item(1, "a", ""));
        stack.push(item(2, "b", ""));
        assert_eq!(stack.items()[0].id, 2);
    }

    #[test]
    fn a_notification_with_a_known_id_replaces_the_old_one() {
        let mut stack = Stack::default();
        stack.push(item(1, "old", ""));
        stack.push(item(2, "b", ""));
        stack.push(item(1, "new", ""));
        assert_eq!(stack.items().len(), 2);
        assert_eq!(stack.items()[0].title, "new");
    }

    #[test]
    fn the_stack_keeps_only_the_newest_notifications() {
        let mut stack = Stack::default();
        for id in 0..MAX_ITEMS as u32 + 5 {
            stack.push(item(id, "x", ""));
        }
        assert_eq!(stack.items().len(), MAX_ITEMS);
        assert_eq!(stack.items()[0].id, MAX_ITEMS as u32 + 4);
    }

    #[test]
    fn dismissing_removes_one_notification() {
        let mut stack = Stack::default();
        stack.push(item(1, "a", ""));
        stack.push(item(2, "b", ""));
        assert_eq!(stack.dismiss(1).map(|item| item.title), Some("a".to_owned()));
        assert_eq!(stack.dismiss(1), None);
        assert_eq!(stack.items().len(), 1);
    }

    #[test]
    fn a_chosen_notification_moves_to_the_top() {
        let mut stack = Stack::default();
        for id in 1..=3 {
            stack.push(item(id, "x", ""));
        }
        stack.bring_to_top(2);
        let ids: Vec<u32> = stack.items().iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![1, 3, 2]);
        stack.bring_to_top(9);
        assert_eq!(stack.items().len(), 3);
    }

    #[test]
    fn ages_grow_and_are_worded_in_the_biggest_unit() {
        let mut stack = Stack::default();
        stack.push(item(1, "a", ""));
        stack.age_by(Duration::from_secs(250));
        assert_eq!(age_text(stack.items()[0].age), "4 m");
        assert_eq!(age_text(Duration::from_secs(5)), "now");
        assert_eq!(age_text(Duration::from_secs(7300)), "2 h");
        assert_eq!(age_text(Duration::from_secs(200_000)), "2 d");
    }

    #[test]
    fn an_app_always_gets_the_same_color_and_a_letter() {
        assert_eq!(app_color("Discord"), app_color("Discord"));
        assert_eq!(app_letter("discord"), "D");
        assert_eq!(app_letter("  \u{1f600} mail"), "M");
        assert_eq!(app_letter(""), "?");
    }

    #[test]
    fn the_compact_view_names_the_sender_and_shows_the_first_words() {
        let message = item(1, "Rafi", "Deploy is green.\nCan you check the hover state?");
        assert_eq!(compact_parts(&message, true), ("Rafi".to_owned(), "Deploy is green.".to_owned()));
    }

    #[test]
    fn private_mode_leaves_the_text_out() {
        let message = item(1, "Rafi", "Deploy is green.");
        assert_eq!(compact_parts(&message, false), ("Rafi".to_owned(), PRIVATE_TEXT.to_owned()));
        assert_eq!(card_body(&message, false), PRIVATE_TEXT);
        assert_eq!(card_body(&message, true), "Deploy is green.");
    }

    #[test]
    fn a_notification_without_a_title_is_named_after_its_app() {
        assert_eq!(compact_parts(&item(1, "", "hello"), true).0, "Discord");
    }

    #[test]
    fn a_long_message_is_written_on_one_line_with_single_spaces() {
        assert_eq!(card_body(&item(1, "a", "one   two\nthree"), true), "one two three");
    }
}
