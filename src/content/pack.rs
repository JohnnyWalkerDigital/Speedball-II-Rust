//! An asset pack: a folder of PNG files and a `pack.json` describing the
//! sprite sheets and holding the palettes.
//!
//! Full-colour images (the match's) are RGBA, with see-through pixels
//! transparent. Images whose pixels are colour numbers (the menus', which
//! fade by palette) are indexed PNGs; a picture's file carries its
//! palette, while a sprite sheet's colours are only there to show it, its
//! real palette being the screen's.
//!
//! A sprite sheet is a grid of equal cells, sixteen to a row unless the
//! sprites are pieces of bigger pictures (the faces); a sheet with sprites
//! of more than one size has a band of rows for each.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::json;
use super::png_files::{self, Indexed, Rgba};
use super::ContentError;
use crate::graphics::ilbm::Picture;
use crate::graphics::image::Image;
use crate::graphics::indexed::{ColorWords, IndexedImage};
use crate::graphics::palette::{amiga_color, color_word, Palette};

/// Bumped when the layout of a pack changes.
const FORMAT: u32 = 1;
const MANIFEST: &str = "pack.json";
const README: &str = "README.txt";
const README_TEXT: &str = "\
Speedball 2 asset pack

Extracted from your copy of the game by `speedball2 --extract`. Play with
it using `speedball2 --assets <this folder>`. Edit the PNG files with any
image editor; the game reads them each time it starts.

Rules:

- match/: full-colour images. Fully transparent pixels are see-through;
  everything else is drawn solid (there is nothing in between).
- menus/ and intro/: indexed-colour images, whose pixels are colour numbers,
  because the menus fade by changing their palette. Keep them indexed when
  you save them.
  - pictures/, intro/title.png, intro/backdrop.png: the palette in the file
    is the picture's own. The Amiga has 4 bits a channel, so only the top
    four bits of each count: use 00, 11, 22 ... EE, FF.
  - Fonts and other sprite sheets: the colours in the file only preview
    the sheet; it is drawn in the palette of the screen it appears on.
    Colour 0 is see-through where the game draws masked.
- Sprite sheets are grids of equal cells, left to right and top to bottom.
  pack.json lists each sheet's cell size, cells to a row and how many
  sprites there are (sheets with more than one size have a band of rows for
  each). To add sprites, enlarge the image and raise the count; the code
  decides which sprite is used where.
- pack.json also holds the match palettes, as #RRGGBB colours (the status
  bar's health bars use them), and the manager screens' palette.
- text/: the menus' and the intro's words, in JSON. In each string:
  - capital letters, digits, spaces and punctuation are themselves (the
    fonts have few marks: the rest draw nothing);
  - _ is a blank the game writes a number into;
  - the panel screens draw their tables with the lines in the text
    (─│┬┴┼├┤), and the league table's arrows are ◢◣, ◥◤ and ◀▶;
  - {x,y} moves to column x and row y (8 and 4 pixels each). Menu strings
    start with one;
  - {#6E} is any other byte.
  The game writes numbers and names into some strings at fixed places, so
  keep those where they are. It checks what it can when it starts, and
  says what is wrong.
- tables/: the game's numbers, in JSON.
  - match.json: the match's lookup tables: speeds, the computer players'
    thinking and look-ahead by intelligence, jumping by speed, tackling,
    equipment, where attackers run to support, where sprites hold the ball.
  - teams.json: each league team's skills, by group (defence, midfield,
    attack, substitutes), and their names on the match panel.
  - squads.json: the squads you start with, the transfer market, the gym's
    prices and the results of matches you are not in.
  - manager_screens.json: the manager screens' keypads (where each key is
    and what it does), the gym's buttons and armour, and their labels.
  - animations.json: the match's animation scripts, each a list of sprite
    numbers shown a frame each, with control words: 'loop' (start again),
    'hold' (stay on the last sprite), 'end_action', 'end_fall', 'hide'.
    Each per-direction animation names its script for each direction (0
    up, then clockwise). Scripts are laid out in the order listed, and an
    entity carries its place in a script over when its script changes, as
    in the original: keep scripts the same length unless you mean to
    change that.
  - start.json: what every match starts from: each entity's record (the
    ball, the pitch's furniture, items), each player's (where they stand
    at kick-off, the zone they patrol), and the roster. Scripts are named
    as in animations.json.
  Skills are always in this order: aggression, attack, defence, speed,
  throwing, power, stamina, intelligence. Tables must keep their sizes.
