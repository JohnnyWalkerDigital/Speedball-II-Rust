//! Headless end-to-end tests against the real disk images.
//!
//! They are skipped (with a note) when the images are not in the default
//! data folder, since the game data cannot be shipped with the code.

use std::rc::Rc;

use crate::config::{Config, GameMode, Platform};
use crate::data::{addr, AtariRom, GameFiles};
use crate::game::entity::Entity;
use crate::game::{Match, MatchSetup};
use crate::graphics::{Assets, Canvas};
use crate::render::MatchRenderer;

struct Harness {
    rom: Rc<AtariRom>,
    files: GameFiles,
}

fn load(platform: Platform) -> Option<Harness> {
    let config = Config::default();
    match GameFiles::load(&config.data_dir, platform) {
        Ok(mut files) => {
            let rom = Rc::new(AtariRom::new(std::mem::take(&mut files.atari_disk)));
            Some(Harness { rom, files })
        }
        Err(error) => {
            eprintln!("skipping: {error}");
            None
        }
    }
}

fn config(platform: Platform, mode: GameMode, seed: u64) -> Config {
    Config {
        platform,
        mode: Some(mode),
        seed,
        ..Config::default()
    }
}

/// Plays a match to the end, rendering every frame. Returns the match and
/// how many frames it took.
fn play(h: &Harness, config: &Config, max_frames: usize) -> (Match, usize) {
    let assets = Assets::load(&h.rom, h.files.amiga.as_ref(), config.platform);
    let mut game = Match::new(MatchSetup::from_config(&h.rom, config), Rc::clone(&h.rom));
    let renderer = MatchRenderer::new(assets, &h.rom);
    let mut canvas = Canvas::new(320, 200);
    for frame in 1..=max_frames {
        let over = game.step();
        renderer.render(&game, &mut canvas);
        if over {
            return (game, frame);
        }
    }
    (game, max_frames)
}

#[test]
fn entity_records_are_well_formed() {
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let addresses = [addr::BALL, addr::LAUNCHER, addr::BIG_BALL, addr::EQUIPMENT]
        .into_iter()
        .chain(addr::COINS)
        .chain(addr::TOKENS)
        .chain(addr::MEDICS)
        .chain(addr::BOUNCE_DOMES)
        .chain(addr::ELECTROBOUNCES);
    for address in addresses {
        let entity = Entity::read(&h.rom, address);
        // The same checks the original constructor made.
        assert_eq!(entity.timer, 0, "{address:#X}");
        assert_eq!(entity.width % 8, 0, "{address:#X}");
    }
}

#[test]
fn every_equipment_piece_maps_to_a_skill() {
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    for piece in 0..8 {
        let selector = h.rom.equipment_skill(piece);
        assert!((72..80).contains(&selector), "piece {piece}: {selector}");
    }
}

#[test]
fn demo_match_plays_to_the_end_on_both_platforms() {
    for platform in [Platform::Atari, Platform::Amiga] {
        let Some(h) = load(platform) else { continue };
        let (game, frames) = play(&h, &config(platform, GameMode::Demo, 1), 20_000);
        assert!(
            game.clock.leg() >= 2,
            "{platform:?}: match did not finish in {frames} frames"
        );
        println!(
            "{platform:?} demo: {} - {} after {frames} frames",
            game.scores().team1,
            game.scores().team2
        );
    }
}

#[test]
fn knockout_matches_are_deterministic_and_eventful() {
    let Some(h) = load(Platform::Amiga) else {
        return;
    };
    let mut any_goals = false;
    for seed in 1..=6 {
        let config = config(Platform::Amiga, GameMode::Knockout, seed);
        let (first, frames) = play(&h, &config, 60_000);
        let (second, _) = play(&h, &config, 60_000);
        assert!(
            first.clock.leg() >= 2,
            "seed {seed}: match did not finish in {frames} frames"
        );
        assert_eq!(
            (first.scores().team1, first.scores().team2),
            (second.scores().team1, second.scores().team2),
            "seed {seed}: same seed must replay identically"
        );
        any_goals |= first.scores().team1 + first.scores().team2 > 0;
        println!(
            "seed {seed}: {} - {}",
            first.scores().team1,
            first.scores().team2
        );
    }
    assert!(
        any_goals,
        "six matches without a single point is implausible"
    );
}

