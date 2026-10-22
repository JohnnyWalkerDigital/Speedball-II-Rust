//! Headless tests of the menus against the real Amiga data, driven by a
//! scripted player. Skipped when the data files are missing.

use std::rc::Rc;
use std::time::Duration;

use crate::config::{Config, Platform};
use crate::data::amiga::{menu_ram, Overlays};
use crate::data::{AtariRom, GameFiles};
use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::graphics::{Assets, Canvas};
use crate::menu::assets::MenuAssets;
use crate::menu::flow::{self, Game};
use crate::menu::host::{Button, ButtonEvent, Closed, Control, Host, SoundBank};
use crate::menu::ram::MenuRam;
use crate::menu::ui::Ui;
use crate::render::MatchRenderer;

fn load_game() -> Option<Game> {
    let config = Config::default();
    let files = match GameFiles::load(&config.data_dir, Platform::Amiga) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("skipping: {error}");
            return None;
        }
    };
    let amiga = files.amiga.as_ref().expect("loaded for the Amiga");
    let overlays = Overlays::new(&amiga.disk).expect("the disk has overlays");
    let ram = MenuRam::new(menu_ram(&amiga.match_dump, &overlays));
    let assets = MenuAssets::load(&ram, &overlays).expect("menu pictures decode");
    let rom = Rc::new(AtariRom::new(files.atari_disk.clone()));
    let renderer = MatchRenderer::new(
        Assets::load(&rom, files.amiga.as_ref(), Platform::Amiga),
        &rom,
    );
    Some(Game::new(rom, ram, assets, renderer, 1))
}

/// What the script sees each frame.
struct Moment<'a> {
    time: Duration,
    frame: &'a Canvas,
    banks: &'a [(Duration, SoundBank)],
}

/// A host whose clock runs as fast as the game can go.
struct ScriptedHost<F: FnMut(&Moment) -> Option<Vec<ButtonEvent>>> {
    script: F,
    time: Duration,
    sounds: Vec<(Duration, Sound)>,
    banks: Vec<(Duration, SoundBank)>,
    /// Stops runaway scripts.
    limit: Duration,
}

impl<F: FnMut(&Moment) -> Option<Vec<ButtonEvent>>> ScriptedHost<F> {
    fn new(limit_seconds: u64, script: F) -> Self {
        Self {
            script,
            time: Duration::ZERO,
            sounds: Vec::new(),
            banks: Vec::new(),
            limit: Duration::from_secs(limit_seconds),
        }
    }
}

impl<F: FnMut(&Moment) -> Option<Vec<ButtonEvent>>> Host for ScriptedHost<F> {
    fn show(&mut self, frame: &Canvas, wait: Duration) -> Result<Vec<ButtonEvent>, Closed> {
        assert!(self.time < self.limit, "script ran past {:?}", self.limit);
        let moment = Moment {
            time: self.time,
            frame,
            banks: &self.banks,
        };
        let events = (self.script)(&moment).ok_or(Closed)?;
        self.time += wait;
        Ok(events)
    }

    fn play_sound(&mut self, sound: Sound) {
        self.sounds.push((self.time, sound));
    }

    fn use_sound_bank(&mut self, bank: SoundBank) {
        self.banks.push((self.time, bank));
    }
}

fn press(button: Button) -> Vec<ButtonEvent> {
    let pressed = ButtonEvent {
        button,
        pressed: true,
        repeat: false,
    };
    vec![pressed, ButtonEvent::release(button)]
}

const FIRE_1: Button = Button::Joystick(Side::Team1, Control::Fire);
const DOWN_1: Button = Button::Joystick(Side::Team1, Control::Down);

fn at(moment: &Moment, seconds: f64) -> bool {
    let t = Duration::from_secs_f64(seconds);
    moment.time >= t && moment.time < t + Duration::from_millis(20)
}

