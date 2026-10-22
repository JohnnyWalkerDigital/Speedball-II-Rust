//! What players physically do: run, tackle, jump, catch and throw.
//!
//! Both the keyboard and the AI (`ai.rs`) drive players through these.
//! Each function notes the original routine it ports.

use super::entity::{is_in_view, Think};
use super::events::Sound;
use super::geometry::{Input, Point, Side};
use super::player::PlayerId;
use super::state::{EntityRef, Match};
use crate::data::addr;

impl Match {
    /// One frame of a player (`Player.Think`).
    pub(crate) fn player_think(&mut self, p: PlayerId) {
        self.update_control(p);
        let entity = &mut self.players[p.0].entity;
        entity.timer = (entity.timer - 1).max(0);
        match entity.think {
            Think::StartMove => self.start_move(p),
            Think::Move => self.keep_moving(p),
            Think::StandingTackle => self.check_tackle_hits(p),
            Think::Throw => self.release_throw(p),
            Think::Stand => self.stand(p),
            Think::Jump => self.land_jump(p),
            Think::TackleRecover => self.recover_from_tackle(p),
            Think::GoalieDive | Think::SlideTackle => {
                self.recover_from_tackle(p);
                self.check_tackle_hits(p);
            }
            Think::Idle | Think::Coins | Think::Token | Think::Equipment | Think::Unknown(_) => {}
        }
        self.push_away_from_opponents(p);
        self.execute_script(EntityRef::Player(p));
        let player = &mut self.players[p.0];
        player.entity.origin = self.rom.sprite_origin(player.entity.sprite);
        self.apply_movement_constraints(p);
    }

    /// Decides who steers this player this frame (`sub_D2DE`).
    fn update_control(&mut self, p: PlayerId) {
        if self.clock_paused {
            self.walk_to_formation(p);
        } else if p == self.controlled.team1 {
            self.control_active_player(p, Side::Team1);
        } else if p == self.controlled.team2 {
            self.control_active_player(p, Side::Team2);
        } else if self.players[p.0].entity.timer == 0 {
            self.ai_support_play(p);
        }
    }

    /// Between goals: head for the kick-off spot (or the final line-up).
    fn walk_to_formation(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        if player.entity.flags.busy() || player.entity.timer != 0 {
            return;
        }
        let target = if self.leg == 2 {
            let base = if player.attacks_up() { 0 } else { 9 };
            self.presentation_position(base + player.index)
        } else {
            player.launch
        };
        let scroll = self.scroll;
        let rom = self.rom.clone();
        let player = &mut self.players[p.0];
        player.reset_think_delay(&rom);
        player.target = target;
        if player.entity.flags.invisible() {
            player.entity.pos = target;
        } else {
            player.entity.update_offscreen(scroll);
            // Players far off-screen simply teleport.
            if player.entity.flags.offscreen() && !is_in_view(target, scroll) {
                player.entity.pos = target;
            }
        }
        self.start_move(p);
    }

    fn control_active_player(&mut self, p: PlayerId, side: Side) {
        self.try_catch_ball(p);
        let on_screen = self.players[p.0].entity.is_well_inside_view(self.scroll);
        if !*self.computer.get(side) && on_screen {
            let input = *self.inputs.get(side);
            self.apply_human_input(p, input);
        } else if self.players[p.0].entity.timer == 0 {
            self.ai_active_player(p);
        }
    }

    // -- Collisions ---------------------------------------------------------------

    /// Undoes movement into nearby opponents (`sub_D448`).
    fn push_away_from_opponents(&mut self, p: PlayerId) {
        if self.clock_paused || self.players[p.0].entity.flags.offscreen() {
            return;
        }
        for q in self.opponents(p) {
            let opponent = &self.players[q.0];
            let close = self.players[p.0].distance_to_opponents[opponent.index] <= 30;
            if opponent.entity.flags.falling() || !close || opponent.entity.flags.offscreen() {
                continue;
            }
            let towards = self.players[p.0]
                .entity
                .dir_bits_towards(opponent.entity.pos);
            let entity = &mut self.players[p.0].entity;
            if entity.flags.falling() && entity.script_index >= 18 {
                entity.velocity = Point::ZERO;
                entity.script_index = 18;
            }
            let v = entity.velocity;
            if (v.y > 0 && towards.down()) || (v.y < 0 && towards.up()) {
                entity.pos.y -= v.y;
            }
            if (v.x > 0 && towards.right()) || (v.x < 0 && towards.left()) {
                entity.pos.x -= v.x;
            }
        }
    }

