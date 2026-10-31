//! Every image a match needs, decoded from the selected platform's files.

use super::field;
use super::image::Image;
use super::palette::{ColorModel, Palette};
use super::planar::{decode_sprites, Layout, Mask, SpriteFormat};
use crate::config::Platform;
use crate::data::files::AmigaFiles;
use crate::data::{addr, AtariRom};

/// The sprite sheets entities draw from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sheet {
    /// Ball, items, indicators, status LEDs (16x16).
    Entities,
    /// Players, medics, launcher and pitch furniture (32x32).
    Players,
    /// `Players` with team 1's blue kit.
    PlayersBlue,
    /// Pitch tiles.
    Pitch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assets {
    pub platform: Platform,
    /// The whole 640x1152 pitch.
    pub pitch: Image,
    pub pitch_tiles: Vec<Image>,
    /// Top panel backgrounds: goal, injured, plain.
    pub panels: Vec<Image>,
    pub panel_font: Vec<Image>,
    pub digits: Vec<Image>,
    pub status_bar: Image,
    pub entities: Vec<Image>,
    pub players: Vec<Image>,
    /// `players` with team 1's blue kit.
    pub players_blue: Vec<Image>,
    pub palette: Palette,
}

/// One `AddSprites` call from the original: a run of sprites at a disk offset.
#[derive(Clone, Copy)]
struct Run<'a> {
    bytes: &'a [u8],
    offset: usize,
    length: usize,
    width: usize,
    height: usize,
    planes: usize,
    mask: Mask,
}

impl Assets {
    /// Palette indices of the kit colours and the status bar background.
    pub const BLUE: usize = 13;
    pub const RED: usize = 15;
    pub const BACKGROUND: usize = 0;

    pub fn sheet(&self, sheet: Sheet) -> &[Image] {
        match sheet {
            Sheet::Entities => &self.entities,
            Sheet::Players => &self.players,
            Sheet::PlayersBlue => &self.players_blue,
            Sheet::Pitch => &self.pitch_tiles,
        }
    }

    pub fn load_atari(rom: &AtariRom) -> Self {
        let disk = rom.disk();
        let palette = Palette::read(
            disk,
            AtariRom::ram_to_disk(addr::PALETTE_MATCH),
            16,
            ColorModel::AtariSt,
        );
        let blue = palette.with_color_copied(Self::BLUE, Self::RED);
        let decode =
            |runs: &[Run], palette: &Palette| decode_runs(runs, palette, Layout::AtariInterleaved);
        let run = |offset, length, width, height, mask| Run {
            bytes: disk,
            offset,
            length,
            width,
            height,
            planes: 4,
            mask,
        };
        use Mask::{Opaque, Transparent};

        let mut pitch_tiles = decode(
            &[
                run(0xB3800, 4608, 32, 32, Opaque),
                run(0xB4A00, 10240, 640, 32, Opaque),
                run(0xB7200, 14336, 32, 32, Opaque),
            ],
            &palette,
        );
        let pitch = field::build_atari(&mut pitch_tiles);
        let player_runs = [
            run(0xA1100, 61440, 32, 32, Transparent),
            run(0xB0100, 5120, 32, 32, Transparent),
        ];
        Self {
            platform: Platform::Atari,
            pitch,
            pitch_tiles,
            panels: decode(&[run(0x63D6A, 9216, 96, 64, Opaque)], &palette),
            panel_font: decode(&[run(0x6616A, 1152, 8, 8, Opaque)], &palette),
            digits: decode(&[run(0x63C2A, 320, 8, 8, Opaque)], &palette),
            status_bar: decode(&[run(0x6322A, 2560, 320, 16, Opaque)], &palette).remove(0),
            entities: decode(
                &[
                    run(0x9E400, 2816, 16, 16, Transparent),
                    run(0x9EF00, 1536, 16, 16, Transparent),
                    run(0x9F500, 128, 16, 16, Transparent),
                    run(0x9F580, 1024, 16, 16, Opaque),
                    run(0x9F980, 1536, 16, 16, Opaque),
                    run(0x9FF80, 1664, 16, 16, Opaque),
                    run(0xA0600, 1536, 16, 16, Opaque),
                    run(0xA0C00, 1024, 16, 16, Opaque),
                    run(0xA1000, 256, 16, 16, Opaque),
                ],
                &palette,
            ),
            players: decode(
                &[
                    player_runs[0],
                    player_runs[1],
                    run(0xB1500, 2560, 32, 32, Transparent),
                    run(0xB1F00, 1024, 32, 32, Opaque),
                    run(0xB2300, 5120, 32, 32, Opaque),
                ],
                &palette,
            ),
            players_blue: decode(&player_runs, &blue),
            palette,
        }
    }

