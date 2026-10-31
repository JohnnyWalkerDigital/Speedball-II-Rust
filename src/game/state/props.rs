//! The entities that are neither players nor arena furniture, and the
//! handles the logic uses to refer to any entity.

use super::Match;
use crate::data::AtariRom;
use crate::game::entity::{Entity, EntityId};
use crate::game::player::PlayerId;

/// A handle to any entity the logic needs to refer to indirectly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityRef {
    Player(PlayerId),
    Ball,
    BigBall,
    /// Invisible "holder" of the ball between goals and during injuries.
    TerrainCenter,
    Launcher,
    Medic(usize),
    Coins(usize),
    Token(usize),
    Equipment,
    Dome(usize),
}

/// Entities that are not players and not part of the arena furniture.
pub struct Props {
    pub ball: Entity,
    pub big_ball: Entity,
    pub terrain_center: Entity,
    pub launcher: Entity,
    pub goal_indicators: [Entity; 2],
    pub player_indicators: [Entity; 2],
    pub medics: [Entity; 2],
    pub coins: [Entity; 4],
    pub tokens: [Entity; 2],
    pub equipment: Entity,
}

impl Props {
    pub(super) fn read(rom: &AtariRom) -> Self {
        let read = |id| rom.start().entity(id);
        Self {
            ball: read(EntityId::Ball),
            big_ball: read(EntityId::BigBall),
            terrain_center: read(EntityId::TerrainCenter),
            launcher: read(EntityId::Launcher),
            goal_indicators: [0, 1].map(|i| read(EntityId::GoalIndicator(i))),
            player_indicators: [0, 1].map(|i| read(EntityId::PlayerIndicator(i))),
            medics: [0, 1].map(|i| read(EntityId::Medic(i))),
            coins: [0, 1, 2, 3].map(|i| read(EntityId::Coin(i))),
            tokens: [0, 1].map(|i| read(EntityId::Token(i))),
            equipment: read(EntityId::Equipment),
        }
    }
}

impl Match {
    pub(crate) fn entity(&self, r: EntityRef) -> &Entity {
        match r {
            EntityRef::Player(p) => &self.players[p.0].entity,
            EntityRef::Ball => &self.props.ball,
            EntityRef::BigBall => &self.props.big_ball,
            EntityRef::TerrainCenter => &self.props.terrain_center,
            EntityRef::Launcher => &self.props.launcher,
            EntityRef::Medic(i) => &self.props.medics[i],
            EntityRef::Coins(i) => &self.props.coins[i],
            EntityRef::Token(i) => &self.props.tokens[i],
            EntityRef::Equipment => &self.props.equipment,
            EntityRef::Dome(i) => &self.arena.domes[i],
        }
    }

    pub(crate) fn entity_mut(&mut self, r: EntityRef) -> &mut Entity {
        match r {
            EntityRef::Player(p) => &mut self.players[p.0].entity,
            EntityRef::Ball => &mut self.props.ball,
            EntityRef::BigBall => &mut self.props.big_ball,
            EntityRef::TerrainCenter => &mut self.props.terrain_center,
            EntityRef::Launcher => &mut self.props.launcher,
            EntityRef::Medic(i) => &mut self.props.medics[i],
            EntityRef::Coins(i) => &mut self.props.coins[i],
            EntityRef::Token(i) => &mut self.props.tokens[i],
            EntityRef::Equipment => &mut self.props.equipment,
            EntityRef::Dome(i) => &mut self.arena.domes[i],
        }
    }

    pub(crate) fn holds_ball(&self, p: PlayerId) -> bool {
        self.holder == Some(EntityRef::Player(p))
    }

    pub(crate) fn holder_player(&self) -> Option<PlayerId> {
        match self.holder {
            Some(EntityRef::Player(p)) => Some(p),
            _ => None,
        }
    }
}
