//! Draws a [`Match`] onto a [`Canvas`].
//!
//! The original redrew parts of the status bar only when they changed and
//! drew from inside the game logic. Here the whole frame is rebuilt from
//! the match state, which gives the same picture and keeps the simulation
//! free of drawing code.

use crate::config::Platform;
use crate::data::{addr, AtariRom};
use crate::game::entity::Entity;
use crate::game::geometry::{Point, Side};
use crate::game::state::{Match, VIEW_HEIGHT};
use crate::game::text::{Text, TextId};
use crate::graphics::field::{
    AMIGA_STARS_LEFT_BOTTOM, AMIGA_STARS_LEFT_TOP, AMIGA_STARS_RIGHT_BOTTOM, AMIGA_STARS_RIGHT_TOP,
    ATARI_WALL_LEFT, ATARI_WALL_RIGHT,
};
use crate::graphics::{Assets, Canvas, Sheet};

pub struct MatchRenderer {
    assets: Assets,
    status_led: Entity,
}

impl MatchRenderer {
    const VIEW_ROWS: Option<usize> = Some(VIEW_HEIGHT as usize);
    const STATUS_Y: i32 = VIEW_HEIGHT;

    pub fn new(assets: Assets, rom: &AtariRom) -> Self {
        Self {
            assets,
            status_led: Entity::read(rom, addr::STATUS_LED),
        }
    }

    pub fn render(&self, m: &Match, canvas: &mut Canvas) {
        canvas.clip_to_rows(None);
        self.draw_status_bar(m, canvas);

        canvas.clip_to_rows(Self::VIEW_ROWS);
        canvas.draw(&self.assets.pitch, -m.scroll.x, -m.scroll.y);
        // The wall stars are pitch tiles in the original, under every sprite.
        self.draw_stars(m, canvas);
        self.draw_sprites(m, canvas);
        if m.clock_paused {
            self.draw_panel(m, canvas);
        }
        canvas.clip_to_rows(None);
    }

    // -- Pitch ---------------------------------------------------------------------

    /// Draws the frame's sprites (live or replayed) in the match's order.
    fn draw_sprites(&self, m: &Match, canvas: &mut Canvas) {
        for e in &m.scene {
            if e.is_electrobounce() {
                self.draw_electrobounce(canvas, e, m.scroll);
            } else if Self::is_dome(e) && self.assets.platform == Platform::Amiga && e.sprite != 0x88 {
                // The Amiga bank only has the "hit" frame of the domes; the
                // rest is part of the pitch tiles.
            } else {
                self.draw_entity(canvas, e, m.scroll);
            }
        }
    }

    fn is_dome(e: &Entity) -> bool {
        addr::BOUNCE_DOMES.contains(&e.address)
    }

    fn draw_stars(&self, m: &Match, canvas: &mut Canvas) {
        let [left, right] = m.arena.stars.wall_tiles();
        let tiles = &self.assets.pitch_tiles;
        let draw = |canvas: &mut Canvas, tile: usize, x: i32, y: i32, size: i32| {
            canvas.draw(&tiles[tile], x * size - m.scroll.x, y * size - m.scroll.y);
        };
        for (wall, variants) in [(0, left), (1, right)] {
            for (i, variant) in variants.into_iter().enumerate() {
                let Some(variant) = variant else { continue };
                let (row, variant) = (i as i32, variant as usize);
                match self.assets.platform {
                    Platform::Atari => {
                        let (base, column, top) = if wall == 0 {
                            (ATARI_WALL_LEFT, 0, 12)
                        } else {
                            (ATARI_WALL_RIGHT, 19, 19)
                        };
                        draw(canvas, base + 6 + variant, column, top + row, 32);
                    }
                    Platform::Amiga => {
                        let (upper, lower, column, top) = if wall == 0 {
                            (AMIGA_STARS_LEFT_TOP, AMIGA_STARS_LEFT_BOTTOM, 0, 12)
                        } else {
                            (AMIGA_STARS_RIGHT_TOP, AMIGA_STARS_RIGHT_BOTTOM, 38, 19)
                        };
                        let y = (top + row) * 2;
                        for dx in 0..2 {
                            draw(
                                canvas,
                                upper + variant * 2 + dx as usize,
                                column + dx,
                                y,
                                16,
                            );
                            draw(
                                canvas,
                                lower + variant * 2 + dx as usize,
                                column + dx,
                                y + 1,
                                16,
                            );
                        }
                    }
                }
            }
        }
    }

