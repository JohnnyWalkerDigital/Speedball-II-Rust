//! The match: all mutable game state and the per-frame update.
//!
//! Behaviour is split across several files, each adding an `impl Match`
//! block: `actions.rs` (player movement, tackles, throws), `ai.rs` (computer
//! decisions), `items.rs`, `token.rs`, `arena.rs` and `medic.rs`.

use std::rc::Rc;

use super::arena::Arena;
use super::entity::{Entity, ScriptEvent};
use super::events::Sound;
use super::geometry::{DirBits, Input, PerTeam, Point, Side};
use super::player::{Player, PlayerId};
use super::replay::Replay;
use super::rng::Rng;
use super::roster::{Roster, Team};
use super::scene::DrawOrder;
use super::setup::MatchSetup;
use super::text::{TextId, TextPanel};
use super::token::ActiveToken;
use crate::config::GameMode;
use crate::data::{addr, AtariRom};

/// Scroll position at which the whole kick-off area is in view.
pub const KICK_OFF_SCROLL_Y: i32 = 484;
pub const PITCH_CENTER: Point = Point::new(320, 576);
pub const VIEW_HEIGHT: i32 = 184;

/// Camera speed: an eighth of the distance plus one, at most 16 pixels.
fn scroll_step(goal: i32, current: i32) -> i32 {
    (((goal - current).abs() >> 3) + 1).min(16)
}

fn scroll_towards(current: i32, goal: i32, max: i32) -> i32 {
    let step = scroll_step(goal, current);
    match goal.cmp(&current) {
        std::cmp::Ordering::Greater => (current + step).min(goal).min(max),
        std::cmp::Ordering::Less => (current - step).max(goal).max(0),
        std::cmp::Ordering::Equal => current,
    }
}

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
    fn read(rom: &AtariRom) -> Self {
        let read = |address| Entity::read(rom, address);
        Self {
            ball: read(addr::BALL),
            big_ball: read(addr::BIG_BALL),
            terrain_center: read(addr::TERRAIN_CENTER),
            launcher: read(addr::LAUNCHER),
            goal_indicators: addr::GOAL_INDICATORS.map(read),
            player_indicators: addr::PLAYER_INDICATORS.map(read),
            medics: addr::MEDICS.map(read),
            coins: addr::COINS.map(read),
            tokens: addr::TOKENS.map(read),
            equipment: read(addr::EQUIPMENT),
        }
    }
}

/// Stretcher sequence state while a player is carried off.
#[derive(Clone, Copy, Debug, Default)]
pub struct Injury {
    pub player: Option<PlayerId>,
    /// Where the front medic stands, relative to the injured player's x.
    pub front_medic_offset: Point,
    /// Where the rear medic is heading.
    pub rear_medic_target: Point,
    /// 0 approaching, 1 first medic arrived, 2 lifting, 3 carrying off.
    pub stage: i32,
}

/// Alternating long/short blink used by indicators and medics.
#[derive(Clone, Copy, Debug)]
struct Blink {
    current: i32,
    next: i32,
    timer: i32,
}

pub struct Match {
    pub(crate) rom: Rc<AtariRom>,
    pub(crate) rng: Rng,
    pub(crate) mode: GameMode,
    pub(crate) players: Vec<Player>,
    pub(crate) props: Props,
    pub(crate) arena: Arena,
    pub(crate) roster: Roster,
    pub(crate) teams: PerTeam<Team>,
    pub(crate) text_panel: TextPanel,
    pub(crate) token: ActiveToken,
    /// Token icon shown in each team's status bar slot.
    pub(crate) status_tokens: PerTeam<Option<i32>>,

    pub(crate) computer: PerTeam<bool>,
    pub(crate) inputs: PerTeam<Input>,
    /// The player each team's joystick (or the AI) is steering.
    pub(crate) controlled: PerTeam<PlayerId>,
    /// Whoever last threw the ball. The original starts this as null; it is
    /// never read before the first throw in practice, so team 1's kick-off
    /// player is a harmless default.
    pub(crate) thrower: PlayerId,
    pub(crate) holder: Option<EntityRef>,
    /// Directions towards the nearest opponents, avoided when passing.
    pub(crate) blocking_dirs: [DirBits; 2],

    pub(crate) scores: PerTeam<i32>,
    bonus_points: PerTeam<i32>,
    time_in_opponents_half: PerTeam<i32>,
    pub(crate) credits_earned: PerTeam<i32>,
    pub(crate) max_credits: PerTeam<i32>,
    pub(crate) coins_credits: i32,

