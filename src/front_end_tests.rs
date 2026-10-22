//! Headless tests of the menus against the real Amiga data, driven by a
//! scripted player. Skipped when the data files are missing.

use std::rc::Rc;
use std::time::Duration;

use crate::config::{Config, Platform};
use crate::data::amiga::{menu_ram, Overlays};
use crate::data::save_disk::SaveDisk;
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
    let disk = std::env::temp_dir().join(format!(
        "sb2-test-{}-{:?}.adf",
        std::process::id(),
        std::thread::current().id()
    ));
    load_game_with_disk(SaveDisk::new(disk))
}

fn load_game_with_disk(save_disk: SaveDisk) -> Option<Game> {
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
    Some(Game::new(rom, ram, assets, renderer, save_disk, 1))
}

/// What the script sees each frame.
struct Moment<'a> {
    time: Duration,
    frame: &'a Canvas,
    banks: &'a [(Duration, SoundBank)],
    sounds: &'a [(Duration, Sound)],
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
            sounds: &self.sounds,
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
const UP_1: Button = Button::Joystick(Side::Team1, Control::Up);
const FIRE_2: Button = Button::Joystick(Side::Team2, Control::Fire);
const UP_2: Button = Button::Joystick(Side::Team2, Control::Up);

/// Gets through the gyms of a two-player game: each player in turn
/// presses up (onto the gym's exit key) and fire, every half second.
fn two_player_autopilot(m: &Moment) -> Vec<ButtonEvent> {
    let step = (m.time.as_millis() / 500) % 4;
    if m.time.as_millis() % 500 >= 20 {
        return vec![];
    }
    press([UP_1, FIRE_1, UP_2, FIRE_2][step as usize])
}

/// Gets through screens by pressing fire every 0.3 seconds. The manager
/// screen's keypad must start on its exit key; when the transfer key
/// flashes (players for sale), the cursor jumps to it, so it moves right
/// twice back to the exit.
#[derive(Default)]
struct Autopilot {
    flash: Option<Duration>,
    rights_to_press: u8,
}

impl Autopilot {
    const FLASH: u8 = 8;

