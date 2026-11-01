//! A player: an entity plus tactics, skills and match statistics.

use std::ops::{Deref, DerefMut, Index, IndexMut};

use super::entity::Entity;
use super::geometry::{DirBits, Point};
use super::roster::{Attributes, AttributesRef, Skill, Skills};
use crate::data::AtariRom;

/// Index into `Match::players`: 0..=8 is team 1, 9..=17 is team 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub usize);

impl PlayerId {
    pub const TEAM_SIZE: usize = 9;

    /// The nine players of a team, in formation order.
    pub fn team(team1: bool) -> impl Iterator<Item = PlayerId> {
        let base = if team1 { 0 } else { Self::TEAM_SIZE };
        (base..base + Self::TEAM_SIZE).map(PlayerId)
    }

    pub fn of(team1: bool, index: usize) -> PlayerId {
        PlayerId(if team1 {
            index
        } else {
            Self::TEAM_SIZE + index
        })
    }
}

/// The players on the pitch, team 1's then team 2's, looked up by
/// [`PlayerId`]. Otherwise a slice of players.
#[derive(Clone, Debug)]
pub struct Players(Vec<Player>);

impl Players {
    pub fn new(players: Vec<Player>) -> Self {
        Self(players)
    }

    /// Every player, team 1's first.
    pub fn ids(&self) -> impl Iterator<Item = PlayerId> {
        (0..self.0.len()).map(PlayerId)
    }
}

impl Deref for Players {
    type Target = [Player];

    fn deref(&self) -> &[Player] {
        &self.0
    }
}

