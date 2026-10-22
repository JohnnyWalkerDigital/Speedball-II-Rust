//! What the game needs from the machine it runs on: a screen, a clock, the
//! controls and the sound chip.
//!
//! The menu code is written like the original, as straight-line code that
//! waits for the next frame; [`Host`] is where it waits. The desktop app
//! implements it with a window, and the tests with a script.

use std::time::Duration;

use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::graphics::Canvas;

/// One PAL vertical blank: the menus' clock.
pub const VBLANK: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Up,
    Down,
    Left,
    Right,
    Fire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Joystick(Side, Control),
    Escape,
    /// The port's own match controls: pause, step a frame, fast forward.
    Pause,
    Step,
    FastForward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonEvent {
    pub button: Button,
    pub pressed: bool,
    /// A key-repeat press while the key is held.
    pub repeat: bool,
}

impl ButtonEvent {
    pub fn release(button: Button) -> Self {
        Self {
            button,
            pressed: false,
            repeat: false,
        }
    }
}

/// Which instrument samples are in sound memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundBank {
    /// Overlay 28, loaded by `load_mgmt_data`.
    Menu,
    /// Overlay 27, loaded by `load_arena_data`.
    Match,
}

/// The player closed the window.
#[derive(Debug, PartialEq, Eq)]
pub struct Closed;

pub trait Host {
    /// Shows `frame`, lets `wait` pass, and returns what the player pressed
    /// and released meanwhile.
    fn show(&mut self, frame: &Canvas, wait: Duration) -> Result<Vec<ButtonEvent>, Closed>;

    fn play_sound(&mut self, sound: Sound);

    fn use_sound_bank(&mut self, bank: SoundBank);

    /// A short note for the window title, such as "paused".
    fn set_status(&mut self, _status: &str) {}
}
