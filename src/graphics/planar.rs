//! Decoding bitplane graphics.
//!
//! Both machines store a pixel's colour index spread across several
//! bitplanes; they differ only in how the planes are laid out in memory.

use super::image::Image;
use super::indexed::IndexedImage;
use super::palette::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Atari ST: planes interleaved every 1 or 2 bytes within each line.
    AtariInterleaved,
    /// Amiga: each plane stored whole, one after another.
    AmigaPlanar,
}

/// Whether colour index 0 is drawn or treated as see-through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mask {
    Opaque,
    Transparent,
}

/// Describes a run of equally sized sprites.
#[derive(Clone, Copy, Debug)]
pub struct SpriteFormat {
    pub width: usize,
    pub height: usize,
    pub planes: usize,
    pub layout: Layout,
}

impl SpriteFormat {
    pub fn bytes_per_sprite(&self) -> usize {
        self.width * self.height / 8 * self.planes
    }

    fn strides(&self) -> Strides {
        match self.layout {
            Layout::AtariInterleaved => {
                let interleave = if self.width > 8 { 2 } else { 1 };
                Strides {
                    interleave,
                    scanline: self.width / 8 * self.planes,
                    plane: interleave,
                }
            }
            Layout::AmigaPlanar => Strides {
                interleave: self.width / 8,
                scanline: self.width / 8,
                plane: self.width * self.height / 8,
            },
        }
    }
}

struct Strides {
    /// Bytes of one plane before the next plane's bytes start.
    interleave: usize,
    scanline: usize,
    plane: usize,
}

/// Decodes as many whole sprites as fit in `bytes`.
pub fn decode_sprites(
    bytes: &[u8],
    format: SpriteFormat,
    palette: &Palette,
    mask: Mask,
) -> Vec<Image> {
    bytes
        .chunks_exact(format.bytes_per_sprite())
        .map(|sprite| decode_sprite(sprite, format, palette, mask))
        .collect()
}

/// Decodes as many whole sprites as fit in `bytes`, keeping colour indices.
pub fn decode_indexed_sprites(bytes: &[u8], format: SpriteFormat) -> Vec<IndexedImage> {
    let strides = format.strides();
    bytes
        .chunks_exact(format.bytes_per_sprite())
        .map(|sprite| {
            let pixels = (0..format.height)
                .flat_map(|y| (0..format.width).map(move |x| (x, y)))
                .map(|(x, y)| color_index(sprite, &strides, format.planes, x, y) as u8)
                .collect();
            IndexedImage::new(format.width, format.height, pixels)
        })
        .collect()
}

fn decode_sprite(bytes: &[u8], format: SpriteFormat, palette: &Palette, mask: Mask) -> Image {
    let strides = format.strides();
    let pixels = (0..format.height)
        .flat_map(|y| (0..format.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let index = color_index(bytes, &strides, format.planes, x, y);
            if index == 0 && mask == Mask::Transparent {
                Image::TRANSPARENT
            } else {
                palette.color(index)
            }
        })
        .collect();
    Image::new(format.width, format.height, pixels)
}

fn color_index(bytes: &[u8], strides: &Strides, planes: usize, x: usize, y: usize) -> usize {
    let byte_x = x / 8;
    let bit = 7 - (x % 8);
    let base = byte_x % strides.interleave
        + (byte_x / strides.interleave) * planes * strides.interleave
        + y * strides.scanline;
    (0..planes)
        .filter(|plane| bytes[base + plane * strides.plane] & (1 << bit) != 0)
        .fold(0, |index, plane| index | 1 << plane)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::palette::ColorModel;

    fn grey_palette() -> Palette {
        let words: Vec<u8> = (0..16u16).flat_map(|i| (i * 0x111).to_be_bytes()).collect();
        Palette::read(&words, 0, 16, ColorModel::Amiga)
    }

    #[test]
    fn decodes_amiga_planes() {
        // 8x1 pixels, 2 planes: plane 0 = 0b1000_0001, plane 1 = 0b1000_0000.
        let format = SpriteFormat {
            width: 8,
            height: 1,
            planes: 2,
            layout: Layout::AmigaPlanar,
        };
        let palette = grey_palette();
        let sprite = &decode_sprites(&[0x81, 0x80], format, &palette, Mask::Transparent)[0];
        assert_eq!(sprite.row(0)[0], palette.color(3));
        assert_eq!(sprite.row(0)[1], Image::TRANSPARENT);
        assert_eq!(sprite.row(0)[7], palette.color(1));
    }

    #[test]
    fn decodes_atari_interleaved_words() {
        // 16x1, 4 planes, word interleaved: bytes are [p0 p0 p1 p1 p2 p2 p3 p3].
        let format = SpriteFormat {
            width: 16,
            height: 1,
            planes: 4,
            layout: Layout::AtariInterleaved,
        };
        let bytes = [0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01];
        let palette = grey_palette();
        let sprite = &decode_sprites(&bytes, format, &palette, Mask::Opaque)[0];
        assert_eq!(sprite.row(0)[0], palette.color(0b0101));
        assert_eq!(sprite.row(0)[15], palette.color(0b1000));
        assert_eq!(sprite.row(0)[3], palette.color(0));
    }
}