    /// Goalkeepers stay in their box; "freeze team" stops opponents (`sub_FB6C`).
    fn apply_movement_constraints(&mut self, p: PlayerId) {
        if self.clock_paused || self.players[p.0].entity.flags.falling() {
            return;
        }
        let frozen = self.is_frozen(p);
        let player = &mut self.players[p.0];
        let e = &mut player.entity;
        if player.attributes.pos == 0 {
            let next_y = e.pos.y + e.velocity.y;
            if (e.velocity.y > 0 && next_y > player.zone_max.y)
                || (e.velocity.y < 0 && next_y < player.zone_min.y)
            {
                e.pos.y -= e.velocity.y;
            }
        }
        if frozen {
            e.pos = e.pos - e.velocity;
        }
    }

    // -- Catching -----------------------------------------------------------------

    /// Picks up a loose ball within reach (`sub_D520`).
    fn try_catch_ball(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        let flags = player.entity.flags;
        if self.holder.is_some()
            || flags.falling()
            || flags.throwing()
            || flags.invisible()
            || player.distance_to_ball > 16
            || self.ball_in_multiplier
        {
            return;
        }
        if player.entity.think == Think::GoalieDive {
            self.goalie_deflect(p);
            return;
        }
        let ball = &self.props.ball;
        if ball.sprite > 2 && !flags.jumping() {
            return; // Ball is overhead.
        }
        let thrower_dir = self.players[self.thrower.0].attributes.initial_dir;
        let is_opponent_ball = player.attributes.initial_dir != thrower_dir;
        if ball.is_charged()
            && self.charged_hits_left != 0
            && self.shield_allows_hit(p)
            && is_opponent_ball
        {
            self.hit_by_charged_ball(p);
            let random = self.players[p.0].random;
            self.play_sound(Sound::knocked_down(random));
            return;
        }

        if flags.jumping() {
            self.props.ball.script = addr::SCRIPT_BALL_HELD_JUMPING;
        } else {
            self.props.ball.script = addr::SCRIPT_BALL_HELD;
            let standing = !flags.busy() && player.entity.velocity.is_zero();
            if standing && !self.props.ball.velocity.is_zero() {
                self.face_incoming_ball(p);
            }
        }
        if !self.props.ball.is_charged() && is_opponent_ball {
            self.play_sound(self.interception_sound(p));
        }
        self.props.ball.script_index = self.players[p.0].entity.script_index;
        self.holder = Some(EntityRef::Player(p));
        if self.props.ball.velocity.is_zero() {
            return;
        }
        self.play_sound(Sound::CATCH);
        if !is_opponent_ball {
            return;
        }
        self.players[p.0].stats.goals_saved += 1;
        self.props.ball.set_charged(false);
    }

    /// The crowd reacts to `p` winning the ball from the other team.
    fn interception_sound(&self, p: PlayerId) -> Sound {
        if self.started_attacking_up(p) {
            Sound::INTERCEPTION_UP
        } else {
            Sound::INTERCEPTION_DOWN
        }
    }

    /// `sub_D632`.
    fn hit_by_charged_ball(&mut self, p: PlayerId) {
        self.charged_hits_left -= 1;
        self.players[p.0].entity.dir = self.props.ball.dir;
        self.hit_by_tackle(p, self.thrower);
        self.knock_down(p, 3);
    }

    /// A diving goalkeeper punches the ball away (`sub_D672`).
    fn goalie_deflect(&mut self, p: PlayerId) {
        if self.props.ball.sprite > 2 {
            return;
        }
        let thrower_dir = self.players[self.thrower.0].attributes.initial_dir;
        let keeper = &self.players[p.0];
        let is_opponent_ball = keeper.attributes.initial_dir != thrower_dir;
        if self.props.ball.is_charged() && self.charged_hits_left != 0 && is_opponent_ball {
            self.hit_by_charged_ball(p);
            return;
        }
        self.play_sound(Sound::THROW);
        if is_opponent_ball {
            self.players[p.0].stats.goals_saved += 1;
            self.props.ball.set_charged(false);
        }
        let keeper = &self.players[p.0];
        let dir = self
            .rom
            .goalie_deflect_dir(keeper.entity.dir, keeper.attributes.initial_dir);
        let thr = keeper.skills().thr;
        self.set_ball_speed_from_throw(((thr / 2) | thr) / 2);
        let ball = &mut self.props.ball;
        ball.dir = dir;
        ball.velocity = self.rom.velocity(dir, 8);
        ball.script = addr::SCRIPT_BALL_THROWN_HIGH;
        ball.script_index = 0;
    }