    pub(crate) clock_paused: bool,
    pub(crate) ball_in_multiplier: bool,
    pub(crate) player_injured: bool,
    launcher_started: bool,
    reverse_side: bool,

    pub(crate) scroll: Point,
    pub(crate) leg: i32,
    leg_duration: i32,
    pub(crate) leg_remaining: i32,
    ticks_left_in_second: i32,
    blink: Blink,

    /// Deceleration threshold for the ball (`_ballSpeedUnk`).
    pub(crate) ball_speed: i32,
    pub(crate) injury: Injury,
    pub(crate) equipment_on_floor: i32,
    pub(crate) equipment_pickups_left: i32,
    /// How many players a charged ball can still knock down.
    pub(crate) charged_hits_left: i32,
    /// Starting positions, also used for the end-of-match line-up.
    presentation: [Point; 18],
    /// The team 1 attacking direction the match started with
    /// (`initial_playing_direction`); picks the goal crowd sound.
    initial_dir_team1: i32,
    /// Sounds since the last call to [`Match::take_sounds`].
    sounds: Vec<Sound>,
    /// `text_overlay_counter`: frames (and lines) drawn since the panel
    /// message last changed. The C# kept it as `TextPanel._unk01`.
    panel_counter: u8,
    /// The end-of-match line-up has been reported.
    finished: bool,
    /// Players and medics in back-to-front order.
    pub(crate) draw_order: DrawOrder,
    /// The sprites on screen this frame, in drawing order.
    pub(crate) scene: Vec<Entity>,
    pub(crate) replay: Replay,
    /// The status bar is only redrawn after the goal replay, so it shows
    /// the score from before the goal while the replay runs.
    scores_before_goal: Option<PerTeam<i32>>,
}

impl Match {
    const TICKS_PER_SECOND: i32 = 50;
    const TICKS_PER_FRAME: i32 = 2;

    pub fn new(setup: MatchSetup, rom: Rc<AtariRom>) -> Self {
        let players = setup.players;
        let presentation = std::array::from_fn(|i| players[i].entity.pos);
        let kick_off = PlayerId::of(true, 8);
        let mut m = Self {
            props: Props::read(&rom),
            arena: Arena::new(&rom),
            text_panel: TextPanel::new(&rom),
            rom,
            rng: setup.rng,
            mode: setup.mode,
            players,
            roster: setup.roster,
            teams: PerTeam::new(setup.team1, setup.team2),
            token: ActiveToken::default(),
            status_tokens: PerTeam::default(),
            computer: setup.computer,
            inputs: PerTeam::default(),
            controlled: PerTeam::new(kick_off, PlayerId::of(false, 8)),
            thrower: kick_off,
            holder: None,
            blocking_dirs: [DirBits::NONE; 2],
            scores: PerTeam::default(),
            bonus_points: PerTeam::default(),
            time_in_opponents_half: PerTeam::default(),
            credits_earned: PerTeam::new(setup.min_credits, setup.min_credits),
            // The original copies team 1's limit to both teams.
            max_credits: PerTeam::new(setup.max_credits.team1, setup.max_credits.team1),
            coins_credits: setup.coins_credits,
            clock_paused: true,
            ball_in_multiplier: false,
            player_injured: false,
            launcher_started: false,
            reverse_side: false,
            scroll: Point::new((640 - 320) / 2, (1152 - VIEW_HEIGHT) / 2),
            leg: match setup.mode {
                GameMode::Practice => 3,
                GameMode::Demo => 1,
                _ => 0,
            },
            leg_duration: setup.leg_duration,
            leg_remaining: setup.leg_duration,
            ticks_left_in_second: Self::TICKS_PER_SECOND,
            blink: Blink {
                current: 6,
                next: 2,
                timer: 6,
            },
            ball_speed: 0,
            injury: Injury {
                stage: -1,
                ..Injury::default()
            },
            equipment_on_floor: 0,
            equipment_pickups_left: 0,
            charged_hits_left: 0,
            presentation,
            initial_dir_team1: setup.initial_dir_team1,
            sounds: Vec::new(),
            panel_counter: 0,
            finished: false,
            draw_order: DrawOrder::default(),
            scene: Vec::new(),
            replay: Replay::default(),
            scores_before_goal: None,
        };

        // Item placement consumes random numbers in this exact order.
        m.play_sound(Sound::GET_READY_KICK_OFF);
        m.place_coins(0);
        for i in 1..4 {
            m.place_coins(i);
            m.props.coins[i].timer = m.props.coins[0].timer;
        }
        m.expire_token();
        m.place_token(0);
        m.place_token(1);
        m.place_equipment();

        if setup.initial_dir_team1 != 0 {
            m.switch_sides();
        }
        for id in (0..m.players.len()).map(PlayerId) {
            m.reload_player_attributes(id);
            m.players[id.0].update_zone_center();
        }
        m.scene = m.compose_scene();
        m
    }

