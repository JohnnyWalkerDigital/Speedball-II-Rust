//! The table screens drawn on the steel panel: the next fixture, the league
//! tables and a round's results.

use super::assets::SmallFont;
use super::flow::Game;
use super::ram::addr;
use super::text::write_digits;
use super::ui::{Page, PANEL_COLOUR};
use crate::game::career::{TeamId, TeamRecord};
use crate::game::geometry::Side;
use crate::game::league::{League, DIVISION_SIZE, MATCHES_PER_ROUND};
use crate::game::roster::Skills;
use crate::game::setup::league_team;

/// The kind of game, for what the fixture screen shows (`match_kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Competition {
    Knockout,
    League,
    Cup,
}

/// The next-fixture lines that hold numbers, in the order
/// `generate_team_fixture_strings` fills them.
const FIXTURE_NUMBERS: [usize; 8] = [
    addr::STR_PWDLP_NUMS,
    addr::STR_LEAGUE_PLACING,
    addr::STR_POINTS_FOR,
    addr::STR_POINTS_AGAINST,
    addr::STR_DEFENCE,
    addr::STR_MIDFIELD,
    addr::STR_ATTACK,
    addr::STR_SUBSTITUTES,
];

/// `display_next_fixture`: the two teams' records and skills side by side.
pub fn next_fixture(game: &Game, competition: Competition, teams: (TeamId, TeamId)) -> Page {
    let ram = &game.ram;
    let mut numbers: Vec<Vec<u8>> = FIXTURE_NUMBERS.iter().map(|&a| ram.string(a)).collect();
    // Brutal Deluxe's column ends at 0x13, the opponent's at 0x31.
    for (team, end) in [(teams.0, 0x13), (teams.1, 0x31)] {
        fill_fixture_column(game, competition, team, end, &mut numbers);
    }
    let lines: Vec<Vec<u8>> = ram
        .pointer_list(addr::STRINGS_NEXT_FIXTURE)
        .into_iter()
        .map(
            |address| match FIXTURE_NUMBERS.iter().position(|&a| a == address) {
                Some(i) => numbers[i].clone(),
                None => ram.string(address),
            },
        )
        .collect();

    let mut page = game_page(game, addr::TITLE_NEXT_FIXTURE);
    page.string_list(&game.assets, &lines);
    // `draw_plays`: "BRUTAL DELUXE  PLAY  OPPONENT" along row 6.
    for (text, x) in [
        (ram.team_name(teams.0), 0x10),
        (ram.string(addr::STR_PLAY), 0x94),
        (ram.team_name(teams.1), 0xC4),
    ] {
        page.alt_text(&game.assets, &at(text, x, 6), SmallFont::Orange);
    }
    page
}

/// `generate_team_fixture_strings` for one team's column.
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
    if competition == Competition::Cup {
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
/// each group's three players (`generate_team_stats`), otherwise the
/// team's templates.
fn skill_groups(game: &Game, team: TeamId) -> [[i32; 8]; 4] {
    let human = match team {
        TeamId::League(0) => Some(Side::Team1),
        TeamId::PlayerTwo => Some(Side::Team2),
        TeamId::League(_) => None,
    };
    match (human, team) {
        (Some(side), _) => {
            let squad = game.career.squads.get(side);
            std::array::from_fn(|group| {
                let players: Vec<[i32; 8]> = squad[group * 3..group * 3 + 3]
                    .iter()
                    .map(|s: &Skills| s.to_array())
                    .collect();
                std::array::from_fn(|skill| {
                    let sum: i32 = players.iter().map(|p| p[skill] & 0xFF).sum();
                    (sum / 3) & 0xFF
                })
            })
        }
        (None, TeamId::League(index)) => league_team(&game.rom, index).groups(),
        (None, TeamId::PlayerTwo) => unreachable!("player 2 is human"),
    }
}

/// `average_8`: the mean of eight bytes, rounded down.
fn average_8(values: [i32; 8]) -> i16 {
    (values.iter().map(|v| v & 0xFF).sum::<i32>() >> 3) as i16
}

/// One division's table (`display_league_placings_div_1`/`_2`), with
/// Brutal Deluxe's name shown or blanked (it blinks).
pub fn league_table(game: &Game, league: &League, division: usize, show_player: bool) -> Page {
    let title = [addr::TITLE_DIVISION_1, addr::TITLE_DIVISION_2][division];
    let first_placing = (division * DIVISION_SIZE) as u8;
    let mut page = game_page(game, title);
    page.string_list(
        &game.assets,
        &game.ram.string_list(addr::STRINGS_LEAGUE_PLACINGS),
    );
    for &team in &league.tables[division] {
        let record = game.career.record(team);
        let row = (record.league_placing - first_placing) * 4 + 8;
        let name = at(game.ram.team_name(team), 0x28, row);
        page.alt_text(&game.assets, &name, SmallFont::Orange);
        let details = placing_string(
            &game.ram.string(addr::STR_LEAGUE_TEAM_DETAILS),
            record,
            first_placing,
        );
        page.alt_text(&game.assets, &at(details, 0x10, row), SmallFont::Orange);
    }
    // Brutal Deluxe's name blinks: `display_team_on_team_slot` draws it,
    // `display_box_on_team_slot` covers it in the panel colour.
    let player = game.career.record(TeamId::League(0)).league_placing;
    if !show_player && League::division_of(player) == division {
        let y = usize::from(player - first_placing) * 16 + 0x20;
        page.fill_box(0x28, 0x60, y, 8, PANEL_COLOUR);
    }
    page
}

/// `construct_league_placing_string`: placing, movement arrow, played,
/// won, drawn, lost, points for and against, and league points.
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

/// `display_league_results`: both divisions' four matches this round.
pub fn results(game: &Game, league: &League, round: usize) -> Page {
    let mut page = game_page(game, addr::TITLE_RESULTS);
    page.string_list(&game.assets, &game.ram.string_list(addr::STRINGS_RESULTS));
    for (division, first_row) in [(0, 0x0A), (1, 0x1E)] {
        for game_index in 0..MATCHES_PER_ROUND {
            let (team1, team2) = league.fixture(division, round, game_index);
            let row = (game_index * 4 + first_row) as u8;
            let (score1, score2) = league.results[division][game_index];
            for (team, x) in [(team1, 0x10), (team2, 0xA0)] {
                page.alt_text(
                    &game.assets,
                    &at(game.ram.team_name(team), x, row),
                    SmallFont::Orange,
                );
            }
            let mut text = game.ram.string(addr::STR_LEAGUE_MATCH_RESULT);
            write_digits(&mut text, 0x18, score1 as i16, 3);
            write_digits(&mut text, 0x31, score2 as i16, 3);
            text[1] = row;
            page.alt_text(&game.assets, &text, SmallFont::Orange);
        }
    }
    page
}

/// The backdrop with a title and the plain panel.
fn game_page(game: &Game, title: usize) -> Page {
    let mut page = Page::titled(&game.assets, &game.ram.title(title));
    page.panel();
    page
}

/// A string with its position bytes replaced.
fn at(mut text: Vec<u8>, x: u8, row: u8) -> Vec<u8> {
    text[0] = x;
    text[1] = row;
    text
}
