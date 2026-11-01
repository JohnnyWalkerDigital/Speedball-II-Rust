//! The cup: four rounds of two-legged ties, with coins worth double and
//! Brutal Deluxe's money doubling after each round from the second.

use super::flow::{
    announce, cup_league_match, display_total_match_result, list_menu, win_shared, Game,
};
use super::manager::Standings;
use super::saving::{self, Snapshot};
use super::ui::{Flow, Ui};
use crate::game::career::team2_won;
use crate::game::career::{cash_cap, BRUTAL_DELUXE};
use crate::game::cup::Cup;
use crate::game::geometry::PerTeam;
use crate::game::save::Progress;
use crate::game::squad::Competition;

/// Plays a new or loaded cup.
pub fn cup(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.set_cash_cap(cash_cap::CUP);
    let (mut cup, mut resume) = if list_menu(ui, game, &game.text.menus.cup)? == 0 {
        // A new cup.
        game.career.market.init(&mut game.rng);
        let cup = Cup::new(&mut game.rng);
        game.career.clear_records();
        game.career.start_cup_cash();
        (cup, None)
    } else {
        // A loaded cup goes straight to the round in progress, its draw as
        // saved.
        let save = saving::load(ui, game, true)?;
        let Snapshot {
            teams,
            initial_dir,
            progress,
        } = saving::restore(game, save);
        let Progress::Cup {
            round, aggregate, ..
        } = progress
        else {
            unreachable!("a cup save")
        };
        let cup = Cup {
            teams: teams.try_into().expect("sixteen teams"),
            round: usize::from(round),
        };
        let aggregate = PerTeam::new(i32::from(aggregate.0), i32::from(aggregate.1));
        (cup, Some((aggregate, initial_dir)))
    };
    loop {
        if resume.is_none() {
            cup.redraw(&mut game.rng);
        }
        announce(ui, game, &game.text.messages.cup_rounds[cup.round])?;
        let ties: Vec<_> = cup.ties().collect();
        for (index, tie) in ties.into_iter().enumerate() {
            let partway = resume
                .take()
                .filter(|(aggregate, _)| *aggregate != PerTeam::new(0, 0));
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
    partway: Option<(PerTeam<i32>, i32)>,
) -> Flow<bool> {
    let teams = PerTeam::new(cup.teams[tie.0], cup.teams[tie.1]);
    let playing = teams.team1 == BRUTAL_DELUXE;
    let standings = |cup, aggregate, initial_dir| Standings::Cup {
        cup,
        tie_index: index,
        tie,
        aggregate,
        initial_dir,
    };
    let (mut total, mut initial_dir) = match partway {
        Some(partway) => {
            announce(ui, game, &game.text.messages.second_leg)?;
            partway
        }
        None => {
            let first = cup_league_match(
                ui,
                game,
                Competition::Cup,
                teams,
                0,
                standings(cup, PerTeam::new(0, 0), 0),
            )?;
            game.career.record_result(teams, first);
            if playing {
                announce(ui, game, &game.text.messages.second_leg)?;
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
        game.career.record_result(teams, leg);
        initial_dir ^= 4;
        total = total + leg;
        if playing {
            display_total_match_result(ui, game, teams, total)?;
        }
        if total.team1 != total.team2 {
            break;
        }
    }
    let second_won = team2_won(total);
    if second_won {
        cup.second_team_won(tie);
    }
    Ok(playing && second_won)
}
