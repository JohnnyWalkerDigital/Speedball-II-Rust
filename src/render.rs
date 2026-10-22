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
use crate::game::replay::ReplayFrame;
use crate::game::sprites::{amiga_sprite, AmigaSprite, Bank, BIG_BALL_FIRST, LAUNCHER_FIRST};
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
    /// The domes as the ST draws them between hits. The Amiga's are pitch
    /// tiles, so replays (recorded the Amiga's way) leave them out.
    idle_domes: [Entity; 2],
}

impl MatchRenderer {
    const VIEW_ROWS: Option<usize> = Some(VIEW_HEIGHT as usize);
    const STATUS_Y: i32 = VIEW_HEIGHT;
    /// `sprites_monitor_final_score`: the panel without "GOAL" or "INJURED".
    const PLAIN_PANEL: usize = 2;

    pub fn new(assets: Assets, rom: &AtariRom) -> Self {
        Self {
            assets,
            status_led: Entity::read(rom, addr::STATUS_LED),
            idle_domes: addr::BOUNCE_DOMES.map(|a| Entity::read(rom, a)),
        }
    }

    pub fn render(&self, m: &Match, canvas: &mut Canvas) {
        canvas.clip_to_rows(None);
        self.draw_status_bar(m, canvas);

        canvas.clip_to_rows(Self::VIEW_ROWS);
        canvas.draw(&self.assets.pitch, -m.scroll().x, -m.scroll().y);
        // The wall stars are pitch tiles in the original, under every sprite.
        self.draw_stars(m, canvas);
        if m.is_replaying() {
            self.draw_idle_domes(canvas, m.scroll());
        }
        self.draw_sprites(&m.scene, m.scroll(), canvas);
        if m.clock_paused {
            self.draw_panel(m, canvas);
        }
        canvas.clip_to_rows(None);
    }

    /// A sped-up match draws nothing on the pitch (`match_tick`): the
    /// view stays as it was, with any panel message drawn over it for
    /// good, while the status bar keeps up. `view` holds the frozen view.
    pub fn render_sped_up(&self, m: &Match, view: &mut Canvas, canvas: &mut Canvas) {
        if m.text_panel_shown() {
            view.clip_to_rows(Self::VIEW_ROWS);
            self.draw_text_panel(m, view);
            view.clip_to_rows(None);
        }
        canvas.pixels_mut().copy_from_slice(view.pixels());
        self.draw_status_bar(m, canvas);
    }

    /// A frame of a replay loaded from a goal save disk
    /// (`load_replay_goal`): the pitch as freshly loaded, the replayed
    /// sprites and, if any, a panel message. The status bar rows are left
    /// as they are.
    pub fn render_saved_replay(
        &self,
        frame: &ReplayFrame,
        message: Option<&Text>,
        canvas: &mut Canvas,
    ) {
        canvas.clip_to_rows(Self::VIEW_ROWS);
        canvas.draw(&self.assets.pitch, -frame.scroll.x, -frame.scroll.y);
        self.draw_idle_domes(canvas, frame.scroll);
        self.draw_sprites(&frame.sprites, frame.scroll, canvas);
        if let Some(message) = message {
            canvas.draw(&self.assets.panels[Self::PLAIN_PANEL], 16, 8);
            self.draw_text(canvas, message);
        }
        canvas.clip_to_rows(None);
    }

    /// A message on the panel over whatever is shown (`disk_error_message`).
    pub fn render_message(&self, message: &Text, canvas: &mut Canvas) {
        canvas.clip_to_rows(Self::VIEW_ROWS);
        canvas.draw(&self.assets.panels[Self::PLAIN_PANEL], 16, 8);
        self.draw_text(canvas, message);
        canvas.clip_to_rows(None);
    }

    /// `zero_screen`: the view in the background colour.
    pub fn render_blank_view(&self, canvas: &mut Canvas) {
        let color = self.assets.palette.color(Assets::BACKGROUND);
        canvas.fill_rect(0, 0, canvas.width(), VIEW_HEIGHT as usize, color);
    }

    /// The blank status bar the Amiga starts with (`init_screen_status_bar`).
    pub fn render_blank_status_bar(&self, canvas: &mut Canvas) {
        canvas.draw(&self.assets.status_bar, 0, Self::STATUS_Y);
    }

    // -- Pitch ---------------------------------------------------------------------

    /// Draws the frame's sprites (live or replayed) in the match's order.
    fn draw_sprites(&self, scene: &[Entity], scroll: Point, canvas: &mut Canvas) {
        for e in scene {
            match self.assets.platform {
                Platform::Amiga => self.draw_entity(canvas, e, scroll),
                Platform::Atari if e.is_electrobounce() => {
                    self.draw_atari_electrobounce(canvas, e, scroll)
                }
                Platform::Atari => self.draw_entity(canvas, e, scroll),
            }
        }
    }

