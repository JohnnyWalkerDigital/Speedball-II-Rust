//! Computer-controlled decisions.
//!
//! This is a faithful port of the original AI: the branch structure and
//! arithmetic are unchanged so matches play out exactly as they did, but
//! each routine is named for what it does (the original label is noted).

use super::entity::Think;
use super::geometry::{DirBits, Point};
use super::player::PlayerId;
use super::state::{EntityRef, Match};

/// Where a goalkeeper slides to, relative to the predicted ball x.
#[derive(Clone, Copy)]
enum Cover {
    /// Keep the predicted x.
    Track,
    /// Move to this x.
    Exactly(i32),
    /// Halfway between the predicted x and this x.
    Halfway(i32),
    /// A quarter of the way from the predicted x to this x.
    Quarter(i32),
}

impl Cover {
    fn apply(self, predicted_x: i32) -> i32 {
        match self {
            Cover::Track => predicted_x,
            Cover::Exactly(x) => x,
            Cover::Halfway(x) => (x + predicted_x) / 2,
            Cover::Quarter(x) => (predicted_x + (x + predicted_x) / 2) / 2,
        }
    }
}

/// The three ways a ball carrier can sidestep.
#[derive(Clone, Copy)]
enum Dodge {
    Wing,
    ZoneLeft,
    ZoneRight,
}

impl Match {
    // =========================================================================
    // Active player (nearest the ball)
    // =========================================================================

    /// `sub_D742`.
    pub(crate) fn ai_active_player(&mut self, p: PlayerId) {
        if self.players[p.0].entity.flags.busy() {
            return;
        }
        self.roll_decision(p);
        let holds = self.holds_ball(p);
        if !holds && self.engage_nearby_player(p) {
            return;
        }
        let opponent = if p == self.controlled.team1 {
            self.controlled.team2
        } else {
            self.controlled.team1
        };
        if holds {
            self.ai_carry_ball(p, opponent);
        } else {
            self.ai_chase(p, opponent);
        }
    }

    fn roll_decision(&mut self, p: PlayerId) {
        let random = self.rng.next_byte();
        let rom = self.rom.clone();
        let player = &mut self.players[p.0];
        player.random = random;
        player.reset_think_delay(&rom);
    }

    /// Without the ball: go for the ball, an item or the opposing carrier.
    fn ai_chase(&mut self, p: PlayerId, opponent: PlayerId) {
        if self.players[p.0].is_goalkeeper() {
            self.ai_goalkeeper_active(p);
            return;
        }
        let mut chase = EntityRef::Player(opponent);
        if !self.players[p.0]
            .entity
            .is_well_inside_view(self.camera.position())
        {
            chase = EntityRef::Ball;
        } else {
            let item = self.find_reachable_item(p, [DirBits::NONE; 2]);
            let item_nearer_than_ball = item.is_some()
                && self.props.ball.distance_to(self.players[p.0].target)
                    <= self.players[p.0].distance_to_ball;
            let me = &self.players[p.0];
            let them = &self.players[opponent.0];
            if item_nearer_than_ball {
                chase = item.expect("checked above");
            } else if self.holder != Some(EntityRef::Player(opponent))
                && !self.ball_in_multiplier
                && (them.entity.flags.falling()
                    || me.skills().agr / 2 <= me.random
                    || me.distance_to_ball <= them.distance_to_ball)
            {
                chase = EntityRef::Ball;
            }
            if me.skills().agr / 2 > me.random {
                self.dive_at_entity(p, chase);
                return;
            }
        }
        self.players[p.0].target = self.players[p.0].predict(&self.rom, self.entity(chase));
        self.start_move(p);
    }

    /// With the ball: run, pass, shoot or try a bonus play.
    fn ai_carry_ball(&mut self, p: PlayerId, opponent: PlayerId) {
        let blocked = self.find_blocking_directions(p, opponent);
        let pass = self.players[p.0].is_goalkeeper() || {
            if self.try_bonus_play(p, blocked) {
                return;
            }
            self.carrier_should_pass(p, opponent, blocked)
        };
        if !pass {
            self.start_move(p);
        } else if self.players[p.0].attributes.pos < 3 {
            self.pass_from_defence(p, blocked);
        } else {
            self.attack_goal(p, opponent);
        }
    }

