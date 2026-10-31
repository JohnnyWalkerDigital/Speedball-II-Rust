//! The table screens drawn on the steel panel: the next fixture, the league
//! tables and a round's results.

use super::assets::SmallFont;
use super::flow::Game;
use super::strings::{FixtureNumber, Statistic, Title};
use super::text::write_digits;
use super::ui::{Page, PANEL_COLOUR};
use crate::game::career::{TeamId, TeamRecord, BRUTAL_DELUXE};
use crate::game::cup::Cup;
use crate::game::geometry::Side;
use crate::game::league::{League, DIVISION_SIZE, MATCHES_PER_ROUND};
use crate::game::setup::league_team;
use crate::game::squad::{self, Competition};

/// The two teams' records and skills side by side.
pub fn next_fixture(game: &Game, competition: Competition, teams: (TeamId, TeamId)) -> Page {
    let text = &game.text;
    let fixture_lines = &text.panels.next_fixture;
    let mut numbers: Vec<Vec<u8>> = FixtureNumber::ALL
        .iter()
        .map(|&number| {
            let line = fixture_lines.iter().find(|l| l.numbers == Some(number));
            line.expect("checked when loaded").text.clone()
        })
        .collect();
    // Brutal Deluxe's column ends at 0x13, the opponent's at 0x31.
    for (team, end) in [(teams.0, 0x13), (teams.1, 0x31)] {
        fill_fixture_column(game, competition, team, end, &mut numbers);
    }
    let lines: Vec<Vec<u8>> = fixture_lines
        .iter()
        .map(|line| match line.numbers {
            Some(number) => numbers[number as usize].clone(),
            None => line.text.clone(),
        })
        .collect();

    let mut page = game_page(game, &text.titles.next_fixture);
    page.string_list(&game.assets, &lines);
    // "BRUTAL DELUXE  PLAY  OPPONENT" along row 6.
    for (text, x) in [
        (text.team_name(teams.0), 0x10),
        (text.panels.play.clone(), 0x94),
        (text.team_name(teams.1), 0xC4),
    ] {
        page.alt_text(&game.assets, &at(text, x, 6), SmallFont::Orange);
    }
    page
}

/// Fills in one team's column of the fixture screen.
fn fill_fixture_column(
    game: &Game,
    competition: Competition,
    team: TeamId,
    end: usize,
    texts: &mut [Vec<u8>],
) {
    let record = game.career.record(team);
    let league = competition == Competition::League;
    let [pwdlp, placing, points_for, points_against, groups @ ..] = texts else {
        unreachable!("eight fixture lines")
    };

    // Played, won, drawn, lost and league points; the cup shows "NA" for
    // the points.
    let mut at = end - 1;
    // Only the knockout and league show league points.
    if !matches!(competition, Competition::Knockout | Competition::League) {
        for c in *b" AN " {
            pwdlp[at] = c;
            at -= 1;
        }
    } else {
        write_digits(pwdlp, at, record.league_points as i16, 4);
        at -= 4;
    }
    at -= 2;
    for value in [record.lost, record.drawn, record.won, record.played] {
        write_digits(pwdlp, at, value as i16, 2);
        at -= 3;
    }

    if league {
        let within_division = match record.league_placing & 7 {
            0 => 8,
            n => n,
        };
        write_digits(placing, end, i16::from(within_division), 2);
        write_digits(points_for, end, record.points_for as i16, 3);
        write_digits(points_against, end, record.points_against as i16, 3);
    } else {
        placing[end] = b'A';
        placing[end - 1] = b'N';
        for text in [points_for, points_against] {
            text[end] = b'A';
            text[end - 1] = b'N';
            text[end - 2] = b' ';
        }
    }

    for (text, group) in groups.iter_mut().zip(skill_groups(game, team)) {
        write_digits(text, end, average_8(group), 3);
    }
}

/// The four groups' skill bytes: a human squad's averaged per skill over
/// each group's three players, otherwise the team's templates.
fn skill_groups(game: &Game, team: TeamId) -> [[i32; 8]; 4] {
    let human = match team {
        BRUTAL_DELUXE => Some(Side::Team1),
        TeamId::PlayerTwo => Some(Side::Team2),
        TeamId::League(_) => None,
    };
    match (human, team) {
        (Some(side), _) => {
            squad::group_averages(game.career.squads.get(side)).map(|g| g.map(i32::from))
        }
        (None, TeamId::League(index)) => league_team(&game.rom, index).groups(),
        (None, TeamId::PlayerTwo) => unreachable!("player 2 is human"),
    }
}

/// The mean of eight bytes, rounded down.
fn average_8(values: [i32; 8]) -> i16 {
    (values.iter().map(|v| v & 0xFF).sum::<i32>() >> 3) as i16
}

/// One division's table, with Brutal Deluxe's name shown or blanked (it
/// blinks).
pub fn league_table(game: &Game, league: &League, division: usize, show_player: bool) -> Page {
    let text = &game.text;
    let title = [&text.titles.division_1, &text.titles.division_2][division];
    let first_placing = (division * DIVISION_SIZE) as u8;
    let mut page = game_page(game, title);
    page.string_list(&game.assets, &text.panels.league_table);
    for &team in &league.tables[division] {
        let record = game.career.record(team);
        let row = (record.league_placing - first_placing) * 4 + 8;
        let name = at(text.team_name(team), 0x28, row);
        page.alt_text(&game.assets, &name, SmallFont::Orange);
        let details = placing_string(&text.panels.league_row, record, first_placing);
        page.alt_text(&game.assets, &at(details, 0x10, row), SmallFont::Orange);
    }
    // Brutal Deluxe's name blinks: while it is hidden, a box in the panel
    // colour covers it.
    let player = game.career.record(BRUTAL_DELUXE).league_placing;
    if !show_player && League::division_of(player) == division {
        let y = usize::from(player - first_placing) * 16 + 0x20;
        page.fill_box(0x28, 0x60, y, 8, PANEL_COLOUR);
    }
    page
}

