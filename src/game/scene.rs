//! What is on screen each frame: the sprites the game draws, in its
//! order. The renderer turns this into pixels; the replay recorder stores it.

use super::entity::Entity;
use super::geometry::Point;
use super::player::PlayerId;
use super::state::{EntityRef, Match, VIEW_HEIGHT};

/// Players and medics, kept in back-to-front order between frames so ties
/// resolve as the original's incremental bubble sort does.
pub struct DrawOrder(Vec<EntityRef>);

impl Default for DrawOrder {
    fn default() -> Self {
        Self(
            (0..18)
                .map(|i| EntityRef::Player(PlayerId(i)))
                .chain([EntityRef::Medic(0), EntityRef::Medic(1)])
                .collect(),
        )
    }
}

impl Match {
    /// The visible sprites, back to front. Background parts of
    /// the pitch (wall stars) are not sprites and are not included.
    pub(crate) fn compose_scene(&mut self) -> Vec<Entity> {
        self.sort_players_vertically();
        let mut scene = Vec::new();
        let mut add = |entity: &Entity| {
            if !entity.flags.invisible() {
                scene.push(entity.clone());
            }
        };
        add(&self.props.launcher);
        for dome in &self.arena.domes {
            add(dome);
        }
        for pad in &self.arena.electrobounces.pads {
            add(pad);
        }
        for light in &self.arena.multipliers.lights {
            add(light);
        }
        for item in self
            .props
            .tokens
            .iter()
            .chain([&self.props.equipment])
            .chain(&self.props.coins)
        {
            add(item);
        }

        let ball_first = self.ball_drawn_behind_players();
        if ball_first {
            add(&self.props.ball);
        }
        for &r in &self.draw_order.0 {
            add(self.entity(r));
        }
        if !ball_first {
            add(&self.props.ball);
        }

        if !self.clock_paused {
            add(&self.props.player_indicators[0]);
            if self.team2_on_pitch() {
                add(&self.props.player_indicators[1]);
            }
            // The Amiga original has no goal markers; these are the ST's.
            for indicator in &self.props.goal_indicators {
                add(indicator);
            }
        }
        add(&self.props.big_ball);
        scene
    }

    /// `sort_players_vertically`: a bubble sort by y, so equal rows keep
    /// their previous order. Fallen players sort 16 pixels higher.
    fn sort_players_vertically(&mut self) {
        let depth = |m: &Match, r: EntityRef| {
            let e = m.entity(r);
            if e.flags.falling() {
                e.pos.y - 16
            } else {
                e.pos.y
            }
        };
        loop {
            let mut swapped = false;
            for i in 0..self.draw_order.0.len() - 1 {
                let (a, b) = (self.draw_order.0[i], self.draw_order.0[i + 1]);
                if depth(self, a) > depth(self, b) {
                    self.draw_order.0.swap(i, i + 1);
                    swapped = true;
                }
            }
            if !swapped {
                break;
            }
        }
    }

    /// Whether the ball is drawn before (under) the players.
    fn ball_drawn_behind_players(&self) -> bool {
        let ball = &self.props.ball;
        let ball_on_floor = ball.velocity.is_zero();
        if self.injury.player.is_some() {
            return ball_on_floor;
        }
        match self.holder {
            None | Some(EntityRef::TerrainCenter) => !ball.is_high() && ball_on_floor,
            Some(holder) => {
                let carrier = self.entity(holder);
                // A jumping carrier facing away from us hides the ball.
                carrier.flags.jumping() && (carrier.dir < 3 || carrier.dir > 5)
            }
        }
    }
}

/// The clipping test: whether any of `entity` is on screen.
/// Horizontal positions use the scroll rounded down to 16 pixels, as the
/// Amiga scrolls the last 15 pixels in hardware.
pub fn on_screen(entity: &Entity, scroll: Point) -> bool {
    let x = entity.pos.x + entity.origin.x - entity.width / 2 - (scroll.x & !0xF);
    let y = entity.pos.y + entity.origin.y - entity.height / 2 - scroll.y;
    x > -entity.width && x < 336 && y > -entity.height && y < VIEW_HEIGHT
}
