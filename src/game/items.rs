//! Pickups lying on the pitch: coins, power-up tokens and equipment.

use super::entity::Think;
use super::events::Sound;
use super::geometry::Point;
use super::player::PlayerId;
use super::roster::Skill;
use super::state::{EntityRef, Match};
use super::token::ActiveToken;
use crate::config::GameMode;

/// Where an item is in its appearance countdown.
enum Appearance {
    Waiting,
    /// The countdown reached zero this frame (the original plays a sound).
    JustAppeared,
    Visible,
}

/// The least x or y an item is placed at.
const ITEM_MARGIN: i32 = 72;

impl Match {
    // -- Placement ---------------------------------------------------------------

    /// Hides a coin and schedules it to reappear somewhere in its quarter
    /// of the pitch. Items sit on the Amiga's 16-pixel grid from 72; the ST
    /// used a 32-pixel grid from 80.
    pub(crate) fn place_coins(&mut self, i: usize) {
        let mut x = ITEM_MARGIN + self.rng.next_word_masked(0xF0);
        // Coins 1 and 3 live on the left half, 2 and 4 on the right.
        if i % 2 == 1 {
            x += 256;
        }
        let mut y = ITEM_MARGIN + self.rng.next_word_masked(0x1F0);
        // Coins 3 and 4 live in the bottom half.
        if i >= 2 {
            y += 512;
        }
        let timer = 32 + self.rng.next_byte_masked(31);
        let coins = &mut self.props.coins[i];
        coins.pos = Point::new(x, y);
        coins.flags.set_invisible(true);
        coins.timer = timer;
    }

    /// Hides a token and picks which power-up it will be.
    pub(crate) fn place_token(&mut self, i: usize) {
        let x = ITEM_MARGIN + self.rng.next_word_masked(0x1F0);
        let mut y = ITEM_MARGIN + self.rng.next_word_masked(0x1F0);
        // Token 1 lives in the top half, token 2 in the bottom half.
        if i == 1 {
            y += 512;
        }
        let (timer, sprite) = loop {
            let random = self.rng.next();
            let timer = 128 + (random & 127) as i32;
            let sprite = ActiveToken::FREEZE_TEAM + ((random >> 16) & 15) as i32;
            let reverse_not_allowed =
                sprite == ActiveToken::REVERSE_CONTROLS && self.mode != GameMode::TwoPlayers;
            if sprite >= ActiveToken::UNUSED || reverse_not_allowed {
                continue;
            }
            break (timer, sprite);
        };
        let token = &mut self.props.tokens[i];
        token.pos = Point::new(x, y);
        token.flags.set_invisible(true);
        token.timer = timer;
        token.sprite = sprite;
    }

    /// Hides the equipment item and picks a new piece.
    pub(crate) fn place_equipment(&mut self) {
        let x = ITEM_MARGIN + self.rng.next_word_masked(0x1F0);
        let y = ITEM_MARGIN + self.rng.next_word_masked(0x3F0);
        let random = self.rng.next();
        let sprite = 80 + ((random >> 16) & 7) as i32;
        let equipment = &mut self.props.equipment;
        equipment.pos = Point::new(x, y);
        equipment.flags.set_invisible(true);
        equipment.timer = (random & 255) as i32;
        equipment.sprite = sprite;
        self.equipment_on_floor = sprite;
        self.equipment_pickups_left = 2;
    }

    // -- Per-frame updates ---------------------------------------------------------

    /// Brings a hidden coin back when its timer runs out, unless both teams
    /// have earned their limit, and lets the controlled players collect it.
    pub(crate) fn update_coins(&mut self, i: usize) {
        if !self.clock_paused {
            if self.props.coins[i].flags.invisible() {
                let coins = &mut self.props.coins[i];
                coins.timer -= 1;
                if coins.timer != 0 {
                    return;
                }
                let team1_capped = self.credits_earned.team1 >= self.max_credits.team1;
                let team2_capped = self.mode != GameMode::TwoPlayers
                    || self.credits_earned.team2 >= self.max_credits.team2;
                if team1_capped && team2_capped {
                    return;
                }
                self.play_sound(Sound::ITEM_APPEARS);
                self.props.coins[i].flags.set_invisible(false);
            }
            if self.props.coins[i].think == Think::Coins {
                self.collect_coins(i);
            }
        }
        self.execute_script(EntityRef::Coins(i));
    }

