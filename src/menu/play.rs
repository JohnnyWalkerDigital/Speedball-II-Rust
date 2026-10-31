//! Running a match: the loop in `play_match` and the keys `run_replay`
//! reads during a goal replay, plus the port's pause, step and fast-forward
//! keys.

use std::time::Duration;

use super::host::{Button, Control, SoundBank};
use super::saving::DISK_ERROR;
use super::strings::MenuText;
use super::ui::{Flow, Stop, Ui, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::data::save_disk::{replay_sector, SaveDisk};
use crate::game::geometry::{Input, PerTeam, Side};
use crate::game::replay::SavedReplay;
use crate::game::text::Text;
use crate::game::Match;
use crate::graphics::Canvas;
use crate::render::MatchRenderer;

/// The original ran on a 1000/31 ms timer.
pub const FRAME_TIME: Duration = Duration::from_millis(1000 / 31);
const FAST_FORWARD_STEPS: usize = 8;
/// A sped-up match ran as fast as the Amiga could run it without drawing;
/// the port runs this many steps a frame.
const SPED_UP_STEPS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Playback {
    Running,
    Paused,
    FastForward,
}

/// How a match is driven.
pub struct Controls<'a> {
    /// Touching a team's controls takes it over from the computer (the
    /// command-line knockout mode); otherwise the computer keeps its teams.
    pub take_over_on_touch: bool,
    /// Stay on the final frame, paused, instead of returning.
    pub hold_at_end: bool,
    /// Where keys 1 to 9 save a goal replay, if they may.
    pub replay_saves: Option<ReplaySaves<'a>>,
    /// `misc_flags` bit 4: the match is sped up. It outlasts the match.
    pub sped_up: &'a mut bool,
    /// F speeds the match up or slows it down (league and cup matches in
    /// manager mode). Elsewhere F is the port's fast forward.
    pub may_speed_up: bool,
}

/// `maybe_serial`: a long the loader leaves at boot, saved with each
/// replay. It was at `$80` in the original's memory.
pub const SERIAL: u32 = 0x167F;

/// Saving goal replays (`save_replay`): the original allowed it in one
/// player games, but not in manager mode.
pub struct ReplaySaves<'a> {
    pub disk: &'a SaveDisk,
    /// [`SERIAL`], saved with each replay.
    pub serial: u32,
    /// "DISK ERROR" on the panel.
    pub disk_error: Text,
}

impl<'a> ReplaySaves<'a> {
    pub fn new(disk: &'a SaveDisk, text: &MenuText) -> Self {
        Self {
            disk,
            serial: SERIAL,
            disk_error: Text::new(text.messages.disk_error.clone()),
        }
    }

    /// Writes a replay to `slot`, asking to retry on a disk error
    /// (`save_replay`). `frame` is what is on screen.
    pub fn save(
        &self,
        ui: &mut Ui,
        renderer: &MatchRenderer,
        frame: &Canvas,
        replay: &SavedReplay,
        slot: u8,
    ) -> Flow {
        let bytes = replay.encode();
        while self.disk.write(replay_sector(slot), &bytes).is_err() {
            if !self.disk_error(ui, renderer, frame)? {
                break;
            }
        }
        Ok(())
    }

    /// `disk_error_message`: player 1's fire retries, Escape gives up.
    fn disk_error(&self, ui: &mut Ui, renderer: &MatchRenderer, frame: &Canvas) -> Flow<bool> {
        let mut shown = frame.clone();
        renderer.render_message(&self.disk_error, &mut shown);
        ui.play_sound(DISK_ERROR);
        ui.clear_input();
        loop {
            ui.show_frame(&shown, FRAME_TIME)?;
            let pressed = ui.next_press(|b| {
                b == Button::Joystick(Side::Team1, Control::Fire) || b == Button::Escape
            });
            match pressed {
                Some(Button::Escape) => return Ok(false),
                Some(_) => return Ok(true),
                None => {}
            }
        }
    }
}

/// Plays `game` to the end. Escape abandons it (`Stop::Abort`), as in the
/// original, except during a goal replay, where it ends the replay.
/// Returns the final score.
pub fn run_match(
    ui: &mut Ui,
    game: &mut Match,
    renderer: &MatchRenderer,
    controls: Controls,
) -> Flow<PerTeam<i32>> {
    ui.use_sound_bank(SoundBank::Match);
    game.set_accelerated(*controls.sped_up);
    let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    renderer.render(game, &mut canvas);
    // What a sped-up match leaves on screen.
    let mut view = canvas.clone();
    ui.transition_to(canvas.clone(), None)?;

    let mut playback = Playback::Running;
    let mut over = false;
    ui.clear_input();
    let result = loop {
        let mut steps = match playback {
            Playback::Running if game.is_accelerated() => SPED_UP_STEPS,
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
                Button::Escape if game.is_replaying() => game.end_replay(),
                Button::Escape => break_out(ui, game)?,
                Button::Number(slot) if game.is_replaying() => {
                    if let Some(saves) = &controls.replay_saves {
                        let replay = game.saved_replay(saves.serial);
                        saves.save(ui, renderer, &canvas, &replay, slot)?;
                    }
                }
                Button::Number(_) => {}
                // `run_replay`: F speeds the match up and ends the replay.
                Button::FastForward if controls.may_speed_up && game.is_replaying() => {
                    game.set_accelerated(true);
                    game.end_replay();
                }
                // `play_match`: F toggles it.
                Button::FastForward if controls.may_speed_up => {
                    game.set_accelerated(!game.is_accelerated());
                }
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
            break game.scores();
        }
        if over {
            playback = Playback::Paused;
        }
        ui.host().set_status(match playback {
            Playback::Running if game.is_accelerated() => "sped up: F for normal speed",
            Playback::Running => "",
            Playback::Paused => "paused: P resume, N step",
            Playback::FastForward => "fast forward",
        });
        if game.is_accelerated() {
            renderer.render_sped_up(game, &mut view, &mut canvas);
        } else {
            renderer.render(game, &mut canvas);
            view.pixels_mut().copy_from_slice(canvas.pixels());
        }
        ui.show_frame(&canvas, FRAME_TIME)?;
    };
    *controls.sped_up = game.is_accelerated();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::data::bytes::BigEndian;
    use crate::data::files::read_amiga_match_dump;

    #[test]
    fn the_serial_is_the_originals() {
        let Ok(dump) = read_amiga_match_dump(&Config::default().data_dir) else {
            eprintln!("skipping: no originals");
            return;
        };
        assert_eq!(dump.u32_at(0x80), SERIAL);
    }
}
