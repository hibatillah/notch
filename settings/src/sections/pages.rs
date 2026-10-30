use gpui::{AnyElement, Context, IntoElement, Render, SharedString, Window, div, prelude::*, rgb, rgba};

use crate::{SettingsWindow, field, sections::apply};

const KNOWN_KEY: &str = "pages.known";
const ORDER_KEY: &str = "navigation.order";
const DISABLED_KEY: &str = "navigation.disabled";

pub struct KnownPage {
    pub id: String,
    pub label: String,
    pub opt_in: bool,
}

/// Reads the notch's `id:Label[:opt_in],...` list; an entry without a label shows its id.
pub fn parse_known(text: Option<&str>) -> Vec<KnownPage> {
    word_list(text)
        .into_iter()
        .filter_map(|entry| {
            let (entry, opt_in) = match entry.strip_suffix(":opt_in") {
                Some(rest) => (rest, true),
                None => (entry.as_str(), false),
            };
            let (id, label) = entry.split_once(':').unwrap_or((entry, ""));
            let (id, label) = (id.trim(), label.trim());
            (!id.is_empty()).then(|| KnownPage {
                id: id.to_owned(),
                label: if label.is_empty() { id } else { label }.to_owned(),
                opt_in,
            })
        })
        .collect()
}

/// Mirrors `PageList::load`: unknown and repeated ids are dropped and pages missing from the list go last.
pub fn current_order(known: &[KnownPage], order_text: Option<&str>) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    for word in word_list(order_text) {
        if known.iter().any(|page| page.id == word) && !order.contains(&word) {
            order.push(word);
        }
    }
    let missing: Vec<String> =
        known.iter().filter(|page| !order.contains(&page.id)).map(|page| page.id.clone()).collect();
    order.extend(missing);
    order
}

/// Mirrors `PageList::load`: without a saved list only the opt-in pages start switched off.
pub fn disabled_pages(known: &[KnownPage], disabled_text: Option<&str>) -> Vec<String> {
    match disabled_text {
        Some(_) => word_list(disabled_text),
        None => known.iter().filter(|page| page.opt_in).map(|page| page.id.clone()).collect(),
    }
}

pub fn word_list(text: Option<&str>) -> Vec<String> {
    text.unwrap_or_default().split(',').map(str::trim).filter(|word| !word.is_empty()).map(str::to_owned).collect()
}

pub fn reorder(order: &mut Vec<String>, from: usize, to: usize) {
    if from == to || from >= order.len() || to >= order.len() {
        return;
    }
    let page = order.remove(from);
    order.insert(to, page);
}

pub fn order_text(order: &[String], disabled: &[String]) -> (String, String) {
    (order.join(","), disabled.join(","))
}

/// Refuses to switch off the last enabled page: the notch would have nothing to open.
pub fn switch_page(order: &[String], disabled: &mut Vec<String>, id: &str, enabled: bool) -> bool {
    let is_disabled = disabled.iter().any(|word| word == id);
    if enabled {
        disabled.retain(|word| word != id);
        return true;
    }
    if is_disabled {
        return true;
    }
    let enabled_count = order.iter().filter(|page| !disabled.contains(page)).count();
    if enabled_count <= 1 {
        return false;
    }
    disabled.push(id.to_owned());
    true
}

struct DraggedPage {
    index: usize,
    label: SharedString,
}

struct DragPreview(SharedString);

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().px_3().py_1().rounded_md().bg(rgb(0x3a3a3a)).text_color(rgb(0xf0f0f0)).child(self.0.clone())
    }
}