    fn collect_coins(&mut self, i: usize) {
        let item = EntityRef::Coins(i);
        if self.pickup_distance(item, self.controlled.team1) <= 16 {
            self.credits_earned.team1 += self.coins_credits;
        } else if self.pickup_distance(item, self.controlled.team2) <= 16 {
            self.credits_earned.team2 += self.coins_credits;
        } else {
            return;
        }
        self.place_coins(i);
        self.play_sound(Sound::COIN);
    }

    /// Shows a token when its timer runs out and lets the controlled players
    /// pick it up.
    pub(crate) fn update_token_item(&mut self, i: usize) {
        if self.clock_paused {
            return;
        }
        match self.appear_after_timer(EntityRef::Token(i)) {
            Appearance::Waiting => return,
            Appearance::JustAppeared => self.play_sound(Sound::TOKEN_APPEARS),
            Appearance::Visible => {}
        }
        if self.props.tokens[i].think != Think::Token {
            return;
        }
        if let Some(picker) = self.nearby_controlled_player(EntityRef::Token(i)) {
            self.activate_token(i, picker);
            self.play_sound(Sound::TOKEN);
        }
    }

    /// Shows the equipment item when its timer runs out and lets the
    /// controlled players pick it up.
    pub(crate) fn update_equipment_item(&mut self) {
        if self.equipment_on_floor == 0 || self.clock_paused {
            return;
        }
        match self.appear_after_timer(EntityRef::Equipment) {
            Appearance::Waiting => return,
            Appearance::JustAppeared => self.play_sound(Sound::ITEM_APPEARS),
            Appearance::Visible => {}
        }
        if self.props.equipment.think != Think::Equipment {
            return;
        }
        if let Some(picker) = self.nearby_controlled_player(EntityRef::Equipment) {
            self.equip_player(picker);
            self.play_sound(Sound::ARMOUR);
        }
    }

    /// Counts the item's appearance timer down and shows it at zero.
    fn appear_after_timer(&mut self, item: EntityRef) -> Appearance {
        let entity = self.entity_mut(item);
        let appearance = if entity.timer == 0 {
            Appearance::Visible
        } else {
            entity.timer -= 1;
            if entity.timer != 0 {
                return Appearance::Waiting;
            }
            Appearance::JustAppeared
        };
        entity.flags.set_invisible(false);
        appearance
    }

    fn nearby_controlled_player(&self, item: EntityRef) -> Option<PlayerId> {
        [self.controlled.team1, self.controlled.team2]
            .into_iter()
            .find(|&p| self.pickup_distance(item, p) <= 16)
    }

    /// Distance from an item to a player, or 17 if out of reach.
    fn pickup_distance(&self, item: EntityRef, p: PlayerId) -> i32 {
        const OUT_OF_REACH: i32 = 17;
        let item = self.entity(item);
        let player = &self.players[p.0].entity;
        if player.flags.falling() || player.flags.jumping() {
            return OUT_OF_REACH;
        }
        let delta = item.pos - player.pos;
        if delta.x.abs() > 16 || delta.y.abs() > 16 {
            return OUT_OF_REACH;
        }
        item.distance_to(player.pos)
    }

    // -- Equipment -------------------------------------------------------------------

    fn equipment_skill(&self, sprite: i32) -> Option<Skill> {
        Skill::from_index(self.rom.equipment_skill(sprite - 80) - 72)
    }

    /// Takes the equipment off the pitch and gives its boost to `p`.
    fn equip_player(&mut self, p: PlayerId) {
        let sprite = self.props.equipment.sprite;
        self.props.equipment.flags.set_invisible(true);
        self.equipment_on_floor = 0;
        self.players[p.0].equipment_sprite = sprite;
        let Some(skill) = self.equipment_skill(sprite) else {
            debug_assert!(false, "equipment sprite {sprite} has no skill");
            return;
        };
        self.players[p.0].equip(skill);
        self.equipment_pickups_left -= 1;
    }

    /// Drops whatever the player was wearing where they stand.
    pub(crate) fn drop_equipment(&mut self, p: PlayerId) {
        let sprite = self.players[p.0].equipment_sprite;
        if sprite == 0 {
            return;
        }
        let at = self.players[p.0].entity.pos;
        let equipment = &mut self.props.equipment;
        equipment.flags.set_invisible(false);
        equipment.pos = Point::new((at.x & 0xFE0) + 16, (at.y & 0xFE0) + 16);
        self.equipment_on_floor = sprite;
        if let Some(skill) = self.equipment_skill(sprite) {
            self.players[p.0].unequip(skill);
        }
        let player = &mut self.players[p.0];
        player.equipment_sprite = 0;
        player.equipment_backup = 0;
        if self.equipment_pickups_left == 0 {
            self.place_equipment();
        }
    }
}