#[test]
fn other_modes_run() {
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    for mode in [GameMode::Practice, GameMode::TwoPlayers] {
        play(&h, &config(Platform::Atari, mode, 3), 3_000);
    }
}

/// Writes frames to `$SPEEDBALL_FRAMES_DIR` as BMP files for eyeballing.
#[test]
#[ignore = "writes images; run manually"]
fn dump_frames() {
    let Ok(dir) = std::env::var("SPEEDBALL_FRAMES_DIR") else {
        return;
    };
    for platform in [Platform::Atari, Platform::Amiga] {
        let Some(h) = load(platform) else { continue };
        let config = config(platform, GameMode::Knockout, 2);
        let assets = Assets::load(&h.rom, h.files.amiga.as_ref(), platform);
        let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
        let renderer = MatchRenderer::new(assets, &h.rom);
        let mut canvas = Canvas::new(320, 200);
        let mut replay_frames = 0;
        for frame in 0..=1800 {
            game.step();
            if game.replay.is_playing() {
                replay_frames += 1;
            }
            if [1, 150, 400, 900, 1795, 1800].contains(&frame) || [2, 60].contains(&replay_frames) {
                renderer.render(&game, &mut canvas);
                let replaying = if game.replay.is_playing() {
                    "_replay"
                } else {
                    ""
                };
                let path = format!("{dir}/{platform:?}_{frame:05}{replaying}.bmp");
                std::fs::write(path, bmp(&canvas)).unwrap();
            }
        }
    }
}

pub(crate) fn bmp(canvas: &Canvas) -> Vec<u8> {
    let (w, h) = (canvas.width() as u32, canvas.height() as u32);
    let size = 54 + w * h * 4;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    for v in [size, 0, 54, 40] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for &pixel in canvas.pixels() {
        out.extend_from_slice(&(pixel | 0xFF00_0000).to_le_bytes());
    }
    out
}

