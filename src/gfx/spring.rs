/// Damped spring that eases a value toward its target and slightly overshoots.
pub struct Spring {
    pub position: f32,
    pub target: f32,
    velocity: f32,
    stiffness: f32,
    damping: f32,
}

const SUBSTEP_SECONDS: f32 = 1.0 / 240.0;
const SETTLE_DISTANCE: f32 = 0.0005;
const SETTLE_VELOCITY: f32 = 0.005;

impl Spring {
    pub fn new(position: f32, stiffness: f32, damping: f32) -> Self {
        Self { position, target: position, velocity: 0.0, stiffness, damping }
    }

    pub fn step(&mut self, elapsed_seconds: f32) {
        let mut remaining = elapsed_seconds;
        while remaining > 0.0 {
            let substep = remaining.min(SUBSTEP_SECONDS);
            let acceleration = self.stiffness * (self.target - self.position) - self.damping * self.velocity;
            self.velocity += acceleration * substep;
            self.position += self.velocity * substep;
            remaining -= substep;
        }
    }

    pub fn is_settled(&self) -> bool {
        (self.target - self.position).abs() < SETTLE_DISTANCE && self.velocity.abs() < SETTLE_VELOCITY
    }

    /// Places the spring at `value` with no motion, so the next target change starts a fresh transition.
    pub fn jump_to(&mut self, value: f32) {
        self.position = value;
        self.target = value;
        self.velocity = 0.0;
    }

    pub fn snap_to_target(&mut self) {
        self.position = self.target;
        self.velocity = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(spring: &mut Spring, seconds: f32) {
        let frame = 1.0 / 60.0;
        for _ in 0..(seconds / frame) as usize {
            spring.step(frame);
        }
    }

    #[test]
    fn a_new_spring_is_settled_at_its_start() {
        assert!(Spring::new(3.0, 200.0, 26.0).is_settled());
    }

    #[test]
    fn settles_on_its_target() {
        let mut spring = Spring::new(0.0, 220.0, 26.0);
        spring.target = 1.0;
        assert!(!spring.is_settled());
        run(&mut spring, 3.0);
        assert!(spring.is_settled());
        assert!((spring.position - 1.0).abs() < 0.001);
    }

    #[test]
    fn a_lightly_damped_spring_overshoots() {
        let mut spring = Spring::new(0.0, 260.0, 22.0);
        spring.target = 1.0;
        let mut highest = 0.0_f32;
        for _ in 0..120 {
            spring.step(1.0 / 60.0);
            highest = highest.max(spring.position);
        }
        assert!(highest > 1.0, "expected overshoot, peaked at {highest}");
    }

    #[test]
    fn jump_to_moves_without_motion() {
        let mut spring = Spring::new(0.0, 220.0, 26.0);
        spring.target = 5.0;
        spring.step(0.05);
        spring.jump_to(2.0);
        assert!(spring.is_settled());
        assert_eq!((spring.position, spring.target), (2.0, 2.0));
    }

    #[test]
    fn snap_to_target_finishes_the_move() {
        let mut spring = Spring::new(0.0, 220.0, 26.0);
        spring.target = 1.0;
        spring.step(0.01);
        spring.snap_to_target();
        assert!(spring.is_settled());
    }
}
