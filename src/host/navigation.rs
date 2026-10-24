//! Moving between pages: which page opens, what the scroll wheel does and where the page dots are. Pure functions
//! and small state machines, so the rules can be tested without a window.

use crate::{
    gfx::{Point, Rect},
    page::Priority,
};

/// Height of the strip at the bottom of the open notch that holds the page dots, in device-independent pixels.
pub const STRIP_HEIGHT: f32 = 16.0;
const DOT_RADIUS: f32 = 2.5;
const ACTIVE_DOT_RADIUS: f32 = 3.2;
const DOT_SPACING: f32 = 14.0;
/// How far from a dot a click still counts as on it.
const DOT_HIT_RADIUS: f32 = 9.0;
/// Distance from the bottom edge to the middle of the dots.
const DOT_CENTER_FROM_BOTTOM: f32 = 9.0;

/// One mouse wheel notch. Finer wheels and touchpads send smaller amounts that add up to this.
const WHEEL_STEP: i32 = 120;
/// After a step by a notched wheel, further notches wait this long.
const WHEEL_COOLDOWN_MILLISECONDS: u64 = 250;
/// After a step by a touchpad or smooth wheel, input is ignored until it has been quiet this long, so one swipe and
/// its coasting move one page.
const GESTURE_GAP_MILLISECONDS: u64 = 200;

/// The pages that can be opened right now, in registration order.
pub fn navigable(priorities: &[Option<Priority>]) -> Vec<usize> {
    priorities.iter().enumerate().filter(|(_, priority)| priority.is_some()).map(|(index, _)| index).collect()
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

/// Centers of the page dots, in device pixels, along the bottom of the open notch.
pub fn dot_centers(count: usize, bounds: Rect, scale: f32) -> Vec<Point> {
    let spacing = DOT_SPACING * scale;
    let first = bounds.center_x() - spacing * (count.saturating_sub(1)) as f32 / 2.0;
    let y = bounds.bottom - DOT_CENTER_FROM_BOTTOM * scale;
    (0..count).map(|position| Point::new(first + spacing * position as f32, y)).collect()
}

/// Radius of a dot, in device pixels.
pub fn dot_radius(is_active: bool, scale: f32) -> f32 {
    (if is_active { ACTIVE_DOT_RADIUS } else { DOT_RADIUS }) * scale
}

/// The dot under `point`, if any.
pub fn dot_at(centers: &[Point], point: Point, scale: f32) -> Option<usize> {
    let reach = DOT_HIT_RADIUS * scale;
    centers.iter().position(|center| (center.x - point.x).abs() <= reach && (center.y - point.y).abs() <= reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOW: Option<Priority> = Some(Priority(1));
    const HIGH: Option<Priority> = Some(Priority(5));

    #[test]
    fn only_pages_with_something_to_show_can_be_opened() {
        assert_eq!(navigable(&[LOW, None, HIGH]), vec![0, 2]);
        assert!(navigable(&[None, None]).is_empty());
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

    #[test]
    fn dots_are_centered_and_evenly_spaced() {
        let bounds = Rect::new(0.0, 0.0, 200.0, 100.0);
        let centers = dot_centers(3, bounds, 1.0);
        assert_eq!(centers.len(), 3);
        assert_eq!(centers[1].x, 100.0);
        assert_eq!(centers[1].x - centers[0].x, centers[2].x - centers[1].x);
        assert!(centers.iter().all(|center| center.y < 100.0 && center.y > 80.0));
    }

    #[test]
    fn a_click_near_a_dot_picks_it_and_a_click_elsewhere_picks_none() {
        let centers = dot_centers(3, Rect::new(0.0, 0.0, 200.0, 100.0), 1.0);
        assert_eq!(dot_at(&centers, Point::new(centers[2].x + 3.0, centers[2].y - 2.0), 1.0), Some(2));
        assert_eq!(dot_at(&centers, Point::new(10.0, 10.0), 1.0), None);
    }

    #[test]
    fn the_active_dot_is_larger() {
        assert!(dot_radius(true, 1.0) > dot_radius(false, 1.0));
    }
}
