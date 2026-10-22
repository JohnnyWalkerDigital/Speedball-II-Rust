//! The cup (`game_cup`): four rounds of two-legged ties, with coins worth
//! double and Brutal Deluxe's money doubling after each round from the
//! second.

use super::flow::{
    cup_league_match, display_total_match_result, list_menu, message, win_shared, Game,
};
use super::manager::Standings;
use super::panels::Competition;
use super::ram::addr;
use super::ui::{Flow, Stop, Ui};
use crate::game::cup::Cup;
use crate::game::geometry::PerTeam;
use crate::game::league::{team2_won, BRUTAL_DELUXE};

/// `game_cup`.
pub fn cup(ui: &mut Ui, game: &mut Game) -> Flow {
    game.coin_value *= 2;
    let result = play_cup(ui, game);
    game.coin_value /= 2;
    result
}

fn play_cup(ui: &mut Ui, game: &mut Game) -> Flow {
    game.career.set_cash_cap(2000);
    let menu = game.ram.menu(addr::MENU_SELECT_NEW_LOAD);
    if list_menu(ui, game, &menu)? != 0 {
        // Loading a saved cup comes with saving (a later phase).
        return Err(Stop::Abort);
    }
    // `init_cup`.
    game.career.market.init(&mut game.rng);
    let mut cup = Cup::new(&mut game.rng);
    game.career.clear_records();
    game.career.money.cash = PerTeam::new(2000, 2000);
    loop {
        cup.redraw(&mut game.rng);
        let text = game.ram.string_at(addr::STRINGS_CUP_ROUNDS, cup.round);
        message(ui, game, addr::TITLE_SPEEDBALL_2_PUSH_START, &text)?;
        let ties: Vec<_> = cup.ties().collect();
        for tie in ties {
            if tie_lost_by_brutal_deluxe(ui, game, &mut cup, tie)? {
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

/// `cup_match`: a two-legged tie, with more legs while the aggregate is
/// level. The winner moves into the tie's first slot. Returns whether
/// Brutal Deluxe were knocked out.
fn tie_lost_by_brutal_deluxe(
    ui: &mut Ui,
    game: &mut Game,
    cup: &mut Cup,
    tie: (usize, usize),
) -> Flow<bool> {
    let teams = (cup.teams[tie.0], cup.teams[tie.1]);
    let playing = teams.0 == BRUTAL_DELUXE;
    let mut initial_dir = 0;
    let first = cup_league_match(
        ui,
        game,
        Competition::Cup,
        teams,
        initial_dir,
        Standings::Cup(cup),
    )?;
    game.career.record_result(per_team(teams), per_team(first));
    let mut total = first;
    if playing {
        initial_dir = 4;
        let text = game.ram.string(addr::STR_SECOND_LEG);
        message(ui, game, addr::TITLE_SPEEDBALL_2_PUSH_START, &text)?;
    }
    loop {
        let leg = cup_league_match(
            ui,
            game,
            Competition::Cup,
            teams,
            initial_dir,
            Standings::Cup(cup),
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
