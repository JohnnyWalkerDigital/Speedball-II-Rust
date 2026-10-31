//! The match's animation scripts: runs of sprite numbers and control words
//! that entities step through a frame at a time, and the per-direction
//! animations that pick one for each of the eight directions.
//!
//! In the original they are one block of memory, and entities point into
//! it. Here they are named scripts, laid out one after another in the same
//! order, so an entity's place in a script is still a position in that
//! block: unedited, every position is the original address. A read past
//! the block (an entity carries its place over when its script changes)
//! reads the original memory there, as the original game did.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use super::addr;
use super::lookups::DIRECTIONS;
use super::rom::AtariRom;
use crate::game::entity::EntityId;

/// The scripts' block of memory.
const BLOCK_START: i32 = 0x54C6;
const BLOCK_END: i32 = 0x5E26;
/// The per-direction animations, one after another in this order.
const ANIMATIONS_START: i32 = addr::ANIMATIONS;

/// A position in the scripts' memory: where an entity's script starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScriptRef(pub i32);

/// A per-direction animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Animation {
    Stand,
    SlideTackle,
    StandingTackle,
    Run,
    Throw,
    Jump,
    KnockedDown,
    Catch,
    GoalieCatchTop,
    GoalieCatchBottom,
    GoalieStandTop,
    GoalieRunTop,
    GoalieStandBottom,
    GoalieRunBottom,
    GoalieDiveTop,
    GoalieDiveBottom,
    Celebrate,
    CelebrateScorer,
    MedicWalk,
}

impl Animation {
    pub const ALL: [Animation; 19] = [
        Animation::Stand,
        Animation::SlideTackle,
        Animation::StandingTackle,
        Animation::Run,
        Animation::Throw,
        Animation::Jump,
        Animation::KnockedDown,
        Animation::Catch,
        Animation::GoalieCatchTop,
        Animation::GoalieCatchBottom,
        Animation::GoalieStandTop,
        Animation::GoalieRunTop,
        Animation::GoalieStandBottom,
        Animation::GoalieRunBottom,
        Animation::GoalieDiveTop,
        Animation::GoalieDiveBottom,
        Animation::Celebrate,
        Animation::CelebrateScorer,
        Animation::MedicWalk,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Animation::Stand => "stand",
            Animation::SlideTackle => "slide_tackle",
            Animation::StandingTackle => "standing_tackle",
            Animation::Run => "run",
            Animation::Throw => "throw",
            Animation::Jump => "jump",
            Animation::KnockedDown => "knocked_down",
            Animation::Catch => "catch",
            Animation::GoalieCatchTop => "goalie_catch_top",
            Animation::GoalieCatchBottom => "goalie_catch_bottom",
            Animation::GoalieStandTop => "goalie_stand_top",
            Animation::GoalieRunTop => "goalie_run_top",
            Animation::GoalieStandBottom => "goalie_stand_bottom",
            Animation::GoalieRunBottom => "goalie_run_bottom",
            Animation::GoalieDiveTop => "goalie_dive_top",
            Animation::GoalieDiveBottom => "goalie_dive_bottom",
            Animation::Celebrate => "celebrate",
            Animation::CelebrateScorer => "celebrate_scorer",
            Animation::MedicWalk => "medic_walk",
        }
    }
}

/// A script the code starts by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    BallThrownHigh,
    BallThrown,
    BallHeldJumping,
    BallHeld,
    BallInMultiplier,
    BallLaunch,
    PlayerOnGround,
    PlayerCarried,
    MedicCarryLeft,
    MedicCarryRight,
    MedicLiftA,
    MedicLiftB,
}

impl Script {
    const ALL: [Script; 12] = [
        Script::BallThrownHigh,
        Script::BallThrown,
        Script::BallHeldJumping,
        Script::BallHeld,
        Script::BallInMultiplier,
        Script::BallLaunch,
        Script::PlayerOnGround,
        Script::PlayerCarried,
        Script::MedicCarryLeft,
        Script::MedicCarryRight,
        Script::MedicLiftA,
        Script::MedicLiftB,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Script::BallThrownHigh => "ball_thrown_high",
            Script::BallThrown => "ball_thrown",
            Script::BallHeldJumping => "ball_held_jumping",
            Script::BallHeld => "ball_held",
            Script::BallInMultiplier => "ball_in_multiplier",
            Script::BallLaunch => "ball_launch",
            Script::PlayerOnGround => "player_on_ground",
            Script::PlayerCarried => "player_carried",
            Script::MedicCarryLeft => "medic_carry_left",
            Script::MedicCarryRight => "medic_carry_right",
            Script::MedicLiftA => "medic_lift_a",
            Script::MedicLiftB => "medic_lift_b",
        }
    }

    fn address(self) -> i32 {
        match self {
            Script::BallThrownHigh => addr::SCRIPT_BALL_THROWN_HIGH,
            Script::BallThrown => addr::SCRIPT_BALL_THROWN,
            Script::BallHeldJumping => addr::SCRIPT_BALL_HELD_JUMPING,
            Script::BallHeld => addr::SCRIPT_BALL_HELD,
            Script::BallInMultiplier => addr::SCRIPT_BALL_IN_MULTIPLIER,
            Script::BallLaunch => addr::SCRIPT_BALL_LAUNCH,
            Script::PlayerOnGround => addr::SCRIPT_PLAYER_ON_GROUND,
            Script::PlayerCarried => addr::SCRIPT_PLAYER_CARRIED,
            Script::MedicCarryLeft => addr::SCRIPT_MEDIC_CARRY_LEFT,
            Script::MedicCarryRight => addr::SCRIPT_MEDIC_CARRY_RIGHT,
            Script::MedicLiftA => addr::SCRIPT_MEDIC_LIFT_A,
            Script::MedicLiftB => addr::SCRIPT_MEDIC_LIFT_B,
        }
    }
}

