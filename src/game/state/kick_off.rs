//! Kick-off: lining up, the ball launcher, swapping ends and the final
//! line-up.

use super::{EntityRef, Match, KICK_OFF_SCROLL_Y, PITCH_CENTER};
use crate::data::animations::Script;
use crate::game::events::Sound;
use crate::game::geometry::{PerTeam, Point, Side};
use crate::game::player::PlayerId;
use crate::game::text::TextId;

impl Match {
    /// While the clock is stopped, waits for everyone to reach their
    /// kick-off spot, then starts the ball launcher.
    pub(super) fn prepare_kick_off(&mut self) {
        if self.launcher_started {
            return;
        }
        if let Some(holder) = self.holder {
            if self.entity(holder).flags.busy() {
                return;
            }
            if self.text_panel.message().is_none() {
                self.show_message(Some(TextId::Vs));
                self.play_sound(Sound::GET_READY);
            }
        }
        self.holder = Some(EntityRef::TerrainCenter);
        self.props.terrain_center.pos = PITCH_CENTER;
        self.props.terrain_center.flags.set_busy(false);
        if self.camera.position().y != KICK_OFF_SCROLL_Y {
            return;
        }
        self.props.ball.flags.set_invisible(true);
        self.expire_token();
        let everyone_in_place = self.players.iter().all(|player| {
            player.entity.pos == player.launch && player.entity.dir == player.attributes.initial_dir
        });
        if !everyone_in_place {
            return;
        }
        self.props.launcher.script_index = 0;
        self.props.launcher.flags.set_invisible(false);
        self.holder = None;
        self.launcher_started = true;
        let ball = &mut self.props.ball;
        ball.flags.set_invisible(true);
        ball.pos = PITCH_CENTER;
        ball.velocity = Point::ZERO;
        self.ball_in_multiplier = false;
        self.play_sound(Sound::LAUNCHER_START);
    }

    /// Steps the launcher animation and fires the ball.
    pub(super) fn run_ball_launcher(&mut self) {
        if !self.launcher_started || self.props.launcher.script_index < 19 {
            return;
        }
        if self.props.launcher.script_index == 19 {
            let ball = &mut self.props.ball;
            ball.flags.set_invisible(false);
            ball.script = self.rom.script(Script::BallLaunch);
            ball.script_index = 0;
            self.play_sound(Sound::BALL_LAUNCH);
            return;
        }
        match self.props.ball.script_index {
            11 => self.props.big_ball.flags.set_invisible(false),
            21 => {
                if !self.clock.is_full_time() {
                    self.clock_paused = false;
                }
                self.launcher_started = false;
                self.inputs = PerTeam::default();
                // The ball is in play; the launcher drops out of sight.
                self.props.launcher.flags.set_invisible(true);
            }
            _ => {}
        }
    }

    /// Whether both teams stand in their presentation line-up, facing
    /// forward: the end of the match.
    pub(super) fn teams_in_final_line_up(&self) -> bool {
        let team1_attacks_up = self.players[0].attacks_up();
        let in_line = |side: Side, base: usize| {
            self.team(side).all(|id| {
                let player = &self.players[id.0];
                player.entity.pos == self.presentation[base + player.index]
                    && player.entity.dir == player.attributes.initial_dir
            })
        };
        let (base1, base2) = if team1_attacks_up { (0, 9) } else { (9, 0) };
        in_line(Side::Team1, base1) && (self.is_practice() || in_line(Side::Team2, base2))
    }

    /// Swaps ends at half time.
    pub(super) fn switch_sides(&mut self) {
        let (team1, team2) = self.players.split_at_mut(PlayerId::TEAM_SIZE);
        for (p1, p2) in team1.iter_mut().zip(team2.iter_mut()) {
            std::mem::swap(&mut p1.zone_min, &mut p2.zone_min);
            std::mem::swap(&mut p1.zone_max, &mut p2.zone_max);
            std::mem::swap(&mut p1.launch, &mut p2.launch);
            std::mem::swap(
                &mut p1.attributes.initial_dir,
                &mut p2.attributes.initial_dir,
            );
            // The original indexes the team tables directly, whatever table
            // the person record actually points at.
            let a1 = &mut self.roster.attributes_team1[p1.person_attributes.index];
            let a2 = &mut self.roster.attributes_team2[p2.person_attributes.index];
            std::mem::swap(&mut a1.initial_dir, &mut a2.initial_dir);
            p1.update_zone_center();
            p2.update_zone_center();
        }
        self.reverse_side = !self.reverse_side;
        self.arena.stars.switch_sides();
    }
}
