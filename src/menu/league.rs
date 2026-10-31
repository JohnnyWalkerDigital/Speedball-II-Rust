//! The league, from `game_league` down: two half-seasons a year, then
//! promotion and the play-off for a team in division 2, or the title for a
//! team in division 1.

use super::flow::{cup_league_match, list_menu, message, wait_fire, win_shared, Game};
use super::host::{Button, Control};
use super::manager::Standings;
use super::panels::{self, Competition};
use super::saving;
use super::ui::{Flow, Stop, Ui};
use crate::game::geometry::Side;
use crate::game::league::{
    team2_won, League, BRUTAL_DELUXE, DIVISION_SIZE, MATCHES_PER_ROUND, ROUNDS,
};
use crate::game::save::Progress;

/// Where a loaded league picks up (`round_num`, `match_num` and the
/// half-season's playing direction).
struct Resume {
    round: usize,
    game: usize,
    initial_dir: i32,
}

/// `game_league`.
pub fn league(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.set_cash_cap(550);
    let (mut league, mut resume) = match list_menu(ui, game, &game.text.menus.league)? {
        choice @ (0 | 1) => {
            if choice == 1 {
                // "TEAM MANAGER": the computer plays Brutal Deluxe's
                // matches, for a guaranteed 300 a match and more prize money.
                game.manager_mode = true;
                game.career.money.start = 300;
                game.career.set_cash_cap(650);
            }
            let league = League::new_game(&mut game.career);
            game.career.market.init(&mut game.rng);
            (league, None)
        }
        _ => {
            let save = saving::load(ui, game, false)?;
            let (teams, initial_dir) = saving::restore(game, &save);
            if game.manager_mode {
                game.career.money.start = 300;
                game.career.money.cap_base = 650;
            }
            let Progress::League { round, game: index } = save.progress else {
                unreachable!("a league save")
            };
            let resume = Resume {
                round: usize::from(round),
                game: usize::from(index),
                initial_dir,
            };
            (League::from_saved(&teams), Some(resume))
        }
    };
    league.update_placings(&mut game.career);
    loop {
        // The second half of a season plays with ends swapped.
        let initial_dir = match resume.take() {
            None => {
                game.career.clear_records();
                for initial_dir in [0, 4] {
                    league.shuffle_fixtures(&mut game.rng);
                    season(ui, game, &mut league, initial_dir, 0, 0)?;
                }
                4
            }
            // Saved during the play-offs.
            Some(r) if r.round >= ROUNDS => r.initial_dir,
            Some(r) => {
                season(ui, game, &mut league, r.initial_dir, r.round, r.game)?;
                if r.initial_dir == 0 {
                    league.shuffle_fixtures(&mut game.rng);
                    season(ui, game, &mut league, 4, 0, 0)?;
                    4
                } else {
                    r.initial_dir
                }
            }
        };
        if placing(game) <= DIVISION_SIZE as u8 {
            if placing(game) == 1 {
                win_shared(ui, &game.assets.league_win)?;
            }
            return Ok(());
        }
        playoffs(ui, game, &mut league, initial_dir)?;
        promote_relegate_announce(ui, game, &mut league)?;
        if placing(game) > DIVISION_SIZE as u8 {
            return Ok(());
        }
        let text = &game.text;
        message(
            ui,
            game,
            &text.titles.speedball_2,
            &text.messages.new_season,
        )?;
    }
}

fn placing(game: &Game) -> u8 {
    game.career.record(BRUTAL_DELUXE).league_placing
}

/// `league_season`: seven rounds, each followed by the results and tables.
/// A loaded game starts part way (`league_season_middle`).
fn season(
    ui: &mut Ui,
    game: &mut Game,
    league: &mut League,
    initial_dir: i32,
    first_round: usize,
    first_game: usize,
) -> Flow {
    for round in first_round..ROUNDS {
        let first_game = if round == first_round { first_game } else { 0 };
        league_round(ui, game, league, round, first_game, initial_dir)?;
        ui.transition(&panels::results(game, league, round))?;
        wait_fire(ui)?;
        league.update_placings(&mut game.career);
        show_league_placings(ui, game, league)?;
    }
    Ok(())
}