pub fn render(window_state: &SettingsWindow, cx: &mut Context<SettingsWindow>) -> AnyElement {
    let settings = &window_state.settings;
    let known = parse_known(settings.get_string(KNOWN_KEY));
    if known.is_empty() {
        return div()
            .child("The notch has not reported its pages yet. Start the notch once, then reopen this window.")
            .into_any_element();
    }
    let order = current_order(&known, settings.get_string(ORDER_KEY));
    let disabled = disabled_pages(&known, settings.get_string(DISABLED_KEY));
    let rows = order.iter().enumerate().map(|(index, id)| {
        let label = known.iter().find(|page| &page.id == id).map(|page| page.label.clone()).unwrap_or_default();
        let shown_label = SharedString::from(label);
        let switch_id = id.clone();
        let switch = field::switch_row(
            shown_label.clone(),
            !disabled.contains(id),
            move |window_state| {
                let settings = &mut window_state.settings;
                settings.reload();
                let known = parse_known(settings.get_string(KNOWN_KEY));
                let order = current_order(&known, settings.get_string(ORDER_KEY));
                let mut disabled = disabled_pages(&known, settings.get_string(DISABLED_KEY));
                let enabled = disabled.contains(&switch_id);
                if switch_page(&order, &mut disabled, &switch_id, enabled) {
                    apply(settings, DISABLED_KEY, &order_text(&order, &disabled).1);
                }
            },
            cx,
        );
        let dragged = DraggedPage { index, label: shown_label };
        div()
            .id(("page-row", index))
            .py_1()
            .rounded_md()
            .cursor_grab()
            .on_drag(dragged, |dragged, _position, _window, cx| cx.new(|_| DragPreview(dragged.label.clone())))
            .drag_over::<DraggedPage>(|style, _dragged, _window, _cx| style.bg(rgba(field::ACCENT_TINT)))
            .on_drop(cx.listener(move |window_state, dragged: &DraggedPage, _window, cx| {
                let settings = &mut window_state.settings;
                settings.reload();
                let known = parse_known(settings.get_string(KNOWN_KEY));
                let mut order = current_order(&known, settings.get_string(ORDER_KEY));
                reorder(&mut order, dragged.index, index);
                apply(settings, ORDER_KEY, &order_text(&order, &[]).0);
                cx.notify();
            }))
            .child(switch)
    });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().child("Drag a page to reorder it. At least one page stays on."))
        .children(rows)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    fn known(text: &str) -> Vec<KnownPage> {
        parse_known(Some(text))
    }

    #[test]
    fn reordering_moves_a_page_to_the_new_position() {
        let mut order = strings(&["clock", "media", "timer", "stats"]);
        reorder(&mut order, 0, 2);
        assert_eq!(order, strings(&["media", "timer", "clock", "stats"]));
        reorder(&mut order, 3, 0);
        assert_eq!(order, strings(&["stats", "media", "timer", "clock"]));
    }

    #[test]
    fn reordering_to_the_same_or_an_out_of_range_position_changes_nothing() {
        let mut order = strings(&["clock", "media", "timer"]);
        let original = order.clone();
        reorder(&mut order, 1, 1);
        reorder(&mut order, 3, 0);
        reorder(&mut order, 0, 3);
        reorder(&mut order, 99, 99);
        assert_eq!(order, original);
        let mut empty: Vec<String> = Vec::new();
        reorder(&mut empty, 0, 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn switching_a_page_off_adds_it_to_disabled() {
        let order = strings(&["clock", "media", "timer"]);
        let mut disabled = Vec::new();
        assert!(switch_page(&order, &mut disabled, "media", false));
        assert_eq!(disabled, strings(&["media"]));
        assert!(switch_page(&order, &mut disabled, "media", false), "switching off twice adds it once");
        assert_eq!(disabled, strings(&["media"]));
        assert!(switch_page(&order, &mut disabled, "media", true));
        assert!(disabled.is_empty());
    }

    #[test]
    fn the_last_enabled_page_cannot_be_switched_off() {
        let order = strings(&["clock", "media"]);
        let mut disabled = strings(&["media"]);
        assert!(!switch_page(&order, &mut disabled, "clock", false));
        assert_eq!(disabled, strings(&["media"]));
    }

    #[test]
    fn order_and_disabled_are_written_as_comma_separated_ids() {
        let (order, disabled) = order_text(&strings(&["media", "clock"]), &strings(&["clock"]));
        assert_eq!((order.as_str(), disabled.as_str()), ("media,clock", "clock"));
        assert_eq!(order_text(&[], &[]), (String::new(), String::new()));
    }

    #[test]
    fn known_pages_are_read_with_their_labels_and_junk_is_skipped() {
        let pages = known("clock:Clock,media:Now Playing,,:Nameless,broken");
        let ids: Vec<&str> = pages.iter().map(|page| page.id.as_str()).collect();
        assert_eq!(ids, ["clock", "media", "broken"]);
        assert_eq!(pages[1].label, "Now Playing");
        assert_eq!(pages[2].label, "broken", "a missing label falls back to the id");
        assert!(parse_known(None).is_empty());
        assert!(parse_known(Some("  ")).is_empty());
    }

    #[test]
    fn the_shown_order_ignores_unknown_and_repeated_ids_and_appends_missing_pages() {
        let pages = known("clock:Clock,media:Media,timer:Timer,stats:Stats");
        let shown = current_order(&pages, Some("timer, nope, timer, clock"));
        assert_eq!(shown, strings(&["timer", "clock", "media", "stats"]));
        assert_eq!(current_order(&pages, None), strings(&["clock", "media", "timer", "stats"]));
        assert!(current_order(&[], Some("clock")).is_empty());
    }

    #[test]
    fn a_saved_order_with_an_unknown_page_survives_a_reorder_without_the_unknown_id() {
        let pages = known("clock:Clock,media:Media,timer:Timer");
        let mut order = current_order(&pages, Some("ghost,timer"));
        reorder(&mut order, 2, 0);
        let (text, _) = order_text(&order, &[]);
        assert_eq!(text, "media,timer,clock");
    }

    #[test]
    fn an_opt_in_page_is_shown_off_when_no_disabled_list_is_saved() {
        let pages = known("clock:Clock,media:Media,stats:Stats:opt_in");
        assert!(pages[2].opt_in && !pages[0].opt_in);
        assert_eq!(pages[2].label, "Stats");
        assert_eq!(disabled_pages(&pages, None), strings(&["stats"]));
        assert!(disabled_pages(&pages, Some("")).is_empty(), "a saved empty list switches everything on");
        assert_eq!(disabled_pages(&pages, Some("media")), strings(&["media"]));
    }

    #[test]
    fn old_two_field_entries_are_not_opt_in() {
        let pages = known("clock:Clock,stats:Stats");
        assert!(disabled_pages(&pages, None).is_empty());
    }

    #[test]
    fn the_first_toggle_of_another_page_keeps_the_opt_in_page_off() {
        let pages = known("clock:Clock,media:Media,stats:Stats:opt_in");
        let order = current_order(&pages, None);
        let mut disabled = disabled_pages(&pages, None);
        assert!(switch_page(&order, &mut disabled, "media", false));
        assert_eq!(order_text(&order, &disabled).1, "stats,media");
    }

    #[test]
    fn the_last_enabled_guard_counts_the_opt_in_default() {
        let pages = known("clock:Clock,stats:Stats:opt_in");
        let order = current_order(&pages, None);
        let mut disabled = disabled_pages(&pages, None);
        assert!(!switch_page(&order, &mut disabled, "clock", false));
        assert_eq!(disabled, strings(&["stats"]));
    }

    #[test]
    fn a_stored_disabled_list_is_trimmed_and_blank_entries_dropped() {
        assert_eq!(word_list(Some(" timer , ,stats")), strings(&["timer", "stats"]));
        assert!(word_list(None).is_empty());
    }
}