/// A table row: placing, movement arrow, played, won, drawn, lost, points
/// for and against, and league points.
fn placing_string(template: &[u8], record: &TeamRecord, first_placing: u8) -> Vec<u8> {
    let mut text = template.to_vec();
    write_digits(&mut text, 0x2E, record.league_points as i16, 3);
    write_digits(&mut text, 0x2A, record.points_against as i16, 3);
    write_digits(&mut text, 0x26, record.points_for as i16, 3);
    let mut end = 0x22;
    for value in [record.lost, record.drawn, record.won, record.played] {
        write_digits(&mut text, end, value as i16, 2);
        end -= 3;
    }
    // Up, level or down since the last match.
    let arrow = match record.league_placing.cmp(&record.old_league_placing) {
        std::cmp::Ordering::Less => *b"no",
        std::cmp::Ordering::Equal => *b"rs",
        std::cmp::Ordering::Greater => *b"pq",
    };
    text[0x30..0x32].copy_from_slice(&arrow);
    write_digits(
        &mut text,
        2,
        i16::from(record.league_placing - first_placing),
        1,
    );
    text
}

/// Both divisions' four matches this round.
pub fn results(game: &Game, league: &League, round: usize) -> Page {
    let strings = &game.text;
    let mut page = game_page(game, &strings.titles.results);
    page.string_list(&game.assets, &strings.panels.results);
    for (division, first_row) in [(0, 0x0A), (1, 0x1E)] {
        for game_index in 0..MATCHES_PER_ROUND {
            let (team1, team2) = league.fixture(division, round, game_index);
            let row = (game_index * 4 + first_row) as u8;
            let (score1, score2) = league.results[division][game_index];
            for (team, x) in [(team1, 0x10), (team2, 0xA0)] {
                page.alt_text(
                    &game.assets,
                    &at(strings.team_name(team), x, row),
                    SmallFont::Orange,
                );
            }
            let mut text = strings.panels.result_row.clone();
            write_digits(&mut text, 0x18, score1 as i16, 3);
            write_digits(&mut text, 0x31, score2 as i16, 3);
            text[1] = row;
            page.alt_text(&game.assets, &text, SmallFont::Orange);
        }
    }
    page
}

/// The last match's numbers for both teams.
/// `None` before any match has been played.
pub fn statistics(game: &Game) -> Option<Page> {
    let last = game.career.last_match?;
    let text = &game.text;
    let mut lines: Vec<Vec<u8>> = text
        .panels
        .statistics
        .iter()
        .map(|l| l.text.clone())
        .collect();
    for (statistics, end) in [(last.statistics.team1, 0x0C), (last.statistics.team2, 0x29)] {
        for (line, shown) in lines.iter_mut().zip(&text.panels.statistics) {
            let value = match shown.numbers {
                None => continue,
                Some(Statistic::ShotsAtGoal) => statistics.shots_at_goal as i16,
                Some(Statistic::GoalsScored) => statistics.goals_scored as i16,
                Some(Statistic::GoalsSaved) => statistics.goals_saved as i16,
                Some(Statistic::TimeInPossession) => statistics.time_in_possession as i16,
                Some(Statistic::SuccessfulTackles) => statistics.successful_tackles as i16,
                Some(Statistic::Substitutions) => statistics.substitutions as i16,
                Some(Statistic::TimeInOpponentsHalf) => statistics.time_in_opponents_half as i16,
                Some(Statistic::BonusPoints) => statistics.bonus_points,
            };
            write_digits(line, end, value, 3);
        }
    }
    let mut page = game_page(game, &text.titles.statistics);
    page.string_list(&game.assets, &lines);
    for (team, x) in [(last.teams.team1, 0x40), (last.teams.team2, 0xA6)] {
        page.alt_text(
            &game.assets,
            &at(text.team_name(team), x, 8),
            SmallFont::Orange,
        );
    }
    Some(page)
}

/// This cup round's ties.
pub fn fixture_list(game: &Game, cup: &Cup) -> Page {
    let text = &game.text;
    let mut page = game_page(game, &text.titles.fixture_list);
    page.string_list(&game.assets, &text.panels.fixture_list);
    for (row, (first, second)) in cup.ties().enumerate() {
        let row = (row * 4 + 0xA) as u8;
        for (text, x) in [
            (text.team_name(cup.teams[first]), 0x10),
            (text.panels.play.clone(), 0x94),
            (text.team_name(cup.teams[second]), 0xB2),
        ] {
            page.alt_text(&game.assets, &at(text, x, row), SmallFont::Orange);
        }
    }
    page
}

/// The backdrop with a title and the plain panel.
fn game_page(game: &Game, title: &Title) -> Page {
    let mut page = Page::titled(&game.assets, title);
    page.panel();
    page
}

/// A string with its position bytes replaced.
fn at(mut text: Vec<u8>, x: u8, row: u8) -> Vec<u8> {
    text[0] = x;
    text[1] = row;
    text
}