#[test]
fn sounds_are_requested_where_the_original_plays_them() {
    use crate::game::events::Sound;
    let Some(h) = load(Platform::Amiga) else {
        return;
    };
    let (mut ice_cream, mut injuries, mut goals, mut matches_over) = (0, 0, 0, 0);
    let mut replay_ice_cream = 0;
    let mut heard = std::collections::HashSet::new();
    for seed in 1..=6 {
        let config = config(Platform::Amiga, GameMode::Knockout, seed);
        let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
        assert_eq!(game.take_sounds(), vec![Sound::GET_READY_KICK_OFF]);
        let mut was_injured = false;
        let mut second_crowd = None;
        for _ in 0..60_000 {
            let goals_before = game.scores().team1 + game.scores().team2;
            let over = game.step();
            if game.player_injured && !was_injured {
                injuries += 1;
            }
            was_injured = game.player_injured;
            let sounds = game.take_sounds();
            if sounds.contains(&Sound::GOAL) {
                goals += 1;
                // `handle_score_goal`: silence, goal and the crowd, then the
                // replay with its panel sound, and the crowd again after it.
                let start = sounds.iter().position(|&s| s == Sound::GOAL).unwrap() - 1;
                let crowd = sounds[start + 2];
                assert!(crowd == Sound::GOAL_CROWD_UP || crowd == Sound::CROWD_CHEER);
                assert_eq!(
                    &sounds[start..start + 4],
                    &[Sound::SILENCE, Sound::GOAL, crowd, Sound::SILENCE_CHANNEL_0]
                );
                assert!(game.replay.is_playing());
                assert!(game.scores().team1 + game.scores().team2 > goals_before);
                second_crowd = Some(crowd);
            } else if let Some(crowd) = second_crowd {
                if !game.replay.is_playing() {
                    assert_eq!(
                        sounds.first(),
                        Some(&crowd),
                        "the crowd again after the replay"
                    );
                    second_crowd = None;
                }
            }
            for sound in sounds {
                assert!(sound.id < 56, "{sound:?} is not in the sound table");
                heard.insert(sound);
                if sound == Sound::ICE_CREAM {
                    if game.replay.is_playing() {
                        replay_ice_cream += 1;
                    } else {
                        assert!(
                            game.injury.stage >= 3,
                            "ice cream only when the medics lift or in a replay"
                        );
                        ice_cream += 1;
                    }
                }
                if sound == Sound::MATCH_OVER {
                    matches_over += 1;
                }
            }
            if over {
                break;
            }
        }
    }
    println!(
        "injuries {injuries}, ice cream {ice_cream} (and {replay_ice_cream} in replays), goals {goals}"
    );
    assert_eq!(matches_over, 6, "one final whistle per match");
    for expected in [
        Sound::LAUNCHER_START,
        Sound::BALL_LAUNCH,
        Sound::SILENCE_CHANNEL_0,
        Sound::THROW,
        Sound::CATCH,
        Sound::TACKLE,
        Sound::SLIDE,
        Sound::INJURY,
        Sound::INTERCEPTION_UP,
        Sound::INTERCEPTION_DOWN,
        Sound::ITEM_APPEARS,
        Sound::TOKEN_APPEARS,
        Sound::END_OF_MATCH,
    ] {
        assert!(heard.contains(&expected), "never heard {expected:?}");
    }
    let wall = [
        Sound::SIDE_WALL_LOW,
        Sound::SIDE_WALL_HIGH,
        Sound::END_WALL_LOW,
        Sound::END_WALL_HIGH,
    ];
    assert!(
        wall.iter().any(|s| heard.contains(s)),
        "the ball never hit a wall"
    );
    println!("{} distinct sound calls heard", heard.len());
    assert!(goals > 0);
    assert!(
        ice_cream > 0 && ice_cream < injuries,
        "a coin flip, so some but not all"
    );
}

#[test]
fn warp_keeps_sideways_drift_only_when_moving_straight() {
    use crate::game::geometry::Point;
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let config = config(Platform::Atari, GameMode::Knockout, 1);
    let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
    let mut warp = |dir: i32| {
        game.holder = None;
        let ball = &mut game.props.ball;
        ball.sprite = 0;
        ball.pos = Point::new(20, 370);
        ball.dir = dir;
        ball.velocity = Point::new(3, -8);
        game.check_warp_gates();
        (game.props.ball.pos.x, game.props.ball.velocity)
    };
    let straight_up = h.rom.velocity(0, 8);
    assert_eq!(warp(0), (608, Point::new(3, straight_up.y)), "drift kept");
    assert_eq!(warp(1).1, h.rom.velocity(1, 8), "diagonal: replaced");
}

#[test]
fn launcher_shows_during_the_launch_and_hides_after() {
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let config = config(Platform::Atari, GameMode::Demo, 1);
    let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
    let mut seen_during_launch = false;
    for _ in 0..2_000 {
        game.step();
        if game.clock_paused && !game.props.launcher.flags.invisible() {
            seen_during_launch = true;
        }
        if !game.clock_paused {
            break;
        }
    }
    assert!(seen_during_launch, "visible while launching");
    assert!(!game.clock_paused, "play started");
    assert!(
        game.props.launcher.flags.invisible(),
        "hidden once the ball is in play"
    );
}