    fn carrier_should_pass(
        &mut self,
        p: PlayerId,
        opponent: PlayerId,
        blocked: [DirBits; 2],
    ) -> bool {
        let them = &self.players[opponent.0];
        if !them.entity.flags.falling() && them.distance_to_ball <= 64 {
            return true; // Under pressure.
        }
        let me = &self.players[p.0];
        let mut int = me.skills().int;
        if me.attributes.pos > 2 {
            int /= 2;
        }
        if int > me.random && self.find_reachable_item(p, blocked).is_some() {
            return false; // Run for the item.
        }
        let me = &mut self.players[p.0];
        let (pos, zone_center) = (me.entity.pos, me.zone_center);
        if me.attacks_up() {
            if pos.y <= zone_center.y {
                return true;
            }
            me.target.y = (pos.y + me.zone_min.y) / 2;
        } else {
            if pos.y >= zone_center.y {
                return true;
            }
            me.target.y = (pos.y + me.zone_max.y) / 2;
        }
        // Run upfield unless every sidestep runs into an opponent.
        let prefer_left = (me.random >> 3) & 1 == 1;
        let chosen = self.choose_dodge(p, blocked, prefer_left);
        blocked.contains(&chosen)
    }

    /// Directions towards the nearest two opponents (`sub_D902`).
    fn find_blocking_directions(&mut self, p: PlayerId, opponent: PlayerId) -> [DirBits; 2] {
        let me = &self.players[p.0];
        let them = &self.players[opponent.0];
        let towards_opponent = if them.entity.flags.falling() {
            DirBits::NONE
        } else {
            me.entity
                .dir_bits_towards(me.predict(&self.rom, &them.entity))
        };
        let mut range = me.pass_range();
        let mut nearest = None;
        for q in self.opponents(p) {
            let other = &self.players[q.0];
            if range < other.distance_to_ball || q == opponent || other.entity.flags.falling() {
                continue;
            }
            nearest = Some(q);
            range = other.distance_to_ball;
        }
        let towards_nearest = nearest.map_or(DirBits::NONE, |q| {
            me.entity
                .dir_bits_towards(me.predict(&self.rom, &self.players[q.0].entity))
        });
        self.blocking_dirs = [towards_opponent, towards_nearest];
        self.blocking_dirs
    }

    /// Targets the first visible pickup not behind an opponent (`sub_D97E`).
    fn find_reachable_item(&mut self, p: PlayerId, blocked: [DirBits; 2]) -> Option<EntityRef> {
        let items = [
            EntityRef::Token(0),
            EntityRef::Token(1),
            EntityRef::Equipment,
            EntityRef::Coins(0),
            EntityRef::Coins(1),
            EntityRef::Coins(2),
            EntityRef::Coins(3),
        ];
        let scroll = self.camera.position();
        for item in items {
            self.entity_mut(item).update_offscreen(scroll);
            let entity = self.entity(item);
            if entity.flags.offscreen() || entity.flags.invisible() {
                continue;
            }
            let item_pos = entity.pos;
            if blocked.contains(&self.players[p.0].entity.dir_bits_towards(item_pos)) {
                continue;
            }
            self.players[p.0].target = item_pos;
            return Some(item);
        }
        None
    }

    /// Tries sidesteps in order until one avoids the blocked directions
    /// (`sub_DA86` / `sub_DB32`).
    fn choose_dodge(&mut self, p: PlayerId, blocked: [DirBits; 2], prefer_left: bool) -> DirBits {
        let (first, second) = if prefer_left {
            (Dodge::ZoneLeft, Dodge::ZoneRight)
        } else {
            (Dodge::ZoneRight, Dodge::ZoneLeft)
        };
        for dodge in [Dodge::Wing, first, second] {
            let bits = self.dodge(p, dodge);
            if !blocked.contains(&bits) {
                return bits;
            }
        }
        let me = &mut self.players[p.0];
        me.target.y = (me.entity.pos.y + me.zone_center.y) / 2;
        for dodge in [first, second] {
            let bits = self.dodge(p, dodge);
            if !blocked.contains(&bits) {
                return bits;
            }
        }
        let me = &mut self.players[p.0];
        me.target.y = if me.attacks_up() {
            (me.entity.pos.y + me.zone_max.y) / 2
        } else {
            (me.entity.pos.y + me.zone_min.y) / 2
        };
        let bits = self.dodge(p, first);
        if !blocked.contains(&bits) {
            return bits;
        }
        self.dodge(p, second)
    }