    fn draw_electrobounce(&self, canvas: &mut Canvas, pad: &Entity, scroll: Point) {
        let index = usize::from(pad.address != addr::ELECTROBOUNCES[0]);
        let (sheet, sprite) = match (self.assets.platform, index) {
            (Platform::Atari, 0) => (Sheet::Pitch, ATARI_WALL_LEFT + 9),
            (Platform::Atari, _) => (Sheet::Pitch, ATARI_WALL_RIGHT + 9),
            (Platform::Amiga, i) => (Sheet::Players, 135 + i),
        };
        if let Some(image) = self.assets.sheet(sheet).get(sprite) {
            let (x, y) = Self::screen_position(pad, scroll);
            canvas.draw(image, x, y);
        }
    }

    fn screen_position(e: &Entity, scroll: Point) -> (i32, i32) {
        (
            e.origin.x + e.pos.x - scroll.x - e.width / 2,
            e.origin.y + e.pos.y - scroll.y - e.height / 2,
        )
    }

    /// `Entity.Draw`.
    fn draw_entity(&self, canvas: &mut Canvas, e: &Entity, scroll: Point) {
        if e.flags.invisible() {
            return;
        }
        let mut sprite = e.sprite;
        if e.is_ball() && e.is_charged() {
            sprite += 11;
        }
        let sheet = match e.gfx {
            addr::GFX_ENTITIES => Sheet::Entities,
            addr::GFX_PLAYERS if e.is_player_or_medic() && e.uses_blue_palette() => {
                Sheet::PlayersBlue
            }
            addr::GFX_PLAYERS => Sheet::Players,
            addr::GFX_BIG_BALL => {
                sprite += 130;
                Sheet::Players
            }
            addr::GFX_PITCH => Sheet::Pitch,
            _ => return,
        };
        if self.assets.platform == Platform::Amiga {
            sprite = Self::amiga_sprite_index(sheet, sprite, e);
        }
        if let Some(image) = usize::try_from(sprite)
            .ok()
            .and_then(|i| self.assets.sheet(sheet).get(i))
        {
            let (x, y) = Self::screen_position(e, scroll);
            canvas.draw(image, x, y);
        }
    }

    /// The Amiga sheets have one extra player-bank frame (the double dome)
    /// and one fewer token than the ST ones the indices come from.
    fn amiga_sprite_index(sheet: Sheet, sprite: i32, e: &Entity) -> i32 {
        const FIRST_EXTRA_FRAME: i32 = 0x87;
        const UNUSED_TOKEN: i32 = 67;
        match sheet {
            Sheet::Players if sprite >= FIRST_EXTRA_FRAME && !e.is_electrobounce() => sprite + 1,
            Sheet::Entities if sprite > UNUSED_TOKEN => sprite - 1,
            _ => sprite,
        }
    }

    // -- Status bar ----------------------------------------------------------------

    fn draw_status_bar(&self, m: &Match, canvas: &mut Canvas) {
        canvas.draw(&self.assets.status_bar, 0, Self::STATUS_Y);
        self.draw_number(canvas, m.displayed_scores().team1, 3, 2 * 8, 187);
        self.draw_number(canvas, m.displayed_scores().team2, 3, 35 * 8, 187);
        self.draw_number(canvas, m.leg_remaining, 2, 19 * 8, 186);

        let palette = &self.assets.palette;
        for (side, x, color) in [
            (Side::Team1, 64, Assets::BLUE),
            (Side::Team2, 224, Assets::RED),
        ] {
            let health = m.players[m.controlled.get(side).0].attributes.health;
            canvas.fill_rect(x, 193, 32, 3, palette.color(Assets::BACKGROUND));
            canvas.fill_rect(
                x,
                193,
                (health / 4).max(0) as usize,
                3,
                palette.color(color),
            );
        }

        for (side, x) in [(Side::Team1, 56), (Side::Team2, 264)] {
            if let Some(sprite) = *m.status_tokens.get(side) {
                self.draw_status_icon(canvas, sprite, x, m.scroll);
            }
        }
        self.draw_status_leds(m, canvas);
    }

    fn draw_number(&self, canvas: &mut Canvas, value: i32, width: usize, x: i32, y: i32) {
        // Negative scores (possible via the wall stars) show as zero.
        let digits = format!("{:0width$}", value.max(0));
        for (i, digit) in digits.bytes().enumerate() {
            if let Some(image) = self.assets.digits.get(usize::from(digit - b'0')) {
                canvas.draw(image, x + i as i32 * 8, y);
            }
        }
    }

