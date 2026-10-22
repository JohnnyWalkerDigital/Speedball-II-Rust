//! Assembling the 640x1152 pitch from tiles.

use super::image::Image;
use crate::data::bytes::BigEndian;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 1152;

/// First tiles of the left and right wall strips in the Atari pitch bank.
pub const ATARI_WALL_LEFT: usize = 10;
pub const ATARI_WALL_RIGHT: usize = ATARI_WALL_LEFT + 14;
/// The Amiga wall star tiles (2x2 tiles each).
pub const AMIGA_STARS_LEFT_TOP: usize = 358;
pub const AMIGA_STARS_RIGHT_TOP: usize = 362;
pub const AMIGA_STARS_LEFT_BOTTOM: usize = 366;
pub const AMIGA_STARS_RIGHT_BOTTOM: usize = 370;

/// The ST version has no tile map, so the original tool hard-codes the
/// layout from 32x32 tiles.
///
/// Like the original, this flips some wall tiles in `tiles` *in place* while
/// mirroring the bottom half; the star and electrobounce tiles drawn during
/// the match rely on that.
pub fn build_atari(tiles: &mut [Image]) -> Image {
    const TOP_BORDER: usize = 9;
    const WL: usize = ATARI_WALL_LEFT;
    const WR: usize = ATARI_WALL_RIGHT;
    let mut field = Image::filled(WIDTH, HEIGHT, 0xFF00_0000);
    let tile = |field: &mut Image, image: &Image, x: usize, y: usize| {
        field.draw(image, (x * 32) as i32, (y * 32) as i32);
    };

    // Walls, top half.
    tile(&mut field, &tiles[TOP_BORDER], 0, 0);
    for (row, offset) in [0, 1, 2, 3, 2, 3, 2, 3, 2, 3, 4, 5, 5, 5, 5, 5, 2]
        .into_iter()
        .enumerate()
    {
        tile(&mut field, &tiles[WL + offset], 0, 1 + row);
    }
    tile(&mut field, &tiles[WL + 10], 0, 18);
    tile(&mut field, &tiles[WL + 11], 1, 18);
    tile(&mut field, &tiles[WL + 12], 0, 19);
    tile(&mut field, &tiles[WL + 13], 1, 19);
    for offset in [0, 1, 2, 3, 4, 8, 9] {
        tiles[WL + offset].flip_vertically();
    }
    for (row, offset) in [2, 3, 2, 3, 4, 3, 2, 8, 2, 3, 2, 3, 2, 1, 0]
        .into_iter()
        .enumerate()
    {
        tile(&mut field, &tiles[WL + offset], 0, 20 + row);
    }
    tiles[TOP_BORDER].flip_vertically();
    tile(&mut field, &tiles[TOP_BORDER], 0, 35);

    for (row, offset) in [0, 1, 2, 3, 2, 3, 2, 8, 2, 3, 4, 3, 2, 3, 2]
        .into_iter()
        .enumerate()
    {
        tile(&mut field, &tiles[WR + offset], 19, 1 + row);
    }
    tile(&mut field, &tiles[WR + 10], 18, 16);
    tile(&mut field, &tiles[WR + 11], 19, 16);
    tile(&mut field, &tiles[WR + 12], 18, 17);
    tile(&mut field, &tiles[WR + 13], 19, 17);
    for offset in 0..8 {
        tiles[WR + offset].flip_vertically();
    }
    for (row, offset) in [2, 5, 5, 5, 5, 5, 4, 3, 2, 3, 2, 3, 2, 3, 2, 1, 0]
        .into_iter()
        .enumerate()
    {
        tile(&mut field, &tiles[WR + offset], 19, 18 + row);
    }

    // Playing surface edges.
    tile(&mut field, &tiles[0], 1, 1);
    for i in 0..8 {
        tile(&mut field, &tiles[1], 2 + i * 2, 1);
        tile(&mut field, &tiles[2], 3 + i * 2, 1);
    }
    tile(&mut field, &tiles[3], 18, 1);
    for y in (2..18).chain(20..34) {
        tile(&mut field, &tiles[7], 1, y);
    }
    for y in (2..16).chain(18..34) {
        tile(&mut field, &tiles[8], 18, y);
    }
    for t in &mut tiles[0..4] {
        t.flip_vertically();
    }
    tile(&mut field, &tiles[0], 1, 34);
    for i in 0..8 {
        tile(&mut field, &tiles[1], 2 + i * 2, 34);
        tile(&mut field, &tiles[2], 3 + i * 2, 34);
    }
    tile(&mut field, &tiles[3], 18, 34);

    // Small squares.
    for x in 2..18 {
        for y in (6..8).chain(16..20).chain(28..30) {
            tile(&mut field, &tiles[4], x, y);
        }
    }

    // Big squares are two tiles high with a 2-pixel seam patched in.
    let seam_left = tiles[5].sub_image(0, 15, 32, 2);
    let seam_right = tiles[6].sub_image(0, 15, 32, 2);
    for i in 0..8 {
        let x = 2 + i * 2;
        for y in [2, 4, 8, 10, 12, 14, 20, 22, 24, 26, 30, 32] {
            for dy in 0..2 {
                tile(&mut field, &tiles[5], x, y + dy);
                tile(&mut field, &tiles[6], x + 1, y + dy);
            }
            let seam_y = ((y + 1) * 32 - 1) as i32;
            field.draw(&seam_left, (x * 32) as i32, seam_y);
            field.draw(&seam_right, ((x + 1) * 32) as i32, seam_y);
        }
    }

    // Close off the last row of big squares.
    let edge_left = tiles[5].sub_image(0, 0, 32, 1);
    let edge_right = tiles[6].sub_image(0, 0, 32, 1);
    for i in 0..8 {
        tile(&mut field, &edge_left, 2 + i * 2, 34);
        tile(&mut field, &edge_right, 3 + i * 2, 34);
    }
    field
}

/// The Amiga version keeps a 40x72 map of 16x16 tiles in RAM. Each entry
/// is a 1-based tile number; negative numbers mean "vertically flipped".
pub fn build_amiga(tiles: &[Image], map: &[u8]) -> Image {
    const COLUMNS: usize = 40;
    const ROWS: usize = 72;
    const DEFAULT_TILE: i32 = 162;
    let mut field = Image::filled(WIDTH, HEIGHT, 0xFFFF_0000);
    let mut flipped: Vec<Option<Image>> = vec![None; tiles.len()];
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let mut cell = i32::from(map.i16_at((row * COLUMNS + column) * 2));
            if cell == 0 {
                cell = DEFAULT_TILE;
            }
            let (x, y) = ((column * 16) as i32, (row * 16) as i32);
            if (1..=tiles.len() as i32).contains(&cell) {
                field.draw(&tiles[cell as usize - 1], x, y);
            } else if let Some(index) = usize::try_from(-cell - 1).ok().filter(|&i| i < tiles.len())
            {
                let image = flipped[index].get_or_insert_with(|| tiles[index].flipped_vertically());
                field.draw(image, x, y);
            }
        }
    }
    field
}
