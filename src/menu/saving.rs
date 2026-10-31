//! Saving and loading leagues and cups.

use super::flow::{list_menu, wait_fire, Game};
use super::ram::addr;
use super::strings::Title;
use super::ui::{Flow, Stop, Ui};
use crate::data::save_disk::{CUP_SECTOR, GAME_SECTORS, LEAGUE_SECTOR};
use crate::game::career::{TeamId, TeamRecord};
use crate::game::events::Sound;
use crate::game::save::{Progress, SaveGame, SavedMarketPlayer, SavedPlayer, SavedTeam};
use crate::game::squad::{SquadPlayer, MARKET_SIZE, SQUAD_SIZE};

/// Bit 1 of the saved flags: one player (the computer has team 2).
const ONE_PLAYER: u16 = 1 << 1;
pub(super) const DISK_ERROR: Sound = Sound { id: 8, priority: 2 };

/// The save screen, then the game onto the save disk. Escape there cancels
/// the save, not the game.
pub fn save(ui: &mut Ui, game: &mut Game, save: SaveGame) -> Flow {
    if !insert_disk(ui, game, &game.text.titles.save_game)? {
        return Ok(());
    }
    let sector = match save.progress {
        Progress::League { .. } => LEAGUE_SECTOR,
        Progress::Cup { .. } => CUP_SECTOR,
    };
    let bytes = save.encode();
    while game.save_disk.write(sector, &bytes).is_err() {
        if !retry(ui, game)? {
            break;
        }
    }
    Ok(())
}

/// The load screen, then the saved game, retrying on a disk error.
/// Cancelling (or Escape) abandons the game.
pub fn load(ui: &mut Ui, game: &mut Game, cup: bool) -> Flow<SaveGame> {
    if !insert_disk(ui, game, &game.text.titles.load_game)? {
        return Err(Stop::Abort);
    }
    let sector = if cup { CUP_SECTOR } else { LEAGUE_SECTOR };
    loop {
        let read = game.save_disk.read(sector, GAME_SECTORS).ok();
        if let Some(save) = read.and_then(|bytes| SaveGame::decode(&bytes, cup)) {
            return Ok(save);
        }
        if !retry(ui, game)? {
            return Err(Stop::Abort);
        }
    }
}

/// "PLEASE INSERT GAME SAVE DISK", then wait for fire. `false` if the
/// player pressed Escape.
fn insert_disk(ui: &mut Ui, game: &Game, title: &Title) -> Flow<bool> {
    let mut page = game.titled_page(title);
    page.big_text(&game.assets, &game.text.messages.insert_game_save_disk);
    ui.transition(&page)?;
    match wait_fire(ui) {
        Ok(()) => Ok(true),
        Err(Stop::Abort) => Ok(false),
        Err(stop) => Err(stop),
    }
}

/// After a disk error: "RETRY" or "CANCEL". `true` to retry.
fn retry(ui: &mut Ui, game: &Game) -> Flow<bool> {
    ui.play_sound(DISK_ERROR);
    match list_menu(ui, game, &game.text.menus.disk_error) {
        Ok(choice) => Ok(choice == 0),
        Err(Stop::Abort) => Ok(false),
        Err(stop) => Err(stop),
    }
}

/// The parts of a save the league and cup fill in themselves.
pub struct Snapshot {
    /// The teams in the save's order.
    pub teams: Vec<TeamId>,
    pub initial_dir: i32,
    pub progress: Progress,
}

/// Builds a save of the career as it stands.
pub fn snapshot(game: &Game, snapshot: Snapshot) -> SaveGame {
    let names = SavedNames::new(game);
    let career = &game.career;
    let player = |p: &SquadPlayer| SavedPlayer {
        name_offset: names.offset(&p.name),
        face: p.face,
        skills: p.skills,
    };
    SaveGame {
        cash: career.money.cash.team1,
        caps: (career.money.cap.team1, career.money.cap.team2),
        initial_dir: snapshot.initial_dir as u16,
        flags: ONE_PLAYER | u16::from(game.manager_mode),
        squad: career.squads.team1.iter().map(player).collect(),
        teams: snapshot
            .teams
            .iter()
            .map(|&team| {
                let record = career.record(team);
                SavedTeam {
                    team_offset: (team_index(team) * addr::TEAM_SIZE) as u32,
                    played: record.played,
                    won: record.won,
                    drawn: record.drawn,
                    lost: record.lost,
                    points_for: record.points_for,
                    points_against: record.points_against,
                    league_points: record.league_points,
                    league_placing: u16::from(record.league_placing),
                    old_league_placing: u16::from(record.old_league_placing),
                }
            })
            .collect(),
        management_rounds: career.market.rounds,
        buyable_count: career.market.buyable_count,
        market: career
            .market
            .players
            .iter()
            .map(|m| SavedMarketPlayer {
                player: player(&m.player),
                change_time: m.change_time,
                buyable: u16::from(m.buyable),
            })
            .collect(),
        progress: snapshot.progress,
    }
}

