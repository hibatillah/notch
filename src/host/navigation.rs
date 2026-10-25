//! Moving between pages: which pages can be reached and in what order, which one opens and what the scroll wheel
//! does. Pure functions and small state machines, so the rules can be tested without a window.

use crate::page::Priority;

/// One mouse wheel notch. A notched wheel sends multiples of it; a touchpad sends much smaller amounts.
const WHEEL_STEP: i32 = 120;
/// How much touchpad scrolling moves the pages by one page.
const SMOOTH_UNITS_PER_PAGE: f32 = 300.0;

/// The order the user put the pages in and which of them are switched on, by page index.
pub struct PageList {
    order: Vec<usize>,
    enabled: Vec<bool>,
}

impl PageList {
    /// Every page on, in registration order.
    pub fn new(count: usize) -> Self {
        Self { order: (0..count).collect(), enabled: vec![true; count] }
    }

    /// Restores the lists saved by `order_text` and `disabled_text`, by page id. Unknown ids are ignored and pages
    /// that are not in `order_text` go last. Without `disabled_text`, only the pages in `opt_in` start switched off.
    pub fn load(ids: &[&str], opt_in: &[bool], order_text: Option<&str>, disabled_text: Option<&str>) -> Self {
        let words = |text: &str| -> Vec<String> {
            text.split(',').map(str::trim).filter(|word| !word.is_empty()).map(str::to_owned).collect()
        };
        let mut order: Vec<usize> = Vec::new();
        for word in order_text.map(words).unwrap_or_default() {
            if let Some(index) = ids.iter().position(|id| *id == word)
                && !order.contains(&index)
            {
                order.push(index);
            }
        }
        let missing: Vec<usize> = (0..ids.len()).filter(|index| !order.contains(index)).collect();
        order.extend(missing);
        let enabled = match disabled_text {
            Some(text) => {
                let disabled = words(text);
                ids.iter().map(|id| !disabled.iter().any(|word| word == id)).collect()
            }
            None => opt_in.iter().map(|is_opt_in| !is_opt_in).collect(),
        };
        Self { order, enabled }
    }

    pub fn order(&self) -> &[usize] {
        &self.order
    }

    pub fn is_enabled(&self, page: usize) -> bool {
        self.enabled.get(page).copied().unwrap_or(false)
    }

    pub fn set_enabled(&mut self, page: usize, is_enabled: bool) {
        if let Some(slot) = self.enabled.get_mut(page) {
            *slot = is_enabled;
        }
    }

    /// Puts `page` at `position` in the order, moving the pages in between.
    pub fn move_to(&mut self, page: usize, position: usize) {
        let Some(from) = self.order.iter().position(|&index| index == page) else { return };
        self.order.remove(from);
        self.order.insert(position.min(self.order.len()), page);
    }

    /// The pages to save in order, as ids.
    pub fn order_text(&self, ids: &[&str]) -> String {
        self.order.iter().filter_map(|&index| ids.get(index).copied()).collect::<Vec<_>>().join(",")
    }

    /// The switched-off pages, as ids.
    pub fn disabled_text(&self, ids: &[&str]) -> String {
        let off = |index: &usize| !self.is_enabled(*index);
        (0..ids.len()).filter(off).map(|index| ids[index]).collect::<Vec<_>>().join(",")
    }

    /// The pages that can be opened right now: switched on and with something to show, in the user's order.
    pub fn navigable(&self, priorities: &[Option<Priority>]) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|&index| self.is_enabled(index) && priorities.get(index).is_some_and(Option::is_some))
            .collect()
    }

    /// The priorities with the switched-off pages removed, for `choose`.
    pub fn effective(&self, priorities: &[Option<Priority>]) -> Vec<Option<Priority>> {
        priorities
            .iter()
            .enumerate()
            .map(|(index, priority)| if self.is_enabled(index) { *priority } else { None })
            .collect()
    }
}

/// The page to show when open: a page that must interrupt (urgent priority) first, then the one the user chose if it
/// has something to show, else the highest priority.
pub fn choose(selected: Option<usize>, priorities: &[Option<Priority>]) -> Option<usize> {
    if let Some(index) = highest(priorities, Priority::URGENT) {
        return Some(index);
    }
    if let Some(index) = selected
        && priorities.get(index).is_some_and(Option::is_some)
    {
        return Some(index);
    }
    highest(priorities, Priority::BACKGROUND)
}

/// The page with the highest priority that is at least `lowest`; the earliest page wins a tie.
fn highest(priorities: &[Option<Priority>], lowest: Priority) -> Option<usize> {
    let mut best: Option<(usize, Priority)> = None;
    for (index, priority) in priorities.iter().enumerate() {
        let Some(priority) = *priority else { continue };
        if priority >= lowest && best.is_none_or(|(_, best_priority)| priority > best_priority) {
            best = Some((index, priority));
        }
    }
    best.map(|(index, _)| index)
}

