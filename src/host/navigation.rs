//! Page selection and scroll state, independent of the window.

use std::time::{Duration, Instant};

use crate::{gfx::Spring, page::Priority};

const WHEEL_STEP: i32 = 120;
const SMOOTH_UNITS_PER_PAGE: f32 = 400.0;
pub const PAGE_DRAG_RESPONSE: (f32, f32) = (1600.0, 80.0);
pub const PAGE_SNAP_RESPONSE: (f32, f32) = (650.0, 51.0);
pub const TOUCHPAD_SNAP_RESPONSE: (f32, f32) = (360.0, 38.0);
/// Wheel messages have no touchpad release phase; wait for a quiet interval before gently centering the page.
const SWIPE_QUIET_TIME: Duration = Duration::from_millis(200);

pub fn page_transition(position: f32) -> Spring {
    Spring::new(position, PAGE_SNAP_RESPONSE.0, PAGE_SNAP_RESPONSE.1)
}

/// The continuous position of one touchpad gesture; only its release destination is rounded.
pub struct Swipe {
    position: f32,
    last_input: Instant,
}

impl Swipe {
    pub fn new(position: f32) -> Self {
        Self { position, last_input: Instant::now() }
    }

    pub fn drag_by(&mut self, pages: f32) -> f32 {
        self.position += pages;
        self.last_input = Instant::now();
        self.position
    }

    pub fn destination(&self) -> f32 {
        self.position.round()
    }

    pub fn quiet_remaining(&self) -> Duration {
        self.quiet_remaining_at(Instant::now())
    }

    fn quiet_remaining_at(&self, now: Instant) -> Duration {
        SWIPE_QUIET_TIME.saturating_sub(now.saturating_duration_since(self.last_input))
    }
}

pub struct PageLayout {
    pub viewport_width: f32,
    pub center_offsets: [f32; 2],
}

pub fn page_layout(first_width: f32, next_width: f32, progress: f32, gap: f32) -> PageLayout {
    let center_distance = (first_width + next_width) / 2.0 + gap;
    PageLayout {
        viewport_width: first_width + (next_width - first_width) * progress,
        center_offsets: [-progress * center_distance, (1.0 - progress) * center_distance],
    }
}

pub struct PageList {
    order: Vec<usize>,
    enabled: Vec<bool>,
}

impl PageList {
    pub fn new(count: usize) -> Self {
        Self { order: (0..count).collect(), enabled: vec![true; count] }
    }

    /// Ignore unknown IDs, append missing pages, and apply opt-in defaults only without saved disables.
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

    pub fn move_to(&mut self, page: usize, position: usize) {
        let Some(from) = self.order.iter().position(|&index| index == page) else { return };
        self.order.remove(from);
        self.order.insert(position.min(self.order.len()), page);
    }

    pub fn order_text(&self, ids: &[&str]) -> String {
        self.order.iter().filter_map(|&index| ids.get(index).copied()).collect::<Vec<_>>().join(",")
    }

    pub fn disabled_text(&self, ids: &[&str]) -> String {
        let off = |index: &usize| !self.is_enabled(*index);
        (0..ids.len()).filter(off).map(|index| ids[index]).collect::<Vec<_>>().join(",")
    }

    pub fn navigable(&self, priorities: &[Option<Priority>]) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|&index| self.is_enabled(index) && priorities.get(index).is_some_and(Option::is_some))
            .collect()
    }

    pub fn effective(&self, priorities: &[Option<Priority>]) -> Vec<Option<Priority>> {
        priorities
            .iter()
            .enumerate()
            .map(|(index, priority)| if self.is_enabled(index) { *priority } else { None })
            .collect()
    }
}

/// The `id:Label[:opt_in]` list the settings window reads as `pages.known`; it lists pages without linking to the notch.
pub fn known_pages_text(ids: &[&str], labels: &[&str], opt_in: &[bool]) -> String {
    debug_assert!(ids.len() == labels.len() && ids.len() == opt_in.len());
    ids.iter()
        .zip(labels)
        .zip(opt_in)
        .map(|((id, label), is_opt_in)| format!("{id}:{label}{}", if *is_opt_in { ":opt_in" } else { "" }))
        .collect::<Vec<_>>()
        .join(",")
}

/// Urgent pages override selection; otherwise retain an available selection before falling back to priority.
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

#[derive(Debug, PartialEq)]
pub enum Scroll {
    Pages(i32),
    Drag(f32),
}

pub const MAX_WHEEL_TRAVEL: f32 = 2.0;

pub fn limit_wheel_travel(origin: f32, target: f32) -> f32 {
    target.clamp(origin - MAX_WHEEL_TRAVEL, origin + MAX_WHEEL_TRAVEL)
}

