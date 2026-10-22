//! Drawing the manager, transfer and gym screens.
//!
//! The original drew these piecemeal, copying parts between its two screen
//! buffers; here each frame is drawn whole from a [`View`], in the same
//! order, so the result matches.

use super::data::{Armour, Keypad, ManagerTables, TextLine};
use super::{BuyMode, Screen, View};
use crate::game::squad::{self, group_averages, max_boost, team_average, Context, Squad};
use crate::game::squad::{MarketPlayer, SkillBytes, SquadPlayer, Upgrade};
use crate::graphics::indexed::{IndexedImage, IndexedScreen};
use crate::menu::assets::ManagerGraphics;
use crate::menu::text::write_digits;
use crate::menu::ui::{Page, SCREEN_HEIGHT, SCREEN_WIDTH};

/// The colours `draw_box_colour` uses here.
const BLACK: u8 = 0;
const BAR: u8 = 10;
const BAR_BOOST: u8 = 12;
const BAR_EMPTY: u8 = 14;
/// The keypad's glow around the chosen key.
const KEY_LIGHT: usize = 29;
/// The save key is shown blank except every fourth management round.
const SAVE_KEY: usize = 6;
const SAVE_KEY_PRESSED: usize = 0xD;
const BLANK_KEY: usize = 0x14;

/// Everything a manager screen shows.
pub struct Scene<'a> {
    pub graphics: &'a ManagerGraphics,
    pub tables: &'a ManagerTables,
    pub squad: &'a Squad,
    pub market: &'a [MarketPlayer],
    pub buyable_count: u16,
    pub rounds: u16,
    pub cash: u16,
    pub upgrades: &'a [Upgrade],
    pub context: Context,
}

/// Draws one of the three screens.
pub fn draw(scene: &Scene, view: &View) -> Page {
    let mut canvas = Canvas::new(scene.graphics);
    match view.screen {
        Screen::Manager => draw_manager(&mut canvas, scene, view),
        Screen::Transfer => draw_transfer(&mut canvas, scene, view),
        Screen::Gym => draw_gym(&mut canvas, scene, view),
    }
    Page::from_screen(canvas.screen, scene.graphics.palette)
}

/// `draw_manager_screen`.
fn draw_manager(canvas: &mut Canvas, scene: &Scene, view: &View) {
    canvas.background(&scene.graphics.manager_map);
    attribute_labels(canvas, scene);
    keypad(canvas, scene, &scene.tables.manager_keypad, view);
    canvas.title(&scene.tables.title_manager, 0);
    current_buyable(canvas, scene, view);
    player_table(canvas, scene, view);
    cash(canvas, scene);
    table_cursor(canvas, scene, view);
}

/// `draw_transfer_screen`.
fn draw_transfer(canvas: &mut Canvas, scene: &Scene, view: &View) {
    canvas.background(&scene.graphics.manager_map);
    if scene.buyable_count > 0 {
        attribute_labels(canvas, scene);
    }
    canvas.title(&scene.tables.title_transfer, -4);
    current_buyable(canvas, scene, view);
    player_table(canvas, scene, view);
    keypad(canvas, scene, &scene.tables.transfer_keypad, view);
    cash(canvas, scene);
    table_cursor(canvas, scene, view);
}

/// `draw_gym_screen`, with the upgrade lights and armour model.
fn draw_gym(canvas: &mut Canvas, scene: &Scene, view: &View) {
    canvas.background(&scene.graphics.gym_map);
    attribute_labels(canvas, scene);
    canvas.title(&scene.tables.title_gym, 0);
    upgrade_description(canvas, scene, view);
    keypad(canvas, scene, &scene.tables.gym_keypad, view);
    for (index, button) in scene.tables.upgrades.iter().enumerate() {
        let lit = view.upgrade_cursor == Some(index);
        let sprite = button.light + usize::from(lit);
        canvas.sprite16(&scene.graphics.lights, sprite, button.x, button.y);
    }
    armour(canvas, scene, view);
    current_buyable(canvas, scene, view);
    cash(canvas, scene);
}

/// `draw_attribute_strings`: AGR, ATT, DEF... beside the stat bars.
fn attribute_labels(canvas: &mut Canvas, scene: &Scene) {
    for (_, label) in &scene.tables.attributes {
        canvas.pixel_text(label);
    }
}

