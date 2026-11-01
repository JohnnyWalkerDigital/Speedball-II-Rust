//! Injuries: the stretcher sequence and substitution.

use super::events::Sound;
use super::geometry::{Point, Side};
use super::player::{count, PlayerId};
use super::roster::PersonRef;
use super::state::{EntityRef, Match, PITCH_CENTER};
use super::text::{TextId, POSITION_NAMES};
use crate::data::animations::{Animation, Script};

/// Medic 0 carries the front of the stretcher, medic 1 the back.
const FRONT: usize = 0;
const REAR: usize = 1;
const MEDIC_SPEED: i32 = 2;

/// Where the stretcher sequence has got to, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum InjuryStage {
    /// No one has been injured yet.
    #[default]
    NotStarted,
    Approaching,
    /// The first medic has arrived.
    Arrived,
    Lifting,
    CarryingOff,
}

impl InjuryStage {
    fn next(self) -> Self {
        match self {
            InjuryStage::NotStarted => InjuryStage::Approaching,
            InjuryStage::Approaching => InjuryStage::Arrived,
            InjuryStage::Arrived => InjuryStage::Lifting,
            InjuryStage::Lifting | InjuryStage::CarryingOff => InjuryStage::CarryingOff,
        }
    }
}

/// Stretcher sequence state while a player is carried off.
#[derive(Clone, Copy, Debug, Default)]
pub struct Injury {
    pub player: Option<PlayerId>,
    /// Where the front medic stands, relative to the injured player's x.
    front_medic_offset: Point,
    /// Where the rear medic is heading.
    rear_medic_target: Point,
    stage: InjuryStage,
}

impl Injury {
    #[cfg(test)]
    pub fn stage(&self) -> InjuryStage {
        self.stage
    }

    /// Whether the medics are lifting the player or carrying them off. Until
    /// then the panel shows plain "INJURED"; from then, the player's
    /// position too.
    pub fn medics_lifting(&self) -> bool {
        self.stage >= InjuryStage::Lifting
    }
}

impl Match {
    /// Stops play and sends the medics on.
    pub(crate) fn start_injury(&mut self, p: PlayerId) {
        if let Some(current) = self.injury.player {
            if self.players[current].entity.flags.busy() {
                return;
            }
        }
        let side = self.side_of(p);
        *self.controlled.get_mut(side) = p;

        let entity = &mut self.players[p].entity;
        entity.script = self.rom.script(Script::PlayerOnGround);
        entity.script_index = 0;
        let mut pos = Point::new(entity.pos.x & 0xFFFE, entity.pos.y & 0xFFFE);
        pos.x = pos.x.clamp(48, 592);
        pos.y = pos.y.clamp(48, 1104);
        entity.pos = pos;

        self.holder = Some(EntityRef::TerrainCenter);
        self.props.terrain_center.flags.set_busy(true);
        self.props.terrain_center.pos = pos;

        let on_left = pos.x <= 320;
        let (rear_x, front_x, offset_x, rear_shift) = if on_left {
            (592, 560, -26, 32)
        } else {
            (48, 80, 26, -32)
        };
        let offset_y = if pos.y <= 576 { pos.y + 32 } else { pos.y - 32 };
        self.injury = Injury {
            player: Some(p),
            front_medic_offset: Point::new(offset_x, offset_y),
            rear_medic_target: Point::new(pos.x + rear_shift, pos.y),
            stage: InjuryStage::Approaching,
        };
        let medics = &mut self.props.medics;
        medics[REAR].pos = Point::new(rear_x, pos.y);
        medics[FRONT].pos = Point::new(front_x, pos.y);
        for medic in medics.iter_mut() {
            medic.flags.set_invisible(false);
        }

        self.clock_paused = true;
        self.player_injured = true;
        self.add_score(side.opponent(), 10);
        self.play_sound(Sound::SILENCE_CHANNEL_0);
        self.play_sound(Sound::INJURY);
        if !self.started_attacking_up(p) {
            self.play_sound(Sound::CROWD_CHEER);
        }
    }

    pub(crate) fn animate_medics(&mut self) {
        if self.props.medics[FRONT].flags.invisible() {
            return;
        }
        let Some(injured) = self.injury.player else {
            return;
        };
        match self.injury.stage {
            InjuryStage::NotStarted => {}
            InjuryStage::Approaching => self.medics_approach(injured),
            InjuryStage::Arrived => self.medics_arrive(injured),
            InjuryStage::Lifting => self.medics_lift(injured),
            InjuryStage::CarryingOff => self.medics_carry_off(injured),
        }
    }

    /// Steers the rear medic towards its target; `false` once it is there.
    fn steer_rear_medic(&mut self, restart_animation: bool) -> bool {
        let rom = &self.rom;
        let rear = &mut self.props.medics[REAR];
        let bits = rear.dir_bits_towards_exact(self.injury.rear_medic_target);
        if !bits.has_direction() {
            return false;
        }
        rear.dir = rom.dir_from_bits(bits);
        rear.set_animation(rom, Animation::MedicWalk, MEDIC_SPEED);
        if restart_animation {
            rear.script_index = 0;
        }
        true
    }

    fn front_medic_target(&self, injured: PlayerId) -> Point {
        let mut target = self.injury.front_medic_offset;
        target.x += self.players[injured].entity.pos.x;
        target
    }

    fn steer_front_medic(&mut self, injured: PlayerId) {
        let rom = &self.rom;
        let target = self.front_medic_target(injured);
        let front = &mut self.props.medics[FRONT];
        let dir = rom.dir_from_bits(front.dir_bits_towards_exact(target));
        if dir < 0 {
            front.velocity = Point::ZERO;
        } else {
            front.dir = dir;
            front.set_animation(rom, Animation::MedicWalk, MEDIC_SPEED);
            front.script_index = 0;
        }
    }

