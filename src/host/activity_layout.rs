//! Newer transients take priority; older ones wait for space.

/// Transients on screen at once; older ones wait until a newer one ends.
pub const MAX_TRANSIENTS: usize = 2;
pub const DIVIDER_WIDTH: f32 = 1.0;
pub const BADGE_EDGE_PADDING: f32 = 10.0;

/// Keeps `order` as the activation order of the activities in `active`: ended ones leave, new ones join at the end.
pub fn track_order<T: Copy + PartialEq>(order: &mut Vec<T>, active: &[T]) {
    order.retain(|item| active.contains(item));
    for &item in active {
        if !order.contains(&item) {
            order.push(item);
        }
    }
}

/// The newest transients that fit on screen, oldest first.
pub fn newest<T>(order: &[T]) -> &[T] {
    &order[order.len().saturating_sub(MAX_TRANSIENTS)..]
}

pub fn total_width(widths: &[f32]) -> f32 {
    widths.iter().sum::<f32>() + DIVIDER_WIDTH * widths.len().saturating_sub(1) as f32
}

pub fn offsets(widths: &[f32]) -> Vec<f32> {
    let mut left = 0.0;
    widths
        .iter()
        .map(|width| {
            let start = left;
            left += width + DIVIDER_WIDTH;
            start
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_activity_joins_the_end_and_an_ended_one_leaves() {
        let mut order = vec![2, 0];
        track_order(&mut order, &[0, 1]);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn the_order_of_activation_is_kept_while_activities_stay() {
        let mut order = vec![3, 1];
        track_order(&mut order, &[1, 3]);
        assert_eq!(order, vec![3, 1]);
    }

    #[test]
    fn only_the_newest_transients_are_shown() {
        assert_eq!(newest(&[4, 5, 6]), &[5, 6]);
        assert_eq!(newest(&[4]), &[4]);
        assert!(newest::<usize>(&[]).is_empty());
    }

    #[test]
    fn dividers_sit_between_transients_only() {
        assert_eq!(total_width(&[100.0]), 100.0);
        assert_eq!(total_width(&[100.0, 50.0]), 100.0 + DIVIDER_WIDTH + 50.0);
        assert_eq!(total_width(&[]), 0.0);
    }

    #[test]
    fn each_transient_starts_after_the_previous_one_and_its_divider() {
        assert_eq!(offsets(&[100.0, 50.0]), vec![0.0, 100.0 + DIVIDER_WIDTH]);
    }
}