    /// Sets the target x for a sidestep; returns its direction
    /// (`sub_DA20` / `sub_DA56` / `sub_DA6E`).
    fn dodge(&mut self, p: PlayerId, dodge: Dodge) -> DirBits {
        let me = &mut self.players[p.0];
        let pos = me.entity.pos;
        let x = match dodge {
            Dodge::Wing => {
                let mut wing = pos;
                me.clamp_x_to_zone(&mut wing);
                if wing.x <= 96 {
                    wing.x = 56;
                } else if wing.x >= 544 {
                    wing.x = 584;
                }
                me.clamp_x_to_zone(&mut wing);
                wing.x
            }
            Dodge::ZoneLeft => (pos.x + me.zone_min.x) / 2,
            Dodge::ZoneRight => (pos.x + me.zone_max.x) / 2,
        };
        me.target.x = x;
        me.entity.dir_bits_towards(Point::new(x, me.target.y))
    }

    // -- Bonus plays ----------------------------------------------------------------

    /// Ramps, stars and electrobounces (`sub_DCCC`).
    fn try_bonus_play(&mut self, p: PlayerId, blocked: [DirBits; 2]) -> bool {
        let charged = self.props.ball.is_charged();
        let clear = |bits: DirBits| charged || !blocked.contains(&bits);
        let me = &self.players[p.0];
        let pos = me.entity.pos;

        // Throw into a score-multiplier ramp from directly below/above it.
        if self.arena.multipliers.level(self.side_of(p)) != 2 {
            let under_left_ramp = (48..=64).contains(&pos.x) && (pos.y > 640 || pos.y < 576);
            let under_right_ramp = (576..=592).contains(&pos.x) && (pos.y > 576 || pos.y < 512);
            if under_left_ramp || under_right_ramp {
                let target = Point::new(pos.x, 576);
                self.players[p.0].target = target;
                if clear(self.players[p.0].entity.dir_bits_towards(target)) {
                    self.throw_low(p);
                    return true;
                }
            }
        }

        // Lob at the wall stars when out on a wing.
        if pos.x <= 213 || pos.x > 426 {
            let target = if pos.x <= 213 {
                Point::new(32, 464)
            } else {
                Point::new(608, 688)
            };
            self.players[p.0].target = target;
            let me = &self.players[p.0];
            let bits = me.entity.dir_bits_towards(target);
            if clear(bits) {
                let dir = self.rom.dir_from_bits(bits);
                let diagonal_upfield = if me.attacks_up() {
                    dir == 7 || dir == 1
                } else {
                    dir == 3 || dir == 5
                };
                if diagonal_upfield {
                    self.throw_high(p);
                    return true;
                }
            }
        }

        // Aggressive players bank the ball off an electrobounce.
        let me = &self.players[p.0];
        if !charged && me.skills().agr / 2 > me.random {
            return self.try_electrobounce_shot(p, 0, blocked)
                || self.try_electrobounce_shot(p, 1, blocked);
        }
        false
    }

    /// `sub_DE30`.
    fn try_electrobounce_shot(&mut self, p: PlayerId, pad: usize, blocked: [DirBits; 2]) -> bool {
        let pad_pos = self.arena.electrobounces.pads[pad].pos;
        let me = &mut self.players[p.0];
        if me.entity.axis_distance(pad_pos, 106) > 106 {
            return false;
        }
        me.target = pad_pos;
        let bits = me.entity.dir_bits_towards(pad_pos);
        if blocked.contains(&bits) {
            return false;
        }
        let dir = self.rom.dir_from_bits(bits);
        if dir == 0 || dir == 4 {
            return false;
        }
        self.throw_low(p);
        true
    }

    // -- Passing and shooting ---------------------------------------------------------

