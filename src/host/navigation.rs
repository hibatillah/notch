//! Moving between pages: which pages can be reached and in what order, which one opens and what the scroll wheel
//! does. Pure functions and small state machines, so the rules can be tested without a window.

use crate::page::Priority;

/// One mouse wheel notch. Finer wheels and touchpads send smaller amounts that add up to this.
const WHEEL_STEP: i32 = 120;
/// After a step by a notched wheel, further notches wait this long.
const WHEEL_COOLDOWN_MILLISECONDS: u64 = 250;
/// After a step by a touchpad or smooth wheel, input is ignored until it has been quiet this long, so one swipe and
/// its coasting move one page.
const GESTURE_GAP_MILLISECONDS: u64 = 200;

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

/// The page to show when open: the one the user chose if it has something to show, else the highest priority.
pub fn choose(selected: Option<usize>, priorities: &[Option<Priority>]) -> Option<usize> {
    if let Some(index) = selected
        && priorities.get(index).is_some_and(Option::is_some)
    {
        return Some(index);
    }
    let mut best: Option<(usize, Priority)> = None;
    for (index, priority) in priorities.iter().enumerate() {
        let Some(priority) = *priority else { continue };
        if best.is_none_or(|(_, best_priority)| priority > best_priority) {
            best = Some((index, priority));
        }
    }
    best.map(|(index, _)| index)
}

/// The page `step` places after `current` among the navigable ones (negative goes back); `None` at either end.
pub fn neighbour(navigable: &[usize], current: usize, step: i32) -> Option<usize> {
    let position = navigable.iter().position(|&index| index == current)? as i32;
    let target = usize::try_from(position + step).ok()?;
    navigable.get(target).copied()
}

/// Turns wheel and touchpad scrolling into page steps. Amounts are positive toward the next page.
#[derive(Default)]
pub struct Wheel {
    total: i32,
    last_input_at: Option<u64>,
    last_step_at: Option<u64>,
    is_locked: bool,
}

impl Wheel {
    /// Takes one scroll event at `now` (milliseconds) and returns the page step it causes: -1, 0 or 1.
    pub fn add(&mut self, amount: i32, now: u64) -> i32 {
        let is_smooth = amount.abs() < WHEEL_STEP;
        let quiet_for = self.last_input_at.map(|last| now.saturating_sub(last));
        self.last_input_at = Some(now);

        if self.is_locked {
            let has_gesture_ended = quiet_for.is_none_or(|gap| gap >= GESTURE_GAP_MILLISECONDS);
            let has_cooled_down =
                self.last_step_at.is_none_or(|at| now.saturating_sub(at) >= WHEEL_COOLDOWN_MILLISECONDS);
            if has_gesture_ended || (!is_smooth && has_cooled_down) {
                self.is_locked = false;
                self.total = 0;
            } else {
                return 0;
            }
        }
        // A change of direction starts over, and so does a pause between gestures.
        if self.total.signum() * amount.signum() < 0 || quiet_for.is_some_and(|gap| gap >= GESTURE_GAP_MILLISECONDS) {
            self.total = 0;
        }
        self.total += amount;
        if self.total.abs() < WHEEL_STEP {
            return 0;
        }
        let step = self.total.signum();
        self.total = 0;
        self.is_locked = true;
        self.last_step_at = Some(now);
        step
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
    fn a_chosen_page_with_nothing_to_show_gives_way_to_the_best_one() {
        assert_eq!(choose(Some(0), &[None, HIGH]), Some(1));
        assert_eq!(choose(Some(7), &[LOW]), Some(0));
    }

    #[test]
    fn stepping_moves_along_the_navigable_pages_and_stops_at_the_ends() {
        let pages = [0, 2, 3];
        assert_eq!(neighbour(&pages, 0, 1), Some(2));
        assert_eq!(neighbour(&pages, 2, 1), Some(3));
        assert_eq!(neighbour(&pages, 3, 1), None);
        assert_eq!(neighbour(&pages, 0, -1), None);
        assert_eq!(neighbour(&pages, 3, -1), Some(2));
    }

    #[test]
    fn stepping_from_a_page_that_is_not_navigable_goes_nowhere() {
        assert_eq!(neighbour(&[0, 2], 1, 1), None);
    }

    #[test]
    fn one_wheel_notch_is_one_step() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(120, 0), 1);
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(-120, 0), -1);
    }

    #[test]
    fn small_amounts_add_up_to_a_step() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(30, 0), 0);
        assert_eq!(wheel.add(30, 16), 0);
        assert_eq!(wheel.add(30, 32), 0);
        assert_eq!(wheel.add(30, 48), 1);
    }

    #[test]
    fn a_coasting_swipe_moves_one_page() {
        let mut wheel = Wheel::default();
        let steps: i32 = (0..40).map(|tick| wheel.add(40, tick * 16)).sum();
        assert_eq!(steps, 1);
    }

    #[test]
    fn a_new_swipe_after_a_pause_moves_again() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(120, 0), 1);
        assert_eq!(wheel.add(60, 20), 0);
        assert_eq!(wheel.add(60, 500), 0);
        assert_eq!(wheel.add(60, 516), 1);
    }

    #[test]
    fn notches_in_quick_succession_wait_for_the_cooldown() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(120, 0), 1);
        assert_eq!(wheel.add(120, 100), 0);
        assert_eq!(wheel.add(120, 300), 1);
    }

    #[test]
    fn reversing_direction_starts_over() {
        let mut wheel = Wheel::default();
        assert_eq!(wheel.add(80, 0), 0);
        assert_eq!(wheel.add(-80, 16), 0);
        assert_eq!(wheel.add(-80, 32), -1);
    }
}
