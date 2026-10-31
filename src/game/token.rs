//! Power-up tokens: what each one does when picked up and when it expires.

use super::events::Sound;
use super::geometry::{Point, Side};
use super::player::PlayerId;
use super::state::{EntityRef, Match};
use crate::data::animations::Script;

/// The token currently in effect, if any.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveToken {
    pub seconds_left: i32,
    /// Sprite of the active token, or 0 when none is active.
    pub sprite: i32,
    pub holder: Option<PlayerId>,
}

impl ActiveToken {
    pub const FREEZE_TEAM: i32 = 55;
    pub const REVERSE_CONTROLS: i32 = 56;
    pub const REDUCE_TEAM: i32 = 57;
    pub const BOOST_TEAM: i32 = 58;
    pub const MANIC: i32 = 59;
    pub const SLOW_TEAM: i32 = 60;
    pub const GRAB_BALL: i32 = 61;
    pub const TRANSPORT: i32 = 62;
    pub const GOAL_DOOR: i32 = 63;
    pub const SHIELD: i32 = 64;
    pub const FULL_ENERGY: i32 = 65;
    pub const ZAP_TEAM: i32 = 66;
    /// Present in the sprite sheet but never handed out.
    pub const UNUSED: i32 = 67;

    const DURATION_SECONDS: i32 = 6;
    /// Status bar sprites for an empty token slot.
    const EMPTY_SLOT: [i32; 2] = [88, 89];
}

impl Match {
    /// Ends the active token's effect (`Token.Dispose`).
    pub(crate) fn expire_token(&mut self) {
        if self.token.sprite == 0 {
            return;
        }
        let holder = self.token.holder.expect("an active token has a holder");
        let side = self.side_of(holder);
        *self.status_tokens.get_mut(side) = Some(match side {
            Side::Team1 => ActiveToken::EMPTY_SLOT[0],
            Side::Team2 => ActiveToken::EMPTY_SLOT[1],
        });

        let affected: Vec<PlayerId> = match self.token.sprite {
            ActiveToken::REDUCE_TEAM | ActiveToken::SLOW_TEAM => self.opponents(holder).collect(),
            ActiveToken::BOOST_TEAM => self.teammates(holder).collect(),
            ActiveToken::MANIC => (0..self.players.len()).map(PlayerId).collect(),
            _ => Vec::new(),
        };
        for id in affected {
            if self.token.sprite == ActiveToken::SLOW_TEAM {
                self.players[id.0].restore_speed();
            } else {
                self.players[id.0].restore_all_skills();
            }
        }
        self.token.sprite = 0;
    }

    /// Applies the token in item slot `item` to `picker` (`sub_FC9C`).
    pub(crate) fn activate_token(&mut self, item: usize, picker: PlayerId) {
        let sprite = self.props.tokens[item].sprite;
        match sprite {
            ActiveToken::FREEZE_TEAM
            | ActiveToken::REVERSE_CONTROLS
            | ActiveToken::GOAL_DOOR
            | ActiveToken::SHIELD => self.hold_token(sprite, picker),
            ActiveToken::REDUCE_TEAM => {
                self.hold_token(sprite, picker);
                for id in self.opponents(picker) {
                    self.players[id.0].boost_all_skills(100);
                }
            }
            ActiveToken::BOOST_TEAM => {
                self.hold_token(sprite, picker);
                for id in self.teammates(picker) {
                    self.players[id.0].boost_all_skills(250);
                }
            }
            ActiveToken::MANIC => {
                self.hold_token(sprite, picker);
                for player in &mut self.players {
                    player.boost_all_skills(250);
                }
            }
            ActiveToken::SLOW_TEAM => {
                self.hold_token(sprite, picker);
                for id in self.opponents(picker) {
                    self.players[id.0].slow_down();
                }
            }
            ActiveToken::GRAB_BALL => {
                self.give_ball_to(picker);
                self.play_sound(Sound::GRAB_BALL);
            }
            ActiveToken::TRANSPORT => {
                // The ball teleports to the team's centre forward.
                let forward = PlayerId::of(self.players[picker.0].entity.is_team1(), 8);
                if !self.players[forward.0].entity.flags.falling() {
                    self.give_ball_to(forward);
                    self.play_sound(Sound::TELEPORT);
                }
            }
            ActiveToken::FULL_ENERGY => {
                self.reload_player_attributes(picker);
                self.play_sound(Sound::FULL_ENERGY);
            }
            ActiveToken::ZAP_TEAM => self.zap_team(picker),
            _ => {}
        }
        self.place_token(item);
    }

    /// Starts a timed token and shows it in the picker's status slot.
    fn hold_token(&mut self, sprite: i32, picker: PlayerId) {
        self.expire_token();
        let side = if picker == self.controlled.team1 {
            Side::Team1
        } else {
            Side::Team2
        };
        *self.status_tokens.get_mut(side) = Some(sprite);
        self.token = ActiveToken {
            seconds_left: ActiveToken::DURATION_SECONDS,
            sprite,
            holder: Some(picker),
        };
    }

    fn give_ball_to(&mut self, p: PlayerId) {
        self.holder = Some(EntityRef::Player(p));
        self.props.ball.script = self.rom.script(Script::BallHeld);
        self.props.ball.script_index = 0;
        self.ball_in_multiplier = false;
    }

    /// Knocks down every visible opponent.
    fn zap_team(&mut self, picker: PlayerId) {
        for id in self.opponents(picker) {
            let flags = self.players[id.0].entity.flags;
            if flags.offscreen() || !self.shield_allows_hit(id) || flags.falling() {
                continue;
            }
            self.hit_by_tackle(id, picker);
            let standing_still = self.players[id.0].entity.velocity == Point::ZERO;
            self.knock_down(id, if standing_still { 3 } else { 4 });
            if self.holds_ball(id) {
                self.holder = None;
            }
        }
        self.play_sound(Sound::ZAP);
    }

    /// "Freeze team": opponents of the holder cannot move.
    pub(crate) fn is_frozen(&self, p: PlayerId) -> bool {
        self.token.sprite == ActiveToken::FREEZE_TEAM && !self.is_token_holder_team(p)
    }

    /// "Reverse controls": opponents of the holder steer backwards.
    pub(crate) fn controls_reversed(&self, p: PlayerId) -> bool {
        if self.token.sprite != ActiveToken::REVERSE_CONTROLS {
            return false;
        }
        let holder = self.token.holder.expect("an active token has a holder");
        let holder_dir = self.players[holder.0].attributes.initial_dir ^ 4;
        holder_dir == self.players[p.0].attributes.initial_dir
    }

    /// "Shield": the holder's team cannot be knocked down.
    pub(crate) fn shield_allows_hit(&self, p: PlayerId) -> bool {
        self.token.sprite != ActiveToken::SHIELD || !self.is_token_holder_team(p)
    }

    fn is_token_holder_team(&self, p: PlayerId) -> bool {
        let holder = self.token.holder.expect("an active token has a holder");
        self.players[holder.0].attributes.initial_dir == self.players[p.0].attributes.initial_dir
    }
}
