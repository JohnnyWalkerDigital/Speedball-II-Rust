//! The desktop shell: window, keyboard and frame pacing.

use std::error::Error;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};

use crate::config::Config;
use crate::data::files::{read_amiga_match_dump, AmigaFiles};
use crate::data::{sounds, AtariRom, GameFiles};
use crate::game::geometry::{Input, Side};
use crate::game::{Match, MatchSetup};
use crate::graphics::{Assets, Canvas};
use crate::render::MatchRenderer;
use crate::sound::SoundOutput;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;
/// The original ran on a 1000/31 ms timer.
const FRAME_TIME: Duration = Duration::from_millis(1000 / 31);
const FAST_FORWARD_STEPS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Playback {
    Running,
    Paused,
    FastForward,
}

#[derive(Clone, Copy)]
enum Control {
    Up,
    Down,
    Left,
    Right,
    Fire,
}

const BINDINGS: &[(Key, Side, Control)] = &[
    (Key::Up, Side::Team1, Control::Up),
    (Key::Down, Side::Team1, Control::Down),
    (Key::Left, Side::Team1, Control::Left),
    (Key::Right, Side::Team1, Control::Right),
    (Key::Space, Side::Team1, Control::Fire),
    (Key::RightCtrl, Side::Team1, Control::Fire),
    (Key::W, Side::Team2, Control::Up),
    (Key::S, Side::Team2, Control::Down),
    (Key::A, Side::Team2, Control::Left),
    (Key::D, Side::Team2, Control::Right),
    (Key::LeftShift, Side::Team2, Control::Fire),
    (Key::LeftCtrl, Side::Team2, Control::Fire),
];

/// Saves the Amiga sound samples as WAV files.
pub fn export_sounds(config: &Config, dir: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let files = GameFiles::load(&config.data_dir, crate::config::Platform::Amiga)?;
    let amiga = files.amiga.expect("loaded for the Amiga");
    let samples = sounds::export_wavs(&amiga.match_dump, dir)?;
    println!(
        "Wrote {samples} raw instrument samples to {} (at {} Hz, which may not be their real pitch)",
        dir.display(),
        sounds::EXPORT_RATE
    );
    let rendered = dir.join("rendered");
    let count = crate::sound::export_sounds(amiga.match_dump, &rendered)?;
    println!(
        "Wrote {count} game sounds, played through the driver, to {}",
        rendered.display()
    );
    Ok(())
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let files = GameFiles::load(&config.data_dir, config.platform)?;
    let rom = Rc::new(AtariRom::new(files.atari_disk));
    let assets = Assets::load(&rom, files.amiga.as_ref(), config.platform);
    let mut game = Match::new(MatchSetup::new(&rom, &config), Rc::clone(&rom));
    let renderer = MatchRenderer::new(assets, &rom);
    let sound = if config.mute {
        None
    } else {
        start_sound(&config, &files.amiga)
    };
    let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    let mut window = open_window(config.scale)?;
    let title = format!(
        "Speedball 2 - {} vs {}",
        game.teams.team1.full_name.to_display_string(),
        game.teams.team2.full_name.to_display_string()
    );
    println!("{title} (seed {})", config.seed);

    let mut playback = Playback::Running;
    let mut shown_playback = None;
    let mut next_frame = Instant::now();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        apply_joystick_keys(&window, &mut game);
        let mut steps = match playback {
            Playback::Running => 1,
            Playback::FastForward => FAST_FORWARD_STEPS,
            Playback::Paused => 0,
        };
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::P if playback == Playback::Paused => playback = Playback::Running,
                Key::P => playback = Playback::Paused,
                Key::F if playback == Playback::FastForward => playback = Playback::Running,
                Key::F => playback = Playback::FastForward,
                Key::N if playback == Playback::Paused => steps = 1,
                _ => {}
            }
        }
        for _ in 0..steps {
            let over = game.step();
            for request in game.take_sounds() {
                if let Some(sound) = &sound {
                    sound.play(request);
                }
            }
            if over {
                // The original also stopped once the teams had lined up.
                playback = Playback::Paused;
                break;
            }
        }

        if shown_playback != Some(playback) {
            let suffix = match playback {
                Playback::Running => "",
                Playback::Paused => " [paused: P resume, N step]",
                Playback::FastForward => " [fast forward]",
            };
            window.set_title(&format!("{title}{suffix}"));
            shown_playback = Some(playback);
        }
        renderer.render(&game, &mut canvas);
        window.update_with_buffer(canvas.pixels(), canvas.width(), canvas.height())?;

        next_frame += FRAME_TIME;
        let now = Instant::now();
        match next_frame.checked_duration_since(now) {
            Some(wait) => thread::sleep(wait),
            None => next_frame = now, // Running late: don't try to catch up.
        }
    }
    Ok(())
}

/// The sound driver runs on the Amiga data, whichever graphics are shown.
/// Without it (or without an audio device) the game is silent.
fn start_sound(config: &Config, amiga: &Option<AmigaFiles>) -> Option<SoundOutput> {
    let dump = match amiga {
        Some(files) => Ok(files.match_dump.clone()),
        None => read_amiga_match_dump(&config.data_dir),
    };
    let started = dump.map_err(|e| e.to_string()).and_then(SoundOutput::start);
    match started {
        Ok(output) => Some(output),
        Err(error) => {
            eprintln!("sound off: {error}");
            None
        }
    }
}

fn open_window(scale: u8) -> Result<Window, minifb::Error> {
    let scale = match scale {
        1 => Scale::X1,
        2 => Scale::X2,
        8 => Scale::X8,
        _ => Scale::X4,
    };
    let mut window = Window::new(
        "Speedball 2",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale,
            ..WindowOptions::default()
        },
    )?;
    // We pace frames ourselves.
    window.set_target_fps(0);
    // Roughly the Windows defaults the original's key handling relied on.
    window.set_key_repeat_delay(0.5);
    window.set_key_repeat_rate(0.033);
    Ok(window)
}

/// Key presses set input bits and release clears them, like the original's
/// key events (the AI also writes to these bits, so they are not simply
/// overwritten from the keyboard state each frame).
fn apply_joystick_keys(window: &Window, game: &mut Match) {
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        for &(_, side, control) in BINDINGS.iter().filter(|(k, _, _)| *k == key) {
            game.set_computer_control(side, false);
            set_control(game.input_mut(side), control, true);
        }
    }
    for key in window.get_keys_released() {
        for &(_, side, control) in BINDINGS.iter().filter(|(k, _, _)| *k == key) {
            set_control(game.input_mut(side), control, false);
        }
    }
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
