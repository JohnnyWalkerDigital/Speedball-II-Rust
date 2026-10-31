//! The match: all mutable game state and the per-frame update.
//!
//! `Match` owns everything; its parts that keep their own rules are small
//! types of their own ([`Clock`], [`Camera`], [`Scoreboard`], [`Props`]).
//! The frame update is split by subject into the modules below, and
//! behaviour elsewhere adds further `impl Match` blocks: `actions.rs`
//! (player movement, tackles, throws), `ai.rs` (computer decisions),
//! `items.rs`, `token.rs`, `arena.rs`, `medic.rs` and `replay.rs`.

mod ball;
mod camera;
mod clock;
mod kick_off;
mod panel;
mod props;
mod scoring;

use crate::data::animations::Script;
use std::rc::Rc;

use super::arena::Arena;
use super::entity::{Entity, ScriptEvent};
use super::events::Sound;
use super::geometry::{DirBits, Input, PerTeam, Point, Side};
use super::medic::Injury;
use super::player::{Player, PlayerId};
use super::replay::Replay;
use super::rng::Rng;
use super::roster::{Roster, Team};
use super::scene::DrawOrder;
use super::setup::MatchKind;
use super::setup::MatchSetup;
use super::text::TextPanel;
use super::token::HeldToken;
use crate::data::AtariRom;

pub use camera::{Camera, KICK_OFF_SCROLL_Y, VIEW_HEIGHT};
pub use clock::Clock;
pub use props::{EntityRef, Props};
pub use scoring::Scoreboard;

pub const PITCH_CENTER: Point = Point::new(320, 576);

pub struct Match {
    pub(crate) rom: Rc<AtariRom>,
    pub(crate) rng: Rng,
    pub(crate) kind: MatchKind,
    pub(crate) players: Vec<Player>,
    pub(crate) props: Props,
    pub(crate) arena: Arena,
    pub(crate) roster: Roster,
    pub(crate) teams: PerTeam<Team>,
    pub(crate) text_panel: TextPanel,
    /// The timed token in effect, if any.
    pub(crate) token: Option<HeldToken>,
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

    pub(crate) score: Scoreboard,
    pub(crate) time_in_opponents_half: PerTeam<i32>,
    pub(crate) credits_earned: PerTeam<i32>,
    pub(crate) max_credits: PerTeam<i32>,
    pub(crate) coins_credits: i32,

    pub(crate) clock_paused: bool,
    pub(crate) ball_in_multiplier: bool,
    pub(crate) player_injured: bool,
    launcher_started: bool,
    reverse_side: bool,

    pub(crate) camera: Camera,
    pub(crate) clock: Clock,

    /// Deceleration threshold for the ball.
    ball_speed: i32,
    pub(crate) injury: Injury,
    pub(crate) equipment_on_floor: i32,
    pub(crate) equipment_pickups_left: i32,
    /// How many players a charged ball can still knock down.
    pub(crate) charged_hits_left: i32,
    /// Starting positions, also used for the end-of-match line-up.
    presentation: [Point; 18],
    /// The team 1 attacking direction the match started with; picks the goal
    /// crowd sound.
    initial_dir_team1: i32,
    /// Sounds since the last call to [`Match::take_sounds`].
    sounds: Vec<Sound>,
    /// The end-of-match line-up has been reported.
    finished: bool,
    /// Players and medics in back-to-front order.
    pub(crate) draw_order: DrawOrder,
    /// The sprites on screen this frame, in drawing order.
    pub(crate) scene: Vec<Entity>,
    pub(crate) replay: Replay,
    /// Sped up, drawing nothing (F in a managed league or cup match).
    pub(crate) accelerated: bool,
}