    /// Runs one frame. Returns `true` once the match is over and both teams
    /// have lined up.
    pub fn step(&mut self) -> bool {
        self.update();
        let over = self.leg == 2
            && self.scroll.y == KICK_OFF_SCROLL_Y
            && !self.launcher_started
            && self.teams_in_final_line_up();
        if over && !self.finished {
            // `play_match` leaves its loop here.
            self.finished = true;
            self.play_sound(Sound::SILENCE);
            self.play_sound(Sound::END_OF_MATCH);
        }
        over
    }

    /// Escape: the match stops where it is, with the same closing sounds
    /// as a finished match (`play_match`).
    pub fn abandon(&mut self) {
        self.play_sound(Sound::SILENCE);
        self.play_sound(Sound::END_OF_MATCH);
    }

    /// Hands a team over to the keyboard (or back to the computer).
    pub fn set_computer_control(&mut self, side: Side, computer: bool) {
        *self.computer.get_mut(side) = computer;
    }

    /// Where the original calls `sound_play`.
    pub(crate) fn play_sound(&mut self, sound: Sound) {
        self.sounds.push(sound);
    }

    /// Returns and clears the sounds requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<Sound> {
        std::mem::take(&mut self.sounds)
    }

    pub fn input_mut(&mut self, side: Side) -> &mut Input {
        self.inputs.get_mut(side)
    }

    // -- Lookups -----------------------------------------------------------------

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

    pub(crate) fn side_of(&self, p: PlayerId) -> Side {
        Side::from_is_team1(self.players[p.0].entity.is_team1())
    }

    pub(crate) fn teammates(&self, p: PlayerId) -> impl Iterator<Item = PlayerId> {
        PlayerId::team(self.players[p.0].entity.is_team1())
    }

    pub(crate) fn opponents(&self, p: PlayerId) -> impl Iterator<Item = PlayerId> {
        PlayerId::team(!self.players[p.0].entity.is_team1())
    }

    pub(crate) fn team(&self, side: Side) -> impl Iterator<Item = PlayerId> {
        PlayerId::team(side == Side::Team1)
    }

    pub(crate) fn is_controlled(&self, p: PlayerId) -> bool {
        p == self.controlled.team1 || p == self.controlled.team2
    }

    pub(crate) fn own_input_mut(&mut self, p: PlayerId) -> &mut Input {
        let side = self.side_of(p);
        self.inputs.get_mut(side)
    }

    pub(crate) fn presentation_position(&self, index: usize) -> Point {
        self.presentation[index]
    }

    pub(crate) fn is_practice(&self) -> bool {
        self.mode == GameMode::Practice
    }

    // -- Frame update --------------------------------------------------------------

    fn update(&mut self) {
        if self.replay.is_playing() {
            if !self.step_replay() {
                self.finish_goal();
                self.update_after_goal_check();
            }
            return;
        }
        if self.clock_paused {
            self.prepare_kick_off();
            self.run_ball_launcher();
        }
        if !self.props.ball.flags.invisible() {
            self.decelerate_ball();
            self.arena_check_multiplier_entry();
            self.arena_follow_multiplier_path();
            self.arena_check_star_hit();
            self.check_warp_gates();
            self.arena_check_dome_hits();
            self.arena_check_electrobounce_hits();
            self.update_distances_to_ball();
            self.update_controlled_players();
        }
        self.animate_medics();
        self.update_distances_between_players();
        self.think_all();
        if self.check_goal() {
            // `handle_score_goal` runs the replay before the frame goes on.
            return;
        }
        self.update_after_goal_check();
    }

    fn update_after_goal_check(&mut self) {
        self.move_all();
        self.update_player_indicators();
        let camera_target = self.carry_ball();
        self.follow_with_camera(camera_target);
        self.tick_clock();
        // `match_tick` draws the frame, recording it for replays.
        self.scene = self.compose_scene();
        self.record_replay_frame();
        self.update_panel_overlay();
    }

    /// The score the status bar shows.
    pub fn displayed_scores(&self) -> PerTeam<i32> {
        self.scores_before_goal.unwrap_or(self.scores)
    }

    /// Changes the top panel's message (`overlay_message`), restarting its
    /// counter as the original does.
    pub(crate) fn show_message(&mut self, message: Option<TextId>) {
        self.text_panel.current = message;
        self.panel_counter = 0;
    }

    /// `match_tick` builds the text panel while the clock is stopped.
    fn update_panel_overlay(&mut self) {
        if !self.clock_paused {
            return;
        }
        let text_panel_shown = if self.player_injured {
            self.injury.stage > 1
        } else {
            self.text_panel.current.is_some()
        };
        if text_panel_shown {
            self.build_panel_overlay();
        }
    }

    /// The bookkeeping in `build_text_overlay`: the panel's lines are drawn
    /// (with a sound) when its counter is 0, and each line drawn and each
    /// frame advance the counter. Every 64 frames of a "scored" or "own
    /// goal" panel, which only shows during the goal replay, the crowd may
    /// shout "ice cream".
    pub(crate) fn build_panel_overlay(&mut self) {
        if self.panel_counter == 0 {
            self.play_sound(Sound::SILENCE_CHANNEL_0);
            let team_names = if self.is_practice() { 1 } else { 2 };
            let lines = match self.text_panel.current {
                Some(TextId::Team1Scored | TextId::Team2Scored) => 2,
                Some(TextId::Vs) => 1 + team_names,
                Some(TextId::HalfTime) => 2 + team_names,
                _ => 1,
            };
            self.panel_counter = lines;
        }
        self.panel_counter = self.panel_counter.wrapping_add(1);
        let goal_message = matches!(
            self.text_panel.current,
            Some(TextId::Team1Scored | TextId::Team2Scored | TextId::OwnGoal)
        );
        if self.panel_counter & 0x3F == 0 && self.rng.high_byte() & 0x20 != 0 && goal_message {
            self.play_sound(Sound::ICE_CREAM);
        }
    }

    pub(crate) fn execute_script(&mut self, r: EntityRef) {
        let rom = Rc::clone(&self.rom);
        match self.entity_mut(r).advance_script(&rom) {
            ScriptEvent::None => {}
            ScriptEvent::ActionEnded => {
                let holds_ball = self.holder == Some(r);
                self.entity_mut(r).finish_action(holds_ball);
            }
            ScriptEvent::KnockedDownEnded => {
                if let EntityRef::Player(p) = r {
                    self.on_knocked_down_ended(p);
                }
            }
        }
    }

    fn on_knocked_down_ended(&mut self, p: PlayerId) {
        if self.players[p.0].attributes.health != 0 {
            let entity = &mut self.players[p.0].entity;
            entity.script = addr::SCRIPT_PLAYER_ON_GROUND;
            entity.script_index = 0;
            entity.flags.set_busy(false);
        } else {
            self.start_injury(p);
        }
    }

    fn think_all(&mut self) {
        for i in 0..PlayerId::TEAM_SIZE {
            self.player_think(PlayerId::of(false, i));
            self.player_think(PlayerId::of(true, i));
        }
        self.execute_script(EntityRef::Launcher);
        self.execute_script(EntityRef::Dome(0));
        self.execute_script(EntityRef::Dome(1));
        self.execute_script(EntityRef::BigBall);
        for i in 0..4 {
            self.update_coins(i);
        }
        self.update_token_item(0);
        self.update_token_item(1);
        self.update_equipment_item();
        self.execute_script(EntityRef::Ball);
        self.execute_script(EntityRef::Medic(0));
        self.execute_script(EntityRef::Medic(1));
    }

    fn move_all(&mut self) {
        let rom = Rc::clone(&self.rom);
        if self.holder.is_none() && !self.ball_in_multiplier {
            let low = self.props.ball.sprite <= 2;
            let margin = if low { 32 } else { 24 };
            let hits = self.props.ball.move_within_walls(&rom, margin);
            // `constrain_sprite`: the ball sounds different high and low.
            if hits.side {
                self.play_sound(if low {
                    Sound::SIDE_WALL_LOW
                } else {
                    Sound::SIDE_WALL_HIGH
                });
            }
            if hits.end {
                self.play_sound(if low {
                    Sound::END_WALL_LOW
                } else {
                    Sound::END_WALL_HIGH
                });
            }
        }
        for player in &mut self.players {
            player.entity.move_within_walls(&rom, 48);
        }
        for medic in &mut self.props.medics {
            medic.move_within_walls(&rom, 48);
        }
    }

    fn update_player_indicators(&mut self) {
        if self.clock_paused {
            return;
        }
        for (slot, side, idle_sprite, carrying_base) in
            [(0, Side::Team1, 33, 28), (1, Side::Team2, 27, 22)]
        {
            let id = *self.controlled.get(side);
            let player = &self.players[id.0];
            let sprite = if self.holds_ball(id) {
                carrying_base + player.attributes.pos
            } else {
                idle_sprite
            };
            let pos = Point::new(
                player.entity.pos.x,
                player.entity.pos.y + player.entity.origin.y,
            );
            let indicator = &mut self.props.player_indicators[slot];
            indicator.pos = pos;
            indicator.sprite = sprite;
        }
    }

    /// Keeps a held ball in the carrier's hands. Returns what the camera
    /// should follow.
    fn carry_ball(&mut self) -> EntityRef {
        let Some(holder) = self.holder else {
            return EntityRef::Ball;
        };
        if !self.clock_paused {
            let carrier = self.entity(holder).clone();
            let offset = self.rom.ball_offset(carrier.sprite);
            let mut delta = Point::new(
                carrier.origin.x + offset.x - 8,
                carrier.origin.y + offset.y - 8,
            );
            if self.props.ball.sprite == 0 {
                delta = delta - Point::new(4, 4);
            }
            let ball = &mut self.props.ball;
            ball.velocity = carrier.velocity;
            ball.pos = carrier.pos + delta;
        }
        holder
    }

    fn follow_with_camera(&mut self, target: EntityRef) {
        let target = self.entity(target).pos;

        let goal_x = target.x - 160;
        // Horizontal scrolling ignores tiny movements during play.
        if self.leg >= 2 || scroll_step(goal_x, self.scroll.x) >= 2 {
            self.scroll.x = scroll_towards(self.scroll.x, goal_x, 320);
        }
        self.scroll.y = scroll_towards(self.scroll.y, target.y - 92, 968);
        let y = self.scroll.y;

        // The goal indicators sit on whichever edge of the view shows a goal.
        let indicator_y = if y <= KICK_OFF_SCROLL_Y {
            y + 1
        } else {
            y + 183
        };
        for indicator in &mut self.props.goal_indicators {
            indicator.pos.y = indicator_y;
        }
    }

    // -- Clock -------------------------------------------------------------------

    fn tick_clock(&mut self) {
        self.ticks_left_in_second -= Self::TICKS_PER_FRAME;
        if self.ticks_left_in_second <= 0 {
            self.ticks_left_in_second = Self::TICKS_PER_SECOND;
            self.on_second_elapsed();
        }
        self.blink.timer -= 1;
        if self.blink.timer < 0 {
            self.blink.timer = self.blink.next;
            std::mem::swap(&mut self.blink.current, &mut self.blink.next);
            for medic in &mut self.props.medics {
                medic.toggle_palette();
            }
            for indicator in self
                .props
                .player_indicators
                .iter_mut()
                .chain(self.props.goal_indicators.iter_mut())
            {
                let hidden = indicator.flags.invisible();
                indicator.flags.set_invisible(!hidden);
            }
        }
    }

    fn on_second_elapsed(&mut self) {
        self.arena_check_full_star_rows();
        if !self.clock_paused {
            self.leg_remaining -= 1;
            self.count_territory_and_possession();
        }
        if self.token.sprite != 0 {
            self.token.seconds_left -= 1;
        }
        // `display_time` ticks while a power-up is active.
        if self.token.sprite != 0 {
            self.play_sound(Sound::POWERUP_TICK);
        }
        if self.token.seconds_left == 0 {
            self.expire_token();
        }
        if !self.clock_paused && self.leg_remaining == 0 {
            self.end_leg();
        }
    }

    fn end_leg(&mut self) {
        self.switch_sides();
        self.leg += 1;
        self.clock_paused = true;
        self.leg_remaining = self.leg_duration;
        let (id, score1_pos, score2_pos) = if self.leg != 2 {
            self.play_sound(Sound::GET_READY);
            (TextId::HalfTime, 14, 21)
        } else {
            self.play_sound(Sound::MATCH_OVER);
            (TextId::MatchOver, 23, 30)
        };
        let (score1, score2) = (self.scores.team1, self.scores.team2);
        let text = self.text_panel.text_mut(id);
        text.write_number(score1_pos, score1, 3);
        text.write_number(score2_pos, score2, 3);
        self.show_message(Some(id));
    }

    fn count_territory_and_possession(&mut self) {
        let in_top_half = self.props.ball.pos.y <= 576;
        let team1_attacks_up = self.players[0].attacks_up();
        let side = if in_top_half == team1_attacks_up {
            Side::Team1
        } else {
            Side::Team2
        };
        *self.time_in_opponents_half.get_mut(side) += 1;
        if let Some(p) = self.holder_player() {
            self.players[p.0].stats.time_in_possession += 1;
        }
    }

    // -- Kick-off ------------------------------------------------------------------

    /// While the clock is stopped, waits for everyone to reach their
    /// kick-off spot, then starts the ball launcher (`HandleAnimGoalToInitLaunch`).
    fn prepare_kick_off(&mut self) {
        if self.launcher_started {
            return;
        }
        if let Some(holder) = self.holder {
            if self.entity(holder).flags.busy() {
                return;
            }
            if self.text_panel.current.is_none() {
                self.show_message(Some(TextId::Vs));
                self.play_sound(Sound::GET_READY);
            }
        }
        self.holder = Some(EntityRef::TerrainCenter);
        self.props.terrain_center.pos = PITCH_CENTER;
        self.props.terrain_center.flags.set_busy(false);
        if self.scroll.y != KICK_OFF_SCROLL_Y {
            return;
        }
        self.props.ball.flags.set_invisible(true);
        self.expire_token();
        let everyone_in_place = self.players.iter().all(|player| {
            player.entity.pos == player.launch && player.entity.dir == player.attributes.initial_dir
        });
        if !everyone_in_place {
            return;
        }
        self.props.launcher.script_index = 0;
        self.props.launcher.flags.set_invisible(false);
        self.holder = None;
        self.launcher_started = true;
        let ball = &mut self.props.ball;
        ball.flags.set_invisible(true);
        ball.pos = PITCH_CENTER;
        ball.velocity = Point::ZERO;
        self.ball_in_multiplier = false;
        self.play_sound(Sound::LAUNCHER_START);
    }

    /// Steps the launcher animation and fires the ball (`HandleDeck`).
    fn run_ball_launcher(&mut self) {
        if !self.launcher_started || self.props.launcher.script_index < 19 {
            return;
        }
        if self.props.launcher.script_index == 19 {
            let ball = &mut self.props.ball;
            ball.flags.set_invisible(false);
            ball.script = addr::SCRIPT_BALL_LAUNCH;
            ball.script_index = 0;
            self.play_sound(Sound::BALL_LAUNCH);
            return;
        }
        match self.props.ball.script_index {
            11 => self.props.big_ball.flags.set_invisible(false),
            21 => {
                if self.leg != 2 {
                    self.clock_paused = false;
                }
                self.launcher_started = false;
                self.inputs = PerTeam::default();
                // The ball is in play; the launcher drops out of sight. (The
                // C# never showed or hid it.)
                self.props.launcher.flags.set_invisible(true);
            }
            _ => {}
        }
    }

    fn teams_in_final_line_up(&self) -> bool {
        let team1_attacks_up = self.players[0].attacks_up();
        let in_line = |side: Side, base: usize| {
            self.team(side).all(|id| {
                let player = &self.players[id.0];
                player.entity.pos == self.presentation[base + player.index]
                    && player.entity.dir == player.attributes.initial_dir
            })
        };
        let (base1, base2) = if team1_attacks_up { (0, 9) } else { (9, 0) };
        in_line(Side::Team1, base1) && (self.is_practice() || in_line(Side::Team2, base2))
    }

    /// Swaps ends at half time.
    fn switch_sides(&mut self) {
        let (team1, team2) = self.players.split_at_mut(PlayerId::TEAM_SIZE);
        for (p1, p2) in team1.iter_mut().zip(team2.iter_mut()) {
            std::mem::swap(&mut p1.zone_min, &mut p2.zone_min);
            std::mem::swap(&mut p1.zone_max, &mut p2.zone_max);
            std::mem::swap(&mut p1.launch, &mut p2.launch);
            std::mem::swap(
                &mut p1.attributes.initial_dir,
                &mut p2.attributes.initial_dir,
            );
            // The original indexes the team tables directly, whatever table
            // the person record actually points at.
            let a1 = &mut self.roster.attributes_team1[p1.person_attributes.index];
            let a2 = &mut self.roster.attributes_team2[p2.person_attributes.index];
            std::mem::swap(&mut a1.initial_dir, &mut a2.initial_dir);
            p1.update_zone_center();
            p2.update_zone_center();
        }
        self.reverse_side = !self.reverse_side;
        self.arena.stars.switch_sides();
    }

    // -- Ball --------------------------------------------------------------------

    /// Slows a free ball down (`UpdateBallVelocity`).
    fn decelerate_ball(&mut self) {
        if (!self.clock_paused && self.holder.is_some()) || self.ball_in_multiplier {
            return;
        }
        let ball = &mut self.props.ball;
        if ball.velocity.is_zero() {
            ball.set_charged(false);
            return;
        }
        if ball.velocity.y != 0 && (ball.pos.y < 48 || ball.pos.y > 1104) {
            return;
        }
        if ball.timer != 0 {
            if self.ball_speed <= ball.timer {
                ball.timer -= 1;
                return;
            }
            self.ball_speed = self.ball_speed / 2 + self.ball_speed / 4;
        }
        let low_ball = ball.sprite == 0;
        for component in [&mut ball.velocity.x, &mut ball.velocity.y] {
            if *component != 0 {
                let slowed = *component - component.signum();
                if slowed != 0 || low_ball {
                    *component = slowed;
                }
            }
        }
        if ball.timer != 0 {
            ball.timer -= 1;
        }
    }

    pub(crate) fn set_ball_speed_from_throw(&mut self, thr: i32) {
        self.props.ball.timer = thr / 2;
        self.ball_speed = thr / 4 + thr / 8 + thr / 16;
    }

    pub(crate) fn thrower_skill_thr(&self) -> i32 {
        self.players[self.thrower.0].skills().thr
    }

    /// The tunnels on the side walls teleport the ball across the pitch.
    pub(crate) fn check_warp_gates(&mut self) {
        if self.holder.is_some() || self.props.ball.sprite > 2 {
            return;
        }
        let ball = &mut self.props.ball;
        let in_gate_row = (355..=385).contains(&ball.pos.y) || (767..=797).contains(&ball.pos.y);
        if !in_gate_row {
            return;
        }
        if ball.pos.x < 32 {
            ball.pos.x = 608;
        } else if ball.pos.x > 608 {
            ball.pos.x = 32;
        } else {
            return;
        }
        // Moving straight up or down, the ball keeps its sideways drift
        // (`step_warp`; the C# overwrote it).
        let sideways = ball.velocity.x;
        ball.velocity = self.rom.velocity(ball.dir, 8);
        if ball.dir & 3 == 0 {
            ball.velocity.x = sideways;
        }
        self.set_ball_speed_from_throw(self.thrower_skill_thr());
        self.play_sound(Sound::WARP);
    }

    fn update_distances_to_ball(&mut self) {
        if self.clock_paused {
            return;
        }
        let center = self.entity(self.holder.unwrap_or(EntityRef::Ball)).pos;
        let scroll = self.scroll;
        for player in &mut self.players {
            player.distance_to_ball = player.entity.distance_to(center);
            player.entity.update_offscreen(scroll);
        }
    }

    /// Hands control to whoever is nearest the ball, unless a player holds it.
    fn update_controlled_players(&mut self) {
        if self.clock_paused {
            return;
        }
        for side in [Side::Team1, Side::Team2] {
            if !self.holds_ball(*self.controlled.get(side)) {
                *self.controlled.get_mut(side) = self.nearest_to_ball(side);
            }
        }
    }

    /// Ties go to the later player, as in the original.
    fn nearest_to_ball(&self, side: Side) -> PlayerId {
        let mut nearest = PlayerId::of(side == Side::Team1, 0);
        let mut best = i32::MAX;
        for id in self.team(side) {
            let distance = self.players[id.0].distance_to_ball;
            if distance <= best {
                best = distance;
                nearest = id;
            }
        }
        nearest
    }

    fn update_distances_between_players(&mut self) {
        if self.clock_paused {
            return;
        }
        for p1 in PlayerId::team(true) {
            if self.players[p1.0].entity.flags.offscreen() {
                continue;
            }
            let e1 = &self.players[p1.0].entity;
            let feet = e1.pos + e1.origin;
            for p2 in PlayerId::team(false) {
                let opponent = &self.players[p2.0];
                if opponent.entity.flags.offscreen() {
                    continue;
                }
                let distance = opponent.entity.distance_to(feet);
                let (i1, i2) = (self.players[p1.0].index, opponent.index);
                self.players[p1.0].distance_to_opponents[i2] = distance;
                self.players[p2.0].distance_to_opponents[i1] = distance;
            }
        }
    }

    // -- Scoring -------------------------------------------------------------------

    /// `handle_score_goal`. Returns `true` if the goal replay started; the
    /// rest of the goal is handled by [`Match::finish_goal`] once it ends.
    fn check_goal(&mut self) -> bool {
        if self.holder.is_some() || self.clock_paused || self.props.ball.sprite > 2 {
            return false;
        }
        let ball = &self.props.ball;
        if !(272..=368).contains(&ball.pos.x) {
            return false;
        }
        // `true`: the goal at the top of the pitch, scored by team 1.
        let mut top_goal = if ball.pos.y < 32 && ball.velocity.y < 0 {
            true
        } else if ball.pos.y > 1120 && ball.velocity.y > 0 {
            false
        } else {
            return false;
        };
        if self.reverse_side {
            top_goal = !top_goal;
        }
        if !self.goal_door_open(top_goal) {
            return false;
        }

        // An own goal is credited to the other team's active player.
        let thrower_team1 = self.players[self.thrower.0].entity.is_team1();
        let message = if thrower_team1 && !top_goal {
            self.thrower = self.controlled.team2;
            TextId::OwnGoal
        } else if !thrower_team1 && top_goal {
            self.thrower = self.controlled.team1;
            TextId::OwnGoal
        } else if thrower_team1 {
            TextId::Team1Scored
        } else {
            TextId::Team2Scored
        };
        self.show_message(Some(message));

        self.props.ball.flags.set_invisible(true);
        self.scores_before_goal = Some(self.scores);
        self.add_score(Side::from_is_team1(top_goal), 10);
        self.props.ball.pos = PITCH_CENTER;
        self.props.ball.velocity = Point::ZERO;
        self.play_sound(Sound::SILENCE);
        self.play_sound(Sound::GOAL);
        let crowd = self.goal_crowd_sound();
        self.play_sound(crowd);
        if self.start_replay() && self.step_replay() {
            return true;
        }
        self.finish_goal();
        false
    }

    /// The end of `handle_score_goal`, after the replay.
    fn finish_goal(&mut self) {
        let crowd = self.goal_crowd_sound();
        self.play_sound(crowd);
        self.clock_paused = true;
        self.show_message(None);
        self.players[self.thrower.0].stats.goals_scored += 1;
        self.start_goal_celebration(self.thrower);
        self.scores_before_goal = None;
    }

    /// `goal_sound_function`: the crowd sound depends on whether the scorer's
    /// team started the match attacking up.
    fn goal_crowd_sound(&self) -> Sound {
        if self.started_attacking_up(self.thrower) {
            Sound::GOAL_CROWD_UP
        } else {
            Sound::CROWD_CHEER
        }
    }

    /// Whether `p`'s team attacked up at kick-off (`initial_playing_direction`
    /// is team 1's). Several crowd sounds depend on it.
    pub(crate) fn started_attacking_up(&self, p: PlayerId) -> bool {
        self.players[p.0].entity.is_team1() == (self.initial_dir_team1 == 0)
    }

    /// `play_sound_downwards`: despite the name, the crowd cheers when the
    /// last thrower's team started attacking up. (It checks the thrower even
    /// for a star row completed by the other team.)
    pub(crate) fn cheer_if_thrower_attacked_up(&mut self) {
        if self.started_attacking_up(self.thrower) {
            self.play_sound(Sound::CROWD_CHEER);
        }
    }

    /// The "goal door" token shuts the holder's own goal.
    fn goal_door_open(&self, top_goal: bool) -> bool {
        if self.token.sprite != ActiveToken::GOAL_DOOR {
            return true;
        }
        let holder = self.token.holder.expect("an active token has a holder");
        if self.players[holder.0].attacks_up() {
            top_goal != self.reverse_side
        } else {
            top_goal == self.reverse_side
        }
    }

    /// Adds points, doubled or halved again by the team's score multiplier.
    pub(crate) fn add_score(&mut self, side: Side, value: i32) {
        self.add_points(side, value);
        match self.arena.multipliers.level(side) {
            0 => {}
            1 => self.add_points(side, value / 2),
            _ => self.add_points(side, value),
        }
    }

    fn add_points(&mut self, side: Side, value: i32) {
        *self.scores.get_mut(side) += value;
        if !self.props.ball.flags.invisible() {
            *self.bonus_points.get_mut(side) += value;
        }
    }

    // -- Squad -------------------------------------------------------------------

    pub(crate) fn reload_player_attributes(&mut self, p: PlayerId) {
        let source = *self.roster.attributes(self.players[p.0].person_attributes);
        self.players[p.0].reset_attributes(&source);
    }
}