    fn step(&mut self, m: &Moment) -> Vec<ButtonEvent> {
        // The same sound ticks down power-ups in a match; ignore those.
        let in_match = m.banks.last().is_some_and(|b| b.1 == SoundBank::Match);
        let latest_flash = m
            .sounds
            .iter()
            .rev()
            .find(|(_, s)| s.id == Self::FLASH && s.priority == 2)
            .map(|(t, _)| *t);
        if !in_match && latest_flash > self.flash.map(|f| f + Duration::from_secs(2)) {
            self.flash = latest_flash;
            self.rights_to_press = 2;
        }
        if let Some(flash) = self.flash {
            if m.time.saturating_sub(flash) < Duration::from_millis(1200) {
                return vec![];
            }
        }
        if self.rights_to_press > 0 {
            if m.time.as_millis() % 100 < 20 {
                self.rights_to_press -= 1;
                return press(Button::Joystick(Side::Team1, Control::Right));
            }
            return vec![];
        }
        keep_firing(m)
    }
}

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
        let events = if at(m, 2.0) || at(m, 4.0) || at(m, 7.0) || at(m, 10.0) {
            // "1 PLAYER GAME", "KNOCKOUT", past the next fixture, then the
            // gym's exit key (up from where it starts).
            press(FIRE_1)
        } else if at(m, 9.5) {
            press(UP_1)
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
        // "2 PLAYER GAME", then "ONE MATCH"; then through the gyms.
        Some(if at(m, 2.0) {
            press(DOWN_1)
        } else if at(m, 4.0) || at(m, 6.0) {
            press(FIRE_1)
        } else if m.time > Duration::from_secs(7) {
            two_player_autopilot(m)
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
        // "1 PLAYER GAME", "KNOCKOUT", past the next fixture; then Brutal
        // Deluxe stand still.
        Some(if at(m, 2.0) || at(m, 4.0) || at(m, 7.0) || at(m, 10.0) {
            press(FIRE_1)
        } else if at(m, 9.5) {
            // Out of the gym: up to its exit key.
            press(UP_1)
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

/// Writes the league's screens to `SPEEDBALL_FRAMES_DIR`: the menus, the
/// next fixture, then what follows the first match.
#[test]
#[ignore = "writes images; run manually"]
fn dump_league_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    let Some(mut game) = load_game() else { return };
    let mut shot = 0;
    let mut host = ScriptedHost::new(1200, |m| {
        let t = m.time.as_secs_f64();
        let after_match = m.banks.len() >= 2;
        let since = m.banks.last().map_or(0.0, |b| t - b.0.as_secs_f64());
        // Before the match: one player, league, new game, past the fixture.
        let presses = [2.0, 4.0, 4.5, 7.0, 10.0];
        let pre = !after_match && m.banks.is_empty() && presses.iter().any(|&p| at(m, p));
        // After it: fire every three seconds, one shot just before each.
        let post_shot = after_match && since > 1.0 && ((since - 2.0) % 3.0).abs() < 0.01;
        let post_press = after_match && since > 1.0 && ((since - 2.5) % 3.0).abs() < 0.01;
        if (m.banks.is_empty() && [3.0, 6.5, 9.5].iter().any(|&p| at(m, p))) || post_shot {
            let path = format!("{dir}/league_{shot:02}.bmp");
            std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
            shot += 1;
        }
        if shot >= 14 {
            return None;
        }
        Some(if at(m, 4.0) {
            press(DOWN_1)
        } else if pre || post_press {
            press(FIRE_1)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
}

/// Presses fire every 0.3 seconds (after a scripted start), to get
/// through screens.
fn keep_firing(m: &Moment) -> Vec<ButtonEvent> {
    let ms = m.time.as_millis();
    if ms % 300 < 20 {
        press(FIRE_1)
    } else {
        vec![]
    }
}

#[test]
fn a_half_season_of_the_league_plays_every_team_seven_times() {
    use crate::game::career::TeamId;
    let Some(mut game) = load_game() else { return };
    game.cursors.keypad = (2, 0);
    let mut pilot = Autopilot::default();
    // Brutal Deluxe play one match a round: seven, each switching the
    // samples in and out.
    let mut host = ScriptedHost::new(3000, |m| {
        if m.banks.len() >= 14 {
            return (m.time < m.banks[13].0 + Duration::from_secs(5)).then(|| pilot.step(m));
        }
        // One player, league (down once), team manager (down once).
        Some(if at(m, 2.0) || at(m, 5.0) || at(m, 8.0) {
            press(FIRE_1)
        } else if at(m, 4.0) || at(m, 7.0) {
            press(DOWN_1)
        } else if m.time > Duration::from_secs(10) {
            pilot.step(m)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);

    assert!(game.manager_mode);
    let career = &game.career;
    let records: Vec<_> = (0..16).map(|i| *career.record(TeamId::League(i))).collect();
    for record in &records {
        assert_eq!(record.played, 7, "{record:?}");
        assert_eq!(record.won + record.drawn + record.lost, 7);
    }
    let wins: u16 = records.iter().map(|r| r.won).sum();
    let losses: u16 = records.iter().map(|r| r.lost).sum();
    assert_eq!(wins, losses);
    let mut placings: Vec<u8> = records.iter().map(|r| r.league_placing).collect();
    placings.sort_unstable();
    assert_eq!(placings, (1..=16).collect::<Vec<u8>>());
    // Team manager mode guarantees 300 a match.
    assert!(career.money.cash.team1 >= 1100 + 7 * 300);
}

#[test]
fn losing_a_cup_tie_ends_the_cup_and_restores_the_coins() {
    use crate::game::career::TeamId;
    let Some(mut game) = load_game() else { return };
    game.cursors.keypad = (2, 0);
    let mut pilot = Autopilot::default();
    let mut host = ScriptedHost::new(3000, |m| {
        if m.banks.len() >= 4 {
            // Both legs played: stop once past the defeat picture (the
            // second leg is recorded after it), before another game.
            return (m.time < m.banks[3].0 + Duration::from_secs(2)).then(|| pilot.step(m));
        }
        // One player, cup (down twice), new game; then fire through.
        Some(if at(m, 2.0) || at(m, 6.0) || at(m, 9.0) {
            press(FIRE_1)
        } else if at(m, 4.0) || at(m, 5.0) {
            press(DOWN_1)
        } else if m.time > Duration::from_secs(11) {
            pilot.step(m)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);

    assert_eq!(game.coin_value, 100, "the cup's double coins end with it");
    let brutal_deluxe = game.career.record(TeamId::League(0));
    assert_eq!(brutal_deluxe.played, 2, "{brutal_deluxe:?}");
    assert_eq!(brutal_deluxe.lost, 2);
    // A new cup gives both players 2000.
    assert!(game.career.money.cash.team1 >= 2000);
    let banks: Vec<_> = host.banks.iter().map(|b| b.1).collect();
    assert_eq!(banks.len(), 4, "knocked out after two legs");
}

/// Writes the cup's round title and the aggregate picture after a tie to
/// `SPEEDBALL_FRAMES_DIR`.
#[test]
#[ignore = "writes images; run manually"]
fn dump_cup_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    let Some(mut game) = load_game() else { return };
    let mut shot = 0;
    let mut host = ScriptedHost::new(3000, |m| {
        let snap = |shot: &mut usize| {
            let path = format!("{dir}/cup_{shot:02}.bmp");
            std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
            *shot += 1;
        };
        if at(m, 11.0) {
            snap(&mut shot);
        }
        if m.banks.len() >= 4 {
            let since = (m.time - m.banks[3].0).as_secs_f64();
            if (since - 1.4).abs() < 0.01
                || (since - 2.4).abs() < 0.01
                || (since - 3.9).abs() < 0.01
            {
                snap(&mut shot);
            }
            let fire = (since - 1.6).abs() < 0.01 || (since - 2.6).abs() < 0.01;
            return (since < 4.0).then(|| if fire { press(FIRE_1) } else { vec![] });
        }
        Some(if at(m, 2.0) || at(m, 6.0) || at(m, 9.0) {
            press(FIRE_1)
        } else if at(m, 4.0) || at(m, 5.0) {
            press(DOWN_1)
        } else if m.time > Duration::from_secs(12) && m.banks.len() % 2 == 0 {
            keep_firing(m)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
}

/// Writes the manager screens to `SPEEDBALL_FRAMES_DIR`: a new league's
/// manager screen, then the gym, buying an upgrade for the team.
#[test]
#[ignore = "writes images; run manually"]
fn dump_manager_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    let Some(mut game) = load_game() else { return };
    let right = Button::Joystick(Side::Team1, Control::Right);
    // (time, button) presses, then shots at the given times.
    let presses: Vec<(f64, Button)> = vec![
        (2.0, FIRE_1), // 1 player
        (4.0, DOWN_1), // league
        (5.0, FIRE_1),
        (8.0, FIRE_1),  // new game
        (11.0, FIRE_1), // past the next fixture
        (16.0, DOWN_1), // to the gym key
        (17.0, FIRE_1), // into the gym
        (21.0, right),  // keypad: fixture -> right key
        (22.0, right),  // off the keypad onto the upgrades
        (23.0, FIRE_1), // choose it
        (24.0, FIRE_1), // buy it
    ];
    let shots = [15.0, 20.5, 22.5, 23.5, 24.5];
    let mut host = ScriptedHost::new(60, |m| {
        if let Some(i) = shots.iter().position(|&s| at(m, s)) {
            let path = format!("{dir}/manager_{i}.bmp");
            std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
        }
        if m.time > Duration::from_secs(25) {
            return None;
        }
        Some(
            presses
                .iter()
                .find(|(t, _)| at(m, *t))
                .map_or(vec![], |&(_, b)| press(b)),
        )
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
}

#[test]
fn the_knockout_gym_trains_the_squad_for_the_match() {
    let Some(mut game) = load_game() else { return };
    let right = Button::Joystick(Side::Team1, Control::Right);
    let left = Button::Joystick(Side::Team1, Control::Left);
    let presses: Vec<(f64, Button)> = vec![
        (2.0, FIRE_1), // 1 player
        (4.0, FIRE_1), // knockout
        (7.0, FIRE_1), // past the next fixture
        (10.0, right), // gym keypad: fixture -> right key
        (10.5, right), // onto the upgrade buttons: power elbows
        (11.0, FIRE_1),
        (11.5, FIRE_1), // buy them for the team
        (12.0, left),   // back onto the keypad
        (12.5, left),
        (13.0, UP_1), // to the exit key
        (13.5, FIRE_1),
    ];
    let mut host = ScriptedHost::new(60, |m| {
        if !m.banks.is_empty() {
            // The match has started.
            return None;
        }
        Some(
            presses
                .iter()
                .find(|(t, _)| at(m, *t))
                .map_or(vec![], |&(_, b)| press(b)),
        )
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert_eq!(host.banks.len(), 1, "on to the match");
    // Power elbows (throwing, ten credits a player) for all twelve.
    assert_eq!(game.career.money.cash.team1, 1100 - 120);
    for player in &game.career.squads.team1 {
        assert_eq!(player.skills[4], 110, "{player:?}");
        assert_eq!(player.skills[0], 100);
    }
    assert!(
        host.sounds.iter().any(|(_, s)| s.id == 0x22),
        "the purchase sound"
    );
}

#[test]
fn a_league_saved_from_the_manager_screen_loads_back() {
    use crate::game::career::TeamId;
    use crate::game::save::{Progress, SaveGame};
    let disk_path = std::env::temp_dir().join(format!("sb2-save-{}.adf", std::process::id()));
    let _ = std::fs::remove_file(&disk_path);
    let Some(mut game) = load_game_with_disk(SaveDisk::new(&disk_path)) else {
        return;
    };
    game.cursors.keypad = (2, 0);
    let left = Button::Joystick(Side::Team1, Control::Left);
    let right = Button::Joystick(Side::Team1, Control::Right);
    // One player, league, team manager, past the next fixture; then on the
    // manager screen: left to the save key, save, back right to exit.
    let presses: Vec<(f64, Button)> = vec![
        (2.0, FIRE_1),
        (4.0, DOWN_1),
        (5.0, FIRE_1),
        (7.0, DOWN_1),
        (8.0, FIRE_1),
        (11.0, FIRE_1),
        (14.0, left),
        (14.5, FIRE_1),
        (17.0, FIRE_1), // "PLEASE INSERT GAME SAVE DISK"
        (20.0, right),
        (20.5, FIRE_1),
    ];
    let mut host = ScriptedHost::new(60, |m| {
        if !m.banks.is_empty() {
            return None; // The match has started.
        }
        Some(
            presses
                .iter()
                .find(|(t, _)| at(m, *t))
                .map_or(vec![], |&(_, b)| press(b)),
        )
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert_eq!(host.banks.len(), 1, "on to the first match");

    let disk = SaveDisk::new(&disk_path);
    let bytes = disk.read(0, 2).unwrap();
    let save = SaveGame::decode(&bytes, false).expect("a league save at sector 0");
    assert_eq!(save.progress, Progress::League { round: 0, game: 0 });
    assert_eq!(save.flags & 1, 1, "team manager");
    assert_eq!(save.cash, 1100);
    assert_eq!(
        save.encode(),
        bytes[..save.encode().len()],
        "the original layout"
    );
    let records = |game: &Game| -> Vec<(u16, u16, u16)> {
        (0..16)
            .map(|i| {
                let r = game.career.record(TeamId::League(i));
                (r.played, r.league_points, r.points_for)
            })
            .collect()
    };
    let saved_records = records(&game);

    // A fresh game loads it: "LOAD GAME" is the third item.
    let Some(mut loaded) = load_game_with_disk(SaveDisk::new(&disk_path)) else {
        return;
    };
    loaded.cursors.keypad = (2, 0);
    let presses: Vec<(f64, Button)> = vec![
        (2.0, FIRE_1),
        (4.0, DOWN_1),
        (5.0, FIRE_1),
        (7.0, DOWN_1),
        (7.5, DOWN_1),
        (8.0, FIRE_1),
        (11.0, FIRE_1), // "PLEASE INSERT GAME SAVE DISK"
        (14.0, FIRE_1), // the next fixture
        (17.0, FIRE_1), // exit the manager screen
    ];
    let mut host = ScriptedHost::new(60, |m| {
        if !m.banks.is_empty() {
            return None;
        }
        Some(
            presses
                .iter()
                .find(|(t, _)| at(m, *t))
                .map_or(vec![], |&(_, b)| press(b)),
        )
    });
    flow::run(&mut Ui::new(&mut host), &mut loaded);
    assert_eq!(host.banks.len(), 1, "the loaded league plays on");
    assert!(loaded.manager_mode);
    assert_eq!(records(&loaded), saved_records);
    // Placings are worked out again from the saved fixture order; with
    // everyone level at the start of a season, they follow it.
    let placing = |i| loaded.career.record(TeamId::League(i)).league_placing;
    assert_eq!(
        placing(0),
        9,
        "Brutal Deluxe stay first in division 2's fixtures"
    );
    assert_eq!(save.teams[0].team_offset, 0);
    assert_eq!(loaded.career.squads.team1[0].name, "BARRY");
    let _ = std::fs::remove_file(&disk_path);
}