    /// Turns to catch a ball flying at a standing player (`sub_F1B4`).
    fn face_incoming_ball(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        let bits = player.entity.dir_bits_towards(self.props.ball.pos);
        if bits.has_direction() {
            let list = match (player.is_goalkeeper(), player.attacks_up()) {
                (true, true) => addr::ANIM_GOALIE_CATCH_TOP,
                (true, false) => addr::ANIM_GOALIE_CATCH_BOTTOM,
                (false, _) => addr::ANIM_CATCH,
            };
            let dir = self.rom.dir_from_bits(bits);
            let script = self.rom.script_for_dir(list, dir);
            let entity = &mut self.players[p.0].entity;
            entity.dir = dir;
            entity.script = script;
            entity.script_index = 0;
            entity.flags.set_busy(true);
        }
        self.players[p.0].entity.think = Think::Idle;
    }

    // -- Keyboard -----------------------------------------------------------------

    /// `sub_D724`.
    fn apply_human_input(&mut self, p: PlayerId, input: Input) {
        let entity = &mut self.players[p.0].entity;
        entity.timer = 1;
        if !entity.flags.busy() {
            entity.think = Think::Idle;
            self.steer_with_input(p, input);
        }
    }

    /// Joystick movement plus fire: throw, jump or tackle (`sub_F47E`).
    fn steer_with_input(&mut self, p: PlayerId, input: Input) {
        let rom = self.rom.clone();
        let mut dir = rom.dir_from_bits(input.dir_bits());
        let (mut list, mut speed) = self.stand_animation(p);
        if dir < 0 {
            self.players[p.0].entity.script_index = 0;
        } else {
            if self.controls_reversed(p) {
                dir ^= 4;
            }
            self.players[p.0].entity.dir = dir;
            (list, speed) = self.run_animation(p);
            self.players[p.0].entity.script_index &= 7;
        }

        if input.fire() {
            if self.holds_ball(p) {
                list = addr::ANIM_THROW;
                speed = 0;
                self.begin_throw(p);
            } else {
                let jump_bonus = self.players[p.0].speed_jump_bonus(&rom);
                self.players[p.0].jump_bonus = jump_bonus;
                let ball_overhead_in_reach = jump_bonus * 6 >= self.players[p.0].distance_to_ball
                    && self.holder.is_none()
                    && !self.ball_in_multiplier
                    && self.props.ball.sprite > 2;
                let entity = &mut self.players[p.0].entity;
                entity.flags.set_busy(true);
                if ball_overhead_in_reach {
                    list = addr::ANIM_JUMP;
                    entity.flags.set_jumping(true);
                    entity.script_index = 0;
                    entity.think = Think::Jump;
                    self.play_sound(Sound::JUMP);
                } else if speed != 0 {
                    (list, speed) = self.lunge_animation(p);
                } else {
                    entity.think = Think::StandingTackle;
                    list = addr::ANIM_STANDING_TACKLE;
                    self.own_input_mut(p).set_fire(false);
                }
            }
        }
        self.players[p.0].entity.set_animation(&rom, list, speed);
    }

    // -- Running ------------------------------------------------------------------

    /// Starts running towards `target`, or stands if already there (`sub_EB7A`).
    pub(crate) fn start_move(&mut self, p: PlayerId) {
        if self.steer_towards_target(p, false) {
            let entity = &mut self.players[p.0].entity;
            entity.script_index &= 7;
            entity.think = Think::Move;
        } else {
            self.stand(p);
        }
    }