impl DerefMut for Players {
    fn deref_mut(&mut self) -> &mut [Player] {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Players {
    type Item = &'a Player;
    type IntoIter = std::slice::Iter<'a, Player>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Players {
    type Item = &'a mut Player;
    type IntoIter = std::slice::IterMut<'a, Player>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl Index<PlayerId> for Players {
    type Output = Player;

    fn index(&self, id: PlayerId) -> &Player {
        &self.0[id.0]
    }
}

impl IndexMut<PlayerId> for Players {
    fn index_mut(&mut self, id: PlayerId) -> &mut Player {
        &mut self.0[id.0]
    }
}

/// Statistics shown after the match.
#[derive(Clone, Copy, Debug, Default)]
/// One player's statistics for the match. The original kept each in a byte,
/// so each wraps at 256.
pub struct MatchStats {
    pub time_in_possession: u8,
    pub goals_scored: u8,
    pub successful_tackles: u8,
    pub goals_saved: u8,
    pub substitutions: u8,
}

/// Adds one to a statistic, wrapping at 256.
pub fn count(stat: &mut u8) {
    *stat = stat.wrapping_add(1);
}

#[derive(Clone, Debug)]
pub struct Player {
    pub entity: Entity,
    /// Formation slot 0..=8 within the team.
    pub index: usize,
    pub distance_to_ball: i32,
    pub target: Point,
    /// Kick-off position.
    pub launch: Point,
    pub zone_center: Point,
    /// Top-left corner of the area this player patrols.
    pub zone_min: Point,
    /// Bottom-right corner of the area this player patrols.
    pub zone_max: Point,
    pub stats: MatchStats,
    /// A random byte rolled at each AI decision.
    pub random: i32,
    /// Extra airtime/reach from speed.
    pub jump_bonus: i32,
    pub attributes: Attributes,
    /// Skills saved while a power-up token overrides them.
    pub saved_skills: Skills,
    pub equipment_sprite: i32,
    pub equipment_backup: i32,
    pub equipment_backup_saved: i32,
    /// The squad record this player's attributes are loaded from.
    pub person_attributes: AttributesRef,
    pub distance_to_opponents: [i32; 9],
}

impl Player {
    pub fn read(rom: &AtariRom, address: i32, index: usize) -> Self {
        let p = address + Entity::SIZE;
        let point = |offset: i32| Point::new(rom.u16(p + offset), rom.u16(p + offset + 2));
        let byte = |offset: i32| rom.u8(p + offset);
        Self {
            entity: Entity::read(rom, address),
            index,
            distance_to_ball: rom.u16(p),
            target: point(2),
            launch: point(6),
            zone_center: point(10),
            // Stored as x1, x2, y1, y2.
            zone_min: Point::new(rom.u16(p + 14), rom.u16(p + 18)),
            zone_max: Point::new(rom.u16(p + 16), rom.u16(p + 20)),
            stats: MatchStats {
                time_in_possession: byte(22) as u8,
                goals_scored: byte(23) as u8,
                successful_tackles: byte(24) as u8,
                goals_saved: byte(25) as u8,
                substitutions: byte(26) as u8,
            },
            random: byte(27),
            jump_bonus: byte(28),
            attributes: Attributes::read(rom, p + 29),
            saved_skills: Skills::from_array(std::array::from_fn(|i| byte(40 + i as i32))),
            equipment_sprite: byte(48),
            equipment_backup: byte(49),
            equipment_backup_saved: byte(50),
            person_attributes: AttributesRef::from_address(rom.u32(p + 52)),
            distance_to_opponents: std::array::from_fn(|i| rom.u16(p + 56 + 2 * i as i32)),
        }
    }

    pub fn skills(&self) -> &Skills {
        &self.attributes.skills
    }

    pub fn is_goalkeeper(&self) -> bool {
        self.attributes.pos == 0
    }

    pub fn is_defender(&self) -> bool {
        self.attributes.pos == 1
    }

    /// One of the attackers (formation positions 3 and up).
    pub fn is_forward(&self) -> bool {
        self.attributes.pos >= 3
    }

    /// Whether this team attacks upwards (initial direction 0).
    pub fn attacks_up(&self) -> bool {
        self.attributes.initial_dir == 0
    }

    /// Reloads attributes from the squad record and clears match stats.
    pub fn reset_attributes(&mut self, source: &Attributes) {
        self.attributes = *source;
        self.equipment_sprite = 0;
        self.stats = MatchStats::default();
    }

    pub fn update_zone_center(&mut self) {
        self.zone_center = Point::new(
            ((self.zone_min.x + self.zone_max.x) / 2) & 0xFFFE,
            ((self.zone_min.y + self.zone_max.y) / 2) & 0xFFFE,
        );
    }

    // -- Skill-derived values ----------------------------------------------

    /// How far this player looks for a pass.
    pub fn pass_range(&self) -> i32 {
        self.skills().int * 2
    }

    /// Sets the delay before the next AI decision.
    pub fn reset_think_delay(&mut self, rom: &AtariRom) {
        self.entity.timer = rom.think_delay(self.skills().int);
    }

    pub fn speed_jump_bonus(&self, rom: &AtariRom) -> i32 {
        rom.jump_bonus(self.skills().spd)
    }

    /// Velocity bonus from speed: 0..=3 for thresholds 140/170/200.
    pub fn speed_bonus(&self) -> i32 {
        let spd = self.skills().spd;
        [140, 170, 200].iter().filter(|&&limit| spd > limit).count() as i32
    }

    // -- Targeting ------------------------------------------------------------

    /// Where `other` will be in a few frames; smarter players look further
    /// ahead. Results are reflected back off the walls.
    pub fn predict(&self, rom: &AtariRom, other: &Entity) -> Point {
        let shift = rom.prediction_shift(self.skills().int);
        let mut target = Point::new(
            other.pos.x + (other.velocity.x << shift),
            other.pos.y + (other.velocity.y << shift),
        );
        reflect_off_walls(&mut target);
        target
    }

    /// Leads `other` like [`predict`](Self::predict) but shortens the
    /// look-ahead until the vertical estimate stays inside this player's zone.
    pub fn lead_within_zone(&self, rom: &AtariRom, other: &Entity) -> Point {
        let mut target = other.pos;
        let mut shift = rom.prediction_shift(self.skills().int);
        loop {
            let dy = other.velocity.y << shift;
            target.y += dy;
            if shift == 0 {
                break;
            }
            let below_zone_ok = !self.attacks_up() || target.y <= self.zone_max.y;
            if below_zone_ok && target.y >= self.zone_min.y {
                break;
            }
            target.y -= dy;
            shift -= 1;
        }
        target.x += other.velocity.x << shift;
        target
    }

    pub fn clamp_x_to_zone(&self, point: &mut Point) {
        if point.x < self.zone_min.x {
            point.x = self.zone_min.x;
        } else if point.x > self.zone_max.x {
            point.x = self.zone_max.x;
        }
    }

    pub fn clamp_y_to_zone(&self, point: &mut Point) {
        if point.y < self.zone_min.y {
            point.y = self.zone_min.y;
        } else if point.y > self.zone_max.y {
            point.y = self.zone_max.y;
        }
    }

    pub fn is_in_zone(&self, point: Point) -> bool {
        (self.zone_min.x..=self.zone_max.x).contains(&point.x)
            && (self.zone_min.y..=self.zone_max.y).contains(&point.y)
    }

    /// Direction bits on each axis where `target` differs.
    pub fn dir_bits_towards_exact(&self, target: Point) -> DirBits {
        self.entity.dir_bits_towards_exact(target)
    }

    /// The direction to face `target`, or the way the team attacks when
    /// already there.
    pub fn dir_towards(&self, rom: &AtariRom, target: Point) -> i32 {
        let bits = self.entity.dir_bits_towards(target);
        if bits.has_direction() {
            rom.dir_from_bits(bits)
        } else {
            self.attributes.initial_dir
        }
    }

    // -- Damage and power-ups -------------------------------------------------

    /// Applies tackle damage from `attacker_pow` and wears every skill down
    /// (dropping equipment is done by the match).
    pub fn take_tackle_damage(&mut self, attacker_pow: i32) {
        let mut damage = (attacker_pow + 150 - self.skills().sta) / 16;
        if damage == 0 {
            damage = 1;
        }
        self.attributes.health = (self.attributes.health - damage).max(0);

        let wear = (damage / 2).max(1);
        self.attributes.skills.for_each_mut(|value| {
            *value -= wear;
            if *value <= 100 {
                *value = 100;
            }
        });
    }

    /// Saves all skills and overrides them with `value` (team tokens).
    pub fn boost_all_skills(&mut self, value: i32) {
        self.saved_skills = self.attributes.skills;
        self.attributes.skills = Skills::uniform(value);
    }

    pub fn restore_all_skills(&mut self) {
        self.attributes.skills = self.saved_skills;
        self.saved_skills = Skills::default();
    }

    pub fn slow_down(&mut self) {
        self.saved_skills.spd = self.attributes.skills.spd;
        self.attributes.skills.spd = 100;
    }

    pub fn restore_speed(&mut self) {
        self.attributes.skills.spd = self.saved_skills.spd;
        self.saved_skills.spd = 0;
    }

    /// Picks up equipment boosting `skill` to 250.
    pub fn equip(&mut self, skill: Skill) {
        let current = *self.attributes.skills.get_mut(skill);
        let saved = self.saved_skills.get_mut(skill);
        if *saved != 0 {
            // A token is active: remember both values, boost both.
            self.equipment_backup_saved = current;
            self.equipment_backup = *saved;
            *saved = 250;
        } else {
            self.equipment_backup = current;
        }
        *self.attributes.skills.get_mut(skill) = 250;
    }

    /// Reverts [`equip`](Self::equip).
    pub fn unequip(&mut self, skill: Skill) {
        let saved = self.saved_skills.get_mut(skill);
        if *saved != 0 {
            *self.attributes.skills.get_mut(skill) = self.equipment_backup_saved;
            *self.saved_skills.get_mut(skill) = self.equipment_backup;
        } else {
            *self.attributes.skills.get_mut(skill) = self.equipment_backup;
        }
    }
}

/// Mirrors a point that overshoots the pitch back inside.
fn reflect_off_walls(p: &mut Point) {
    if p.x <= 32 {
        p.x = 64 - p.x;
    } else if p.x >= 608 {
        p.x = 1216 - p.x;
    }
    if p.y <= 32 {
        p.y = 64 - p.y;
    } else if p.y >= 1120 {
        p.y = 2240 - p.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_ids_cover_both_halves() {
        let team1: Vec<_> = PlayerId::team(true).collect();
        let team2: Vec<_> = PlayerId::team(false).collect();
        assert_eq!(team1.first(), Some(&PlayerId(0)));
        assert_eq!(team2.last(), Some(&PlayerId(17)));
        assert_eq!(PlayerId::of(false, 8), PlayerId(17));
    }

    #[test]
    fn reflection_keeps_targets_on_the_pitch() {
        let mut p = Point::new(20, 1130);
        reflect_off_walls(&mut p);
        assert_eq!(p, Point::new(44, 1110));
    }
}