    /// Defenders and midfielders look for a pass upfield (`sub_DBE8`).
    fn pass_from_defence(&mut self, p: PlayerId, mut blocked: [DirBits; 2]) {
        if self.props.ball.is_charged() {
            blocked = [DirBits::NONE; 2];
            self.blocking_dirs = blocked;
        }
        let me = &self.players[p.0];
        let my_pos = me.attributes.pos;
        let mut range = me.pass_range();
        let mut receiver = None;
        // Prefer the next line up, then fall back one line at a time.
        let mut min_pos = if me.is_goalkeeper() { 2 } else { my_pos + 1 };
        loop {
            for q in self.teammates(p) {
                let mate = &self.players[q.0];
                if mate.entity.flags.falling()
                    || min_pos > mate.attributes.pos
                    || q == p
                    || !self.is_pass_lane_clear(p, q, blocked)
                    || range < mate.distance_to_ball
                {
                    continue;
                }
                range = mate.distance_to_ball;
                receiver = Some(q);
            }
            if receiver.is_some() || min_pos < my_pos {
                break;
            }
            min_pos -= 1;
            if min_pos == 0 {
                break;
            }
        }
        let Some(q) = receiver else {
            self.clear_ball(p);
            return;
        };
        let mate = &self.players[q.0];
        let me = &self.players[p.0];
        let target = me.predict(&self.rom, &mate.entity);
        let short_pass = (!me.is_goalkeeper() || mate.attributes.pos == 1)
            && me.skills().thr * 2 > mate.distance_to_ball;
        self.players[p.0].target = target;
        if short_pass {
            self.throw_low(p);
        } else {
            self.throw_high(p);
        }
    }

    /// `sub_E05C`.
    fn is_pass_lane_clear(
        &self,
        passer: PlayerId,
        receiver: PlayerId,
        blocked: [DirBits; 2],
    ) -> bool {
        let passer = &self.players[passer.0];
        let target = passer.predict(&self.rom, &self.players[receiver.0].entity);
        !blocked.contains(&passer.entity.dir_bits_towards(target))
    }

    /// Forwards go for goal, or lay it off to another forward (`sub_DF2E`).
    fn attack_goal(&mut self, p: PlayerId, opponent: PlayerId) {
        if !self.props.ball.is_charged() {
            let goal = self.aim_at_goal(p);
            let bits = self.players[p.0].entity.dir_bits_towards(goal);
            let blocked = self.blocking_dirs;
            if bits == blocked[0] || (!bits.up() && !bits.down()) {
                let them = &self.players[opponent.0];
                if them.entity.flags.falling() || them.distance_to_ball > 64 {
                    let mut target = self.players[p.0].entity.pos;
                    self.support_position(p, p, &mut target);
                    self.players[p.0].target = target;
                    let bits = self.players[p.0].entity.dir_bits_towards(target);
                    if bits.has_direction() && !blocked.contains(&bits) {
                        self.start_move(p);
                        return;
                    }
                }
                if let Some(q) = self.find_forward_to_pass_to(p, blocked) {
                    let me = &self.players[p.0];
                    let mate = &self.players[q.0];
                    let short_pass = me.skills().thr * 2 > mate.distance_to_ball;
                    self.players[p.0].target = me.predict(&self.rom, &mate.entity);
                    if short_pass {
                        self.throw_low(p);
                    } else {
                        self.throw_high(p);
                    }
                    return;
                }
            }
        }
        self.shoot(p);
    }

    fn find_forward_to_pass_to(&self, p: PlayerId, blocked: [DirBits; 2]) -> Option<PlayerId> {
        let mut range = self.players[p.0].pass_range();
        let mut receiver = None;
        for min_pos in [3, 2] {
            for q in self.teammates(p) {
                let mate = &self.players[q.0];
                if min_pos > mate.attributes.pos
                    || mate.entity.flags.falling()
                    || q == p
                    || !self.is_pass_lane_clear(p, q, blocked)
                    || range < mate.distance_to_ball
                {
                    continue;
                }
                range = mate.distance_to_ball;
                receiver = Some(q);
            }
            if receiver.is_some() {
                break;
            }
        }
        receiver
    }