    /// `sub_EC0C`.
    fn keep_moving(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        let near = player.entity.axis_distance(player.target, 32) <= 32;
        // Far from the target, only re-steer at the start of a stride.
        if !near && player.entity.script_index != 0 {
            return;
        }
        if self.steer_towards_target(p, near) {
            return;
        }
        // Arrived. Defenders of the controlled pair then chase the carrier.
        let chase_carrier = self.is_controlled(p)
            && !self.players[p.0].is_goalkeeper()
            && !self.clock_paused
            && self.holder.is_some()
            && !self.holds_ball(p);
        if chase_carrier {
            let carrier = self.entity(self.holder.expect("checked above"));
            self.players[p.0].target = self.players[p.0].predict(&self.rom, carrier);
            self.start_move(p);
        } else {
            self.stand(p);
        }
    }

    /// Points the player at `target`, snapping when within 4 pixels.
    /// Returns `false` if already there.
    fn steer_towards_target(&mut self, p: PlayerId, exact: bool) -> bool {
        let player = &mut self.players[p.0];
        let target = player.target;
        let mut bits = if exact {
            player.dir_bits_towards_exact(target)
        } else {
            player.entity.dir_bits_towards(target)
        };
        if !bits.has_direction() {
            return false;
        }
        let pos = &mut player.entity.pos;
        if (target.x - pos.x).abs() < 4 {
            pos.x = target.x;
            bits.clear_horizontal();
        }
        if (target.y - pos.y).abs() < 4 {
            pos.y = target.y;
            bits.clear_vertical();
        }
        if *pos == target {
            return false;
        }
        player.entity.dir = self.rom.dir_from_bits(bits);
        let (list, speed) = self.run_animation(p);
        let rom = self.rom.clone();
        self.players[p.0].entity.set_animation(&rom, list, speed);
        true
    }

    /// Animation list and speed for running (`sub_ECF6`).
    fn run_animation(&self, p: PlayerId) -> (i32, i32) {
        let player = &self.players[p.0];
        let list = match (
            player.is_goalkeeper() && !self.clock_paused,
            player.attacks_up(),
        ) {
            (true, true) => addr::ANIM_GOALIE_RUN_TOP,
            (true, false) => addr::ANIM_GOALIE_RUN_BOTTOM,
            (false, _) => addr::ANIM_RUN,
        };
        let spd = player.skills().spd;
        let mut speed = 4;
        if spd > 140 {
            speed += 1;
            if spd > 200 {
                speed += 1;
            }
        }
        // Carrying the ball slows you down.
        if !self.holds_ball(p) && speed != 7 {
            speed += 1;
        }
        (list, speed)
    }

    /// Animation list for standing; goalkeepers also face up-pitch (`sub_F25C`).
    fn stand_animation(&mut self, p: PlayerId) -> (i32, i32) {
        let player = &mut self.players[p.0];
        if player.is_goalkeeper() && !self.clock_paused {
            player.entity.dir = player.attributes.initial_dir;
            let list = if player.attacks_up() {
                addr::ANIM_GOALIE_STAND_TOP
            } else {
                addr::ANIM_GOALIE_STAND_BOTTOM
            };
            return (list, 0);
        }
        (addr::ANIM_STAND, 0)
    }

    /// Stops; while the clock is stopped, turns one step towards the
    /// default direction (`sub_F212`).
    pub(crate) fn stand(&mut self, p: PlayerId) {
        let paused = self.clock_paused;
        let player = &mut self.players[p.0];
        let facing = player.attributes.initial_dir;
        if paused && player.entity.dir != facing {
            let step = if facing == 0 { -1 } else { 1 };
            player.entity.dir = (player.entity.dir + step + 8) % 8;
            player.entity.timer = 2;
        }
        let (list, speed) = self.stand_animation(p);
        let rom = self.rom.clone();
        let entity = &mut self.players[p.0].entity;
        entity.set_animation(&rom, list, speed);
        entity.script_index = 0;
        entity.think = Think::Idle;
    }

    // -- Tackling and jumping -------------------------------------------------------

