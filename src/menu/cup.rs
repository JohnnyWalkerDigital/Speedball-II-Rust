//! The cup: four rounds of two-legged ties, with coins worth double and
//! Brutal Deluxe's money doubling after each round from the second.

use super::flow::{
    cup_league_match, display_total_match_result, list_menu, message, win_shared, Game,
};
use super::manager::Standings;
use super::panels::Competition;
use super::saving;
use super::ui::{Flow, Ui};
use crate::game::cup::Cup;
use crate::game::geometry::PerTeam;
use crate::game::league::{team2_won, BRUTAL_DELUXE};
use crate::game::save::Progress;

/// Plays a new or loaded cup, with coins worth double while it lasts.
pub fn cup(ui: &mut Ui, game: &mut Game) -> Flow {
    game.coin_value *= 2;
    let result = play_cup(ui, game);
    game.coin_value /= 2;
    result
}

fn play_cup(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.set_cash_cap(2000);
    let (mut cup, mut resume) = if list_menu(ui, game, &game.text.menus.cup)? == 0 {
        // A new cup.
        game.career.market.init(&mut game.rng);
        let cup = Cup::new(&mut game.rng);
        game.career.clear_records();
        game.career.money.cash = PerTeam::new(2000, 2000);
        (cup, None)
    } else {
        // A loaded cup goes straight to the round in progress, its draw as
        // saved.
        let save = saving::load(ui, game, true)?;
        let (teams, initial_dir) = saving::restore(game, &save);
        let Progress::Cup {
            round, aggregate, ..
        } = save.progress
        else {
            unreachable!("a cup save")
        };
        let cup = Cup {
            teams: teams.try_into().expect("sixteen teams"),
            round: usize::from(round),
        };
        let aggregate = (i32::from(aggregate.0), i32::from(aggregate.1));
        (cup, Some((aggregate, initial_dir)))
    };
    loop {
        if resume.is_none() {
            cup.redraw(&mut game.rng);
        }
        let text = &game.text;
        let round = &text.messages.cup_rounds[cup.round];
        message(ui, game, &text.titles.speedball_2, round)?;
        let ties: Vec<_> = cup.ties().collect();
        for (index, tie) in ties.into_iter().enumerate() {
            let partway = resume.take().filter(|(aggregate, _)| *aggregate != (0, 0));
            if tie_lost_by_brutal_deluxe(ui, game, &mut cup, index, tie, partway)? {
                return Ok(());
            }
        }
        cup.round += 1;
        if cup.round > 1 {
            let cash = &mut game.career.money.cash.team1;
            *cash = cash.wrapping_shl(1);
        }
        if cup.is_finished() {
            return win_shared(ui, &game.assets.cup_win);
        }
    }
}

/// A two-legged tie, with more legs while the aggregate is level. The
/// winner moves into the tie's first slot. Returns whether Brutal Deluxe
/// were knocked out. A tie loaded part way (with a first-leg score) skips
/// its first leg.
fn tie_lost_by_brutal_deluxe(
    ui: &mut Ui,
    game: &mut Game,
    cup: &mut Cup,
    index: usize,
    tie: (usize, usize),
    partway: Option<((i32, i32), i32)>,
) -> Flow<bool> {
    let teams = (cup.teams[tie.0], cup.teams[tie.1]);
    let playing = teams.0 == BRUTAL_DELUXE;
    let standings = |cup, aggregate, initial_dir| Standings::Cup {
        cup,
        tie_index: index,
        tie,
        aggregate,
        initial_dir,
    };
    let (mut total, mut initial_dir) = match partway {
        Some(partway) => {
            let text = &game.text;
            message(
                ui,
                game,
                &text.titles.speedball_2,
                &text.messages.second_leg,
            )?;
            partway
        }
        None => {
            let first = cup_league_match(
                ui,
                game,
                Competition::Cup,
                teams,
                0,
                standings(cup, (0, 0), 0),
            )?;
            game.career.record_result(per_team(teams), per_team(first));
            if playing {
                let text = &game.text;
                message(
                    ui,
                    game,
                    &text.titles.speedball_2,
                    &text.messages.second_leg,
                )?;
                (first, 4)
            } else {
                (first, 0)
            }
        }
    };
    loop {
        let leg = cup_league_match(
            ui,
            game,
            Competition::Cup,
            teams,
            initial_dir,
            standings(cup, total, initial_dir),
        )?;
        game.career.record_result(per_team(teams), per_team(leg));
        initial_dir ^= 4;
        total = (total.0 + leg.0, total.1 + leg.1);
        if playing {
            display_total_match_result(ui, game, teams, total)?;
        }
        if total.0 != total.1 {
            break;
        }
    }
    let second_won = team2_won(total);
    if second_won {
        cup.second_team_won(tie);
    }
    Ok(playing && second_won)
}

fn per_team<T>(pair: (T, T)) -> PerTeam<T> {
    PerTeam::new(pair.0, pair.1)
}