- sound/: what the sound driver plays.
  - sounds.json: by sound number (from 0), the sequence each of the four
    channels plays (0 for none); the tremolo and vibrato effects; and
    the pitch tables, note periods from each starting octave (lower is
    higher). Sounds that never play can be listed in 'silent'.
  - game/: the match's sounds. sequences.txt holds the sequences, each
    starting 'sequence N'. Numbers are notes (in semitones, from the
    instrument's octave); the commands are: tempo (beats a minute),
    length (of the notes after, in quarter beats), rest (wait a length,
    the note sounding on), volume (0 to 63), instrument, effect,
    transpose (add quarter semitones; 0 resets), set_transpose,
    repeat N ... next (plays the part N + 1 times), call N ... return,
    jump N, restart, stop, envelope, no_envelope, loop_effects. 'byte 0xNN' is any other byte. # starts a comment.
    instruments.json lists the samples in the order they are laid out
    and each instrument by number: its sample, whether it loops (from
    loop_start, in bytes), its octave (0 to 2), and a length in words
    if it plays other than the whole sample (it can run on into the
    samples after). samples/ holds them as 8-bit mono WAV files (16-bit
    is read too); the rate does not matter, notes set the pitch.
  - menus/samples/: the samples the menus have instead of the match's.
    The menus play the match's sequences and instruments.
  - intro/: the intro's music, laid out like game/.
  Each bank is laid out in memory in the order listed, as in the
  original, so a sample that changes length moves the ones after it.

Don't share a pack you extracted: the art belongs to the game's owners.
";
/// Sprites to a row, unless a sheet says otherwise.
pub const COLUMNS: usize = 16;

/// `pack.json`.
#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    /// Each sheet's bands of sprites, by file name.
    sheets: BTreeMap<String, Vec<Band>>,
    /// Palettes as `#RRGGBB` colours, by name.
    palettes: BTreeMap<String, Vec<String>>,
}

/// A run of same-sized sprites in a sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Band {
    cell_width: usize,
    cell_height: usize,
    count: usize,
    /// Cells to a row.
    columns: usize,
}

impl Band {
    fn columns(&self) -> usize {
        self.count.clamp(1, self.columns)
    }

    fn rows(&self) -> usize {
        self.count.div_ceil(self.columns)
    }

    fn width(&self) -> usize {
        self.columns() * self.cell_width
    }

    fn height(&self) -> usize {
        self.rows() * self.cell_height
    }
}

/// Writes a pack.
pub struct PackWriter {
    dir: PathBuf,
    manifest: Manifest,
}