/// `draw_keypad`, then the chosen key lit or pressed.
fn keypad(canvas: &mut Canvas, scene: &Scene, keypad: &Keypad, view: &View) {
    let save_hidden = scene.rounds & 3 != 0;
    for key in keypad.iter().flatten().flatten() {
        let mut sprite = key.sprite;
        if save_hidden && sprite == SAVE_KEY {
            sprite = BLANK_KEY;
        }
        canvas.sprite32(&scene.graphics.buttons, sprite, key.x, key.y, 0);
    }
    let Some(chosen) = view.key_highlight else {
        return;
    };
    let Some(key) = keypad[chosen.1][chosen.0] else {
        return;
    };
    let sprite = if view.key_pressed {
        if save_hidden && key.pressed_sprite == SAVE_KEY_PRESSED {
            BLANK_KEY
        } else {
            key.pressed_sprite
        }
    } else {
        KEY_LIGHT
    };
    canvas.sprite32(&scene.graphics.buttons, sprite, key.x, key.y, 0);
}

/// `draw_current_buyable`: the stats, picture and details of whatever the
/// keypad's left and right keys go through.
fn current_buyable(canvas: &mut Canvas, scene: &Scene, view: &View) {
    if view.mode == BuyMode::Market && scene.buyable_count == 0 {
        canvas.pixel_text(&scene.tables.no_players_on_the_market);
        return;
    }
    match view.mode {
        BuyMode::Player => {
            let player = &scene.squad[view.index];
            let limit = player_limit(scene, player);
            stat_bars(canvas, scene, &player.skills, limit);
            buying_player(canvas, scene, player, Some(view.index));
        }
        BuyMode::Group => {
            let limit = Limit::Uniform(max_boost(scene.squad, scene.context));
            stat_bars(
                canvas,
                scene,
                &group_averages(scene.squad)[view.index],
                limit,
            );
            clear_buying_text(canvas);
            canvas.logo(scene.graphics, view.index * 3 + 3, 0x18, 0xF);
        }
        BuyMode::Team => {
            let limit = Limit::Uniform(max_boost(scene.squad, scene.context));
            stat_bars(canvas, scene, &team_average(scene.squad), limit);
            clear_buying_text(canvas);
            canvas.logo(scene.graphics, 0, 0x18, 0xF);
        }
        BuyMode::Market => {
            let player = &scene.market[view.index].player;
            stat_bars(
                canvas,
                scene,
                &player.skills,
                Limit::PerSkill(player.skills),
            );
            buying_player(canvas, scene, player, None);
        }
    }
}

/// How far each stat bar's "can be boosted" part reaches.
enum Limit {
    Uniform(u8),
    PerSkill(SkillBytes),
}

/// A bought player can be trained back up to his market skills; the rest
/// to the gym's limit.
fn player_limit(scene: &Scene, player: &SquadPlayer) -> Limit {
    if player.is_from_market() {
        let skills = squad::market_player(scene.market, player.face)
            .map_or(player.skills, |m| m.player.skills);
        Limit::PerSkill(skills)
    } else {
        Limit::Uniform(max_boost(scene.squad, scene.context))
    }
}

/// `draw_stats`: a bar per skill, a fifth of its value long, then the
/// part the gym could add, then the empty rest; and the number.
fn stat_bars(canvas: &mut Canvas, scene: &Scene, skills: &SkillBytes, limit: Limit) {
    const LEFT: usize = 0x1B;
    const FULL: usize = 50;
    const NUMBER_X: usize = 0x50;
    for (skill, (row, _)) in scene.tables.attributes.iter().enumerate() {
        let y = (*row + 1) as usize;
        let value = usize::from(skills[skill]) / 5;
        let max = match &limit {
            Limit::Uniform(max) => usize::from(*max) / 5,
            Limit::PerSkill(maxes) => usize::from(maxes[skill]) / 5,
        };
        canvas.fill(LEFT, value, y, 5, BAR);
        let mut end = value;
        if value < max {
            canvas.fill(LEFT + value, max - value, y, 5, BAR_BOOST);
            end = max;
        }
        if end < FULL {
            canvas.fill(LEFT + end, FULL - end, y, 5, BAR_EMPTY);
        }
        canvas.fill(NUMBER_X, 0x12, y, 5, BLACK);
        let mut number = b"000".to_vec();
        write_digits(&mut number, 2, i16::from(skills[skill]), 3);
        canvas.green_text_at(&number, NUMBER_X as i32, *row);
    }
}