    /// Picks a spot in the goal mouth and maybe curves the shot by holding
    /// left/right on the team's input (`sub_DEA4`).
    fn aim_at_goal(&mut self, p: PlayerId) -> Point {
        let me = &self.players[p.0];
        let (my_x, random, int, attacks_up) =
            (me.entity.pos.x, me.random, me.skills().int, me.attacks_up());
        let input = self.own_input_mut(p);
        let x = if my_x > 320 {
            input.set_left(true);
            320 - 16
        } else if my_x < 320 {
            input.set_right(true);
            320 + 16
        } else if (random >> 6) & 1 == 0 {
            input.set_left(true);
            272
        } else {
            input.set_right(true);
            368
        };
        if int / 2 <= random {
            input.set_left(false);
            input.set_right(false);
        }
        Point::new(x, if attacks_up { 0 } else { 1152 })
    }

    /// `sub_EF50`.
    fn shoot(&mut self, p: PlayerId) {
        let goal = self.aim_at_goal(p);
        let me = &mut self.players[p.0];
        me.target = goal;
        if me.entity.distance_to(goal) <= me.skills().thr * 2 {
            self.throw_low(p);
        } else {
            self.throw_high(p);
        }
    }

    /// Lobs it upfield, angling for a wall if the goal is blocked (`sub_EF7C`).
    fn clear_ball(&mut self, p: PlayerId) {
        let goal = self.aim_at_goal(p);
        let blocked = self.blocking_dirs[0];
        let me = &mut self.players[p.0];
        me.target = goal;
        if me.entity.dir_bits_towards(goal) == blocked {
            let x = if (me.random >> 4) & 1 == 1 { 608 } else { 32 };
            let dx = (x - me.entity.pos.x).abs();
            me.target = Point::new(
                x,
                if me.attacks_up() {
                    me.entity.pos.y - dx
                } else {
                    me.entity.pos.y + dx
                },
            );
        }
        self.throw_high(p);
    }

    // =========================================================================
    // Close quarters
    // =========================================================================

    /// Tackle or avoid an opponent within reach (`sub_E854`).
    fn engage_nearby_player(&mut self, p: PlayerId) -> bool {
        if self.players[p.0].entity.flags.offscreen() {
            return false;
        }
        for q in self.opponents(p) {
            let me = &self.players[p.0];
            let them = &self.players[q.0];
            if them.entity.flags.falling()
                || me.distance_to_opponents[them.index] > 30
                || them.entity.flags.offscreen()
            {
                continue;
            }
            let attack = !self.holds_ball(p)
                && (me.is_goalkeeper() || self.holds_ball(q) || me.skills().agr > me.random);
            if !attack {
                self.run_away_from(p, q);
            } else if self.is_controlled(p) {
                self.dive_at_entity(p, EntityRef::Player(q));
            } else {
                self.standing_tackle(p, q);
            }
            return true;
        }

        // Keep out of the way of our own active player.
        let mate = *self.controlled.get(self.side_of(p));
        let mate_entity = &self.players[mate.0].entity;
        if mate_entity.flags.falling() || mate == p {
            return false;
        }
        let feet = mate_entity.pos + mate_entity.origin;
        let me = &self.players[p.0].entity;
        if me.axis_distance(feet, 30) <= 30 && me.distance_to(feet) <= 32 {
            self.run_away_from(p, mate);
            return true;
        }
        false
    }

    // =========================================================================
    // Off-ball players
    // =========================================================================

    /// Mark an opponent or take up a supporting position (`sub_E076`).
    pub(crate) fn ai_support_play(&mut self, p: PlayerId) {
        if self.players[p.0].entity.flags.busy() {
            return;
        }
        self.roll_decision(p);
        if self.engage_nearby_player(p) {
            return;
        }
        if self.players[p.0].is_goalkeeper() {
            self.ai_goalkeeper_support(p);
            return;
        }
        let me = &self.players[p.0];
        let mut target = None;
        if !me.entity.flags.offscreen() && me.skills().agr / 2 > me.random {
            if let Some(q) = self.find_opponent_to_mark(p) {
                target = Some(me.predict(&self.rom, &self.players[q.0].entity));
            }
        }
        let target = target.unwrap_or_else(|| self.supporting_position(p));
        self.players[p.0].target = target;
        self.start_move(p);
    }