    /// Dives (tackle) at a point, or jumps for an overhead ball (`sub_E982`).
    pub(crate) fn dive_at(&mut self, p: PlayerId, target: Point) {
        let rom = self.rom.clone();
        let jump_bonus = self.players[p.0].speed_jump_bonus(&rom);
        let player = &mut self.players[p.0];
        player.target = target;
        player.entity.flags.set_busy(true);
        player.jump_bonus = jump_bonus;
        let bits = player.entity.dir_bits_towards(target);
        let jump_for_ball = self.holder.is_none()
            && jump_bonus * 6 >= player.distance_to_ball
            && !self.ball_in_multiplier
            && self.props.ball.sprite > 2;
        if !jump_for_ball {
            player.entity.dir = if bits.has_direction() {
                rom.dir_from_bits(bits)
            } else {
                player.attributes.initial_dir
            };
            let (list, speed) = self.lunge_animation(p);
            self.players[p.0].entity.set_animation(&rom, list, speed);
            return;
        }
        let speed = if bits.has_direction() {
            player.entity.dir = rom.dir_from_bits(bits);
            4 + player.speed_bonus()
        } else {
            0
        };
        let entity = &mut player.entity;
        entity.set_animation(&rom, addr::ANIM_JUMP, speed);
        entity.script_index = 0;
        entity.flags.set_jumping(true);
        entity.think = Think::Jump;
        self.play_sound(Sound::JUMP);
    }

    /// `sub_EA56`.
    pub(crate) fn dive_at_entity(&mut self, p: PlayerId, target: EntityRef) {
        let point = self.players[p.0].predict(&self.rom, self.entity(target));
        self.dive_at(p, point);
    }

    /// Goalkeeper dive or sliding tackle (`sub_EA62`).
    fn lunge_animation(&mut self, p: PlayerId) -> (i32, i32) {
        self.play_sound(Sound::SLIDE);
        let ball_flying = !self.props.ball.velocity.is_zero();
        let holder_none = self.holder.is_none();
        let player = &mut self.players[p.0];
        player.entity.flags.set_tackling(true);
        player.entity.script_index = 0;
        let sideways = player.entity.dir & 3 != 0;
        if player.is_goalkeeper() && holder_none && ball_flying && sideways {
            player.entity.think = Think::GoalieDive;
            let list = if player.attacks_up() {
                addr::ANIM_GOALIE_DIVE_TOP
            } else {
                addr::ANIM_GOALIE_DIVE_BOTTOM
            };
            (list, 8)
        } else {
            player.entity.think = Think::SlideTackle;
            (addr::ANIM_SLIDE_TACKLE, 5 + player.speed_bonus())
        }
    }

    /// Standing tackle towards an opponent (`sub_EAF4`).
    pub(crate) fn standing_tackle(&mut self, p: PlayerId, opponent: PlayerId) {
        let rom = self.rom.clone();
        let opponent_pos = self.players[opponent.0].entity.pos;
        let player = &mut self.players[p.0];
        let bits = player.entity.dir_bits_towards(opponent_pos);
        player.entity.dir = if bits.has_direction() {
            rom.dir_from_bits(bits)
        } else {
            player.attributes.initial_dir
        };
        let entity = &mut player.entity;
        entity.script_index = 0;
        entity.flags.set_busy(true);
        entity.set_animation(&rom, addr::ANIM_STANDING_TACKLE, 0);
        entity.think = Think::StandingTackle;
    }

    /// Runs directly away from `threat` (`sub_EB38`).
    pub(crate) fn run_away_from(&mut self, p: PlayerId, threat: PlayerId) {
        let threat_pos = self.players[threat.0].entity.pos;
        let player = &mut self.players[p.0];
        let bits = player.entity.dir_bits_towards(threat_pos);
        player.entity.dir = if bits.has_direction() {
            (self.rom.dir_from_bits(bits) + 4) & 7
        } else {
            player.attributes.initial_dir
        };
        player.entity.script_index &= 7;
        let (list, speed) = self.run_animation(p);
        let rom = self.rom.clone();
        let entity = &mut self.players[p.0].entity;
        entity.set_animation(&rom, list, speed);
        entity.think = Think::Idle;
    }

    /// Knocks over the first opponent in reach, if the tackle succeeds
    /// (`sub_ED56`).
    fn check_tackle_hits(&mut self, p: PlayerId) {
        if self.clock_paused || self.players[p.0].entity.flags.offscreen() {
            return;
        }
        for victim in self.opponents(p) {
            let target = &self.players[victim.0];
            let in_reach = self.players[p.0].distance_to_opponents[target.index] <= 30;
            if target.entity.flags.falling()
                || !in_reach
                || target.entity.flags.offscreen()
                || !self.shield_allows_hit(victim)
            {
                continue;
            }
            self.play_sound(Sound::TACKLE);
            self.players[p.0].entity.think = Think::TackleRecover;
            if self.rng.next_byte() > self.tackle_success_chance(victim, p) {
                return;
            }
            self.land_tackle(p, victim);
            return;
        }
    }