/// What one scroll event asks for.
#[derive(Debug, PartialEq)]
pub enum Scroll {
    /// Whole pages, from a notched wheel: each notch is one page, and many notches go many pages.
    Pages(i32),
    /// A fraction of a page, from a touchpad: the pages follow the fingers.
    Drag(f32),
}

/// Sorts a scroll amount (positive toward the next page) into whole pages or a drag.
pub fn classify_scroll(amount: i32) -> Scroll {
    if amount.abs() >= WHEEL_STEP {
        Scroll::Pages(amount / WHEEL_STEP)
    } else {
        Scroll::Drag(amount as f32 / SMOOTH_UNITS_PER_PAGE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOW: Option<Priority> = Some(Priority(1));
    const HIGH: Option<Priority> = Some(Priority(5));

    const IDS: [&str; 4] = ["clock", "media", "timer", "stats"];

    #[test]
    fn only_pages_with_something_to_show_can_be_opened() {
        let list = PageList::new(3);
        assert_eq!(list.navigable(&[LOW, None, HIGH]), vec![0, 2]);
        assert!(list.navigable(&[None, None, None]).is_empty());
    }

    #[test]
    fn pages_are_reached_in_the_users_order_and_switched_off_ones_are_skipped() {
        let mut list = PageList::new(3);
        list.move_to(2, 0);
        assert_eq!(list.navigable(&[LOW, LOW, LOW]), vec![2, 0, 1]);
        list.set_enabled(0, false);
        assert_eq!(list.navigable(&[LOW, LOW, LOW]), vec![2, 1]);
        assert_eq!(list.effective(&[LOW, LOW, LOW]), vec![None, LOW, LOW]);
    }

    #[test]
    fn moving_a_page_shifts_the_ones_in_between() {
        let mut list = PageList::new(4);
        list.move_to(0, 2);
        assert_eq!(list.order(), &[1, 2, 0, 3]);
        list.move_to(3, 0);
        assert_eq!(list.order(), &[3, 1, 2, 0]);
        list.move_to(1, 99);
        assert_eq!(list.order(), &[3, 2, 0, 1]);
    }

    #[test]
    fn the_saved_order_and_choices_come_back_by_id() {
        let mut list = PageList::new(4);
        list.move_to(3, 1);
        list.set_enabled(2, false);
        let (order, disabled) = (list.order_text(&IDS), list.disabled_text(&IDS));
        assert_eq!(order, "clock,stats,media,timer");
        assert_eq!(disabled, "timer");
        let restored = PageList::load(&IDS, &[false; 4], Some(&order), Some(&disabled));
        assert_eq!(restored.order(), list.order());
        assert!(!restored.is_enabled(2) && restored.is_enabled(3));
    }

    #[test]
    fn unknown_and_repeated_ids_are_ignored_and_missing_pages_go_last() {
        let list = PageList::load(&IDS, &[false; 4], Some("timer, nope, timer, clock"), None);
        assert_eq!(list.order(), &[2, 0, 1, 3]);
    }

    #[test]
    fn without_a_saved_choice_only_opt_in_pages_start_switched_off() {
        let list = PageList::load(&IDS, &[false, false, false, true], None, None);
        assert!(list.is_enabled(0) && !list.is_enabled(3));
        let saved = PageList::load(&IDS, &[false, false, false, true], None, Some(""));
        assert!(saved.is_enabled(3));
    }

    #[test]
    fn without_a_choice_the_highest_priority_opens() {
        assert_eq!(choose(None, &[LOW, HIGH, LOW]), Some(1));
        assert_eq!(choose(None, &[None, None]), None);
    }

    #[test]
    fn the_chosen_page_opens_even_when_another_ranks_higher() {
        assert_eq!(choose(Some(0), &[LOW, HIGH]), Some(0));
    }

    #[test]
    fn an_urgent_page_comes_before_the_chosen_one() {
        const URGENT: Option<Priority> = Some(Priority::URGENT);
        assert_eq!(choose(Some(0), &[LOW, URGENT]), Some(1));
        assert_eq!(choose(None, &[URGENT, URGENT]), Some(0));
    }

    #[test]
    fn a_chosen_page_with_nothing_to_show_gives_way_to_the_best_one() {
        assert_eq!(choose(Some(0), &[None, HIGH]), Some(1));
        assert_eq!(choose(Some(7), &[LOW]), Some(0));
    }

    #[test]
    fn a_wheel_notch_is_one_page_and_several_notches_are_several_pages() {
        assert_eq!(classify_scroll(120), Scroll::Pages(1));
        assert_eq!(classify_scroll(-120), Scroll::Pages(-1));
        assert_eq!(classify_scroll(360), Scroll::Pages(3));
    }

    #[test]
    fn a_touchpad_scroll_is_a_fraction_of_a_page() {
        assert_eq!(classify_scroll(30), Scroll::Drag(0.1));
        assert_eq!(classify_scroll(-60), Scroll::Drag(-0.2));
    }
}
