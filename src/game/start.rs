//! What a match starts from: every entity's record (the ball, the pitch's
//! furniture, items, indicators), the eighteen players' records with their
//! kick-off places and the zones they patrol, and the roster of attributes
//! and names.
//!
//! Read from the ST disk ([`MatchStart::read`]) or an asset pack, where
//! scripts, behaviours and sprite banks are written by name
//! ([`StartTable`]).

use serde::{Deserialize, Serialize};

use super::entity::{Entity, EntityId, Flags, Gfx, Think};
use super::geometry::Point;
use super::player::{MatchStats, Player};
use super::roster::{Attributes, AttributesRef, Roster, Skills};
use crate::data::animations::{snake_case, Animations, ScriptRef};
use crate::data::AtariRom;

/// Players on the pitch, both teams.
const PLAYERS: usize = 18;

#[derive(Clone, Debug, Default)]
pub struct MatchStart {
    entities: Vec<Entity>,
    players: Vec<Player>,
    pub roster: Option<Roster>,
}

impl MatchStart {
    pub fn read(rom: &AtariRom) -> Self {
        let mut players = Vec::new();
        let mut entities = Vec::new();
        for (id, address) in EntityId::records() {
            match id {
                EntityId::Player(i) => players.push(Player::read(rom, address, i as usize % 9)),
                _ => entities.push(Entity::read(rom, address)),
            }
        }
        Self {
            entities,
            players,
            roster: Some(Roster::read(rom)),
        }
    }

    /// A copy of an entity's starting record.
    pub fn entity(&self, id: EntityId) -> Entity {
        self.entities
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("no starting record for {id:?}"))
            .clone()
    }

    /// The players, team 1's then team 2's.
    pub fn players(&self) -> Vec<Player> {
        self.players.clone()
    }

    pub fn roster(&self) -> Roster {
        self.roster.clone().expect("a roster")
    }

    /// The records as a pack writes them, naming scripts in `animations`.
    pub fn table(&self, animations: &Animations) -> StartTable {
        StartTable {
            entities: self
                .entities
                .iter()
                .map(|e| EntityStart::new(e, animations))
                .collect(),
            players: self
                .players
                .iter()
                .map(|p| PlayerStart::new(p, animations))
                .collect(),
            roster: self.roster(),
        }
    }
}

/// Starting records as an asset pack holds them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartTable {
    pub entities: Vec<EntityStart>,
    /// Team 1's nine, then team 2's.
    pub players: Vec<PlayerStart>,
    pub roster: Roster,
}

impl StartTable {
    /// The records, with scripts found in `animations`.
    pub fn resolve(&self, animations: &Animations) -> Result<MatchStart, String> {
        let entities: Vec<Entity> = self
            .entities
            .iter()
            .map(|e| e.entity(animations))
            .collect::<Result<_, _>>()?;
        for (id, _) in EntityId::records() {
            let count = entities.iter().filter(|e| e.id == id).count();
            if !matches!(id, EntityId::Player(_)) && count != 1 {
                return Err(format!(
                    "entities must have one \"{}\"",
                    snake_case(&format!("{id:?}"))
                ));
            }
        }
        if self.players.len() != PLAYERS {
            return Err(format!("players must have {PLAYERS} players"));
        }
        let players = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| p.player(i, animations))
            .collect::<Result<_, _>>()?;
        check_roster(&self.roster)?;
        Ok(MatchStart {
            entities,
            players,
            roster: Some(self.roster.clone()),
        })
    }
}

fn check_roster(roster: &Roster) -> Result<(), String> {
    let squads = [
        roster.attributes_team1.len(),
        roster.attributes_team2.len(),
        roster.persons_team1.len(),
        roster.persons_team2.len(),
    ];
    if squads.iter().any(|&n| n != Roster::SQUAD_SIZE) {
        return Err(format!(
            "each team's attributes and persons in roster must have {} entries",
            Roster::SQUAD_SIZE
        ));
    }
    if roster.attributes_market.len() != 18 {
        return Err("roster.attributes_market must have 18 entries".into());
    }
    Ok(())
}

/// A name, or a number where the original holds something this port has
/// no name for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Named {
    Name(String),
    Number(i64),
}