    pub fn load_amiga(files: &AmigaFiles) -> Self {
        let dump = &files.match_dump[..];
        let disk = &files.disk[..];
        let palette = Palette::read(dump, 0xF2, 32, ColorModel::Amiga);
        let blue = palette.with_color_copied(Self::BLUE, Self::RED);
        let decode =
            |runs: &[Run], palette: &Palette| decode_runs(runs, palette, Layout::AmigaPlanar);
        let run = |bytes, offset, length, size: (usize, usize), planes, mask| Run {
            bytes,
            offset,
            length,
            width: size.0,
            height: size.1,
            planes,
            mask,
        };
        use Mask::{Opaque, Transparent};
        const SMALL: (usize, usize) = (16, 16);
        const LARGE: (usize, usize) = (32, 32);

        let pitch_tiles = decode(&[run(disk, 0xC2C00, 59840, SMALL, 5, Opaque)], &palette);
        let pitch = field::build_amiga(&pitch_tiles, &dump[0x27A..0x27A + 5760]);
        let player_runs = [
            run(disk, 0xAFC80, 61440, LARGE, 4, Transparent),
            run(disk, 0xBEC80, 5120, LARGE, 4, Transparent),
        ];
        Self {
            platform: Platform::Amiga,
            pitch,
            pitch_tiles,
            panels: decode(
                &[run(dump, 0x173BE, 3840 * 3, (96, 64), 5, Opaque)],
                &palette,
            ),
            panel_font: decode(&[run(dump, 0x1AFBE, 1440, (8, 8), 5, Opaque)], &palette),
            digits: decode(&[run(dump, 0x1B55E, 400, (8, 8), 5, Opaque)], &palette),
            status_bar: decode(&[run(disk, 0x68398, 2560, (320, 16), 4, Opaque)], &palette)
                .remove(0),
            entities: decode(
                &[
                    run(disk, 0xAD000, 2816, SMALL, 4, Transparent),
                    run(disk, 0xADB00, 1536, SMALL, 4, Transparent),
                    run(disk, 0xAE100, 128, SMALL, 4, Transparent),
                    run(disk, 0xAE180, 1024, SMALL, 4, Opaque),
                    run(disk, 0xAE580, 1536, SMALL, 4, Opaque),
                    run(disk, 0xAEB80, 1536, SMALL, 4, Transparent),
                    run(disk, 0xAF180, 1536, SMALL, 4, Transparent),
                    run(disk, 0xAF780, 1024, SMALL, 4, Transparent),
                    run(disk, 0xAFB80, 256, SMALL, 4, Opaque),
                ],
                &palette,
            ),
            players: decode(
                &[
                    player_runs[0],
                    player_runs[1],
                    run(disk, 0xC0080, 2560, LARGE, 4, Transparent),
                    run(disk, 0xC0A80, 1280, LARGE, 5, Opaque),
                    run(disk, 0xC0F80, 640, LARGE, 5, Opaque),
                    run(disk, 0xC1200, 6400, LARGE, 5, Opaque),
                ],
                &palette,
            ),
            players_blue: decode(&player_runs, &blue),
            palette,
        }
    }
}

fn decode_runs(runs: &[Run], palette: &Palette, layout: Layout) -> Vec<Image> {
    runs.iter()
        .flat_map(|run| {
            let format = SpriteFormat {
                width: run.width,
                height: run.height,
                planes: run.planes,
                layout,
            };
            decode_sprites(
                &run.bytes[run.offset..run.offset + run.length],
                format,
                palette,
                run.mask,
            )
        })
        .collect()
}
