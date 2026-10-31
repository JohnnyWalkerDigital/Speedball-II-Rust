//! The desktop shell: a window, the keyboard, the speakers and the clock,
//! offered to the game as a [`Host`].

use std::error::Error;
use std::path::Path;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};

use crate::config::{Config, Platform};
use crate::content::{self, AmigaOriginals, ContentSource, Originals, Pack};
use crate::data::amiga::{menu_ram, Overlays};
use crate::data::files::{
    read_amiga_files, read_amiga_intro_dump, read_amiga_match_dump, AmigaFiles,
};
use crate::data::lookups::Lookups;
use crate::data::save_disk::SaveDisk;
use crate::data::{sounds, AtariRom, GameFiles};
use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::game::roster::Teams;
use crate::game::{Match, MatchSetup};
use crate::graphics::{Assets, Canvas};
use crate::menu::flow::{self, Game, MenuContent};
use crate::menu::host::{Button, ButtonEvent, Closed, Control, Host, SoundBank};
use crate::menu::intro::{self, IntroAssets};
use crate::menu::play::{run_match, Controls};
use crate::menu::ram::MenuRam;
use crate::menu::ui::{Ui, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::render::MatchRenderer;
use crate::sound::SoundOutput;

const WINDOW_TITLE: &str = "Speedball 2";

const BINDINGS: &[(Key, Button)] = &[
    (Key::Up, Button::Joystick(Side::Team1, Control::Up)),
    (Key::Down, Button::Joystick(Side::Team1, Control::Down)),
    (Key::Left, Button::Joystick(Side::Team1, Control::Left)),
    (Key::Right, Button::Joystick(Side::Team1, Control::Right)),
    (Key::Space, Button::Joystick(Side::Team1, Control::Fire)),
    (Key::RightCtrl, Button::Joystick(Side::Team1, Control::Fire)),
    (Key::W, Button::Joystick(Side::Team2, Control::Up)),
    (Key::S, Button::Joystick(Side::Team2, Control::Down)),
    (Key::A, Button::Joystick(Side::Team2, Control::Left)),
    (Key::D, Button::Joystick(Side::Team2, Control::Right)),
    (Key::LeftShift, Button::Joystick(Side::Team2, Control::Fire)),
    (Key::LeftCtrl, Button::Joystick(Side::Team2, Control::Fire)),
    (Key::Escape, Button::Escape),
    (Key::P, Button::Pause),
    (Key::N, Button::Step),
    (Key::F, Button::FastForward),
    (Key::Key1, Button::Number(1)),
    (Key::Key2, Button::Number(2)),
    (Key::Key3, Button::Number(3)),
    (Key::Key4, Button::Number(4)),
    (Key::Key5, Button::Number(5)),
    (Key::Key6, Button::Number(6)),
    (Key::Key7, Button::Number(7)),
    (Key::Key8, Button::Number(8)),
    (Key::Key9, Button::Number(9)),
];

/// Saves the Amiga sound samples as WAV files.
pub fn export_sounds(config: &Config, dir: &Path) -> Result<(), Box<dyn Error>> {
    let files = GameFiles::load(&config.data_dir, Platform::Amiga)?;
    let amiga = files.amiga.expect("loaded for the Amiga");
    let samples = sounds::export_wavs(&amiga.match_dump, dir)?;
    println!(
        "Wrote {samples} raw instrument samples to {} (at {} Hz, which may not be their real pitch)",
        dir.display(),
        sounds::EXPORT_RATE
    );
    let rendered = dir.join("rendered");
    let count = crate::sound::export_sounds(amiga.match_dump, &rendered)?;
    println!(
        "Wrote {count} game sounds, played through the driver, to {}",
        rendered.display()
    );
    Ok(())
}

/// The originals the game needs, as loaded.
struct Loaded {
    rom: AtariRom,
    amiga: Option<AmigaFiles>,
    intro_dump: Option<Vec<u8>>,
}

impl Loaded {
    /// The ST disk always; the Amiga files for Amiga graphics, and for the
    /// menus (unless `--mode` skips them).
    fn load(config: &Config) -> Result<Self, Box<dyn Error>> {
        let files = GameFiles::load(&config.data_dir, config.platform)?;
        let amiga = match files.amiga {
            Some(amiga) => Some(amiga),
            None if config.mode.is_none() => {
                Some(read_amiga_files(&config.data_dir).map_err(|e| {
                    format!("the menus need the Amiga files ({e}); use --mode to skip them")
                })?)
            }
            None => None,
        };
        let intro_dump = if config.mode.is_none() {
            read_amiga_intro_dump(&config.data_dir)
                .map_err(|error| eprintln!("no intro: {error}"))
                .ok()
        } else {
            None
        };
        Ok(Self {
            rom: AtariRom::new(files.atari_disk),
            amiga,
            intro_dump,
        })
    }
}

/// The menus' memory, rebuilt from the Amiga disk and dump.
fn amiga_menu_ram(amiga: &AmigaFiles) -> Result<(Overlays<'_>, MenuRam), Box<dyn Error>> {
    let overlays = Overlays::new(&amiga.disk).ok_or("the Amiga disk has no overlay directory")?;
    let ram = MenuRam::new(menu_ram(&amiga.match_dump, &overlays));
    Ok((overlays, ram))
}

/// The graphics, text and tables a game needs, from the originals or
/// `--assets`.
struct Content {
    lookups: Lookups,
    teams: Teams,
    matches: Assets,
    menus: Option<MenuContent>,
    intro: Option<IntroAssets>,
}

impl Content {
    fn load(
        config: &Config,
        originals: &Originals,
        with_menus: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let pack = config.assets.as_deref().map(Pack::open).transpose()?;
        let source: &dyn ContentSource = match &pack {
            Some(pack) => pack,
            None => originals,
        };
        let lookups = source.lookups()?;
        let teams = source.teams()?;
        let matches = source.match_graphics(config.platform)?;
        if !with_menus {
            return Ok(Self {
                lookups,
                teams,
                matches,
                menus: None,
                intro: None,
            });
        }
        let intro = source.intro_graphics()?.zip(source.intro_text()?);
        Ok(Self {
            lookups,
            teams,
            matches,
            menus: Some(MenuContent {
                assets: source.menu_graphics()?,
                text: source.menu_text()?,
                tables: source.menu_tables()?,
                manager: source.manager_tables()?,
            }),
            intro: intro.map(|(graphics, text)| IntroAssets { graphics, text }),
        })
    }
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let loaded = Loaded::load(&config)?;
    let with_menus = config.mode.is_none();
    let menu_memory = match (&loaded.amiga, with_menus) {
        (Some(amiga), true) => Some(amiga_menu_ram(amiga)?),
        _ => None,
    };
    let originals = Originals {
        rom: &loaded.rom,
        amiga: loaded.amiga.as_ref().zip(menu_memory.as_ref()).map(
            |(files, (overlays, menu_ram))| AmigaOriginals {
                files,
                overlays,
                menu_ram,
            },
        ),
        intro_dump: loaded.intro_dump.as_deref(),
    };
    let content = Content::load(&config, &originals, with_menus)?;
    let mut rom = loaded.rom;
    rom.set_lookups(content.lookups);
    rom.set_teams(content.teams);
    let renderer = MatchRenderer::new(content.matches, &rom);
    let rom = Rc::new(rom);
    match (menu_memory, loaded.amiga.as_ref(), content.menus) {
        (Some((_, ram)), Some(amiga), Some(menus)) => {
            // The intro's music is in its dump.
            let intro = content.intro.zip(loaded.intro_dump);
            play_with_menus(&config, rom, renderer, amiga, ram, menus, intro)
        }
        _ => play_one_match(&config, rom, renderer),
    }
}

/// `--extract`: writes the graphics as an asset pack.
pub fn extract(config: &Config, dir: &Path) -> Result<(), Box<dyn Error>> {
    // Whatever originals there are: the Amiga ones are optional.
    let files = GameFiles::load(&config.data_dir, Platform::Atari)?;
    let rom = AtariRom::new(files.atari_disk);
    let amiga = read_amiga_files(&config.data_dir)
        .map_err(|error| eprintln!("no Amiga graphics or menus: {error}"))
        .ok();
    let menu_memory = amiga.as_ref().map(amiga_menu_ram).transpose()?;
    let intro_dump = read_amiga_intro_dump(&config.data_dir)
        .map_err(|error| eprintln!("no intro: {error}"))
        .ok();
    let originals = Originals {
        rom: &rom,
        amiga: amiga
            .as_ref()
            .zip(menu_memory.as_ref())
            .map(|(files, (overlays, menu_ram))| AmigaOriginals {
                files,
                overlays,
                menu_ram,
            }),
        intro_dump: intro_dump.as_deref(),
    };
    let extracted = content::extract(&originals, dir)?;
    let platforms: Vec<_> = extracted
        .platforms
        .iter()
        .map(|p| format!("{p:?}"))
        .collect();
    println!(
        "Wrote an asset pack to {}: match graphics ({}){}{}. Play with it using --assets.",
        dir.display(),
        platforms.join(", "),
        if extracted.menus { ", menus" } else { "" },
        if extracted.intro { ", intro" } else { "" },
    );
    Ok(())
}

/// `--mode`: one match, as before the menus existed.
fn play_one_match(
    config: &Config,
    rom: Rc<AtariRom>,
    renderer: MatchRenderer,
) -> Result<(), Box<dyn Error>> {
    let mut game = Match::new(MatchSetup::from_config(&rom, config), Rc::clone(&rom));
    let title = format!(
        "{WINDOW_TITLE} - {} vs {}",
        game.teams.team1.full_name.to_display_string(),
        game.teams.team2.full_name.to_display_string()
    );
    println!("{title} (seed {})", config.seed);
    let sound = SoundBanks::without_menus(config);
    let mut host = WindowHost::open(config.scale, &title, sound)?;
    let mut ui = Ui::new(&mut host);
    let mut sped_up = false;
    let controls = Controls {
        take_over_on_touch: true,
        hold_at_end: true,
        replay_saves: None,
        sped_up: &mut sped_up,
        may_speed_up: false,
    };
    // Escape or closing the window ends it either way.
    let _ = run_match(&mut ui, &mut game, &renderer, controls);
    Ok(())
}

fn play_with_menus(
    config: &Config,
    rom: Rc<AtariRom>,
    renderer: MatchRenderer,
    amiga: &AmigaFiles,
    ram: MenuRam,
    menus: MenuContent,
    intro: Option<(IntroAssets, Vec<u8>)>,
) -> Result<(), Box<dyn Error>> {
    let (intro, intro_dump) = intro.unzip();
    let sound = SoundBanks::with_menus(config, &amiga.match_dump, ram.bytes(), intro_dump);
    let mut host = WindowHost::open(config.scale, WINDOW_TITLE, sound)?;
    let mut ui = Ui::new(&mut host);
    if let Some(intro) = &intro {
        if intro::run(&mut ui, intro).is_err() {
            return Ok(()); // The window was closed.
        }
    }
    let save_disk = SaveDisk::new(&config.save_disk);
    let mut game = Game::new(rom, ram, menus, renderer, save_disk, config.seed);
    flow::run(&mut ui, &mut game);
    Ok(())
}

/// The sound driver and the two sets of samples it switches between.
struct SoundBanks {
    output: SoundOutput,
    match_ram: Vec<u8>,
    menu_ram: Option<Vec<u8>>,
    intro_ram: Option<Vec<u8>>,
}

impl SoundBanks {
    fn with_menus(
        config: &Config,
        match_dump: &[u8],
        menu_ram: &[u8],
        intro_ram: Option<Vec<u8>>,
    ) -> Option<Self> {
        if config.mute {
            return None;
        }
        let output = start_sound(menu_ram.to_vec())?;
        Some(Self {
            output,
            match_ram: match_dump.to_vec(),
            menu_ram: Some(menu_ram.to_vec()),
            intro_ram,
        })
    }

    /// The sound driver runs on the Amiga data, whichever graphics are
    /// shown. Without it (or without an audio device) the game is silent.
    fn without_menus(config: &Config) -> Option<Self> {
        if config.mute {
            return None;
        }
        let dump = match read_amiga_match_dump(&config.data_dir) {
            Ok(dump) => dump,
            Err(error) => {
                eprintln!("sound off: {error}");
                return None;
            }
        };
        let output = start_sound(dump.clone())?;
        Some(Self {
            output,
            match_ram: dump,
            menu_ram: None,
            intro_ram: None,
        })
    }

    fn use_bank(&self, bank: SoundBank) {
        let ram = match bank {
            SoundBank::Match => &self.match_ram,
            SoundBank::Menu => match &self.menu_ram {
                Some(ram) => ram,
                None => return,
            },
            SoundBank::Intro => match &self.intro_ram {
                Some(ram) => ram,
                None => return,
            },
        };
        self.output.set_ram(ram.clone());
    }
}

fn start_sound(ram: Vec<u8>) -> Option<SoundOutput> {
    match SoundOutput::start(ram) {
        Ok(output) => Some(output),
        Err(error) => {
            eprintln!("sound off: {error}");
            None
        }
    }
}

/// A window paced to the game's clock.
struct WindowHost {
    window: Window,
    sound: Option<SoundBanks>,
    title: String,
    status: String,
    next_frame: Instant,
}

impl WindowHost {
    fn open(scale: u8, title: &str, sound: Option<SoundBanks>) -> Result<Self, minifb::Error> {
        let scale = match scale {
            1 => Scale::X1,
            2 => Scale::X2,
            8 => Scale::X8,
            _ => Scale::X4,
        };
        let mut window = Window::new(
            title,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
            WindowOptions {
                scale,
                ..WindowOptions::default()
            },
        )?;
        // We pace frames ourselves.
        window.set_target_fps(0);
        // Roughly the Windows defaults the C# port's key handling relied on.
        window.set_key_repeat_delay(0.5);
        window.set_key_repeat_rate(0.033);
        Ok(Self {
            window,
            sound,
            title: title.to_string(),
            status: String::new(),
            next_frame: Instant::now(),
        })
    }

    /// Key presses and releases since the last frame. Held keys repeat, as
    /// the C# port's match controls relied on.
    fn button_events(&self) -> Vec<ButtonEvent> {
        let fresh = self.window.get_keys_pressed(KeyRepeat::No);
        let mut events = Vec::new();
        for key in self.window.get_keys_pressed(KeyRepeat::Yes) {
            for &(_, button) in BINDINGS.iter().filter(|(k, _)| *k == key) {
                events.push(ButtonEvent {
                    button,
                    pressed: true,
                    repeat: !fresh.contains(&key),
                });
            }
        }
        for key in self.window.get_keys_released() {
            for &(_, button) in BINDINGS.iter().filter(|(k, _)| *k == key) {
                events.push(ButtonEvent::release(button));
            }
        }
        events
    }
}

impl Host for WindowHost {
    fn show(&mut self, frame: &Canvas, wait: Duration) -> Result<Vec<ButtonEvent>, Closed> {
        if !self.window.is_open() {
            return Err(Closed);
        }
        self.window
            .update_with_buffer(frame.pixels(), frame.width(), frame.height())
            .map_err(|_| Closed)?;
        let events = self.button_events();

        self.next_frame += wait;
        let now = Instant::now();
        match self.next_frame.checked_duration_since(now) {
            Some(wait) => thread::sleep(wait),
            None => self.next_frame = now, // Running late: don't try to catch up.
        }
        Ok(events)
    }

    fn play_sound(&mut self, sound: Sound) {
        if let Some(banks) = &self.sound {
            banks.output.play(sound);
        }
    }

    fn use_sound_bank(&mut self, bank: SoundBank) {
        if let Some(banks) = &self.sound {
            banks.use_bank(bank);
        }
    }

    fn set_status(&mut self, status: &str) {
        if status != self.status {
            self.status = status.to_string();
            let title = match status {
                "" => self.title.clone(),
                _ => format!("{} [{status}]", self.title),
            };
            self.window.set_title(&title);
        }
    }
}