const THINKS: [(Think, &str); 13] = [
    (Think::StartMove, "start_move"),
    (Think::Move, "move"),
    (Think::StandingTackle, "standing_tackle"),
    (Think::Throw, "throw"),
    (Think::Stand, "stand"),
    (Think::Jump, "jump"),
    (Think::TackleRecover, "tackle_recover"),
    (Think::GoalieDive, "goalie_dive"),
    (Think::SlideTackle, "slide_tackle"),
    (Think::Idle, "idle"),
    (Think::Coins, "coins"),
    (Think::Token, "token"),
    (Think::Equipment, "equipment"),
];

const GFX: [(Gfx, &str); 4] = [
    (Gfx::Entities, "entities"),
    (Gfx::Players, "players"),
    (Gfx::BigBall, "big_ball"),
    (Gfx::Pitch, "pitch"),
];

/// The flag byte's bits, lowest first. Bit 3 is a player's team (set for
/// team 1), the ball's charge, a medic's colour.
const FLAGS: [&str; 8] = [
    "busy",
    "jumping",
    "throwing",
    "bit3",
    "falling",
    "tackling",
    "offscreen",
    "invisible",
];

fn name_of<T: PartialEq + Copy>(list: &[(T, &str)], value: T) -> Option<String> {
    list.iter()
        .find(|&&(v, _)| v == value)
        .map(|&(_, n)| n.to_string())
}

fn named<T: Copy>(list: &[(T, &str)], what: &str, name: &Named) -> Result<Option<T>, String> {
    match name {
        Named::Number(_) => Ok(None),
        Named::Name(name) => list
            .iter()
            .find(|&&(_, n)| n == name)
            .map(|&(v, _)| Some(v))
            .ok_or_else(|| {
                let names: Vec<_> = list.iter().map(|&(_, n)| n).collect();
                format!("\"{name}\" is not a {what}: one of {}", names.join(", "))
            }),
    }
}

/// An entity's starting record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityStart {
    pub id: String,
    /// Which sprites it is drawn from.
    pub gfx: Named,
    /// What it does each frame.
    pub think: Named,
    /// The animation script it starts on.
    pub script: Named,
    pub size: [i32; 2],
    pub pos: [i32; 2],
    pub velocity: [i32; 2],
    pub script_index: i32,
    /// 0 up, then clockwise.
    pub dir: i32,
    pub timer: i32,
    pub sprite: i32,
    pub flags: Vec<String>,
    /// From its position to where its sprite is drawn.
    pub origin: [i32; 2],
}

impl EntityStart {
    fn new(e: &Entity, animations: &Animations) -> Self {
        let pair = |p: Point| [p.x, p.y];
        Self {
            id: snake_case(&format!("{:?}", e.id)),
            gfx: match e.gfx {
                Gfx::Unknown(pointer) => Named::Number(i64::from(pointer)),
                gfx => Named::Name(name_of(&GFX, gfx).expect("a named bank")),
            },
            think: match e.think {
                Think::Unknown(pointer) => Named::Number(i64::from(pointer)),
                think => Named::Name(name_of(&THINKS, think).expect("a named think")),
            },
            script: match animations.name_at(e.script) {
                Some(name) => Named::Name(name.to_string()),
                None => Named::Number(i64::from(e.script.0)),
            },
            size: [e.width, e.height],
            pos: pair(e.pos),
            velocity: pair(e.velocity),
            script_index: e.script_index,
            dir: e.dir,
            timer: e.timer,
            sprite: e.sprite,
            flags: (0..8)
                .filter(|bit| e.flags.bits() & (1 << bit) != 0)
                .map(|bit| FLAGS[bit].to_string())
                .collect(),
            origin: pair(e.origin),
        }
    }