/// `draw_buying_player` and `draw_player_to_buy`: the face, position,
/// value and name.
fn buying_player(canvas: &mut Canvas, scene: &Scene, player: &SquadPlayer, slot: Option<usize>) {
    canvas.face(scene.graphics, usize::from(player.face), 0x13, 0xF);
    clear_buying_text(canvas);
    let position: Vec<u8> = match slot {
        // Squad places say where they play; a bought player on the bench
        // says where he can play.
        Some(slot) if slot < 9 || !player.is_from_market() => {
            scene.tables.position_names[slot].to_vec()
        }
        _ => {
            let row = market_row(scene, player);
            scene.tables.def_mid_fwd[row * 4..row * 4 + 4].to_vec()
        }
    };
    canvas.green_text_at(&position, 0x38, 0x23);
    let value = squad::player_value(player, scene.upgrades, scene.context);
    canvas.green_text_at(&four_digits(value), 0x38, 0x29);
    canvas.green_text_at(player.name.as_bytes(), 0x38, 0x2F);
}

/// The row (0 to 2) a market player plays in.
fn market_row(scene: &Scene, player: &SquadPlayer) -> usize {
    let position = squad::market_player(scene.market, player.face)
        .map_or(player.position, |m| m.player.position);
    usize::from(position.saturating_sub(1)).min(2)
}

fn clear_buying_text(canvas: &mut Canvas) {
    canvas.fill(0x38, 0x2A, 0x24, 0x11, BLACK);
}

fn four_digits(value: u16) -> Vec<u8> {
    let mut text = b"0000".to_vec();
    write_digits(&mut text, 3, value as i16, 4);
    text
}

/// `draw_player_table`: nine faces of the squad (the table scrolls to
/// show the substitutes), each with its position's first letters.
fn player_table(canvas: &mut Canvas, scene: &Scene, view: &View) {
    let table_y = view.table_y();
    for slot in view.table_offset..view.table_offset + 9 {
        let player = &scene.squad[slot];
        let (x, y) = scene.tables.table_places[slot];
        let (x, y) = (x + scene.tables.table_x, y + table_y);
        canvas.face(scene.graphics, usize::from(player.face), x, y);
        let label: Vec<u8> = if player.is_from_market() && slot >= 9 {
            vec![scene.tables.def_mid_fwd[market_row(scene, player) * 4]]
        } else {
            scene.tables.position_names[slot][..2].to_vec()
        };
        canvas.green_text_at(&label, x - 8, y + 0x20);
    }
}

/// `draw_player_table_cursor`: a box round the chosen squad place, and
/// that player's value underneath.
fn table_cursor(canvas: &mut Canvas, scene: &Scene, view: &View) {
    let slot = view.grid_slot();
    let (x, y) = scene.tables.table_places[slot];
    if (view.table_offset..view.table_offset + 9).contains(&slot) {
        let x = x + scene.tables.table_x - 0xB;
        let y = y + view.table_y() - 8;
        canvas.screen.xor_outline(x, y, 0x30, 0x30, 1);
    }
    canvas.fill(201, 24, 190, 5, BLACK);
    let value = squad::player_value(&scene.squad[slot], scene.upgrades, scene.context);
    canvas.green_text_at(&four_digits(value), 201, 189);
}

/// `redraw_cash`.
fn cash(canvas: &mut Canvas, scene: &Scene) {
    canvas.fill(36, 40, 169, 8, BLACK);
    let mut text = scene.tables.cash_template.clone();
    write_digits(&mut text, 6, scene.cash as i16, 5);
    canvas.cell_text(&scene.graphics.cash_font, &text, -5, -3);
}

/// `display_selected_upgrade_description`: the upgrade's name and price
/// for the current way of buying, or why a purchase failed.
fn upgrade_description(canvas: &mut Canvas, scene: &Scene, view: &View) {
    canvas.fill(120, 42, 16, 30, BLACK);
    let button = &scene.tables.upgrades[view.selected_upgrade];
    let lines: &[TextLine; 3] = match view.message {
        Some(super::Message::NotEnoughCash) => &scene.tables.not_enough_cash,
        Some(super::Message::MaximumBoost) => &scene.tables.maximum_boost,
        None => &button.description,
    };
    for line in &lines[..2] {
        canvas.green_text_at(&line.text, line.x, line.y);
    }
    // The third line of an upgrade is its price, "000 CRD".
    let last = &lines[2];
    let mut text = last.text.clone();
    if view.message.is_none() {
        let cost = button
            .upgrade
            .price
            .wrapping_mul(view.mode.cost_multiplier());
        write_digits(&mut text, 2, cost as i16, 3);
    }
    canvas.green_text_at(&text, last.x, last.y);
}