pub fn classify_scroll(amount: i32, follows_fingers: bool) -> Scroll {
    if !follows_fingers && amount.abs() >= WHEEL_STEP {
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
    fn known_pages_text_joins_ids_and_labels() {
        let text = known_pages_text(&["clock", "stats"], &["Clock", "Stats"], &[false, true]);
        assert_eq!(text, "clock:Clock,stats:Stats:opt_in");
        assert_eq!(known_pages_text(&[], &[], &[]), "");
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
        assert_eq!(classify_scroll(120, false), Scroll::Pages(1));
        assert_eq!(classify_scroll(-120, false), Scroll::Pages(-1));
        assert_eq!(classify_scroll(360, false), Scroll::Pages(3));
    }

    #[test]
    fn a_fast_wheel_cannot_queue_more_than_two_pages_ahead() {
        assert_eq!(limit_wheel_travel(3.0, 4.0), 4.0);
        assert_eq!(limit_wheel_travel(3.0, 9.0), 5.0);
        assert_eq!(limit_wheel_travel(3.0, -9.0), 1.0);
        assert_eq!(limit_wheel_travel(3.0, 4.6), 4.6);
    }

    #[test]
    fn a_touchpad_scroll_is_a_fraction_of_a_page() {
        assert_eq!(classify_scroll(30, false), Scroll::Drag(0.075));
        assert_eq!(classify_scroll(-60, false), Scroll::Drag(-0.15));
        assert_eq!(classify_scroll(120, true), Scroll::Drag(0.3));
        assert_eq!(classify_scroll(-240, true), Scroll::Drag(-0.6));
    }

    #[test]
    fn pauses_between_touchpad_updates_keep_the_gesture_open() {
        let swipe = Swipe::new(0.0);
        let last_input = swipe.last_input;
        assert_eq!(swipe.quiet_remaining_at(last_input + Duration::from_millis(90)), Duration::from_millis(110));
        assert_eq!(swipe.quiet_remaining_at(last_input + Duration::from_millis(150)), Duration::from_millis(50));
        assert_eq!(swipe.quiet_remaining_at(last_input + Duration::from_millis(200)), Duration::ZERO);
    }

    #[test]
    fn touchpad_position_is_continuous_across_multiple_pages() {
        let mut swipe = Swipe::new(3.0);
        assert_eq!(swipe.drag_by(2.25), 5.25);
        assert_eq!(swipe.destination(), 5.0);
        assert_eq!(swipe.drag_by(-4.5), 0.75);
    }

    #[test]
    fn a_swipe_only_rounds_the_release_destination_not_the_finger_position() {
        let mut swipe = Swipe::new(2.0);
        swipe.drag_by(0.3);
        assert!((swipe.position - 2.3).abs() < 0.001);
        assert_eq!(swipe.destination(), 2.0);
        swipe.drag_by(0.25);
        assert!((swipe.position - 2.55).abs() < 0.001);
        assert_eq!(swipe.destination(), 3.0);
        swipe.drag_by(-1.1);
        assert!((swipe.position - 1.45).abs() < 0.001);
        assert_eq!(swipe.destination(), 1.0);
    }

    #[test]
    fn interrupting_a_transition_starts_at_the_visible_position() {
        let mut swipe = Swipe::new(1.25);
        assert_eq!(swipe.drag_by(0.125), 1.375);
        assert_eq!(swipe.position, 1.375);
        assert_eq!(swipe.destination(), 1.0);
    }

    #[test]
    fn touchpad_release_is_gentler_than_wheel_navigation_and_still_settles_without_bounce() {
        let mut touchpad = Spring::new(0.65, TOUCHPAD_SNAP_RESPONSE.0, TOUCHPAD_SNAP_RESPONSE.1);
        let mut wheel = page_transition(0.65);
        touchpad.target = 1.0;
        wheel.target = 1.0;
        let mut previous = touchpad.position;
        for frame in 0..120 {
            touchpad.step(1.0 / 120.0);
            wheel.step(1.0 / 120.0);
            assert!(touchpad.position >= previous && touchpad.position <= 1.0);
            previous = touchpad.position;
            if frame == 11 {
                assert!(touchpad.position < wheel.position, "touchpad release must pull less strongly");
            }
        }
        assert!(touchpad.is_settled());
    }

    #[test]
    fn unequal_page_widths_share_one_gap_and_resize_the_viewport_progressively() {
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let layout = page_layout(600.0, 300.0, progress, 24.0);
            assert_eq!(layout.viewport_width, 600.0 - 300.0 * progress);
            let first_right = layout.center_offsets[0] + 300.0;
            let next_left = layout.center_offsets[1] - 150.0;
            assert_eq!(next_left - first_right, 24.0);
            let reverse = page_layout(300.0, 600.0, 1.0 - progress, 24.0);
            assert_eq!(layout.viewport_width, reverse.viewport_width);
        }
        let quarter = page_layout(600.0, 300.0, 0.25, 24.0);
        let next_left = quarter.center_offsets[1] - 150.0;
        assert!(next_left < quarter.viewport_width / 2.0, "the shorter next page is already visible at 25 percent");
    }

    #[test]
    fn page_transitions_arrive_quickly_without_overshooting_and_then_settle() {
        for refresh_rate in [60, 120] {
            let mut transition = page_transition(0.0);
            transition.target = 1.0;
            let mut previous = 0.0;
            for frame in 0..refresh_rate / 2 {
                transition.step(1.0 / refresh_rate as f32);
                assert!(transition.position >= previous && transition.position <= 1.0);
                previous = transition.position;
                if frame == refresh_rate / 4 {
                    assert!(transition.position > 0.98, "still at {} after 250 ms", transition.position);
                }
            }
            assert!(transition.is_settled());
        }
    }
}