    fn land_tackle(&mut self, p: PlayerId, victim: PlayerId) {
        let victim_random = self.players[victim.0].random;
        self.play_sound(Sound::knocked_down(victim_random));
        self.players[p.0].stats.successful_tackles += 1;
        self.hit_by_tackle(victim, p);

        let attacker = &self.players[p.0].entity;
        let attacker_falling = attacker.flags.falling();
        let dir = if attacker_falling {
            (attacker.dir + 4) & 7
        } else {
            attacker.dir
        };
        let speed = if attacker.flags.tackling() { 4 } else { 3 };
        let target = &mut self.players[victim.0].entity;
        let previous_think = target.think;
        target.dir = dir;
        self.knock_down(victim, speed);
        // A victim caught mid-tackle keeps its tackle routine running.
        if matches!(
            previous_think,
            Think::GoalieDive | Think::SlideTackle | Think::StandingTackle
        ) {
            self.players[victim.0].entity.think = previous_think;
        }

        if !self.holds_ball(victim) {
            return;
        }
        if attacker_falling {
            self.holder = None;
        } else {
            // A clean tackle steals the ball.
            self.holder = Some(EntityRef::Player(p));
            self.props.ball.script = addr::SCRIPT_BALL_HELD;
            self.props.ball.script_index = 0;
            self.play_sound(self.interception_sound(p));
        }
    }

    /// Puts a player on the floor, sliding at `speed` in its current direction.
    pub(crate) fn knock_down(&mut self, p: PlayerId, speed: i32) {
        let rom = self.rom.clone();
        let entity = &mut self.players[p.0].entity;
        entity.script_index = 0;
        entity.flags.set_busy(true);
        entity.flags.set_falling(true);
        entity.think = Think::Idle;
        entity.set_animation(&rom, addr::ANIM_KNOCKED_DOWN, speed);
    }

    /// Chance out of 256 that `attacker`'s tackle on `defender` lands
    /// (`sub_EE9C`).
    fn tackle_success_chance(&self, defender: PlayerId, attacker: PlayerId) -> i32 {
        const TACKLING_PENALTY: i32 = 16;
        const JUMPING_PENALTY: i32 = 32;
        let defender = &self.players[defender.0];
        let attacker = &self.players[attacker.0];
        let mut defence = defender.skills().def;
        if defender.is_goalkeeper() {
            defence = (defence + defence / 2).min(255);
        }
        let angle = (defender.entity.dir - attacker.entity.dir + 8) & 7;
        defence -= self.rom.tackle_def_malus(angle);
        if attacker.entity.flags.tackling() {
            defence -= TACKLING_PENALTY;
        }
        if defender.entity.flags.jumping() {
            defence -= JUMPING_PENALTY;
        }
        (attacker.skills().att + 256 - defence) / 2
    }

    /// Damage from a tackle; the victim drops any equipment (`sub_EEFC`).
    pub(crate) fn hit_by_tackle(&mut self, victim: PlayerId, attacker: PlayerId) {
        let power = self.players[attacker.0].skills().pow;
        self.players[victim.0].take_tackle_damage(power);
        self.drop_equipment(victim);
    }

    /// Lands after the jump's airtime (`sub_F290`).
    fn land_jump(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        if player.jump_bonus + 2 > player.entity.script_index {
            return;
        }
        // The original always clears player 1's fire button here.
        self.inputs.team1.set_fire(false);
        if self.holds_ball(p) {
            self.props.ball.script_index = 18;
        }
        let entity = &mut self.players[p.0].entity;
        entity.script_index = 18;
        entity.think = Think::Idle;
        entity.flags.set_jumping(false);
        self.play_sound(Sound::LAND);
    }

    /// `sub_F2E2`.
    fn recover_from_tackle(&mut self, p: PlayerId) {
        let player = &self.players[p.0];
        // Byte arithmetic: a zero bonus wraps to 255.
        if (player.jump_bonus + 255) & 0xFF > player.entity.script_index {
            return;
        }
        let entity = &mut self.players[p.0].entity;
        entity.script_index = 15;
        entity.think = Think::Idle;
        self.own_input_mut(p).set_fire(false);
        if !self.clock_paused {
            self.play_sound(Sound::SILENCE_CHANNEL_0);
        }
    }