/// `league_round`: four matches in each division, Brutal Deluxe's
/// division first each time.
fn league_round(
    ui: &mut Ui,
    game: &mut Game,
    league: &mut League,
    round: usize,
    first_game: usize,
    initial_dir: i32,
) -> Flow {
    for game_index in first_game..MATCHES_PER_ROUND {
        let own = League::division_of(placing(game));
        for division in [own, 1 - own] {
            let teams = league.fixture(division, round, game_index);
            let standings = Standings::League {
                league,
                round,
                game: game_index,
                initial_dir,
            };
            let scores =
                cup_league_match(ui, game, Competition::League, teams, initial_dir, standings)?;
            game.career.record_result(per_team(teams), per_team(scores));
            league.results[division][game_index] = scores;
        }
    }
    Ok(())
}

/// `show_league_placings`: a division's table with Brutal Deluxe's name
/// blinking. Up and down switch division; fire carries on.
pub(super) fn show_league_placings(ui: &mut Ui, game: &Game, league: &League) -> Flow {
    const BLINK_VBLANKS: u32 = 16;
    let mut division = League::division_of(placing(game));
    let pages = |division| {
        [
            panels::league_table(game, league, division, true),
            panels::league_table(game, league, division, false),
        ]
    };
    let mut shown = pages(division);
    ui.transition(&shown[0])?;
    ui.clear_input();
    let mut frame = 0;
    loop {
        ui.vblank()?;
        frame += 1;
        // `frame_timer` bit 4 picks the name or the blank.
        ui.show_page(&shown[usize::from(frame / BLINK_VBLANKS % 2 == 1)]);
        let wanted = |b| {
            b == Button::Escape
                || [Control::Up, Control::Down, Control::Fire]
                    .iter()
                    .any(|&c| b == Button::Joystick(Side::Team1, c))
        };
        match ui.next_press(wanted) {
            Some(Button::Escape) => return Err(Stop::Abort),
            Some(Button::Joystick(_, Control::Fire)) => return Ok(()),
            Some(Button::Joystick(_, control)) => {
                division = usize::from(control == Control::Down);
                shown = pages(division);
                ui.transition(&shown[0])?;
                ui.clear_input();
            }
            _ => {}
        }
    }
}

/// `league_playoffs`: seventh in division 1 plays second in division 2
/// until one wins; if the division 2 team wins, they swap divisions. The
/// ends swap before each match.
fn playoffs(ui: &mut Ui, game: &mut Game, league: &mut League, mut initial_dir: i32) -> Flow {
    let teams = (league.tables[0][6], league.tables[1][1]);
    if teams.0 == BRUTAL_DELUXE || teams.1 == BRUTAL_DELUXE {
        let text = &game.text;
        message(ui, game, &text.titles.speedball_2, &text.messages.playoff)?;
    }
    let scores = loop {
        initial_dir ^= 4;
        // A save here comes back to the end of the season.
        let standings = Standings::League {
            league,
            round: ROUNDS,
            game: MATCHES_PER_ROUND,
            initial_dir,
        };
        let scores =
            cup_league_match(ui, game, Competition::League, teams, initial_dir, standings)?;
        if scores.0 != scores.1 {
            break scores;
        }
    };
    if !team2_won(scores) {
        return Ok(());
    }
    league.promote_relegate(teams.0, teams.1, &mut game.career);
    let record = game.career.record(BRUTAL_DELUXE);
    if record.league_placing == 7 && record.old_league_placing != 7 {
        win_shared(ui, &game.assets.promotion)?;
    } else if record.league_placing == 10 && record.old_league_placing != 10 {
        relegated(ui, game)?;
    }
    Ok(())
}

/// `league_promote_relegate_announce`: the top of division 2 and the
/// bottom of division 1 swap.
fn promote_relegate_announce(ui: &mut Ui, game: &mut Game, league: &mut League) -> Flow {
    league.promote_relegate(league.tables[0][7], league.tables[1][0], &mut game.career);
    match placing(game) {
        8 => win_shared(ui, &game.assets.promotion),
        9 => relegated(ui, game),
        _ => Ok(()),
    }
}

fn relegated(ui: &mut Ui, game: &Game) -> Flow {
    let text = &game.text;
    message(ui, game, &text.titles.speedball_2, &text.messages.relegated)
}

fn per_team<T>(pair: (T, T)) -> crate::game::geometry::PerTeam<T> {
    crate::game::geometry::PerTeam::new(pair.0, pair.1)
}
