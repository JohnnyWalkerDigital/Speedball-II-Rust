//! The ball in play: its movement and speed, the warp gates, carrying it,
//! and who is nearest to it.

use std::rc::Rc;

use super::{EntityRef, Match};
use crate::game::events::Sound;
use crate::game::geometry::{Point, Side};
use crate::game::player::PlayerId;

impl Match {
    /// Moves everything that moves by itself, keeping it inside the walls.
    pub(super) fn move_all(&mut self) {
        let rom = Rc::clone(&self.rom);
        if self.holder.is_none() && !self.ball_in_multiplier {
            let low = !self.props.ball.is_high();
            let margin = if low { 32 } else { 24 };
            let hits = self.props.ball.move_within_walls(&rom, margin);
            // The ball sounds different high and low.
            if hits.side {
                self.play_sound(if low {
                    Sound::SIDE_WALL_LOW
                } else {
                    Sound::SIDE_WALL_HIGH
                });
            }
            if hits.end {
                self.play_sound(if low {
                    Sound::END_WALL_LOW
                } else {
                    Sound::END_WALL_HIGH
                });
            }
        }
        for player in &mut self.players {
            player.entity.move_within_walls(&rom, 48);
        }
        for medic in &mut self.props.medics {
            medic.move_within_walls(&rom, 48);
        }
    }

    /// Keeps a held ball in the carrier's hands. Returns what the camera
    /// should follow.
    pub(super) fn carry_ball(&mut self) -> EntityRef {
        let Some(holder) = self.holder else {
            return EntityRef::Ball;
        };
        if !self.clock_paused {
            let carrier = self.entity(holder).clone();
            let offset = self.rom.ball_offset(carrier.sprite);
            let mut delta = Point::new(
                carrier.origin.x + offset.x - 8,
                carrier.origin.y + offset.y - 8,
            );
            if self.props.ball.sprite == 0 {
                delta = delta - Point::new(4, 4);
            }
            let ball = &mut self.props.ball;
            ball.velocity = carrier.velocity;
            ball.pos = carrier.pos + delta;
        }
        holder
    }

    /// Slows a free ball down.
    pub(super) fn decelerate_ball(&mut self) {
        if (!self.clock_paused && self.holder.is_some()) || self.ball_in_multiplier {
            return;
        }
        let ball = &mut self.props.ball;
        if ball.velocity.is_zero() {
            ball.set_charged(false);
            return;
        }
        if ball.velocity.y != 0 && (ball.pos.y < 48 || ball.pos.y > 1104) {
            return;
        }
        if ball.timer != 0 {
            if self.ball_speed <= ball.timer {
                ball.timer -= 1;
                return;
            }
            self.ball_speed = self.ball_speed / 2 + self.ball_speed / 4;
        }
        let low_ball = ball.sprite == 0;
        for component in [&mut ball.velocity.x, &mut ball.velocity.y] {
            if *component != 0 {
                let slowed = *component - component.signum();
                if slowed != 0 || low_ball {
                    *component = slowed;
                }
            }
        }
        if ball.timer != 0 {
            ball.timer -= 1;
        }
    }

    pub(crate) fn set_ball_speed_from_throw(&mut self, thr: i32) {
        self.props.ball.timer = thr / 2;
        self.ball_speed = thr / 4 + thr / 8 + thr / 16;
    }

    pub(crate) fn thrower_skill_thr(&self) -> i32 {
        self.players[self.thrower].skills().thr
    }

    /// The tunnels on the side walls teleport the ball across the pitch.
    pub(crate) fn check_warp_gates(&mut self) {
        if self.holder.is_some() || self.props.ball.is_high() {
            return;
        }
        let ball = &mut self.props.ball;
        let in_gate_row = (355..=385).contains(&ball.pos.y) || (767..=797).contains(&ball.pos.y);
        if !in_gate_row {
            return;
        }
        if ball.pos.x < 32 {
            ball.pos.x = 608;
        } else if ball.pos.x > 608 {
            ball.pos.x = 32;
        } else {
            return;
        }
        // Moving straight up or down, the ball keeps its sideways drift.
        let sideways = ball.velocity.x;
        ball.velocity = self.rom.velocity(ball.dir, 8);
        if ball.dir & 3 == 0 {
            ball.velocity.x = sideways;
        }
        self.set_ball_speed_from_throw(self.thrower_skill_thr());
        self.play_sound(Sound::WARP);
    }

    pub(super) fn update_distances_to_ball(&mut self) {
        if self.clock_paused {
            return;
        }
        let center = self.entity(self.holder.unwrap_or(EntityRef::Ball)).pos;
        let scroll = self.camera.position();
        for player in &mut self.players {
            player.distance_to_ball = player.entity.distance_to(center);
            player.entity.update_offscreen(scroll);
        }
    }

    /// Hands control to whoever is nearest the ball, unless a player holds it.
    pub(super) fn update_controlled_players(&mut self) {
        if self.clock_paused {
            return;
        }
        for side in [Side::Team1, Side::Team2] {
            if !self.holds_ball(*self.controlled.get(side)) {
                *self.controlled.get_mut(side) = self.nearest_to_ball(side);
            }
        }
    }

    /// Ties go to the later player, as in the original.
    fn nearest_to_ball(&self, side: Side) -> PlayerId {
        let mut nearest = PlayerId::of(side == Side::Team1, 0);
        let mut best = i32::MAX;
        for id in self.team(side) {
            let distance = self.players[id].distance_to_ball;
            if distance <= best {
                best = distance;
                nearest = id;
            }
        }
        nearest
    }

    pub(super) fn update_distances_between_players(&mut self) {
        if self.clock_paused {
            return;
        }
        for p1 in PlayerId::team(true) {
            if self.players[p1].entity.flags.offscreen() {
                continue;
            }
            let e1 = &self.players[p1].entity;
            let feet = e1.pos + e1.origin;
            for p2 in PlayerId::team(false) {
                let opponent = &self.players[p2];
                if opponent.entity.flags.offscreen() {
                    continue;
                }
                let distance = opponent.entity.distance_to(feet);
                let (i1, i2) = (self.players[p1].index, opponent.index);
                self.players[p1].distance_to_opponents[i2] = distance;
                self.players[p2].distance_to_opponents[i1] = distance;
            }
        }
    }
}