    fn medics_approach(&mut self, injured: PlayerId) {
        if self.steer_rear_medic(true) {
            self.steer_front_medic(injured);
            return;
        }
        let injured_pos = self.players[injured].entity.pos;
        let injury = &mut self.injury;
        injury.rear_medic_target.y = injured_pos.y;
        if injured_pos.x <= 320 {
            injury.front_medic_offset.x = -26;
            injury.rear_medic_target.x -= 4;
        } else {
            injury.front_medic_offset.x = 26;
            injury.rear_medic_target.x += 4;
        }
        injury.stage = injury.stage.next();
        self.medics_arrive(injured);
    }

    fn medics_arrive(&mut self, injured: PlayerId) {
        if self.steer_rear_medic(false) {
            self.steer_front_medic(injured);
            return;
        }
        self.show_message(Some(TextId::Injured));
        let name = POSITION_NAMES[self.players[injured].index];
        self.text_panel.text_mut(TextId::Injured).overwrite(2, name);

        self.props.medics[REAR].velocity.y = 0;
        self.injury.front_medic_offset.y = self.injury.rear_medic_target.y;
        let (speed, script, exit_x) = if self.players[injured].entity.pos.x <= 320 {
            (MEDIC_SPEED, self.rom.script(Script::MedicCarryRight), 592)
        } else {
            (-MEDIC_SPEED, self.rom.script(Script::MedicCarryLeft), 48)
        };
        self.players[injured].entity.velocity.x = speed;
        let rear = &mut self.props.medics[REAR];
        rear.velocity.x = speed;
        rear.script = script;
        self.injury.rear_medic_target.x = exit_x;
        self.injury.stage = self.injury.stage.next();
        self.medics_lift(injured);
    }

    fn medics_lift(&mut self, injured: PlayerId) {
        let rom = &self.rom;
        let target = self.front_medic_target(injured);
        let front = &mut self.props.medics[FRONT];
        if target.y != front.pos.y {
            let dir = rom
                .dir_from_bits(front.dir_bits_towards_exact(target))
                .max(0);
            front.dir = dir;
            front.set_animation(rom, Animation::MedicWalk, MEDIC_SPEED);
            return;
        }
        // The original reads the seed without advancing it, so this does not
        // change the random sequence.
        if self.rng.high_byte() & (1 << 5) != 0 {
            self.play_sound(Sound::ICE_CREAM);
        }
        self.players[injured].entity.script = self.rom.script(Script::PlayerCarried);
        let rear_walks_left =
            self.props.medics[REAR].script == self.rom.script(Script::MedicCarryLeft);
        let (front_script, rear_script) = if rear_walks_left {
            (
                self.rom.script(Script::MedicLiftB),
                self.rom.script(Script::MedicLiftA),
            )
        } else {
            (
                self.rom.script(Script::MedicLiftA),
                self.rom.script(Script::MedicLiftB),
            )
        };
        self.props.medics[FRONT].script = front_script;
        self.props.medics[REAR].script = rear_script;
        self.injury.stage = self.injury.stage.next();
        self.medics_carry_off(injured);
    }

    fn medics_carry_off(&mut self, injured: PlayerId) {
        self.props.medics[FRONT].velocity = self.props.medics[REAR].velocity;
        let front_x = self.props.medics[FRONT].pos.x + 16;
        if front_x >= self.camera.position().x && front_x - 352 <= self.camera.position().x {
            return; // Still on screen.
        }

        self.play_sound(Sound::SILENCE);
        self.player_injured = false;
        self.substitute(injured);
        self.show_message(Some(TextId::Vs));
        self.play_sound(Sound::GET_READY);
        let substitutions = self.players[injured].stats.substitutions;
        self.reload_player_attributes(injured);

        // The substitute runs on from the touchline.
        let player = &mut self.players[injured];
        player.stats.substitutions = substitutions;
        count(&mut player.stats.substitutions);
        player.entity.timer = 25;
        player.target = PITCH_CENTER;
        player.entity.flags.set_busy(false);
        player.entity.pos.x = if player.entity.pos.x <= 320 { 48 } else { 592 };
        player.entity.pos.y = if player.attacks_up() {
            576 + 32
        } else {
            576 - 32
        };
        self.start_move(injured);

        self.injury.player = None;
        self.props.terrain_center.flags.set_busy(false);
        for medic in &mut self.props.medics {
            medic.flags.set_invisible(true);
        }
    }

    /// Saves the injured player's worn-down skills, then rotates the squad:
    /// the first substitute comes on and the injured player goes to the
    /// back of the bench.
    fn substitute(&mut self, injured: PlayerId) {
        let player = &self.players[injured];
        let skills = player.attributes.skills;
        let index = player.index;
        self.roster.attributes_mut(player.person_attributes).skills = skills;

        let side = self.side_of(injured);
        let slot = |i: usize| match side {
            Side::Team1 => PersonRef::Team1(i),
            Side::Team2 => PersonRef::Team2(i),
        };
        let injured_slot = slot(index);
        let attributes = self.roster.person(injured_slot).attributes;
        self.roster.attributes_mut(attributes).round_skills_down();

        let roster = &mut self.roster;
        roster.copy_person(injured_slot, PersonRef::Temp);
        roster.copy_person(slot(9), injured_slot);
        roster.copy_person(slot(10), slot(9));
        roster.copy_person(slot(11), slot(10));
        roster.copy_person(PersonRef::Temp, slot(11));
    }
}