/// The control words a script can hold between its sprite numbers.
const CONTROLS: [(u16, &str); 5] = [
    // Back to the start.
    (0xFFFF, "loop"),
    // Stay on the last sprite.
    (0xFFFE, "hold"),
    // A one-shot action (a throw, a tackle) is over.
    (0xFFFD, "end_action"),
    // Disappear.
    (0xFFFC, "hide"),
    // A knocked-down player has landed: stop and check their health.
    (0xFFFB, "end_fall"),
];

/// A sprite number, or a control word by name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Frame {
    Sprite(u16),
    Control(String),
}

impl Frame {
    fn from_word(word: u16) -> Self {
        match CONTROLS.iter().find(|&&(w, _)| w == word) {
            Some(&(_, name)) => Frame::Control(name.to_string()),
            None => Frame::Sprite(word),
        }
    }

    fn word(&self) -> Result<u16, String> {
        match self {
            Frame::Sprite(word) => Ok(*word),
            Frame::Control(name) => CONTROLS
                .iter()
                .find(|&&(_, n)| n == name)
                .map(|&(w, _)| w)
                .ok_or_else(|| {
                    let names: Vec<_> = CONTROLS.iter().map(|&(_, n)| n).collect();
                    format!("\"{name}\" is not one of {}", names.join(", "))
                }),
        }
    }
}

/// A script and the names it is started by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedScript {
    pub names: Vec<String>,
    pub frames: Vec<Frame>,
}

/// An animation's script for each direction (0 up, then clockwise), by
/// name; `null` for none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectionScripts {
    pub name: String,
    pub directions: Vec<Option<String>>,
}

/// The scripts, in memory order, and the animations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationTable {
    pub scripts: Vec<NamedScript>,
    pub animations: Vec<DirectionScripts>,
}

/// [`AnimationTable`] laid out as the game reads it.
#[derive(Clone, Debug, Default)]
pub struct Animations {
    table: AnimationTable,
    words: Vec<u16>,
    by_name: HashMap<String, ScriptRef>,
    /// Each script's first name, by where it starts.
    first_names: HashMap<ScriptRef, String>,
    directions: Vec<Vec<ScriptRef>>,
    scripts: Vec<ScriptRef>,
}

impl PartialEq for Animations {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table
    }
}

impl AnimationTable {
    pub fn read(rom: &AtariRom) -> Self {
        let in_block = |address: i32| (BLOCK_START..BLOCK_END).contains(&address);
        // Every place a script starts, and what starts it there.
        let mut starts: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        let mut name = |address: i32, name: String| {
            assert!(in_block(address), "{name} is outside the scripts");
            starts.entry(address).or_default().push(name);
        };
        for script in Script::ALL {
            name(script.address(), script.name().to_string());
        }
        let animations = Animation::ALL
            .iter()
            .enumerate()
            .map(|(i, animation)| {
                let list = ANIMATIONS_START + i as i32 * 32;
                let directions = (0..DIRECTIONS as i32)
                    .map(|dir| {
                        let address = rom.u32(list + dir * 4);
                        if address == 0 {
                            return None;
                        }
                        let script = format!("{}_{dir}", animation.name());
                        name(address, script.clone());
                        Some(script)
                    })
                    .collect();
                DirectionScripts {
                    name: animation.name().to_string(),
                    directions,
                }
            })
            .collect();
        for (id, address) in EntityId::records() {
            let script = rom.u32(address + 12);
            if in_block(script) {
                name(script, snake_case(&format!("{id:?}")));
            }
        }
        assert_eq!(
            starts.keys().next(),
            Some(&BLOCK_START),
            "the block starts with a script"
        );
        let bounds: Vec<i32> = starts.keys().copied().chain([BLOCK_END]).collect();
        let scripts = starts
            .into_iter()
            .zip(bounds.windows(2))
            .map(|((_, names), pair)| NamedScript {
                names,
                frames: (pair[0]..pair[1])
                    .step_by(2)
                    .map(|at| Frame::from_word(rom.u16(at) as u16))
                    .collect(),
            })
            .collect();
        Self {
            scripts,
            animations,
        }
    }