#[test]
fn goals_are_replayed_at_half_speed_then_play_resumes() {
    use crate::game::text::TextId;
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let config = config(Platform::Atari, GameMode::Knockout, 1);
    let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
    let mut replays = 0;
    for _ in 0..60_000 {
        let score = game.scores();
        game.step();
        if !game.replay.is_playing() {
            continue;
        }
        replays += 1;
        assert_ne!(game.scores(), score, "a replay starts on a goal");
        assert_eq!(game.displayed_scores(), score, "the status bar waits");
        assert!(matches!(
            game.text_panel.message(),
            Some(TextId::Team1Scored | TextId::Team2Scored | TextId::OwnGoal)
        ));

        // Each recorded frame shows for two steps; the game itself is frozen.
        let frozen = game.clock.remaining();
        let ball = game.props.ball.pos;
        let mut steps = 1;
        while game.replay.is_playing() {
            game.step();
            steps += 1;
            assert_eq!(game.clock.remaining(), frozen);
            assert!(steps < 2000, "a replay is bounded by the buffer");
        }
        println!("replay {replays}: {steps} steps");
        assert!(steps >= 4, "replayed {steps} steps");
        assert_eq!(game.props.ball.pos, ball);
        assert_eq!(game.displayed_scores(), game.scores());
        assert_eq!(game.text_panel.message(), None);
        assert!(game.clock_paused, "the celebration follows the replay");
        if replays == 3 {
            break;
        }
    }
    assert_eq!(replays, 3);
}

#[test]
fn firing_skips_the_replay() {
    use crate::game::geometry::Side;
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let config = config(Platform::Atari, GameMode::Knockout, 1);
    let mut game = Match::new(MatchSetup::from_config(&h.rom, &config), Rc::clone(&h.rom));
    while !game.replay.is_playing() {
        game.step();
    }
    game.step();
    assert!(game.replay.is_playing());
    game.input_mut(Side::Team2).set_fire(true);
    game.step();
    assert!(!game.replay.is_playing());
}

#[test]
fn substitutions_change_the_squad_for_good() {
    use crate::game::geometry::Side;
    use crate::game::setup::{MatchRules, SquadSkills};
    use crate::game::squad::{SquadPlayer, SQUAD_SIZE};
    let Some(h) = load(Platform::Atari) else {
        return;
    };
    let squad: [SquadPlayer; SQUAD_SIZE] = std::array::from_fn(|i| SquadPlayer {
        name: format!("P{i}"),
        face: i as u16 * 3,
        position: (i / 3) as u8,
        skills: [104; 8],
    });
    // Find a match where Brutal Deluxe lose a player to injury (weak, and
    // against Super Nashwan, that does not take long).
    for seed in 0..40 {
        let mut rules = MatchRules::one_player(&h.rom, 15);
        rules.team1.skills = SquadSkills::Players(Box::new(squad.clone()));
        rules.computer.team1 = true;
        let mut game = Match::new(MatchSetup::new(&h.rom, rules, seed), Rc::clone(&h.rom));
        for _ in 0..20_000 {
            if game.step() {
                break;
            }
        }
        let after = game.squad_after(Side::Team1, &squad);
        if game.statistics().team1.substitutions == 0 {
            continue;
        }
        // Each injury sends the player to the end of the bench, skills
        // rounded down to 100, and moves the bench up: the same twelve
        // players, reordered, some of them weaker for good.
        let mut names: Vec<_> = after.iter().map(|p| p.name.clone()).collect();
        assert_ne!(
            names,
            squad.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        );
        names.sort_by_key(|n| n[1..].parse::<usize>().unwrap());
        assert_eq!(names, (0..12).map(|i| format!("P{i}")).collect::<Vec<_>>());
        assert!(after
            .iter()
            .all(|p| p.skills == [104; 8] || p.skills == [100; 8]));
        assert!(
            after[11].skills == [100; 8],
            "the last injured is on the bench"
        );
        return;
    }
    panic!("no substitution in 40 matches");
}