    /// Draws an entity-bank sprite centred in the status bar at `x`.
    fn draw_status_icon(&self, canvas: &mut Canvas, sprite: i32, x: i32, scroll: Point) {
        let mut icon = self.status_led.clone();
        icon.sprite = sprite;
        icon.pos = Point::new(scroll.x + x, scroll.y + Self::STATUS_Y + 8);
        self.draw_entity(canvas, &icon, scroll);
    }

    /// Lights the LED for each active player's formation line.
    fn draw_status_leds(&self, m: &Match, canvas: &mut Canvas) {
        const OFF_SPRITE_OFFSET: i32 = 6;
        for slot in 0..6 {
            let (x, sprite) = m.rom.status_led(slot);
            self.draw_status_icon(canvas, sprite + OFF_SPRITE_OFFSET, x, m.scroll);
        }
        let team1 = &m.players[m.controlled.team1.0];
        let (sprite, x) = position_light(Side::Team1, team1.attributes.pos);
        self.draw_status_icon(canvas, sprite, x, m.scroll);

        let team2 = &m.players[m.controlled.team2.0];
        if !team2.entity.flags.invisible() {
            let (sprite, x) = position_light(Side::Team2, team2.attributes.pos);
            self.draw_status_icon(canvas, sprite, x, m.scroll);
        }
    }

    // -- Top panel -------------------------------------------------------------------

    fn draw_panel(&self, m: &Match, canvas: &mut Canvas) {
        const GOAL: usize = 0;
        const INJURED: usize = 1;
        let plain_panel = if m.player_injured {
            (m.injury.stage <= 1).then_some(INJURED)
        } else {
            m.text_panel.current.is_none().then_some(GOAL)
        };
        match plain_panel {
            Some(panel) => canvas.draw(&self.assets.panels[panel], 16, 8),
            None => self.draw_text_panel(m, canvas),
        }
    }

    fn draw_text_panel(&self, m: &Match, canvas: &mut Canvas) {
        const BACKGROUND: usize = 2;
        canvas.draw(&self.assets.panels[BACKGROUND], 16, 8);
        let Some(id) = m.text_panel.current else {
            return;
        };
        self.draw_text(canvas, m.text_panel.text(id));
        match id {
            TextId::HalfTime | TextId::Vs => {
                if id == TextId::HalfTime {
                    self.draw_text(canvas, m.text_panel.text(TextId::Vs));
                }
                self.draw_text(canvas, &m.teams.team1.short_name.with_row(6));
                if !m.is_practice() {
                    self.draw_text(canvas, &m.teams.team2.short_name.with_row(12));
                }
            }
            TextId::Team1Scored => self.draw_text(canvas, &m.teams.team1.short_name.with_row(7)),
            TextId::Team2Scored => self.draw_text(canvas, &m.teams.team2.short_name.with_row(7)),
            _ => {}
        }
    }

    fn draw_text(&self, canvas: &mut Canvas, text: &Text) {
        let mut cursor = Point::ZERO;
        let mut bytes = text.bytes().iter().copied();
        while let Some(c) = bytes.next() {
            match c {
                b' ' => cursor.x += 1,
                c if c < b' ' => {
                    cursor = Point::new(i32::from(c), i32::from(bytes.next().unwrap_or(0)));
                }
                c => {
                    let glyph = c
                        .checked_sub(b'A')
                        .and_then(|i| self.assets.panel_font.get(usize::from(i)));
                    if let Some(glyph) = glyph {
                        canvas.draw(glyph, cursor.x * 8, cursor.y * 4);
                    }
                    cursor.x += 1;
                }
            }
        }
    }
}

/// The status bar light for the active player's formation line, as
/// `(sprite, x)` (`green_light_active_player`). Defence and the goalkeeper
/// share the first light. (The C# tested team 1's player for team 2's
/// midfield light.)
fn position_light(side: Side, pos: i32) -> (i32, i32) {
    let line = match pos {
        0 | 1 => 0,
        2 => 1,
        _ => 2,
    };
    match side {
        Side::Team1 => [(43, 104), (44, 120), (45, 136)][line],
        Side::Team2 => [(48, 216), (47, 200), (46, 184)][line],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_team_lights_its_own_players_line() {
        assert_eq!(position_light(Side::Team1, 0), (43, 104));
        assert_eq!(position_light(Side::Team1, 4), (45, 136));
        assert_eq!(position_light(Side::Team2, 1), (48, 216));
        assert_eq!(position_light(Side::Team2, 2), (47, 200));
        assert_eq!(position_light(Side::Team2, 3), (46, 184));
    }
}