    fn supporting_position(&self, p: PlayerId) -> Point {
        let me = &self.players[p.0];
        let mut mate = *self.controlled.get(self.side_of(p));
        let mate_entity = &self.players[mate.0].entity;

        // Pounce on a loose ball when our active player is down.
        let ball = &self.props.ball;
        let ball_next = ball.pos + ball.velocity;
        const SPRITE_FLAT_ON_BACK: i32 = 94;
        if mate_entity.flags.falling()
            && mate_entity.sprite == SPRITE_FLAT_ON_BACK
            && (me.zone_min.y..=me.zone_max.y).contains(&ball_next.y)
        {
            return ball_next;
        }

        let mut target = mate_entity.pos + mate_entity.velocity;
        let mate_pos = self.players[mate.0].attributes.pos;
        if mate_pos >= 3 && me.attributes.pos >= 3 {
            self.support_position(p, mate, &mut target);
        } else if me.attributes.pos == 1 {
            // Defenders shadow their goalkeeper once the ball is upfield.
            if mate_pos >= 2 {
                mate = PlayerId::of(me.entity.is_team1(), 0);
                let keeper = &self.players[mate.0].entity;
                target = keeper.pos + keeper.velocity;
            }
            self.position_along(p, mate, &mut target);
        } else if target.x > me.zone_min.x && target.x < me.zone_max.x {
            self.position_between(p, mate, &mut target);
        } else {
            self.position_along(p, mate, &mut target);
        }
        target
    }

    /// The opponent in our zone we are closest to (`sub_E188`).
    fn find_opponent_to_mark(&self, p: PlayerId) -> Option<PlayerId> {
        let me = &self.players[p.0];
        let mut range = me.pass_range();
        let mut marked = None;
        for q in self.opponents(p) {
            let them = &self.players[q.0];
            if them.entity.flags.falling() || them.entity.flags.offscreen() {
                continue;
            }
            if !me.is_in_zone(me.predict(&self.rom, &them.entity)) {
                continue;
            }
            let distance = me.distance_to_opponents[them.index];
            if range <= distance {
                continue;
            }
            marked = Some(q);
            range = distance;
        }
        marked
    }

    /// Vertical position between our zone and `mate` (`sub_E246`).
    fn position_between(&self, p: PlayerId, mate: PlayerId, target: &mut Point) {
        let me = &self.players[p.0];
        let mate_ahead = self.players[mate.0].attributes.pos.min(3) > me.attributes.pos;
        let (zone_min, zone_center, zone_max) = (me.zone_min.y, me.zone_center.y, me.zone_max.y);
        let towards_max = |t: &Point| {
            if t.y >= zone_center {
                zone_max
            } else {
                zone_center
            }
        };
        let towards_min = |t: &Point| {
            if t.y <= zone_center {
                zone_min
            } else {
                zone_center
            }
        };
        target.y += if mate_ahead == me.attacks_up() {
            towards_max(target)
        } else {
            towards_min(target)
        };
        target.y /= 2;
        me.clamp_y_to_zone(target);
    }

    /// Diagonal support position relative to `mate` (`sub_E2B8`).
    fn position_along(&self, p: PlayerId, mate: PlayerId, target: &mut Point) {
        let me = &self.players[p.0];
        let them = &self.players[mate.0];
        target.x = (target.x + me.zone_center.x) / 2;
        me.clamp_x_to_zone(target);
        let dx = (target.x - them.entity.pos.x).abs();
        let mate_behind = them.attributes.pos.min(3) <= me.attributes.pos;
        let zone_center = me.zone_center.y;
        // `Some(offset)`: step diagonally by `offset`; `None`: match the
        // mate's next y.
        let diagonal = match (mate_behind, me.attacks_up()) {
            (true, true) | (false, false) => (target.y >= zone_center).then_some(-dx),
            (true, false) | (false, true) => (target.y <= zone_center).then_some(dx),
        };
        match diagonal {
            Some(offset) => {
                target.y += offset;
                self.slide_into_zone(p, target);
            }
            None => {
                target.y = them.entity.pos.y + them.entity.velocity.y;
                me.clamp_y_to_zone(target);
            }
        }
    }

    /// Pulls an out-of-zone point back in, trading y overshoot for x (`sub_E344`).
    fn slide_into_zone(&self, p: PlayerId, target: &mut Point) {
        let me = &self.players[p.0];
        let edge = if target.y < me.zone_min.y {
            me.zone_min.y
        } else if target.y > me.zone_max.y {
            me.zone_max.y
        } else {
            return;
        };
        let overshoot = (target.y - edge).abs();
        target.y = edge;
        if target.x <= me.zone_center.x {
            target.x += overshoot;
        } else {
            target.x -= overshoot;
        }
        me.clamp_x_to_zone(target);
    }

