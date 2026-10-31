//! Power-up tokens: what each one does when picked up and when it expires.

use super::events::Sound;
use super::geometry::{Point, Side};
use super::player::PlayerId;
use super::state::{EntityRef, Match};
use crate::data::animations::Script;

/// The power-up tokens, numbered by their sprites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    FreezeTeam = 55,
    ReverseControls,
    ReduceTeam,
    BoostTeam,
    Manic,
    SlowTeam,
    GrabBall,
    Transport,
    GoalDoor,
    Shield,
    FullEnergy,
    ZapTeam,
}

/// Whose skills a token changes, from the picker's point of view.
enum Affected {
    Opponents,
    Team,
    Everyone,
}

impl Token {
    const ALL: [Token; 12] = [
        Token::FreezeTeam,
        Token::ReverseControls,
        Token::ReduceTeam,
        Token::BoostTeam,
        Token::Manic,
        Token::SlowTeam,
        Token::GrabBall,
        Token::Transport,
        Token::GoalDoor,
        Token::Shield,
        Token::FullEnergy,
        Token::ZapTeam,
    ];
    /// The first token's sprite; the rest follow in order.
    pub const FIRST_SPRITE: i32 = Token::FreezeTeam as i32;
    const DURATION_SECONDS: i32 = 6;
    /// Status bar sprites for an empty token slot.
    const EMPTY_SLOT: [i32; 2] = [88, 89];

    /// The token drawn as `sprite`. The sprite after the last token is in
    /// the sheet but is never handed out.
    pub fn from_sprite(sprite: i32) -> Option<Token> {
        Self::ALL.into_iter().find(|t| t.sprite() == sprite)
    }

    pub fn sprite(self) -> i32 {
        self as i32
    }

    /// Whether its effect lasts a while, shown in the status bar, rather
    /// than happening once.
    fn is_timed(self) -> bool {
        !matches!(
            self,
            Token::GrabBall | Token::Transport | Token::FullEnergy | Token::ZapTeam
        )
    }

    /// Whose skills it changes while it lasts.
    fn affected(self) -> Option<Affected> {
        match self {
            Token::ReduceTeam | Token::SlowTeam => Some(Affected::Opponents),
            Token::BoostTeam => Some(Affected::Team),
            Token::Manic => Some(Affected::Everyone),
            _ => None,
        }
    }
}

/// A timed token in effect.
#[derive(Clone, Copy, Debug)]
pub struct HeldToken {
    pub token: Token,
    pub holder: PlayerId,
    pub seconds_left: i32,
}

impl Match {
    /// Ends the timed token's effect, if one is held.
    pub(crate) fn expire_token(&mut self) {
        let Some(HeldToken { token, holder, .. }) = self.token.take() else {
            return;
        };
        let side = self.side_of(holder);
        *self.status_tokens.get_mut(side) = Some(match side {
            Side::Team1 => Token::EMPTY_SLOT[0],
            Side::Team2 => Token::EMPTY_SLOT[1],
        });
        for id in self.affected_by(token, holder) {
            if token == Token::SlowTeam {
                self.players[id.0].restore_speed();
            } else {
                self.players[id.0].restore_all_skills();
            }
        }
    }

    /// The players whose skills `token` changes when `holder` picks it up.
    fn affected_by(&self, token: Token, holder: PlayerId) -> Vec<PlayerId> {
        match token.affected() {
            Some(Affected::Opponents) => self.opponents(holder).collect(),
            Some(Affected::Team) => self.teammates(holder).collect(),
            Some(Affected::Everyone) => (0..self.players.len()).map(PlayerId).collect(),
            None => Vec::new(),
        }
    }

    /// Applies the token in item slot `item` to `picker`.
    pub(crate) fn activate_token(&mut self, item: usize, picker: PlayerId) {
        let Some(token) = Token::from_sprite(self.props.tokens[item].sprite) else {
            self.place_token(item);
            return;
        };
        if token.is_timed() {
            self.hold_token(token, picker);
            for id in self.affected_by(token, picker) {
                let player = &mut self.players[id.0];
                match token {
                    Token::ReduceTeam => player.boost_all_skills(100),
                    Token::SlowTeam => player.slow_down(),
                    _ => player.boost_all_skills(250),
                }
            }
        }
        match token {
            Token::GrabBall => {
                self.give_ball_to(picker);
                self.play_sound(Sound::GRAB_BALL);
            }
            Token::Transport => {
                // The ball teleports to the team's centre forward.
                let forward = PlayerId::of(self.players[picker.0].entity.is_team1(), 8);
                if !self.players[forward.0].entity.flags.falling() {
                    self.give_ball_to(forward);
                    self.play_sound(Sound::TELEPORT);
                }
            }
            Token::FullEnergy => {
                self.reload_player_attributes(picker);
                self.play_sound(Sound::FULL_ENERGY);
            }
            Token::ZapTeam => self.zap_team(picker),
            _ => {}
        }
        self.place_token(item);
    }

    /// Starts a timed token and shows it in the picker's status slot.
    fn hold_token(&mut self, token: Token, picker: PlayerId) {
        self.expire_token();
        let side = if picker == self.controlled.team1 {
            Side::Team1
        } else {
            Side::Team2
        };
        *self.status_tokens.get_mut(side) = Some(token.sprite());
        self.token = Some(HeldToken {
            token,
            holder: picker,
            seconds_left: Token::DURATION_SECONDS,
        });
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
        self.held_token_holder(Token::FreezeTeam)
            .is_some_and(|holder| !self.same_team(holder, p))
    }

    /// "Reverse controls": opponents of the holder steer backwards.
    pub(crate) fn controls_reversed(&self, p: PlayerId) -> bool {
        self.held_token_holder(Token::ReverseControls)
            .is_some_and(|holder| {
                let holder_dir = self.players[holder.0].attributes.initial_dir ^ 4;
                holder_dir == self.players[p.0].attributes.initial_dir
            })
    }

    /// "Shield": the holder's team cannot be knocked down.
    pub(crate) fn shield_allows_hit(&self, p: PlayerId) -> bool {
        !self
            .held_token_holder(Token::Shield)
            .is_some_and(|holder| self.same_team(holder, p))
    }

    /// Who holds `token`, if it is the one in effect.
    pub(crate) fn held_token_holder(&self, token: Token) -> Option<PlayerId> {
        self.token
            .filter(|held| held.token == token)
            .map(|held| held.holder)
    }

    /// Whether `a` and `b` play for the same team, by their direction.
    fn same_team(&self, a: PlayerId, b: PlayerId) -> bool {
        self.players[a.0].attributes.initial_dir == self.players[b.0].attributes.initial_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_their_sprites() {
        for (i, token) in Token::ALL.into_iter().enumerate() {
            assert_eq!(token.sprite(), Token::FIRST_SPRITE + i as i32);
            assert_eq!(Token::from_sprite(token.sprite()), Some(token));
        }
        assert_eq!(Token::from_sprite(Token::FIRST_SPRITE - 1), None);
        assert_eq!(Token::from_sprite(Token::FIRST_SPRITE + 12), None);
    }
}