impl Match {
    pub fn new(setup: MatchSetup, rom: Rc<AtariRom>) -> Self {
        let players = setup.players;
        let presentation = std::array::from_fn(|i| players[i].entity.pos);
        let kick_off = PlayerId::of(true, 8);
        let mut m = Self {
            props: Props::read(&rom),
            arena: Arena::new(&rom),
            text_panel: TextPanel::new(),
            rom,
            rng: setup.rng,
            kind: setup.kind,
            players,
            roster: setup.roster,
            teams: PerTeam::new(setup.team1, setup.team2),
            token: None,
            status_tokens: PerTeam::default(),
            computer: setup.computer,
            inputs: PerTeam::default(),
            controlled: PerTeam::new(kick_off, PlayerId::of(false, 8)),
            thrower: kick_off,
            holder: None,
            blocking_dirs: [DirBits::NONE; 2],
            score: Scoreboard::default(),
            time_in_opponents_half: PerTeam::default(),
            credits_earned: PerTeam::new(setup.min_credits, setup.min_credits),
            max_credits: setup.max_credits,
            coins_credits: setup.coins_credits,
            clock_paused: true,
            ball_in_multiplier: false,
            player_injured: false,
            launcher_started: false,
            reverse_side: false,
            camera: Camera::default(),
            clock: Clock::new(setup.kind.first_leg, setup.leg_duration),
            ball_speed: 0,
            injury: Injury::default(),
            equipment_on_floor: 0,
            equipment_pickups_left: 0,
            charged_hits_left: 0,
            presentation,
            initial_dir_team1: setup.initial_dir_team1,
            sounds: Vec::new(),
            finished: false,
            draw_order: DrawOrder::default(),
            scene: Vec::new(),
            replay: Replay::default(),
            accelerated: false,
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

    // -- Driving the match ---------------------------------------------------------

    /// Runs one frame. Returns `true` once the match is over and both teams
    /// have lined up.
    pub fn step(&mut self) -> bool {
        self.update();
        let over = self.clock.is_full_time()
            && self.camera.position().y == KICK_OFF_SCROLL_Y
            && !self.launcher_started
            && self.teams_in_final_line_up();
        if over && !self.finished {
            // The match loop ends here.
            self.finished = true;
            self.play_sound(Sound::SILENCE);
            self.play_sound(Sound::END_OF_MATCH);
        }
        over
    }

    /// Escape: the match stops where it is, with the same closing sounds
    /// as a finished match.
    pub fn abandon(&mut self) {
        self.play_sound(Sound::SILENCE);
        self.play_sound(Sound::END_OF_MATCH);
    }

    /// Hands a team over to the keyboard (or back to the computer).
    pub fn set_computer_control(&mut self, side: Side, computer: bool) {
        *self.computer.get_mut(side) = computer;
    }

    pub fn input_mut(&mut self, side: Side) -> &mut Input {
        self.inputs.get_mut(side)
    }

    /// Speeds the match up (F in a managed league or cup match): no frames
    /// are drawn or recorded and goals are not replayed.
    pub fn set_accelerated(&mut self, on: bool) {
        self.accelerated = on;
    }

    pub fn is_accelerated(&self) -> bool {
        self.accelerated
    }

    /// Returns and clears the sounds requested since the last call.
    pub fn take_sounds(&mut self) -> Vec<Sound> {
        std::mem::take(&mut self.sounds)
    }

    /// Records a sound for the sound driver to play.
    pub(crate) fn play_sound(&mut self, sound: Sound) {
        self.sounds.push(sound);
    }

    // -- What the outside sees -----------------------------------------------------

    /// The scores as they stand.
    pub fn scores(&self) -> PerTeam<i32> {
        self.score.points()
    }

    /// The score the status bar shows.
    pub fn displayed_scores(&self) -> PerTeam<i32> {
        self.score.displayed()
    }

    /// The top-left of the view on the pitch.
    pub fn scroll(&self) -> Point {
        self.camera.position()
    }

    // -- Lookups -------------------------------------------------------------------

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

    /// Whether team 2 is on the pitch (not in practice).
    pub(crate) fn team2_on_pitch(&self) -> bool {
        self.kind.team2_on_pitch
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
            // A goal runs the replay before the frame goes on.
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
        // Draws the frame, recording it for replays, unless the match is
        // sped up.
        if !self.accelerated {
            self.scene = self.compose_scene();
            self.record_replay_frame();
        }
        self.update_panel_overlay();
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
            entity.script = self.rom.script(Script::PlayerOnGround);
            entity.script_index = 0;
            entity.flags.set_busy(false);
        } else {
            self.start_injury(p);
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

    pub(crate) fn reload_player_attributes(&mut self, p: PlayerId) {
        let source = *self.roster.attributes(self.players[p.0].person_attributes);
        self.players[p.0].reset_attributes(&source);
    }
}