    /// Attacking support from the tactics grid near the goal (`sub_E382`).
    fn support_position(&self, p: PlayerId, mate: PlayerId, target: &mut Point) {
        let me = &self.players[p.0];
        let them = &self.players[mate.0].entity;
        // Normalise to "left half, attacking up".
        if target.x >= 320 {
            target.x = 639 - target.x;
        }
        if !me.attacks_up() {
            target.y = 1151 - target.y;
        }
        if target.y >= 208 {
            // Not near goal yet: plain diagonal support.
            *target = them.pos + them.velocity;
            self.position_along(p, mate, target);
            return;
        }
        let column = (target.x - 32) / 32;
        let row = (target.y - 32) / 32;
        *target = self
            .rom
            .support_position(me.attributes.pos == 4, column, row);
        if them.pos.x >= 320 {
            target.x = 639 - target.x;
        }
        if !me.attacks_up() {
            target.y = 1151 - target.y;
        }
        me.clamp_x_to_zone(target);
    }

    // =========================================================================
    // Goalkeepers
    // =========================================================================

    /// Off-ball goalkeeper: shuffle along the goal line (`sub_E456`).
    fn ai_goalkeeper_support(&mut self, p: PlayerId) {
        let threat = self.holder.unwrap_or(EntityRef::Ball);
        let threat_entity = self.entity(threat);
        let me = &self.players[p.0];
        let mut target = me.lead_within_zone(&self.rom, threat_entity);
        let line = if me.attacks_up() {
            me.zone_max.y
        } else {
            me.zone_min.y
        };
        let thrown = threat == EntityRef::Ball || threat_entity.think == Think::Throw;
        let shifted_right = |t: Point| t.x + (line - t.y).abs();
        let shifted_left = |t: Point| t.x - (line - t.y).abs();

        let cover = if target.x < 272 {
            let x = shifted_right(target);
            Cover::Exactly(if x <= 352 { x.max(256) } else { 320 })
        } else if target.x > 368 {
            let x = shifted_left(target);
            Cover::Exactly(if x >= 288 { x.min(384) } else { 320 })
        } else {
            let close = if me.attacks_up() {
                target.y >= 1024
            } else {
                target.y <= 128
            };
            // Which way the threat is heading, mirrored for the top goal.
            let heading = match (me.attacks_up(), threat_entity.dir) {
                _ if !close => None,
                (true, 4) | (false, 0) => Some(Heading::Straight),
                (true, 3) | (false, 1) => Some(Heading::Right),
                (true, 5) | (false, 7) => Some(Heading::Left),
                _ => Some(Heading::Other),
            };
            match heading {
                None => Cover::Track,
                Some(Heading::Straight) if thrown => Cover::Track,
                Some(Heading::Right) => {
                    let x = shifted_right(target);
                    if x > 352 {
                        if shifted_left(target) >= 288 {
                            Cover::Halfway(shifted_left(target))
                        } else {
                            Cover::Track
                        }
                    } else if thrown {
                        Cover::Exactly(x)
                    } else {
                        Cover::Halfway(x)
                    }
                }
                Some(Heading::Left) => {
                    let x = shifted_left(target);
                    if x < 288 {
                        if shifted_right(target) <= 352 {
                            Cover::Halfway(shifted_right(target))
                        } else {
                            Cover::Track
                        }
                    } else if thrown {
                        Cover::Exactly(x)
                    } else {
                        Cover::Halfway(x)
                    }
                }
                Some(Heading::Straight | Heading::Other) => {
                    if shifted_left(target) < 288 {
                        let x = shifted_right(target);
                        if x > 352 {
                            Cover::Track
                        } else {
                            Cover::Halfway(x)
                        }
                    } else if shifted_right(target) <= 352 {
                        Cover::Track
                    } else {
                        Cover::Halfway(shifted_left(target))
                    }
                }
            }
        };
        target.x = cover.apply(target.x);
        target.y = line;
        me.clamp_x_to_zone(&mut target);
        self.players[p.0].target = target;
        self.start_move(p);
    }