impl PackWriter {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
            manifest: Manifest {
                format: FORMAT,
                ..Manifest::default()
            },
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.png"))
    }

    pub fn image(&mut self, name: &str, image: &Image) -> Result<(), ContentError> {
        png_files::write_rgba(
            &self.path(name),
            &Rgba {
                width: image.width(),
                height: image.height(),
                pixels: image.pixels().to_vec(),
            },
        )
    }

    pub fn sheet(&mut self, name: &str, sprites: &[Image]) -> Result<(), ContentError> {
        let cells: Vec<Cell<u32>> = sprites
            .iter()
            .map(|s| Cell {
                width: s.width(),
                height: s.height(),
                pixels: s.pixels(),
            })
            .collect();
        let (bands, sheet) = lay_out(&cells, COLUMNS);
        self.manifest.sheets.insert(name.into(), bands);
        png_files::write_rgba(&self.path(name), &sheet.into())
    }

    /// A sheet of colour numbers, `columns` to a row, shown in `preview`'s
    /// colours.
    pub fn indexed_sheet(
        &mut self,
        name: &str,
        sprites: &[IndexedImage],
        columns: usize,
        preview: &ColorWords,
    ) -> Result<(), ContentError> {
        let cells: Vec<Cell<u8>> = sprites
            .iter()
            .map(|s| Cell {
                width: s.width(),
                height: s.height(),
                pixels: s.pixels(),
            })
            .collect();
        let (bands, sheet) = lay_out(&cells, columns);
        self.manifest.sheets.insert(name.into(), bands);
        png_files::write_indexed(
            &self.path(name),
            &Indexed {
                width: sheet.width,
                height: sheet.height,
                pixels: sheet.pixels,
                palette: rgb_list(preview),
            },
        )
    }

    pub fn picture(&mut self, name: &str, picture: &Picture) -> Result<(), ContentError> {
        png_files::write_indexed(
            &self.path(name),
            &Indexed {
                width: picture.image.width(),
                height: picture.image.height(),
                pixels: picture.image.pixels().to_vec(),
                palette: rgb_list(&picture.palette),
            },
        )
    }

    pub fn palette(&mut self, name: &str, palette: &Palette) {
        let colors = palette
            .colors()
            .iter()
            .map(|&argb| format!("#{:06X}", argb & 0xFF_FFFF))
            .collect();
        self.manifest.palettes.insert(name.into(), colors);
    }

    pub fn color_words(&mut self, name: &str, words: &ColorWords) {
        let palette = Palette::from_colors(words.iter().map(|&w| amiga_color(w)).collect());
        self.palette(name, &palette);
    }

    /// Writes `value` as `{name}.json`.
    pub fn json<T: Serialize>(&mut self, name: &str, value: &T) -> Result<(), ContentError> {
        let path = self.dir.join(format!("{name}.json"));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| ContentError::io(dir, e))?;
        }
        std::fs::write(&path, json::to_string(value)).map_err(|e| ContentError::io(&path, e))
    }

    /// Writes `name` (a path in the pack, with its extension).
    pub fn file(&mut self, name: &str, bytes: &[u8]) -> Result<(), ContentError> {
        let path = self.dir.join(name);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| ContentError::io(dir, e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| ContentError::io(&path, e))
    }

    /// Writes `pack.json`, and a README for whoever edits the pack.
    pub fn finish(self) -> Result<(), ContentError> {
        let readme = self.dir.join(README);
        std::fs::write(&readme, README_TEXT).map_err(|e| ContentError::io(&readme, e))?;
        let path = self.dir.join(MANIFEST);
        let json = serde_json::to_string_pretty(&self.manifest)
            .map_err(|e| ContentError::Manifest(e.to_string()))?;
        std::fs::write(&path, json + "\n").map_err(|e| ContentError::io(&path, e))
    }
}

/// Reads a pack.
pub struct Pack {
    dir: PathBuf,
    manifest: Manifest,
}

impl Pack {
    pub fn open(dir: &Path) -> Result<Self, ContentError> {
        let path = dir.join(MANIFEST);
        let json = std::fs::read_to_string(&path).map_err(|e| ContentError::io(&path, e))?;
        let manifest: Manifest = serde_json::from_str(&json)
            .map_err(|e| ContentError::Manifest(format!("{}: {e}", path.display())))?;
        if manifest.format != FORMAT {
            return Err(ContentError::Manifest(format!(
                "{} is format {}; this game reads format {FORMAT}",
                path.display(),
                manifest.format
            )));
        }
        Ok(Self {
            dir: dir.to_owned(),
            manifest,
        })
    }

    /// Whether the pack has a folder `name`.
    pub fn has_dir(&self, name: &str) -> bool {
        self.dir.join(name).is_dir()
    }

    pub fn json_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    /// Where `name` (a path in the pack, with its extension) is.
    pub fn file_path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Reads `name`, which must be there.
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>, ContentError> {
        let path = self.file_path(name);
        if !path.exists() {
            return Err(ContentError::Missing(path.display().to_string()));
        }
        std::fs::read(&path).map_err(|e| ContentError::io(&path, e))
    }

    /// Reads `{name}.json`, which must be there and pass `check`.
    pub fn checked_json<T: DeserializeOwned>(
        &self,
        name: &str,
        check: impl FnOnce(&T) -> Result<(), String>,
    ) -> Result<T, ContentError> {
        let path = self.json_path(name);
        let value = self
            .json(name)?
            .ok_or_else(|| ContentError::Missing(path.display().to_string()))?;
        check(&value).map_err(|problems| ContentError::invalid(&path, problems))?;
        Ok(value)
    }