    /// Lays the scripts out, finding what would stop the match using them.
    pub fn lay_out(self) -> Result<Animations, String> {
        let mut words = Vec::new();
        let mut by_name = HashMap::new();
        let mut first_names = HashMap::new();
        for script in &self.scripts {
            let start = ScriptRef(BLOCK_START + words.len() as i32 * 2);
            if script.names.is_empty() {
                return Err("every script needs a name".into());
            }
            first_names.insert(start, script.names[0].clone());
            for name in &script.names {
                if by_name.insert(name.clone(), start).is_some() {
                    return Err(format!("two scripts are called \"{name}\""));
                }
            }
            for frame in &script.frames {
                words.push(
                    frame
                        .word()
                        .map_err(|e| format!("{}: {e}", script.names[0]))?,
                );
            }
        }
        let find = |name: &str| {
            by_name
                .get(name)
                .copied()
                .ok_or_else(|| format!("there is no script called \"{name}\""))
        };
        if self.animations.len() != Animation::ALL.len() {
            return Err(format!(
                "there must be {} animations, in the original's order",
                Animation::ALL.len()
            ));
        }
        let mut directions = Vec::new();
        for (animation, entry) in Animation::ALL.iter().zip(&self.animations) {
            if entry.name != animation.name() || entry.directions.len() != DIRECTIONS {
                return Err(format!(
                    "the animation \"{}\" must come here, with {DIRECTIONS} directions",
                    animation.name()
                ));
            }
            let scripts: Result<Vec<_>, _> = entry
                .directions
                .iter()
                .map(|script| script.as_deref().map_or(Ok(ScriptRef(0)), find))
                .collect();
            directions.push(scripts?);
        }
        let scripts: Result<Vec<_>, _> = Script::ALL.iter().map(|s| find(s.name())).collect();
        Ok(Animations {
            scripts: scripts?,
            directions,
            by_name,
            first_names,
            words,
            table: self,
        })
    }
}

impl Animations {
    pub fn read(rom: &AtariRom) -> Self {
        AnimationTable::read(rom)
            .lay_out()
            .expect("the original's scripts lay out")
    }

    /// The script word at `at`, if it is in the block.
    pub(super) fn word(&self, at: i32) -> Option<i32> {
        let offset = at - BLOCK_START;
        if offset < 0 || offset % 2 != 0 {
            return None;
        }
        self.words.get(offset as usize / 2).map(|&w| i32::from(w))
    }

    /// The script for `dir`, if `dir` is in the table (the animations lie
    /// end to end, so a direction past either end is the neighbour's).
    pub(super) fn for_dir(&self, animation: Animation, dir: i32) -> Option<ScriptRef> {
        super::lookups::in_run(&self.directions, DIRECTIONS, animation as i32, dir)
    }

    pub fn script(&self, script: Script) -> ScriptRef {
        self.scripts[script as usize]
    }

    pub fn named(&self, name: &str) -> Option<ScriptRef> {
        self.by_name.get(name).copied()
    }

    /// The name of the script starting at `script`, if one does.
    pub fn name_at(&self, script: ScriptRef) -> Option<&str> {
        self.first_names.get(&script).map(String::as_str)
    }
}

/// Where each animation list is in memory, for reads past the table.
pub(super) fn list_address(animation: Animation, dir: i32) -> i32 {
    ANIMATIONS_START + animation as i32 * 32 + dir * 4
}

/// `BigBall` as `big_ball`, `Coin(0)` as `coin_0`.
pub(crate) fn snake_case(debug: &str) -> String {
    let mut out = String::new();
    for c in debug.chars() {
        match c {
            'A'..='Z' => {
                if !out.is_empty() && !out.ends_with('_') {
                    out.push('_');
                }
                out.push(c.to_ascii_lowercase());
            }
            '(' => out.push('_'),
            ')' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_names_read_naturally() {
        assert_eq!(snake_case("BigBall"), "big_ball");
        assert_eq!(snake_case("BounceDome(0)"), "bounce_dome_0");
    }

    #[test]
    fn frames_name_their_controls() {
        assert_eq!(Frame::from_word(0xFFFF), Frame::Control("loop".into()));
        assert_eq!(Frame::from_word(0x12), Frame::Sprite(0x12));
        assert_eq!(Frame::Control("hold".into()).word(), Ok(0xFFFE));
        assert!(Frame::Control("jump".into()).word().is_err());
    }
}