#[test]
fn the_demo_starts_after_thirty_idle_seconds_then_returns_to_the_menu() {
    let Some(mut game) = load_game() else { return };
    let mut host = ScriptedHost::new(600, |m| {
        // Stop once the demo has been and gone.
        (m.banks.len() < 2 || m.time < m.banks[1].0 + Duration::from_secs(1)).then(Vec::new)
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    let banks: Vec<_> = host.banks.iter().map(|b| b.1).collect();
    assert_eq!(&banks[..2], [SoundBank::Match, SoundBank::Menu]);
    // 1500 frames on the menu, after fading it in.
    let start = host.banks[0].0.as_secs_f64();
    assert!((30.0..32.0).contains(&start), "demo started at {start}s");
}

#[test]
fn escape_on_the_main_menu_quits() {
    let Some(mut game) = load_game() else { return };
    let mut host = ScriptedHost::new(60, |m| {
        Some(if at(m, 2.0) {
            press(Button::Escape)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert!(host.time < Duration::from_secs(3));
}

#[test]
fn escape_leaves_a_match_for_the_main_menu() {
    let Some(mut game) = load_game() else { return };
    let mut escaped = None;
    let mut host = ScriptedHost::new(120, |m| {
        if m.banks.len() == 2 {
            // Back on the menus: done once the main menu is up again.
            return (m.time < m.banks[1].0 + Duration::from_secs(3)).then(Vec::new);
        }
        let events = if at(m, 2.0) || at(m, 4.0) {
            // "1 PLAYER GAME", then "KNOCKOUT".
            press(FIRE_1)
        } else if m.banks.len() == 1
            && escaped.is_none()
            && m.time > m.banks[0].0 + Duration::from_secs(5)
        {
            escaped = Some(m.time);
            press(Button::Escape)
        } else {
            vec![]
        };
        Some(events)
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    let banks: Vec<_> = host.banks.iter().map(|b| b.1).collect();
    assert_eq!(banks, [SoundBank::Match, SoundBank::Menu]);
    // The menu sounds: select twice (and nothing for the match's end).
    let selects = host.sounds.iter().filter(|(_, s)| s.id == 0x16).count();
    assert_eq!(selects, 2);
    // The end-of-match sounds play when leaving.
    let leaving: Vec<_> = host
        .sounds
        .iter()
        .filter(|(t, _)| *t >= host.banks[1].0 - Duration::from_millis(40))
        .map(|(_, s)| s.id)
        .collect();
    assert_eq!(&leaving[..2], [0, 39]);
}

#[test]
fn two_player_matches_have_two_legs_with_ends_swapped() {
    let Some(mut game) = load_game() else { return };
    let mut host = ScriptedHost::new(3000, |m| {
        if m.banks.len() >= 4 {
            // Two legs played (each switches the samples in and out).
            return None;
        }
        // "2 PLAYER GAME", then "ONE MATCH".
        Some(if at(m, 2.0) {
            press(DOWN_1)
        } else if at(m, 4.0) || at(m, 6.0) {
            press(FIRE_1)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    let banks: Vec<_> = host.banks.iter().map(|b| b.1).collect();
    assert_eq!(
        banks,
        [
            SoundBank::Match,
            SoundBank::Menu,
            SoundBank::Match,
            SoundBank::Menu
        ]
    );
}

/// Writes frames of the menus to `SPEEDBALL_FRAMES_DIR` as BMPs.
#[test]
#[ignore = "writes images; run manually"]
fn dump_menu_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    let Some(mut game) = load_game() else { return };
    let shots = [1.5, 3.5, 5.5, 7.5, 9.5];
    let mut host = ScriptedHost::new(60, |m| {
        if let Some(i) = shots.iter().position(|&s| at(m, s)) {
            let path = format!("{dir}/menu_{i}.bmp");
            std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
        }
        if m.time > Duration::from_secs(10) {
            return None;
        }
        Some(if at(m, 2.0) {
            press(DOWN_1)
        } else if at(m, 4.0) || at(m, 6.0) {
            press(FIRE_1)
        } else if at(m, 8.0) {
            press(Button::Escape)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
}

/// Writes the screens after each match (one-player knockout, then two
/// players) to `SPEEDBALL_FRAMES_DIR`.
#[test]
#[ignore = "writes images; run manually"]
fn dump_result_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    for (name, down) in [("knockout", false), ("two_players", true)] {
        let Some(mut game) = load_game() else { return };
        let mut host = ScriptedHost::new(3000, |m| {
            if let Some(&(t, SoundBank::Menu)) = m.banks.last() {
                for (shot, after) in [(0, 2.0), (1, 7.5), (2, 13.0)] {
                    if at(m, t.as_secs_f64() + after) {
                        let path = format!("{dir}/{name}_{}_{shot}.bmp", m.banks.len());
                        std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
                    }
                }
            }
            if m.banks.len() >= 4 {
                return None;
            }
            Some(if at(m, 2.0) && down {
                press(DOWN_1)
            } else if at(m, 4.0) || at(m, 6.0) {
                press(FIRE_1)
            } else {
                vec![]
            })
        });
        flow::run(&mut Ui::new(&mut host), &mut game);
    }
}

#[test]
fn a_knockout_defeat_is_recorded_and_paid() {
    use crate::game::career::TeamId;
    let Some(mut game) = load_game() else { return };
    let mut host = ScriptedHost::new(600, |m| {
        if m.banks.len() == 2 {
            // Past the defeat picture, on "GAME OVER".
            return (m.time < m.banks[1].0 + Duration::from_secs(8)).then(Vec::new);
        }
        // "1 PLAYER GAME", then "KNOCKOUT"; then Brutal Deluxe stand still.
        Some(if at(m, 2.0) || at(m, 4.0) {
            press(FIRE_1)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);

    let career = &game.career;
    let last = career.last_match.expect("the match was played to the end");
    assert_eq!(last.teams.team2, TeamId::League(1));
    let revolver = career.record(TeamId::League(1));
    assert_eq!((revolver.played, revolver.won), (1, 1));
    let brutal_deluxe = career.record(TeamId::League(0));
    assert_eq!((brutal_deluxe.played, brutal_deluxe.lost), (1, 1));
    assert_eq!(brutal_deluxe.points_against, revolver.points_for);
    assert!(last.statistics.team2.goals_scored > 0);
    assert!(last.statistics.team2.time_in_possession > 0);

    // What each team earned (at most 1100) is banked; the cap for the next
    // match grows by whatever it fell short.
    for (cash, cap) in [
        (career.money.cash.team1, career.money.cap.team1),
        (career.money.cash.team2, career.money.cap.team2),
    ] {
        let prize = cash - 1100;
        assert!(prize <= 1100 && prize % 100 == 0, "earned {prize}");
        assert_eq!(cap, 1100 + (1100 - prize));
    }
    assert!(career.money.cash.team2 > 1100, "Revolver picked up coins");
}