    /// Reads `{name}.json`, `None` if there is none.
    pub fn json<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, ContentError> {
        let path = self.json_path(name);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&path).map_err(|e| ContentError::io(&path, e))?;
        serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| ContentError::invalid(&path, e.to_string()))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.png"))
    }

    fn bands(&self, name: &str) -> Result<&[Band], ContentError> {
        let bands = self
            .manifest
            .sheets
            .get(name)
            .ok_or_else(|| ContentError::Missing(format!("sheet \"{name}\" in {MANIFEST}")))?;
        if bands.iter().any(|band| band.columns == 0) {
            return Err(ContentError::Manifest(format!(
                "sheet \"{name}\" in {MANIFEST} has a band with no columns"
            )));
        }
        Ok(bands)
    }

    pub fn image(&self, name: &str) -> Result<Image, ContentError> {
        let png = png_files::read_rgba(&self.path(name))?;
        Ok(Image::new(png.width, png.height, png.pixels))
    }

    pub fn sheet(&self, name: &str) -> Result<Vec<Image>, ContentError> {
        let path = self.path(name);
        let png = png_files::read_rgba(&path)?;
        let sheet = Grid {
            width: png.width,
            height: png.height,
            pixels: png.pixels,
        };
        cut(&sheet, self.bands(name)?, &path).map(|cells| {
            cells
                .into_iter()
                .map(|c| Image::new(c.width, c.height, c.pixels))
                .collect()
        })
    }

    pub fn indexed_sheet(&self, name: &str) -> Result<Vec<IndexedImage>, ContentError> {
        let path = self.path(name);
        let png = png_files::read_indexed(&path)?;
        let sheet = Grid {
            width: png.width,
            height: png.height,
            pixels: png.pixels,
        };
        cut(&sheet, self.bands(name)?, &path).map(|cells| {
            cells
                .into_iter()
                .map(|c| IndexedImage::new(c.width, c.height, c.pixels))
                .collect()
        })
    }

    pub fn picture(&self, name: &str) -> Result<Picture, ContentError> {
        let png = png_files::read_indexed(&self.path(name))?;
        let mut palette = [0; 32];
        for (word, rgb) in palette.iter_mut().zip(&png.palette) {
            *word = color_word(*rgb);
        }
        Ok(Picture {
            image: IndexedImage::new(png.width, png.height, png.pixels),
            palette,
        })
    }

    pub fn palette(&self, name: &str) -> Result<Palette, ContentError> {
        let colors =
            self.manifest.palettes.get(name).ok_or_else(|| {
                ContentError::Missing(format!("palette \"{name}\" in {MANIFEST}"))
            })?;
        colors
            .iter()
            .map(|hex| parse_color(hex).map(|[r, g, b]| u32::from_be_bytes([0xFF, r, g, b])))
            .collect::<Result<_, _>>()
            .map(Palette::from_colors)
    }

    /// A palette as Amiga colour words (four bits a channel).
    pub fn color_words(&self, name: &str) -> Result<ColorWords, ContentError> {
        let palette = self.palette(name)?;
        let mut words = [0; 32];
        for (word, &argb) in words.iter_mut().zip(palette.colors()) {
            let [_, r, g, b] = argb.to_be_bytes();
            *word = color_word([r, g, b]);
        }
        Ok(words)
    }
}

/// A sprite in a sheet.
struct Cell<'a, P> {
    width: usize,
    height: usize,
    pixels: &'a [P],
}

/// A whole sheet, or a sprite cut from one.
struct Grid<P> {
    width: usize,
    height: usize,
    pixels: Vec<P>,
}

impl From<Grid<u32>> for Rgba {
    fn from(grid: Grid<u32>) -> Self {
        Rgba {
            width: grid.width,
            height: grid.height,
            pixels: grid.pixels,
        }
    }
}

