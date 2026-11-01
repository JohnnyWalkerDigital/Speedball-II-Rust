//! Command-line configuration.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Which original release the graphics come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Atari,
    Amiga,
}

/// The kind of match to set up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    /// One player against the computer. From the command line, two random
    /// teams, both computer controlled until you touch the controls.
    Knockout,
    /// Two humans; team stats are all 170.
    TwoPlayers,
    /// Team 1 alone on the pitch.
    Practice,
    /// The attract-mode demo: two computer teams, 20 second halves.
    Demo,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub platform: Platform,
    /// Skip the menus and play one match of this kind.
    pub mode: Option<GameMode>,
    pub seed: u64,
    pub scale: u8,
    pub team1: Option<usize>,
    pub team2: Option<usize>,
    /// Write the Amiga sound samples here as WAV files instead of playing.
    pub export_sounds: Option<PathBuf>,
    pub mute: bool,
    /// The game save disk image.
    pub save_disk: PathBuf,
    /// Write the graphics here as an asset pack, and exit.
    pub extract: Option<PathBuf>,
    /// Take the graphics from this asset pack instead of the originals.
    pub assets: Option<PathBuf>,
}

pub const TEAM_COUNT: usize = 16;

pub const USAGE: &str = "\
Speedball 2 match engine

USAGE:
    speedball2 [OPTIONS]

OPTIONS:
    --mode <MODE>          Skip the menus and play one match:
                           knockout | two-players | practice | demo
    --team1 <0-15>         With --mode: team for player 1 (default: random)
    --team2 <0-15>         With --mode: team for player 2 (default: random)
    --seed <N>             Random seed, for reproducible matches
    --scale <1|2|4|8>      Window scale (default: 4)
    --data <DIR>           Folder with the disk images (default: originals)
    --export-sounds <DIR>  Save the raw instrument samples, and every game sound
                           rendered through the driver, as WAV files and exit
    --mute                 No sound
    --save-disk <FILE>     Save disk image for leagues and cups
                           (default: saves/save-disk.adf)
    --extract <DIR>        Write the graphics as an editable asset pack of
                           PNG files, and exit
    --assets <DIR>         Take the graphics from an asset pack
    -h, --help             Show this help

CONTROLS:
    Player 1: arrow keys + Space / Right Ctrl (also drives the menus)
    Player 2: W A S D + Left Shift / Left Ctrl
    With --mode, touching a team's controls takes it over from the computer.
    In a match: P pause, N step one frame while paused, F fast forward.
    Esc: leave a match or menu; on the main menu, quit.";

impl Config {
    /// Parses arguments. `Ok(None)` means help was requested.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut config = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.as_str() {
                "-h" | "--help" => return Ok(None),
                // The original accepted -Atari / -Amiga.
                "--atari" | "-Atari" => config.platform = Platform::Atari,
                "--amiga" | "-Amiga" => config.platform = Platform::Amiga,
                "--mode" => config.mode = Some(parse_mode(&value("--mode")?)?),
                "--team1" => config.team1 = Some(parse_team(&value("--team1")?)?),
                "--team2" => config.team2 = Some(parse_team(&value("--team2")?)?),
                "--seed" => {
                    config.seed = value("--seed")?
                        .parse()
                        .map_err(|_| "--seed must be a number".to_string())?;
                }
                "--scale" => config.scale = parse_scale(&value("--scale")?)?,
                "--data" => config.data_dir = PathBuf::from(value("--data")?),
                "--export-sounds" => {
                    config.export_sounds = Some(PathBuf::from(value("--export-sounds")?));
                }
                "--mute" => config.mute = true,
                "--save-disk" => config.save_disk = PathBuf::from(value("--save-disk")?),
                "--extract" => config.extract = Some(PathBuf::from(value("--extract")?)),
                "--assets" => config.assets = Some(PathBuf::from(value("--assets")?)),
                other => return Err(format!("unknown argument `{other}`")),
            }
        }
        if config.team1.is_some() && config.team1 == config.team2 {
            return Err("--team1 and --team2 must differ".into());
        }
        Ok(Some(config))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("originals"),
            platform: Platform::Amiga,
            mode: None,
            seed: time_seed(),
            scale: 4,
            team1: None,
            team2: None,
            export_sounds: None,
            mute: false,
            save_disk: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("saves/save-disk.adf"),
            extract: None,
            assets: None,
        }
    }
}

fn parse_mode(s: &str) -> Result<GameMode, String> {
    match s {
        "knockout" => Ok(GameMode::Knockout),
        "two-players" => Ok(GameMode::TwoPlayers),
        "practice" => Ok(GameMode::Practice),
        "demo" => Ok(GameMode::Demo),
        _ => Err(format!("unknown mode `{s}`")),
    }
}

fn parse_team(s: &str) -> Result<usize, String> {
    match s.parse::<usize>() {
        Ok(team) if team < TEAM_COUNT => Ok(team),
        _ => Err(format!("team must be 0-{}, got `{s}`", TEAM_COUNT - 1)),
    }
}

fn parse_scale(s: &str) -> Result<u8, String> {
    match s {
        "1" | "2" | "4" | "8" => Ok(s.parse().expect("validated")),
        _ => Err(format!("scale must be 1, 2, 4 or 8, got `{s}`")),
    }
}

fn time_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Config>, String> {
        Config::from_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_options() {
        let config = parse(&["--atari", "--mode", "demo", "--seed", "7", "--team1", "3"])
            .unwrap()
            .unwrap();
        assert_eq!(config.platform, Platform::Atari);
        assert_eq!(config.mode, Some(GameMode::Demo));
        assert_eq!(config.seed, 7);
        assert_eq!(config.team1, Some(3));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--team1", "16"]).is_err());
        assert!(parse(&["--team1", "2", "--team2", "2"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--help"]).unwrap().is_none());
    }
}
