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
use crate::menu::strings::MenuText;
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
    let text = MenuText::read(&ram);
    Some(Game::new(rom, ram, text, assets, renderer, save_disk, 1))
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

/// What a scripted player presses at a moment.
type Script = dyn Fn(&Moment) -> Vec<ButtonEvent>;

/// Writes the frame to `SPEEDBALL_FRAMES_DIR`, if set.
fn dump_frame(m: &Moment, name: &str) {
    if let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") {
        let path = format!("{dir}/{name}.bmp");
        std::fs::write(path, crate::simulation_tests::bmp(m.frame)).unwrap();
    }
}

/// Whether this is the frame `seconds` after `start`.
fn at_since(moment: &Moment, start: Duration, seconds: f64) -> bool {
    let t = start + Duration::from_secs_f64(seconds);
    moment.time >= t && moment.time < t + Duration::from_millis(20)
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
    if std::env::var("SPEEDBALL_FRAMES_DIR").is_err() {
        return;
    }
    // The knockout: one player, knockout, past the fixture, out of the gym
    // (up to its exit key), then Brutal Deluxe stand still and lose.
    let knockout = |m: &Moment| {
        if at(m, 2.0) || at(m, 4.0) || at(m, 7.0) || at(m, 10.0) {
            press(FIRE_1)
        } else if at(m, 9.5) {
            press(UP_1)
        } else {
            vec![]
        }
    };
    // Two players: "2 PLAYER GAME", "ONE MATCH", then through the gyms.
    let two_players = |m: &Moment| {
        if at(m, 2.0) {
            press(DOWN_1)
        } else if at(m, 4.0) || at(m, 6.0) {
            press(FIRE_1)
        } else if m.time > Duration::from_secs(7) {
            two_player_autopilot(m)
        } else {
            vec![]
        }
    };
    let games: [(&str, usize, &Script); 2] =
        [("knockout", 2, &knockout), ("two_players", 4, &two_players)];
    for (name, matches_banks, script) in games {
        let Some(mut game) = load_game() else { return };
        let mut host = ScriptedHost::new(3000, |m| {
            // After each match, three shots of what follows, hands off.
            if let Some(&(back, SoundBank::Menu)) = m.banks.last() {
                let since = (m.time - back).as_secs_f64();
                for (shot, after) in [(0, 2.0), (1, 7.5), (2, 13.0)] {
                    if at_since(m, back, after) {
                        dump_frame(m, &format!("{name}_{}_{shot}", m.banks.len()));
                    }
                }
                if since < 14.0 {
                    return Some(vec![]);
                }
                if m.banks.len() >= matches_banks {
                    return None;
                }
            }
            Some(script(m))
        });
        flow::run(&mut Ui::new(&mut host), &mut game);
        assert!(
            host.banks.len() >= matches_banks,
            "{name}: the matches were played"
        );
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
    if std::env::var("SPEEDBALL_FRAMES_DIR").is_err() {
        return;
    }
    let Some(mut game) = load_game() else { return };
    game.cursors.keypad = (2, 0);
    let mut pilot = Autopilot::default();
    let mut shot = 0;
    let mut host = ScriptedHost::new(1200, |m| {
        let mut snap = |m: &Moment| {
            dump_frame(m, &format!("league_{shot:02}"));
            shot += 1;
        };
        match m.banks {
            // The menus: one player, league, new game, past the fixture,
            // then off the manager screen.
            [] => {
                if [3.0, 6.5, 9.5].iter().any(|&t| at(m, t)) {
                    snap(m);
                }
                Some(if at(m, 4.0) {
                    press(DOWN_1)
                } else if [2.0, 4.5, 7.0, 10.0].iter().any(|&t| at(m, t)) {
                    press(FIRE_1)
                } else if m.time > Duration::from_secs(11) {
                    pilot.step(m)
                } else {
                    vec![]
                })
            }
            // Brutal Deluxe stand still through the match.
            [_] => Some(vec![]),
            // After it: a shot every three seconds, each followed by fire,
            // up to the next manager screen.
            [.., (back, _)] => {
                let back = *back;
                if (1..12).any(|n| at_since(m, back, 3.0 * n as f64 - 1.0)) {
                    snap(m);
                }
                if shot >= 9 {
                    return None;
                }
                let fire = (1..12).any(|n| at_since(m, back, 3.0 * n as f64 - 0.5));
                Some(if fire { press(FIRE_1) } else { vec![] })
            }
        }
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert_eq!(shot, 9, "every shot taken");
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
    if std::env::var("SPEEDBALL_FRAMES_DIR").is_err() {
        return;
    }
    let Some(mut game) = load_game() else { return };
    game.cursors.keypad = (2, 0);
    let mut pilot = Autopilot::default();
    let mut shot = 0;
    let mut host = ScriptedHost::new(3000, |m| {
        let mut snap = |m: &Moment| {
            dump_frame(m, &format!("cup_{shot:02}"));
            shot += 1;
        };
        if at(m, 11.0) {
            snap(m);
        }
        // After the second leg: the result, the defeat and what follows.
        if let Some(&(back, _)) = m.banks.get(3) {
            if [1.4, 2.4, 3.9].iter().any(|&t| at_since(m, back, t)) {
                snap(m);
            }
            let fire = [1.6, 2.6].iter().any(|&t| at_since(m, back, t));
            let more = m.time < back + Duration::from_secs(4);
            return more.then(|| if fire { press(FIRE_1) } else { vec![] });
        }
        // One player, cup (down twice), new game; then fire through.
        Some(if at(m, 2.0) || at(m, 6.0) || at(m, 9.0) {
            press(FIRE_1)
        } else if at(m, 4.0) || at(m, 5.0) {
            press(DOWN_1)
        } else if m.time > Duration::from_secs(12) {
            pilot.step(m)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert_eq!(shot, 4, "every shot taken");
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

#[test]
fn a_goal_replay_saved_in_a_match_plays_back_from_the_menu() {
    use crate::data::save_disk::{replay_sector, REPLAY_SECTORS};
    use crate::game::replay::{Replay, ReplayTick, SavedReplay};
    use crate::menu::play::FRAME_TIME;
    use crate::menu::replays::replay_goals;
    let disk_path = std::env::temp_dir().join(format!("sb2-replay-{}.adf", std::process::id()));
    let _ = std::fs::remove_file(&disk_path);
    let Some(mut game) = load_game_with_disk(SaveDisk::new(&disk_path)) else {
        return;
    };
    // A knockout match where Brutal Deluxe stand still: key 3 during the
    // first goal's replay, then leave the match.
    let mut saved_at = None;
    let mut host = ScriptedHost::new(600, |m| {
        let goal = m
            .sounds
            .iter()
            .find(|(_, s)| *s == Sound::GOAL)
            .map(|(t, _)| *t);
        if let Some(goal) = goal {
            if saved_at.is_none() && m.time > goal {
                dump_frame(m, "replay_in_match");
                saved_at = Some(m.time);
                return Some(press(Button::Number(3)));
            }
            return (m.time < goal + Duration::from_secs(1)).then(Vec::new);
        }
        Some(if at(m, 2.0) || at(m, 4.0) || at(m, 7.0) || at(m, 10.0) {
            press(FIRE_1)
        } else if at(m, 9.5) {
            press(UP_1)
        } else {
            vec![]
        })
    });
    flow::run(&mut Ui::new(&mut host), &mut game);
    assert!(saved_at.is_some(), "a goal was scored");

    let disk = SaveDisk::new(&disk_path);
    let bytes = disk.read(replay_sector(3), REPLAY_SECTORS).unwrap();
    let saved = SavedReplay::decode(&bytes).expect("a replay in slot 3");
    assert_eq!(saved.encode(), bytes, "the original layout");
    assert_eq!(&bytes[0xBF4..0xBF8], &0x167Fu32.to_be_bytes(), "the serial");
    assert!(disk
        .read(replay_sector(2), REPLAY_SECTORS)
        .unwrap()
        .iter()
        .all(|&b| b == 0));
    let mut replay = Replay::from_saved(&saved);
    assert!(replay.start());
    let mut frames = 0;
    loop {
        match replay.tick() {
            ReplayTick::Frame(frame) => {
                frames += 1;
                assert!(!frame.sprites.is_empty());
            }
            ReplayTick::Same => {}
            ReplayTick::Over => break,
        }
    }
    assert!(frames > 20, "{frames} frames");

    // "REPLAY GOALS": fire on the insert-disk screen, then watch.
    let mut started = None;
    let mut shows = 0;
    let mut host = ScriptedHost::new(60, |m| {
        if at(m, 2.0) {
            dump_frame(m, "replay_insert_disk");
            started = Some(m.time);
            return Some(press(FIRE_1));
        }
        shows += 1;
        if shows % 10 == 0 {
            dump_frame(m, &format!("replay_viewer_{shows}"));
        }
        Some(vec![])
    });
    let mut ui = Ui::new(&mut host);
    assert_eq!(replay_goals(&mut ui, &game), Ok(()));
    let watched = host.time - started.expect("past the first screen");
    assert!(
        watched >= FRAME_TIME * 2 * frames,
        "{watched:?} for {frames} frames"
    );
    std::fs::remove_file(&disk_path).unwrap();
}

#[test]
fn f_speeds_up_a_team_managers_league_match() {
    let match_time = |speed_up: bool| {
        let mut game = load_game()?;
        game.cursors.keypad = (2, 0);
        let mut pilot = Autopilot::default();
        let mut pressed = false;
        let mut host = ScriptedHost::new(3000, |m| {
            if m.banks.len() >= 2 {
                return None; // The first match is over.
            }
            if speed_up && m.banks.len() == 1 {
                let t = (m.time - m.banks[0].0).as_millis();
                if t % 3000 < 32 {
                    dump_frame(m, &format!("sped_up_{}", t / 3000));
                }
            }
            // After the fade in, which drops earlier presses.
            if speed_up
                && !pressed
                && m.banks.len() == 1
                && m.time > m.banks[0].0 + Duration::from_secs(2)
            {
                pressed = true;
                return Some(press(Button::FastForward));
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
        assert_eq!(game.sped_up, speed_up, "it lasts beyond the match");
        Some(host.banks[1].0 - host.banks[0].0)
    };
    let Some(normal) = match_time(false) else {
        return;
    };
    let sped_up = match_time(true).unwrap();
    assert!(sped_up * 4 < normal, "{sped_up:?} against {normal:?}");
}

#[test]
fn the_intro_runs_until_fire_then_loads_the_game() {
    use crate::data::files::read_amiga_intro_dump;
    use crate::menu::intro::{self, IntroAssets, IntroGraphics, IntroText};
    let Ok(dump) = read_amiga_intro_dump(&Config::default().data_dir) else {
        eprintln!("skipping: no intro dump");
        return;
    };
    let graphics = IntroGraphics::load(&dump).expect("the intro pictures decode");
    let assets = IntroAssets {
        graphics,
        text: IntroText::read(&dump),
    };
    let mut shot = 0;
    let mut host = ScriptedHost::new(300, |m| {
        let t = m.time.as_millis();
        if t % 2000 < 20 {
            dump_frame(m, &format!("intro_{shot:03}"));
            shot += 1;
        }
        // Fire is held from 60 seconds in until 63 (the intro checks it
        // between fades).
        Some(if (60_000..60_020).contains(&t) {
            vec![ButtonEvent {
                button: FIRE_1,
                pressed: true,
                repeat: false,
            }]
        } else if (63_000..63_020).contains(&t) {
            vec![ButtonEvent::release(FIRE_1)]
        } else {
            vec![]
        })
    });
    assert_eq!(intro::run(&mut Ui::new(&mut host), &assets), Ok(()));
    let music: Vec<_> = host.sounds.iter().filter(|(_, s)| s.id == 0x2C).collect();
    assert_eq!(music.len(), 1, "the music starts once");
    assert!(host.sounds.iter().any(|(_, s)| s.id == 0x36), "typing");
    assert_eq!(
        host.sounds.last().map(|(_, s)| s.id),
        Some(0x2D),
        "the music stops"
    );
    assert_eq!(
        host.banks.iter().map(|b| b.1).collect::<Vec<_>>(),
        [SoundBank::Intro, SoundBank::Menu]
    );
    // After fire is let go, "LOADING GAME" is typed at two steps of five
    // vertical blanks a letter.
    assert!(host.time > Duration::from_secs(63) + Duration::from_millis(12 * 200));
    assert!(host.time < Duration::from_secs(70));
}