/// Lays sprites out in bands of same-sized cells.
fn lay_out<P: Copy + Default>(sprites: &[Cell<P>], columns: usize) -> (Vec<Band>, Grid<P>) {
    let mut bands: Vec<Band> = Vec::new();
    for sprite in sprites {
        match bands.last_mut() {
            Some(band) if (band.cell_width, band.cell_height) == (sprite.width, sprite.height) => {
                band.count += 1;
            }
            _ => bands.push(Band {
                cell_width: sprite.width,
                cell_height: sprite.height,
                count: 1,
                columns,
            }),
        }
    }
    let width = bands.iter().map(Band::width).max().unwrap_or(0);
    let height = bands.iter().map(Band::height).sum();
    let mut sheet = Grid {
        width,
        height,
        pixels: vec![P::default(); width * height],
    };
    let mut sprites = sprites.iter();
    let mut top = 0;
    for band in &bands {
        for (i, sprite) in sprites.by_ref().take(band.count).enumerate() {
            let x = (i % band.columns) * band.cell_width;
            let y = top + (i / band.columns) * band.cell_height;
            for row in 0..sprite.height {
                let start = (y + row) * width + x;
                sheet.pixels[start..start + sprite.width]
                    .copy_from_slice(&sprite.pixels[row * sprite.width..(row + 1) * sprite.width]);
            }
        }
        top += band.height();
    }
    (bands, sheet)
}

/// Cuts a sheet back into its sprites.
fn cut<P: Copy>(
    sheet: &Grid<P>,
    bands: &[Band],
    path: &Path,
) -> Result<Vec<Grid<P>>, ContentError> {
    let width = bands.iter().map(Band::width).max().unwrap_or(0);
    let height: usize = bands.iter().map(Band::height).sum();
    if sheet.width < width || sheet.height < height {
        return Err(ContentError::png(
            path,
            format!(
                "is {}x{}, but its sprites in {MANIFEST} need {width}x{height}",
                sheet.width, sheet.height
            ),
        ));
    }
    let mut sprites = Vec::new();
    let mut top = 0;
    for band in bands {
        for i in 0..band.count {
            let x = (i % band.columns) * band.cell_width;
            let y = top + (i / band.columns) * band.cell_height;
            let pixels = (0..band.cell_height)
                .flat_map(|row| {
                    let start = (y + row) * sheet.width + x;
                    sheet.pixels[start..start + band.cell_width].iter().copied()
                })
                .collect();
            sprites.push(Grid {
                width: band.cell_width,
                height: band.cell_height,
                pixels,
            });
        }
        top += band.height();
    }
    Ok(sprites)
}

/// Colour words as 8-bit RGB.
fn rgb_list(words: &ColorWords) -> Vec<[u8; 3]> {
    words
        .iter()
        .map(|&word| {
            let [_, r, g, b] = amiga_color(word).to_be_bytes();
            [r, g, b]
        })
        .collect()
}

fn parse_color(hex: &str) -> Result<[u8; 3], ContentError> {
    let bad = || ContentError::Manifest(format!("\"{hex}\" is not a #RRGGBB colour"));
    let digits = hex
        .strip_prefix('#')
        .filter(|d| d.len() == 6)
        .ok_or_else(bad)?;
    let value = u32::from_str_radix(digits, 16).map_err(|_| bad())?;
    let [_, r, g, b] = value.to_be_bytes();
    Ok([r, g, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_of_mixed_sizes_lay_out_in_bands_and_cut_back() {
        let sprite = |w: usize, h: usize, v: u8| (w, h, vec![v; w * h]);
        let sprites: Vec<_> = (0..20)
            .map(|i| sprite(8, 8, i))
            .chain([sprite(16, 4, 99)])
            .collect();
        let cells: Vec<Cell<u8>> = sprites
            .iter()
            .map(|(w, h, p)| Cell {
                width: *w,
                height: *h,
                pixels: p,
            })
            .collect();
        let (bands, sheet) = lay_out(&cells, COLUMNS);
        assert_eq!(bands.len(), 2);
        assert_eq!((sheet.width, sheet.height), (128, 8 + 8 + 4));
        let back = cut(&sheet, &bands, Path::new("test.png")).unwrap();
        assert_eq!(back.len(), 21);
        for (original, cut) in sprites.iter().zip(&back) {
            assert_eq!(
                (original.0, original.1, &original.2),
                (cut.width, cut.height, &cut.pixels)
            );
        }
    }

    #[test]
    fn colours_are_hex_and_amiga_words_keep_four_bits() {
        assert_eq!(parse_color("#12AbF0").unwrap(), [0x12, 0xAB, 0xF0]);
        assert!(parse_color("12ABF0").is_err());
        assert_eq!(color_word([0xAA, 0x55, 0xFF]), 0xA5F);
        assert_eq!(rgb_list(&[0xA5F; 32])[0], [0xAA, 0x55, 0xFF]);
    }
}
