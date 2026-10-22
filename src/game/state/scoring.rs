//! Points and goals (`handle_score_goal` and the score multipliers).

use super::{Match, PITCH_CENTER};
use crate::game::events::Sound;
use crate::game::geometry::{PerTeam, Point, Side};
use crate::game::player::{count, PlayerId};
use crate::game::text::TextId;
use crate::game::token::ActiveToken;

/// Each team's points, the share of them won with the ball in play, and
/// what the status bar shows.
#[derive(Clone, Debug, Default)]
pub struct Scoreboard {
    points: PerTeam<i32>,
    /// Points scored while the ball was in play (the statistics screen's
    /// "bonus points").
    bonus: PerTeam<i32>,
    /// The status bar is only redrawn after the goal replay, so it shows
    /// the score from before the goal while the replay runs.
    shown_before_goal: Option<PerTeam<i32>>,
}

impl Scoreboard {
    pub fn points(&self) -> PerTeam<i32> {
        self.points
    }

    pub fn bonus(&self) -> PerTeam<i32> {
        self.bonus
    }

    /// The score the status bar shows.
    pub fn displayed(&self) -> PerTeam<i32> {
        self.shown_before_goal.unwrap_or(self.points)
    }

    fn add(&mut self, side: Side, value: i32, ball_in_play: bool) {
        *self.points.get_mut(side) += value;
        if ball_in_play {
            *self.bonus.get_mut(side) += value;
        }
    }

    /// Points taken off (an opponent's lit wall star).
    pub fn deduct(&mut self, side: Side, value: i32) {
        *self.points.get_mut(side) -= value;
    }

    /// Keeps the status bar on the current score until
    /// [`Scoreboard::release_display`].
    fn hold_display(&mut self) {
        self.shown_before_goal = Some(self.points);
    }

    fn release_display(&mut self) {
        self.shown_before_goal = None;
    }
}

impl Match {
    /// `handle_score_goal`. Returns `true` if the goal replay started; the
    /// rest of the goal is handled by [`Match::finish_goal`] once it ends.
    pub(super) fn check_goal(&mut self) -> bool {
        if self.holder.is_some() || self.clock_paused || self.props.ball.sprite > 2 {
            return false;
        }
        let ball = &self.props.ball;
        if !(272..=368).contains(&ball.pos.x) {
            return false;
        }
        // `true`: the goal at the top of the pitch, scored by team 1.
        let mut top_goal = if ball.pos.y < 32 && ball.velocity.y < 0 {
            true
        } else if ball.pos.y > 1120 && ball.velocity.y > 0 {
            false
        } else {
            return false;
        };
        if self.reverse_side {
            top_goal = !top_goal;
        }
        if !self.goal_door_open(top_goal) {
            return false;
        }

        // An own goal is credited to the other team's active player.
        let thrower_team1 = self.players[self.thrower.0].entity.is_team1();
        let message = if thrower_team1 && !top_goal {
            self.thrower = self.controlled.team2;
            TextId::OwnGoal
        } else if !thrower_team1 && top_goal {
            self.thrower = self.controlled.team1;
            TextId::OwnGoal
        } else if thrower_team1 {
            TextId::Team1Scored
        } else {
            TextId::Team2Scored
        };
        self.show_message(Some(message));

        self.props.ball.flags.set_invisible(true);
        self.score.hold_display();
        self.add_score(Side::from_is_team1(top_goal), 10);
        self.props.ball.pos = PITCH_CENTER;
        self.props.ball.velocity = Point::ZERO;
        self.play_sound(Sound::SILENCE);
        self.play_sound(Sound::GOAL);
        let crowd = self.goal_crowd_sound();
        self.play_sound(crowd);
        if self.start_replay() && self.step_replay() {
            return true;
        }
        self.finish_goal();
        false
    }

    /// The end of `handle_score_goal`, after the replay.
    pub(super) fn finish_goal(&mut self) {
        let crowd = self.goal_crowd_sound();
        self.play_sound(crowd);
        self.clock_paused = true;
        self.show_message(None);
        count(&mut self.players[self.thrower.0].stats.goals_scored);
        self.start_goal_celebration(self.thrower);
        self.score.release_display();
    }

    /// `goal_sound_function`: the crowd sound depends on whether the scorer's
    /// team started the match attacking up.
    fn goal_crowd_sound(&self) -> Sound {
        if self.started_attacking_up(self.thrower) {
            Sound::GOAL_CROWD_UP
        } else {
            Sound::CROWD_CHEER
        }
    }

    /// Whether `p`'s team attacked up at kick-off (`initial_playing_direction`
    /// is team 1's). Several crowd sounds depend on it.
    pub(crate) fn started_attacking_up(&self, p: PlayerId) -> bool {
        self.players[p.0].entity.is_team1() == (self.initial_dir_team1 == 0)
    }

    /// `play_sound_downwards`: despite the name, the crowd cheers when the
    /// last thrower's team started attacking up. (It checks the thrower even
    /// for a star row completed by the other team.)
    pub(crate) fn cheer_if_thrower_attacked_up(&mut self) {
        if self.started_attacking_up(self.thrower) {
            self.play_sound(Sound::CROWD_CHEER);
        }
    }

    /// The "goal door" token shuts the holder's own goal.
    fn goal_door_open(&self, top_goal: bool) -> bool {
        if self.token.sprite != ActiveToken::GOAL_DOOR {
            return true;
        }
        let holder = self.token.holder.expect("an active token has a holder");
        if self.players[holder.0].attacks_up() {
            top_goal != self.reverse_side
        } else {
            top_goal == self.reverse_side
        }
    }

    /// Adds points, doubled or halved again by the team's score multiplier.
    pub(crate) fn add_score(&mut self, side: Side, value: i32) {
        let ball_in_play = !self.props.ball.flags.invisible();
        self.score.add(side, value, ball_in_play);
        let bonus = match self.arena.multipliers.level(side) {
            0 => return,
            1 => value / 2,
            _ => value,
        };
        self.score.add(side, bonus, ball_in_play);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_bar_holds_the_old_score_through_the_replay() {
        let mut score = Scoreboard::default();
        score.hold_display();
        score.add(Side::Team2, 10, false);
        assert_eq!(score.displayed(), PerTeam::new(0, 0));
        score.release_display();
        assert_eq!(score.displayed(), PerTeam::new(0, 10));
        assert_eq!(
            score.bonus(),
            PerTeam::new(0, 0),
            "the ball was not in play"
        );
    }
}