    fn entity(&self, animations: &Animations) -> Result<Entity, String> {
        let id = EntityId::records()
            .into_iter()
            .map(|(id, _)| id)
            .find(|&id| snake_case(&format!("{id:?}")) == self.id)
            .ok_or_else(|| format!("\"{}\" is not an entity", self.id))?;
        let number = |n: &Named| match n {
            Named::Number(n) => *n,
            Named::Name(_) => unreachable!("names are looked up"),
        };
        let gfx = named(&GFX, "sprite bank", &self.gfx)?
            .unwrap_or_else(|| Gfx::Unknown(number(&self.gfx) as u32));
        let think = named(&THINKS, "think", &self.think)?
            .unwrap_or_else(|| Think::Unknown(number(&self.think) as i32));
        let script = match &self.script {
            Named::Number(n) => ScriptRef(*n as i32),
            Named::Name(name) => animations
                .named(name)
                .ok_or_else(|| format!("{}: there is no script called \"{name}\"", self.id))?,
        };
        let mut flags = 0;
        for flag in &self.flags {
            let bit = FLAGS
                .iter()
                .position(|f| f == flag)
                .ok_or_else(|| format!("\"{flag}\" is not a flag: one of {}", FLAGS.join(", ")))?;
            flags |= 1 << bit;
        }
        let point = |[x, y]: [i32; 2]| Point::new(x, y);
        Ok(Entity {
            id,
            gfx,
            think,
            script,
            width: self.size[0],
            height: self.size[1],
            pos: point(self.pos),
            velocity: point(self.velocity),
            script_index: self.script_index,
            dir: self.dir,
            timer: self.timer,
            sprite: self.sprite,
            flags: Flags::from_bits(flags),
            origin: point(self.origin),
        })
    }
}

/// A player's starting record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerStart {
    pub entity: EntityStart,
    pub distance_to_ball: i32,
    pub target: [i32; 2],
    /// Where they stand at kick-off.
    pub launch: [i32; 2],
    pub zone_center: [i32; 2],
    /// The area they patrol: top-left and bottom-right.
    pub zone_min: [i32; 2],
    pub zone_max: [i32; 2],
    /// Time in possession, goals scored, tackles, goals saved, substitutions.
    pub stats: [u8; 5],
    pub random: i32,
    pub jump_bonus: i32,
    pub attributes: Attributes,
    pub saved_skills: Skills,
    /// The equipment worn, and the skill values it replaced.
    pub equipment: [i32; 3],
    /// The squad record their attributes come from.
    pub person_attributes: AttributesRef,
    pub distance_to_opponents: [i32; 9],
}

impl PlayerStart {
    fn new(p: &Player, animations: &Animations) -> Self {
        let pair = |p: Point| [p.x, p.y];
        let s = &p.stats;
        Self {
            entity: EntityStart::new(&p.entity, animations),
            distance_to_ball: p.distance_to_ball,
            target: pair(p.target),
            launch: pair(p.launch),
            zone_center: pair(p.zone_center),
            zone_min: pair(p.zone_min),
            zone_max: pair(p.zone_max),
            stats: [
                s.time_in_possession,
                s.goals_scored,
                s.successful_tackles,
                s.goals_saved,
                s.substitutions,
            ],
            random: p.random,
            jump_bonus: p.jump_bonus,
            attributes: p.attributes,
            saved_skills: p.saved_skills,
            equipment: [
                p.equipment_sprite,
                p.equipment_backup,
                p.equipment_backup_saved,
            ],
            person_attributes: p.person_attributes,
            distance_to_opponents: p.distance_to_opponents,
        }
    }

    fn player(&self, number: usize, animations: &Animations) -> Result<Player, String> {
        let entity = self.entity.entity(animations)?;
        if entity.id != EntityId::Player(number as u8) {
            return Err(format!(
                "player {number}'s entity must be \"player_{number}\", not \"{}\"",
                self.entity.id
            ));
        }
        let point = |[x, y]: [i32; 2]| Point::new(x, y);
        let [time_in_possession, goals_scored, successful_tackles, goals_saved, substitutions] =
            self.stats;
        Ok(Player {
            entity,
            index: number % 9,
            distance_to_ball: self.distance_to_ball,
            target: point(self.target),
            launch: point(self.launch),
            zone_center: point(self.zone_center),
            zone_min: point(self.zone_min),
            zone_max: point(self.zone_max),
            stats: MatchStats {
                time_in_possession,
                goals_scored,
                successful_tackles,
                goals_saved,
                substitutions,
            },
            random: self.random,
            jump_bonus: self.jump_bonus,
            attributes: self.attributes,
            saved_skills: self.saved_skills,
            equipment_sprite: self.equipment[0],
            equipment_backup: self.equipment[1],
            equipment_backup_saved: self.equipment[2],
            person_attributes: self.person_attributes,
            distance_to_opponents: self.distance_to_opponents,
        })
    }
}
