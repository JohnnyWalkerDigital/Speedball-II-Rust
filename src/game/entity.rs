//! The 40-byte entity record shared by players, the ball, items and pitch
//! furniture.

use super::geometry::{bit_accessors, DirBits, Point};
use crate::data::{addr, AtariRom};

/// What an entity does each frame. In the original this is a code pointer;
/// the address of each routine is noted for cross-reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Think {
    /// `0xEB7A`: pick an animation towards `target` and start moving.
    StartMove,
    /// `0xEC0C`: keep steering towards `target`.
    Move,
    /// `0xED56`: standing tackle, checking for a hit every frame.
    StandingTackle,
    /// `0xF078`: wind-up; releases the ball on frame 4.
    Throw,
    /// `0xF212`: stop and face the default direction.
    Stand,
    /// `0xF290`: in the air; lands after a speed-dependent delay.
    Jump,
    /// `0xF2E2`: getting back up after a tackle attempt.
    TackleRecover,
    /// `0xF334`: goalkeeper dive (also tackles).
    GoalieDive,
    /// `0xF33C`: sliding tackle.
    SlideTackle,
    /// `0xF344`: nothing (the routine is a bare `rts`).
    Idle,
    /// `0xFC3C`: coin pickup.
    Coins,
    /// `0xFC9C`: power-up token pickup.
    Token,
    /// `0xFD92`: equipment pickup.
    Equipment,
    /// A pointer this port does not know about; behaves like `Idle`.
    Unknown(i32),
}

impl Think {
    pub fn from_address(address: i32) -> Self {
        match address {
            0xEB7A => Think::StartMove,
            0xEC0C => Think::Move,
            0xED56 => Think::StandingTackle,
            0xF078 => Think::Throw,
            0xF212 => Think::Stand,
            0xF290 => Think::Jump,
            0xF2E2 => Think::TackleRecover,
            0xF334 => Think::GoalieDive,
            0xF33C => Think::SlideTackle,
            0xF344 => Think::Idle,
            0xFC3C => Think::Coins,
            0xFC9C => Think::Token,
            0xFD92 => Think::Equipment,
            other => Think::Unknown(other),
        }
    }
}

/// The entity's flag byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags(u8);

impl Flags {
    bit_accessors! {
        busy / set_busy = 1 << 0;
        jumping / set_jumping = 1 << 1;
        throwing / set_throwing = 1 << 2;
        // Bit 3 is overloaded: team membership for players, "charged" for
        // the ball and the flashing colour for medics. Use the named
        // accessors on `Entity` instead.
        bit3 / set_bit3 = 1 << 3;
        falling / set_falling = 1 << 4;
        tackling / set_tackling = 1 << 5;
        offscreen / set_offscreen = 1 << 6;
        invisible / set_invisible = 1 << 7;
    }

    /// Clears the "doing an action" flags (jumping, throwing, falling, tackling).
    fn clear_action(&mut self) {
        self.set_jumping(false);
        self.set_throwing(false);
        self.set_falling(false);
        self.set_tackling(false);
    }
}

/// Something the animation script asked the engine to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptEvent {
    None,
    /// `0xFFFB`: the knocked-down animation ended; check the player's health.
    KnockedDownEnded,
    /// `0xFFFD`: a one-shot action animation (throw, tackle...) ended.
    ActionEnded,
}

#[derive(Clone, Debug)]
pub struct Entity {
    /// Original RAM address; identifies the entity's role.
    pub address: i32,
    pub gfx: u32,
    pub think: Think,
    /// RAM address of the current animation script.
    pub script: i32,
    pub width: i32,
    pub height: i32,
    pub pos: Point,
    pub velocity: Point,
    pub script_index: i32,
    /// One of 8 directions: 0 is up, increasing clockwise.
    pub dir: i32,
    pub timer: i32,
    pub sprite: i32,
    pub flags: Flags,
    /// Offset from `pos` to where the sprite is drawn.
    pub origin: Point,
}

impl Entity {
    pub const SIZE: i32 = 0x28;

    pub fn read(rom: &AtariRom, address: i32) -> Self {
        Self {
            address,
            gfx: rom.u32(address) as u32,
            // +4 holds a draw routine pointer the port does not need.
            think: Think::from_address(rom.u32(address + 8)),
            script: rom.u32(address + 12),
            width: rom.u16(address + 16),
            height: rom.u16(address + 18),
            pos: Point::new(rom.u16(address + 20), rom.u16(address + 22)),
            velocity: Point::new(rom.u16(address + 24), rom.u16(address + 26)),
            script_index: rom.u16(address + 28),
            dir: rom.u8(address + 30),
            timer: rom.u8(address + 31),
            sprite: rom.u16(address + 32),
            flags: Flags(rom.u8(address + 34) as u8),
            origin: Point::new(rom.i16(address + 36), rom.i16(address + 38)),
        }
    }

