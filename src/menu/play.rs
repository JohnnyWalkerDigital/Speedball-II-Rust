//! Running a match: the loop in `play_match`, plus the port's pause, step
//! and fast-forward keys.

use std::time::Duration;

use super::host::{Button, Control, SoundBank};
use super::ui::{Flow, Stop, Ui, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::game::geometry::{Input, PerTeam};
use crate::game::Match;
use crate::graphics::Canvas;
use crate::render::MatchRenderer;

/// The original ran on a 1000/31 ms timer.
pub const FRAME_TIME: Duration = Duration::from_millis(1000 / 31);
const FAST_FORWARD_STEPS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Playback {
    Running,
    Paused,
    FastForward,
}

/// How a match is driven.
#[derive(Clone, Copy)]
pub struct Controls {
    /// Touching a team's controls takes it over from the computer (the
    /// command-line knockout mode); otherwise the computer keeps its teams.
    pub take_over_on_touch: bool,
    /// Stay on the final frame, paused, instead of returning.
    pub hold_at_end: bool,
}

/// Plays `game` to the end. Escape abandons it (`Stop::Abort`), as in the
/// original. Returns the final score.
pub fn run_match(
    ui: &mut Ui,
    game: &mut Match,
    renderer: &MatchRenderer,
    controls: Controls,
) -> Flow<PerTeam<i32>> {
    ui.use_sound_bank(SoundBank::Match);
    let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    renderer.render(game, &mut canvas);
    ui.transition_to(canvas.clone(), None)?;

    let mut playback = Playback::Running;
    let mut over = false;
    ui.clear_input();
    let result = loop {
        let mut steps = match playback {
            Playback::Running => 1,
            Playback::FastForward => FAST_FORWARD_STEPS,
            Playback::Paused => 0,
        };
        while let Some(event) = ui.next_event() {
            match event.button {
                Button::Joystick(side, control) => {
                    if event.pressed && controls.take_over_on_touch {
                        game.set_computer_control(side, false);
                    }
                    set_control(game.input_mut(side), control, event.pressed);
                }
                _ if !event.pressed || event.repeat => {}
                Button::Escape => break_out(ui, game)?,
                Button::Pause if playback == Playback::Paused => playback = Playback::Running,
                Button::Pause => playback = Playback::Paused,
                Button::FastForward if playback == Playback::FastForward => {
                    playback = Playback::Running
                }
                Button::FastForward => playback = Playback::FastForward,
                Button::Step if playback == Playback::Paused => steps = 1,
                Button::Step => {}
            }
        }
        if !over {
            for _ in 0..steps {
                over = game.step();
                for sound in game.take_sounds() {
                    ui.play_sound(sound);
                }
                if over {
                    break;
                }
            }
        }
        if over && !controls.hold_at_end {
            break game.scores;
        }
        if over {
            playback = Playback::Paused;
        }
        ui.host().set_status(match playback {
            Playback::Running => "",
            Playback::Paused => "paused: P resume, N step",
            Playback::FastForward => "fast forward",
        });
        renderer.render(game, &mut canvas);
        ui.show_frame(&canvas, FRAME_TIME)?;
    };
    ui.host().set_status("");
    ui.use_sound_bank(SoundBank::Menu);
    Ok(result)
}

/// Escape during a match: the end-of-match sounds, then back to the menu.
fn break_out(ui: &mut Ui, game: &mut Match) -> Flow {
    game.abandon();
    for sound in game.take_sounds() {
        ui.play_sound(sound);
    }
    ui.host().set_status("");
    ui.use_sound_bank(SoundBank::Menu);
    Err(Stop::Abort)
}

fn set_control(input: &mut Input, control: Control, on: bool) {
    match control {
        Control::Up => input.set_up(on),
        Control::Down => input.set_down(on),
        Control::Left => input.set_left(on),
        Control::Right => input.set_right(on),
        Control::Fire => input.set_fire(on),
    }
}
