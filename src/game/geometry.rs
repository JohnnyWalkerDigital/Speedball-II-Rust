//! Small value types shared by the whole engine.

use std::ops::{Add, Sub};

/// A position or vector in pitch pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

macro_rules! bit_accessors {
    ($($get:ident / $set:ident = $mask:expr;)*) => {
        $(
            pub fn $get(self) -> bool {
                self.0 & $mask != 0
            }

            pub fn $set(&mut self, on: bool) {
                if on {
                    self.0 |= $mask;
                } else {
                    self.0 &= !$mask;
                }
            }
        )*
    };
}

pub(crate) use bit_accessors;

/// A joystick-style direction: any combination of up/down/left/right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirBits(u8);

impl DirBits {
    pub const NONE: DirBits = DirBits(0);

    pub fn from_bits(bits: u8) -> Self {
        Self(bits & 0x0F)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    bit_accessors! {
        up / set_up = 1 << 0;
        down / set_down = 1 << 1;
        left / set_left = 1 << 2;
        right / set_right = 1 << 3;
    }

    pub fn has_direction(self) -> bool {
        self.0 != 0
    }

    pub fn clear_horizontal(&mut self) {
        self.set_left(false);
        self.set_right(false);
    }

    pub fn clear_vertical(&mut self) {
        self.set_up(false);
        self.set_down(false);
    }
}

/// One team's controller state, laid out like the original input byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Input(u8);

impl Input {
    bit_accessors! {
        up / set_up = 1 << 0;
        down / set_down = 1 << 1;
        left / set_left = 1 << 2;
        right / set_right = 1 << 3;
        fire / set_fire = 1 << 7;
    }

    pub fn dir_bits(self) -> DirBits {
        DirBits::from_bits(self.0)
    }
}

/// One of the two teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Team1,
    Team2,
}

impl Side {
    pub fn from_is_team1(is_team1: bool) -> Self {
        if is_team1 {
            Side::Team1
        } else {
            Side::Team2
        }
    }

    pub fn opponent(self) -> Self {
        match self {
            Side::Team1 => Side::Team2,
            Side::Team2 => Side::Team1,
        }
    }
}

/// A value per team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerTeam<T> {
    pub team1: T,
    pub team2: T,
}

impl<T> PerTeam<T> {
    pub fn new(team1: T, team2: T) -> Self {
        Self { team1, team2 }
    }

    pub fn get(&self, side: Side) -> &T {
        match side {
            Side::Team1 => &self.team1,
            Side::Team2 => &self.team2,
        }
    }

    pub fn get_mut(&mut self, side: Side) -> &mut T {
        match side {
            Side::Team1 => &mut self.team1,
            Side::Team2 => &mut self.team2,
        }
    }
}

impl<T> From<(T, T)> for PerTeam<T> {
    fn from((team1, team2): (T, T)) -> Self {
        Self::new(team1, team2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_bits_round_trip() {
        let mut bits = DirBits::NONE;
        bits.set_up(true);
        bits.set_right(true);
        assert_eq!(bits.bits(), 0b1001);
        bits.clear_horizontal();
        assert_eq!(bits, DirBits::from_bits(1));
        assert!(bits.has_direction());
    }

    #[test]
    fn input_exposes_direction_bits_only() {
        let mut input = Input::default();
        input.set_left(true);
        input.set_fire(true);
        assert_eq!(input.dir_bits().bits(), 0b0100);
        assert!(input.fire());
    }
}