    // -- Role -----------------------------------------------------------------

    pub fn is_ball(&self) -> bool {
        self.address == addr::BALL
    }

    pub fn is_player_or_medic(&self) -> bool {
        (addr::PLAYERS_TEAM1..addr::MEDICS_END).contains(&self.address)
    }

    pub fn is_electrobounce(&self) -> bool {
        (addr::ELECTROBOUNCES[0]..addr::PLAYERS_TEAM1).contains(&self.address)
    }

    /// Players: which team. (Shares bit 3 with `is_charged`.)
    pub fn is_team1(&self) -> bool {
        self.flags.bit3()
    }

    /// Ball: it went through an electrobounce and now knocks players over.
    pub fn is_charged(&self) -> bool {
        self.flags.bit3()
    }

    pub fn set_charged(&mut self, charged: bool) {
        self.flags.set_bit3(charged);
    }

    /// Medics: alternate between the red and blue palettes.
    pub fn toggle_palette(&mut self) {
        let blue = self.flags.bit3();
        self.flags.set_bit3(!blue);
    }

    pub fn uses_blue_palette(&self) -> bool {
        self.flags.bit3()
    }

    // -- Animation -------------------------------------------------------------

    /// Advances the animation script by one frame (`ExecuteOpcode`).
    pub fn advance_script(&mut self, rom: &AtariRom) -> ScriptEvent {
        if self.flags.invisible() {
            return ScriptEvent::None;
        }
        self.sprite = rom.script_word(self.script, self.script_index);
        self.script_index += 1;
        match rom.script_word(self.script, self.script_index) {
            0xFFFF => self.script_index = 0,
            0xFFFE => self.script_index -= 1,
            0xFFFB => {
                self.velocity = Point::ZERO;
                self.script_index -= 1;
                self.flags.clear_action();
                self.think = Think::Idle;
                return ScriptEvent::KnockedDownEnded;
            }
            0xFFFD => return ScriptEvent::ActionEnded,
            0xFFFC => {
                self.flags.set_invisible(true);
                self.script_index = 0;
            }
            _ => {}
        }
        ScriptEvent::None
    }

    /// Handles [`ScriptEvent::ActionEnded`]: go back to standing, or resume
    /// moving if the entity neither holds the ball nor just threw it.
    pub fn finish_action(&mut self, holds_ball: bool) {
        self.think = if !holds_ball && !self.flags.throwing() {
            Think::StartMove
        } else {
            Think::Stand
        };
        self.flags.set_busy(false);
        self.flags.clear_action();
    }

    /// Selects the script for the current direction from a per-direction
    /// list and sets the matching velocity (`sub_F5A8`).
    pub fn set_animation(&mut self, rom: &AtariRom, list: i32, speed: i32) {
        self.script = rom.script_for_dir(list, self.dir);
        debug_assert_ne!(
            self.script, 0,
            "no script for dir {} in {list:#X}",
            self.dir
        );
        self.velocity = rom.velocity(self.dir, speed);
    }

    // -- Movement --------------------------------------------------------------

    /// Applies velocity, clamping to the pitch walls; the ball bounces off
    /// them, and reports which walls it bounced off.
    pub fn move_within_walls(&mut self, rom: &AtariRom, margin: i32) -> WallBounces {
        let mut bounces_off = WallBounces::default();
        if self.flags.invisible() {
            return bounces_off;
        }
        let bounces = self.is_ball();

        let mut hit_wall = false;
        if 640 - margin < self.pos.x {
            self.pos.x = 640 - margin;
            hit_wall = true;
        } else if margin > self.pos.x {
            self.pos.x = margin;
            hit_wall = true;
        }
        if hit_wall && bounces {
            self.velocity.x = -self.velocity.x;
            self.dir = rom.mirror_dir_x(self.dir);
            bounces_off.side = true;
        }
        self.pos.x += self.velocity.x;

        let mut hit_wall = false;
        if 1152 - margin < self.pos.y {
            self.pos.y = 1152 - margin;
            hit_wall = true;
        } else if margin > self.pos.y {
            self.pos.y = margin;
            hit_wall = true;
        }
        if hit_wall && bounces {
            self.velocity.y = -self.velocity.y;
            self.dir = rom.mirror_dir_y(self.dir);
            self.timer -= self.timer / 2;
            bounces_off.end = true;
        }
        self.pos.y += self.velocity.y;
        bounces_off
    }