    fn draw_idle_domes(&self, canvas: &mut Canvas, scroll: Point) {
        if self.assets.platform == Platform::Atari {
            for dome in &self.idle_domes {
                self.draw_entity(canvas, dome, scroll);
            }
        }
    }

    fn draw_stars(&self, m: &Match, canvas: &mut Canvas) {
        let [left, right] = m.arena.stars.wall_tiles();
        let tiles = &self.assets.pitch_tiles;
        let draw = |canvas: &mut Canvas, tile: usize, x: i32, y: i32, size: i32| {
            canvas.draw(
                &tiles[tile],
                x * size - m.scroll().x,
                y * size - m.scroll().y,
            );
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

    /// The ST's electrobounces are pitch tiles.
    fn draw_atari_electrobounce(&self, canvas: &mut Canvas, pad: &Entity, scroll: Point) {
        let base = if pad.address == addr::ELECTROBOUNCES[0] {
            ATARI_WALL_LEFT
        } else {
            ATARI_WALL_RIGHT
        };
        if let Some(image) = self.assets.sheet(Sheet::Pitch).get(base + 9) {
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
        let sprite = match self.assets.platform {
            Platform::Amiga => Self::amiga_sheet_sprite(e),
            Platform::Atari => Self::atari_sheet_sprite(e),
        };
        if let Some(image) = sprite.and_then(|(sheet, index)| {
            usize::try_from(index)
                .ok()
                .and_then(|i| self.assets.sheet(sheet).get(i))
        }) {
            let (x, y) = Self::screen_position(e, scroll);
            canvas.draw(image, x, y);
        }
    }

    fn atari_sheet_sprite(e: &Entity) -> Option<(Sheet, i32)> {
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
                sprite += BIG_BALL_FIRST;
                Sheet::Players
            }
            addr::GFX_PITCH => Sheet::Pitch,
            _ => return None,
        };
        Some((sheet, sprite))
    }

    /// The Amiga's banks are laid out one after another in the port's
    /// sheets.
    fn amiga_sheet_sprite(e: &Entity) -> Option<(Sheet, i32)> {
        let AmigaSprite { bank, index } = amiga_sprite(e)?;
        Some(match bank {
            Bank::Entities => (Sheet::Entities, index),
            Bank::Players if e.is_player_or_medic() && e.uses_blue_palette() => {
                (Sheet::PlayersBlue, index)
            }
            Bank::Players => (Sheet::Players, index),
            Bank::BigBall => (Sheet::Players, BIG_BALL_FIRST + index),
            Bank::Launcher => (Sheet::Players, LAUNCHER_FIRST + index),
        })
    }

    // -- Status bar ----------------------------------------------------------------

    fn draw_status_bar(&self, m: &Match, canvas: &mut Canvas) {
        canvas.draw(&self.assets.status_bar, 0, Self::STATUS_Y);
        self.draw_number(canvas, m.displayed_scores().team1, 3, 2 * 8, 187);
        self.draw_number(canvas, m.displayed_scores().team2, 3, 35 * 8, 187);
        self.draw_number(canvas, m.clock.remaining(), 2, 19 * 8, 186);

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
                self.draw_status_icon(canvas, sprite, x, m.scroll());
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
            self.draw_status_icon(canvas, sprite + OFF_SPRITE_OFFSET, x, m.scroll());
        }
        let team1 = &m.players[m.controlled.team1.0];
        let (sprite, x) = position_light(Side::Team1, team1.attributes.pos);
        self.draw_status_icon(canvas, sprite, x, m.scroll());

        let team2 = &m.players[m.controlled.team2.0];
        if !team2.entity.flags.invisible() {
            let (sprite, x) = position_light(Side::Team2, team2.attributes.pos);
            self.draw_status_icon(canvas, sprite, x, m.scroll());
        }
    }

    // -- Top panel -------------------------------------------------------------------

    fn draw_panel(&self, m: &Match, canvas: &mut Canvas) {
        const GOAL: usize = 0;
        const INJURED: usize = 1;
        let plain_panel = if m.player_injured {
            (m.injury.stage <= 1).then_some(INJURED)
        } else {
            m.text_panel.message().is_none().then_some(GOAL)
        };
        match plain_panel {
            Some(panel) => canvas.draw(&self.assets.panels[panel], 16, 8),
            None => self.draw_text_panel(m, canvas),
        }
    }

    fn draw_text_panel(&self, m: &Match, canvas: &mut Canvas) {
        canvas.draw(&self.assets.panels[Self::PLAIN_PANEL], 16, 8);
        let Some(id) = m.text_panel.message() else {
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