/// Puts a loaded game back into the career. Returns the teams
/// in the save's order and the playing direction.
pub fn restore(game: &mut Game, save: &SaveGame) -> (Vec<TeamId>, i32) {
    let people = SavedNames::new(game);
    let name = |offset: u32| people.name(game, offset);
    let squad: Vec<(String, u16, [u8; 8])> = save
        .squad
        .iter()
        .map(|p| (name(p.name_offset), p.face, p.skills))
        .collect();
    let market: Vec<(String, &SavedMarketPlayer)> = save
        .market
        .iter()
        .map(|m| (name(m.player.name_offset), m))
        .collect();
    let career = &mut game.career;
    for (player, (name, face, skills)) in career.squads.team1.iter_mut().zip(squad) {
        player.name = name;
        player.face = face;
        player.skills = skills;
    }
    for (player, (name, saved)) in career.market.players.iter_mut().zip(market) {
        player.player.name = name;
        player.player.face = saved.player.face;
        player.player.skills = saved.player.skills;
        player.change_time = saved.change_time;
        player.buyable = saved.buyable != 0;
    }
    career.market.rounds = save.management_rounds;
    career.market.buyable_count = save.buyable_count;
    let teams = save
        .teams
        .iter()
        .map(|saved| {
            let team = TeamId::League(saved.team_offset as usize / addr::TEAM_SIZE);
            *career.record_mut(team) = TeamRecord {
                played: saved.played,
                won: saved.won,
                drawn: saved.drawn,
                lost: saved.lost,
                points_for: saved.points_for,
                points_against: saved.points_against,
                league_points: saved.league_points,
                league_placing: saved.league_placing as u8,
                old_league_placing: saved.old_league_placing as u8,
            };
            team
        })
        .collect();
    career.money.cash.team1 = save.cash;
    career.money.cap.team1 = save.caps.0;
    career.money.cap.team2 = save.caps.1;
    game.manager_mode = save.flags & 1 != 0;
    (teams, save.initial_dir as u8 as i32)
}

/// Saves name players by their name's offset from `addr::STR_JAMS` in the
/// original's memory. Names are only ever the squad's own or the market
/// players', the same people, in the same order, as the tables'.
struct SavedNames {
    names: Vec<String>,
    offsets: Vec<u32>,
}

impl SavedNames {
    fn new(game: &Game) -> Self {
        let ram = &game.ram;
        let tables = &game.tables;
        let pointers = (0..SQUAD_SIZE)
            .map(|i| ram.pointer_at(addr::BASE_TEAM_NAMES + i * 4))
            .chain((0..MARKET_SIZE).map(|i| {
                let record = ram.pointer_at(addr::PLAYERS_MARKET + i * 4);
                ram.pointer_at(record + 4)
            }));
        Self {
            names: (tables.base_identities.iter().map(|i| i.name.clone()))
                .chain(tables.market.iter().map(|m| m.player.name.clone()))
                .collect(),
            offsets: pointers.map(|p| (p - addr::STR_JAMS) as u32).collect(),
        }
    }

    /// The offset saved for `name`.
    fn offset(&self, name: &str) -> u32 {
        self.names
            .iter()
            .position(|n| n == name)
            .map_or(0, |i| self.offsets[i])
    }

    /// The name a save's offset stands for. An offset no one has reads the
    /// original's memory there, as the original would.
    fn name(&self, game: &Game, offset: u32) -> String {
        match self.offsets.iter().position(|&o| o == offset) {
            Some(i) => self.names[i].clone(),
            None => game.ram.c_string(addr::STR_JAMS + offset as usize),
        }
    }
}

fn team_index(team: TeamId) -> usize {
    match team {
        TeamId::League(index) => index,
        TeamId::PlayerTwo => unreachable!("player 2 is never saved"),
    }
}