    // -- Throwing -------------------------------------------------------------------

    /// Throws towards `target` with fire held, which lobs the ball (`sub_EFD8`).
    pub(crate) fn throw_high(&mut self, p: PlayerId) {
        self.own_input_mut(p).set_fire(true);
        self.start_throw(p);
    }

    /// Throws towards `target` along the ground (`sub_EFFC`).
    pub(crate) fn throw_low(&mut self, p: PlayerId) {
        self.own_input_mut(p).set_fire(false);
        self.start_throw(p);
    }

    /// `sub_F020`.
    fn start_throw(&mut self, p: PlayerId) {
        let rom = self.rom.clone();
        let player = &mut self.players[p.0];
        let bits = player.entity.dir_bits_towards(player.target);
        player.entity.dir = if bits.has_direction() {
            rom.dir_from_bits(bits)
        } else {
            player.attributes.initial_dir
        };
        player.entity.set_animation(&rom, addr::ANIM_THROW, 0);
        self.begin_throw(p);
    }

    fn begin_throw(&mut self, p: PlayerId) {
        let entity = &mut self.players[p.0].entity;
        entity.script_index = 0;
        entity.flags.set_busy(true);
        entity.flags.set_throwing(true);
        entity.think = Think::Throw;
        self.thrower = p;
        self.play_sound(Sound::THROW);
    }

    /// Lets go of the ball on frame 4 of the throw (`sub_F078`).
    fn release_throw(&mut self, p: PlayerId) {
        if self.players[p.0].entity.script_index < 4 {
            return;
        }
        if !self.clock_paused {
            self.holder = None;
        }
        let thr = self.players[p.0].skills().thr;
        self.set_ball_speed_from_throw(thr);
        let dir = self.players[p.0].entity.dir;
        let ball = &mut self.props.ball;
        ball.dir = dir;
        ball.velocity = self.rom.velocity(dir, 8);
        ball.script = addr::SCRIPT_BALL_THROWN;
        ball.script_index = 0;

        let side = self.side_of(p);
        self.charged_hits_left = self.arena.multipliers.level(side) + 1;
        let computer = *self.computer.get(side);
        let input = self.inputs.get_mut(side);
        let held = *input;
        input.set_fire(false);
        if computer {
            input.set_left(false);
            input.set_right(false);
        }

        // Fire held: lob. A direction held at right angles adds curve.
        let ball = &mut self.props.ball;
        if held.fire() {
            ball.script = addr::SCRIPT_BALL_THROWN_HIGH;
        }
        if ball.velocity.x == 0 {
            if held.left() {
                ball.velocity.x = -4;
            } else if held.right() {
                ball.velocity.x = 4;
            }
        } else if ball.velocity.y == 0 {
            if held.up() {
                ball.velocity.y = -4;
            } else if held.down() {
                ball.velocity.y = 4;
            }
        }
        self.players[p.0].entity.think = Think::Idle;
    }

    // -- Goals ----------------------------------------------------------------------

    /// The scorer's team celebrates; the scorer runs back holding the ball
    /// (`sub_CEEA`).
    pub(crate) fn start_goal_celebration(&mut self, scorer: PlayerId) {
        let rom = self.rom.clone();
        for id in self.teammates(scorer) {
            let flags = self.players[id.0].entity.flags;
            if flags.falling() || flags.invisible() {
                continue;
            }
            let player = &mut self.players[id.0];
            let entity = &mut player.entity;
            entity.flags.set_busy(true);
            entity.script_index = 0;
            entity.dir &= 4;
            let (list, speed) = if id == scorer {
                entity.dir = player.attributes.initial_dir ^ 4;
                if self.holder.is_none() {
                    self.holder = Some(EntityRef::Player(id));
                }
                (addr::ANIM_CELEBRATE_SCORER, 5)
            } else {
                (addr::ANIM_CELEBRATE, 0)
            };
            let entity = &mut self.players[id.0].entity;
            entity.set_animation(&rom, list, speed);
            entity.think = Think::Idle;
        }
    }
}