    // -- Geometry --------------------------------------------------------------

    /// Approximate distance from this entity's feet to `target`
    /// (`CalcVector`: an octagonal approximation of the Euclidean norm).
    pub fn distance_to(&self, target: Point) -> i32 {
        let dx = (self.pos.x - target.x).abs();
        let dy = (self.pos.y + self.origin.y - target.y).abs();
        approximate_length(dx, dy)
    }

    /// Cheap rejection test: returns the horizontal distance if it exceeds
    /// `limit`, otherwise the vertical one (`UnkInRect`).
    pub fn axis_distance(&self, target: Point, limit: i32) -> i32 {
        let dx = (target.x - self.pos.x).abs();
        if dx > limit {
            return dx;
        }
        (target.y - self.pos.y - self.origin.y).abs()
    }

    /// Direction bits towards `target`, ignoring the minor axis when the
    /// target lies close to a straight line (`GetDirBitsToTargetBetter`).
    pub fn dir_bits_towards(&self, target: Point) -> DirBits {
        let dx = target.x - self.pos.x;
        let dy = target.y - self.pos.y;
        let half_dx = dx.abs() / 2;
        let half_dy = dy.abs() / 2;
        let mut bits = DirBits::NONE;
        bits.set_down(dy > half_dx);
        bits.set_up(dy < -half_dx);
        bits.set_right(dx > half_dy);
        bits.set_left(dx < -half_dy);
        bits
    }

    /// Direction bits towards `target` on each axis that differs at all
    /// (`GetDirBitsToTargetSimple`).
    pub fn dir_bits_towards_exact(&self, target: Point) -> DirBits {
        let mut bits = DirBits::NONE;
        if target.x != self.pos.x {
            bits.set_right(target.x > self.pos.x);
            bits.set_left(target.x < self.pos.x);
        }
        if target.y != self.pos.y {
            bits.set_down(target.y > self.pos.y);
            bits.set_up(target.y < self.pos.y);
        }
        bits
    }

    /// Flags the entity as off-screen unless it is visible in the 320x184 view.
    pub fn update_offscreen(&mut self, scroll: Point) {
        let on_screen = !self.flags.invisible() && is_in_view(self.pos, scroll);
        self.flags.set_offscreen(!on_screen);
    }

    /// Whether the entity is at least 16 pixels inside the view.
    pub fn is_well_inside_view(&self, scroll: Point) -> bool {
        let x = self.pos.x - scroll.x;
        let y = self.pos.y - scroll.y;
        (16..=168).contains(&y) && (16..=304).contains(&x)
    }
}

/// Which walls the ball bounced off this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WallBounces {
    /// The left or right wall.
    pub side: bool,
    /// The top or bottom wall.
    pub end: bool,
}

/// Whether a pitch position is inside the 320x184 match view.
pub fn is_in_view(pos: Point, scroll: Point) -> bool {
    let x = pos.x - scroll.x;
    let y = pos.y - scroll.y;
    (0..=184).contains(&y) && (0..=320).contains(&x)
}

/// The 68000 routine's integer approximation of `sqrt(a² + b²)`.
fn approximate_length(a: i32, b: i32) -> i32 {
    let (max, min) = if a < b { (b, a) } else { (a, b) };
    let length = max + (min >> 1);
    let mut correction = min >> 2;
    if length >= min << 1 {
        if length >= min << 2 {
            correction += correction >> 1;
        }
    } else {
        correction >>= 1;
    }
    length - correction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approximate_length_is_close_to_euclidean() {
        assert_eq!(approximate_length(0, 0), 0);
        assert_eq!(approximate_length(16, 0), 16);
        for (a, b) in [(30, 40), (100, 7), (12, 12), (3, 200)] {
            let exact = f64::from(a * a + b * b).sqrt();
            let approx = f64::from(approximate_length(a, b));
            assert!(
                (approx - exact).abs() / exact < 0.12,
                "{a},{b}: {approx} vs {exact}"
            );
        }
    }

    #[test]
    fn think_addresses_round_trip() {
        assert_eq!(Think::from_address(0xF078), Think::Throw);
        assert_eq!(Think::from_address(0x1234), Think::Unknown(0x1234));
    }
}