    /// Active goalkeeper: dive at a shot or come off the line (`sub_E61A`).
    fn ai_goalkeeper_active(&mut self, p: PlayerId) {
        let holder_is_teammate = self.holder_player().is_some_and(|h| {
            self.players[h.0].attributes.initial_dir == self.players[p.0].attributes.initial_dir
        });
        let threat = self.holder.unwrap_or(EntityRef::Ball);
        let rom = self.rom.clone();

        if !holder_is_teammate {
            if !self.players[p.0].entity.flags.offscreen() {
                let jump_bonus = self.players[p.0].speed_jump_bonus(&rom);
                self.players[p.0].jump_bonus = jump_bonus;
                if jump_bonus * 8 >= self.players[p.0].distance_to_ball {
                    let target = self.players[p.0].lead_within_zone(&rom, self.entity(threat));
                    self.dive_at(p, target);
                    return;
                }
            }
            if self.entity(threat).velocity.is_zero() {
                let agr = self.players[p.0].skills().agr;
                if agr / 2 > self.rng.next_byte() {
                    // Hold position.
                    let me = &mut self.players[p.0];
                    let mut target = me.entity.pos;
                    me.clamp_x_to_zone(&mut target);
                    me.clamp_y_to_zone(&mut target);
                    me.target = target;
                    self.start_move(p);
                    return;
                }
            }
        }

        let threat_entity = self.entity(threat);
        let me = &self.players[p.0];
        let mut target = me.lead_within_zone(&rom, threat_entity);
        let line = if me.attacks_up() { 1120 } else { 32 };
        let thrown = threat == EntityRef::Ball || threat_entity.think == Think::Throw;
        let shifted_right = |t: Point| t.x + (line - t.y).abs();
        let shifted_left = |t: Point| t.x - (line - t.y).abs();

        let cover = if target.x < 272 {
            let x = shifted_right(target);
            Cover::Halfway(if x <= 368 { x.max(272) } else { 320 })
        } else if target.x > 368 {
            // The original's bounds look inverted here; kept as is.
            let x = shifted_left(target);
            Cover::Halfway(if x <= 272 { x } else { 320 })
        } else {
            let close = if me.attacks_up() {
                target.y >= 1024
            } else {
                target.y <= 128
            };
            let heading = match (me.attacks_up(), threat_entity.dir) {
                _ if !close => None,
                (true, 4) | (false, 0) => Some(Heading::Straight),
                (true, 3) | (false, 1) => Some(Heading::Right),
                (true, 5) | (false, 7) => Some(Heading::Left),
                _ => Some(Heading::Other),
            };
            match heading {
                None => Cover::Track,
                Some(Heading::Straight) if thrown => Cover::Track,
                Some(Heading::Right) => {
                    let x = shifted_right(target);
                    if x > 368 {
                        if shifted_left(target) >= 272 {
                            Cover::Quarter(shifted_left(target))
                        } else {
                            Cover::Track
                        }
                    } else if thrown {
                        Cover::Halfway(x)
                    } else {
                        Cover::Quarter(x)
                    }
                }
                Some(Heading::Left) => {
                    let x = shifted_left(target);
                    if x < 272 {
                        if shifted_right(target) <= 368 {
                            Cover::Quarter(shifted_right(target))
                        } else {
                            Cover::Track
                        }
                    } else if thrown {
                        Cover::Halfway(x)
                    } else {
                        Cover::Quarter(x)
                    }
                }
                Some(Heading::Straight | Heading::Other) => {
                    if shifted_left(target) < 272 {
                        let x = shifted_right(target);
                        if x > 368 {
                            Cover::Track
                        } else {
                            Cover::Quarter(x)
                        }
                    } else if shifted_right(target) <= 352 {
                        Cover::Track
                    } else {
                        Cover::Quarter(shifted_left(target))
                    }
                }
            }
        };
        target.x = cover.apply(target.x);
        target.y = (target.y + line) / 2;
        me.clamp_x_to_zone(&mut target);
        me.clamp_y_to_zone(&mut target);
        self.players[p.0].target = target;
        self.start_move(p);
    }
}

/// Where a ball or carrier near the goal is heading, from the keeper's view.
#[derive(Clone, Copy)]
enum Heading {
    Straight,
    Right,
    Left,
    Other,
}
