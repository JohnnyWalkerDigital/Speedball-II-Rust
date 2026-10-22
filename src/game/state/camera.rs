//! The view onto the pitch and how it follows the play.

use super::{EntityRef, Match};
use crate::game::geometry::Point;

/// Scroll position at which the whole kick-off area is in view.
pub const KICK_OFF_SCROLL_Y: i32 = 484;
/// Rows of the screen showing the pitch; the status bar is below.
pub const VIEW_HEIGHT: i32 = 184;
const VIEW_WIDTH: i32 = 320;
const PITCH_WIDTH: i32 = 640;
const PITCH_HEIGHT: i32 = 1152;
/// Where the followed entity sits in the view.
const FOCUS: Point = Point::new(160, 92);

/// The top-left corner of the view on the pitch (`x_origin`, `y_origin`).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    position: Point,
}

impl Default for Camera {
    /// Centred on the pitch.
    fn default() -> Self {
        Self {
            position: Point::new(
                (PITCH_WIDTH - VIEW_WIDTH) / 2,
                (PITCH_HEIGHT - VIEW_HEIGHT) / 2,
            ),
        }
    }
}

impl Camera {
    pub fn position(&self) -> Point {
        self.position
    }

    /// Puts the view somewhere straight away (a replayed frame).
    pub fn jump_to(&mut self, position: Point) {
        self.position = position;
    }

    /// Scrolls towards `target`. During play, horizontal scrolling ignores
    /// movements of a pixel or so; with `exact` it follows every one.
    pub fn follow(&mut self, target: Point, exact: bool) {
        let goal_x = target.x - FOCUS.x;
        if exact || scroll_step(goal_x, self.position.x) >= 2 {
            self.position.x = scroll_towards(self.position.x, goal_x, PITCH_WIDTH - VIEW_WIDTH);
        }
        self.position.y = scroll_towards(
            self.position.y,
            target.y - FOCUS.y,
            PITCH_HEIGHT - VIEW_HEIGHT,
        );
    }
}

/// Camera speed: an eighth of the distance plus one, at most 16 pixels.
fn scroll_step(goal: i32, current: i32) -> i32 {
    (((goal - current).abs() >> 3) + 1).min(16)
}

fn scroll_towards(current: i32, goal: i32, max: i32) -> i32 {
    let step = scroll_step(goal, current);
    match goal.cmp(&current) {
        std::cmp::Ordering::Greater => (current + step).min(goal).min(max),
        std::cmp::Ordering::Less => (current - step).max(goal).max(0),
        std::cmp::Ordering::Equal => current,
    }
}

impl Match {
    /// `update_camera`, and the goal indicators on the view's edge.
    pub(super) fn follow_with_camera(&mut self, target: EntityRef) {
        let target = self.entity(target).pos;
        self.camera.follow(target, self.clock.leg() >= 2);
        let y = self.camera.position().y;
        // The goal indicators sit on whichever edge of the view shows a goal.
        let indicator_y = if y <= KICK_OFF_SCROLL_Y {
            y + 1
        } else {
            y + 183
        };
        for indicator in &mut self.props.goal_indicators {
            indicator.pos.y = indicator_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_closes_an_eighth_of_the_gap_plus_one() {
        assert_eq!(scroll_towards(0, 80, 320), 11);
        assert_eq!(scroll_towards(100, 99, 320), 99);
        // At most 16 pixels, and never past the pitch's edges.
        assert_eq!(scroll_towards(0, 1000, 320), 16);
        assert_eq!(scroll_towards(310, 400, 320), 320);
        assert_eq!(scroll_towards(5, -100, 320), 0);
    }

    #[test]
    fn play_ignores_one_pixel_sideways_moves() {
        let mut camera = Camera::default();
        let start = camera.position();
        let target = start + FOCUS + Point::new(1, 0);
        camera.follow(target, false);
        assert_eq!(camera.position().x, start.x);
        camera.follow(target, true);
        assert_eq!(camera.position().x, start.x + 1);
    }
}
