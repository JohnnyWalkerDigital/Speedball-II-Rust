//! The match clock: legs, the seconds left in each, and the blinking of
//! the indicators and medics.

use super::Match;
use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::game::player::{count, PlayerId};
use crate::game::text::TextId;

const TICKS_PER_SECOND: i32 = 50;
const TICKS_PER_FRAME: i32 = 2;
/// The leg after which the match is over.
const FULL_TIME: i32 = 2;

/// Which leg is being played and how long it has left.
#[derive(Clone, Debug)]
pub struct Clock {
    /// 0 and 1 are the two halves; 2 is full time. (Practice starts at 3,
    /// so it never ends; the demo starts at 1, so it is one half.)
    leg: i32,
    leg_duration: i32,
    /// Seconds left in the leg.
    remaining: i32,
    ticks_left_in_second: i32,
    blink: Blink,
}

impl Clock {
    pub fn new(first_leg: i32, leg_duration: i32) -> Self {
        Self {
            leg: first_leg,
            leg_duration,
            remaining: leg_duration,
            ticks_left_in_second: TICKS_PER_SECOND,
            blink: Blink::default(),
        }
    }

    pub fn leg(&self) -> i32 {
        self.leg
    }

    pub fn is_full_time(&self) -> bool {
        self.leg == FULL_TIME
    }

    /// Seconds left in the leg, as the status bar shows them.
    pub fn remaining(&self) -> i32 {
        self.remaining
    }

    /// Advances a frame's worth of ticks. Returns whether a second passed.
    fn tick_frame(&mut self) -> bool {
        self.ticks_left_in_second -= TICKS_PER_FRAME;
        if self.ticks_left_in_second > 0 {
            return false;
        }
        self.ticks_left_in_second = TICKS_PER_SECOND;
        true
    }

    fn count_down(&mut self) {
        self.remaining -= 1;
    }

    fn leg_over(&self) -> bool {
        self.remaining == 0
    }

    /// Starts the next leg. Returns whether that was the last.
    fn start_next_leg(&mut self) -> bool {
        self.leg += 1;
        self.remaining = self.leg_duration;
        self.is_full_time()
    }
}

/// Alternating long/short blink used by indicators and medics.
#[derive(Clone, Copy, Debug)]
struct Blink {
    current: i32,
    next: i32,
    timer: i32,
}

impl Default for Blink {
    fn default() -> Self {
        Self {
            current: 6,
            next: 2,
            timer: 6,
        }
    }
}

impl Blink {
    /// Advances a frame. Returns whether the blink toggled.
    fn tick(&mut self) -> bool {
        self.timer -= 1;
        if self.timer >= 0 {
            return false;
        }
        self.timer = self.next;
        std::mem::swap(&mut self.current, &mut self.next);
        true
    }
}

impl Match {
    pub(super) fn tick_clock(&mut self) {
        if self.clock.tick_frame() {
            self.on_second_elapsed();
        }
        if self.clock.blink.tick() {
            for medic in &mut self.props.medics {
                medic.toggle_palette();
            }
            for indicator in self
                .props
                .player_indicators
                .iter_mut()
                .chain(self.props.goal_indicators.iter_mut())
            {
                let hidden = indicator.flags.invisible();
                indicator.flags.set_invisible(!hidden);
            }
        }
    }

    fn on_second_elapsed(&mut self) {
        self.arena_check_full_star_rows();
        if !self.clock_paused {
            self.clock.count_down();
            self.count_territory_and_possession();
        }
        if let Some(held) = &mut self.token {
            held.seconds_left -= 1;
            let expired = held.seconds_left == 0;
            // The clock ticks audibly while a power-up is active.
            self.play_sound(Sound::POWERUP_TICK);
            if expired {
                self.expire_token();
            }
        }
        if !self.clock_paused && self.clock.leg_over() {
            self.end_leg();
        }
    }

    /// Half time or full time.
    fn end_leg(&mut self) {
        self.switch_sides();
        self.clock_paused = true;
        let (id, score1_pos, score2_pos) = if !self.clock.start_next_leg() {
            self.play_sound(Sound::GET_READY);
            (TextId::HalfTime, 14, 21)
        } else {
            self.play_sound(Sound::MATCH_OVER);
            (TextId::MatchOver, 23, 30)
        };
        let scores = self.score.points();
        let text = self.text_panel.text_mut(id);
        text.write_number(score1_pos, scores.team1, 3);
        text.write_number(score2_pos, scores.team2, 3);
        self.show_message(Some(id));
    }

    fn count_territory_and_possession(&mut self) {
        let in_top_half = self.props.ball.pos.y <= 576;
        let team1_attacks_up = self.players[PlayerId(0)].attacks_up();
        let side = if in_top_half == team1_attacks_up {
            Side::Team1
        } else {
            Side::Team2
        };
        *self.time_in_opponents_half.get_mut(side) += 1;
        if let Some(p) = self.holder_player() {
            count(&mut self.players[p].stats.time_in_possession);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_is_twenty_five_frames() {
        let mut clock = Clock::new(0, 90);
        let frames = (1..=60).find(|_| clock.tick_frame());
        assert_eq!(frames, Some(25));
    }

    #[test]
    fn the_second_leg_ends_the_match() {
        let mut clock = Clock::new(0, 2);
        clock.count_down();
        clock.count_down();
        assert!(clock.leg_over());
        assert!(!clock.start_next_leg(), "half time");
        assert_eq!(clock.remaining(), 2);
        assert!(clock.start_next_leg(), "full time");
    }

    #[test]
    fn the_blink_alternates_long_and_short() {
        let mut blink = Blink::default();
        let toggles: Vec<usize> = (1..=20).filter(|_| blink.tick()).collect();
        assert_eq!(toggles, [7, 10, 17, 20]);
    }
}