/// `highlight_armour`: the model, with the chosen upgrade's pieces lit.
fn armour(canvas: &mut Canvas, scene: &Scene, view: &View) {
    let draw = |canvas: &mut Canvas, piece: &Armour, lit: bool| {
        let sprite = piece.sprite + usize::from(lit);
        canvas.sprite32(&scene.graphics.armour, sprite, piece.x, piece.y, 1);
    };
    for piece in &scene.tables.armour_pieces {
        draw(canvas, piece, false);
    }
    for piece in &scene.tables.upgrades[view.selected_upgrade].armours {
        draw(canvas, piece, true);
    }
}

/// A screen being drawn with the manager graphics.
struct Canvas<'a> {
    screen: IndexedScreen,
    graphics: &'a ManagerGraphics,
}

impl<'a> Canvas<'a> {
    fn new(graphics: &'a ManagerGraphics) -> Self {
        Self {
            screen: IndexedScreen::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            graphics,
        }
    }

    /// `draw_management_background`: 16x16 tiles from a 20x13 map.
    fn background(&mut self, map: &[u8]) {
        for (i, &tile) in map.iter().enumerate() {
            let (x, y) = ((i % 20) as i32 * 16, (i / 20) as i32 * 16);
            if let Some(image) = self.graphics.tiles.get(usize::from(tile.wrapping_sub(1))) {
                self.screen.draw_opaque(image, x, y);
            }
        }
    }

    /// A 16x16 unmasked sprite centred on (`x`, `y`).
    fn sprite16(&mut self, images: &[IndexedImage], index: usize, x: i32, y: i32) {
        if let Some(image) = images.get(index) {
            self.screen.draw_opaque(image, x - 8, y - 8);
        }
    }

    /// A 32x32 masked sprite centred on (`x`, `y`), nudged by `offset`.
    fn sprite32(&mut self, images: &[IndexedImage], index: usize, x: i32, y: i32, offset: i32) {
        if let Some(image) = images.get(index) {
            self.screen.draw(image, x - 16 + offset, y - 16 + offset);
        }
    }

    /// `draw_face`: a 48x48 face from 3x3 pieces; (`x`, `y`) is the centre
    /// of the top-left piece.
    fn face(&mut self, graphics: &ManagerGraphics, first: usize, x: i32, y: i32) {
        self.pieces(&graphics.faces, first, 18, x, y);
    }

    /// `draw_group_logo`: the same for a logo.
    fn logo(&mut self, graphics: &ManagerGraphics, first: usize, x: i32, y: i32) {
        self.pieces(&graphics.logos, first, 15, x, y);
    }

    fn pieces(&mut self, images: &[IndexedImage], first: usize, stride: usize, x: i32, y: i32) {
        for row in 0..3 {
            for column in 0..3 {
                let index = first + row * stride + column;
                self.sprite16(images, index, x + column as i32 * 16, y + row as i32 * 16);
            }
        }
    }

    fn fill(&mut self, x: usize, width: usize, y: usize, height: usize, colour: u8) {
        self.screen.fill(x, y, width, height, colour);
    }

    /// `draw_string_pixel` at a starting point, six pixels a letter.
    fn green_text_at(&mut self, text: &[u8], x: i32, y: i32) {
        self.pixel_text_from(text, x, y);
    }

    /// `draw_string_pixel` for text that starts with its own position.
    fn pixel_text(&mut self, text: &[u8]) {
        self.pixel_text_from(text, 0, 0);
    }

    fn pixel_text_from(&mut self, text: &[u8], mut x: i32, mut y: i32) {
        let mut bytes = text.iter().copied();
        while let Some(c) = bytes.next() {
            match c {
                0 => break,
                c if c < b' ' => {
                    x = i32::from(c);
                    y = i32::from(bytes.next().unwrap_or(0));
                }
                b' ' => x += 6,
                c => {
                    let index = usize::from(c.wrapping_sub(b'A'));
                    if let Some(glyph) = self.graphics.green_font.get(index) {
                        self.screen.draw(glyph, x, y);
                    }
                    x += 6;
                }
            }
        }
    }

    /// A screen title in the big title letters (`draw_string`, one row up).
    fn title(&mut self, text: &[u8], x_offset: i32) {
        let font = &self.graphics.title_font;
        self.cell_text(font, text, x_offset, -1);
    }

    /// `draw_string` with an offset: 8-pixel columns, 4-pixel rows.
    fn cell_text(&mut self, font: &[IndexedImage], text: &[u8], x_offset: i32, y_offset: i32) {
        for glyph in crate::menu::text::small_glyphs(text) {
            if let Some(image) = font.get(glyph.index) {
                self.screen
                    .draw(image, glyph.x + x_offset, glyph.y + y_offset);
            }
        }
    }
}
