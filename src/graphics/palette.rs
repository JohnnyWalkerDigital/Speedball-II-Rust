//! 12-bit hardware palettes.

use crate::data::bytes::BigEndian;

/// How a palette word maps to 8-bit RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorModel {
    /// Atari ST: 3 bits per channel.
    AtariSt,
    /// Amiga OCS: 4 bits per channel.
    Amiga,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    colors: Vec<u32>,
}

impl Palette {
    /// Out-of-range indices render red, as in the original tool.
    const MISSING: u32 = 0xFFFF_0000;
    const ST_LEVELS: [u32; 8] = [0x00, 0x24, 0x49, 0x6D, 0x92, 0xB6, 0xDB, 0xFF];

    /// Reads `count` big-endian palette words starting at `offset`.
    pub fn read(bytes: &[u8], offset: usize, count: usize, model: ColorModel) -> Self {
        let colors = (0..count)
            .map(|i| Self::to_argb(bytes.u16_at(offset + 2 * i), model))
            .collect();
        Self { colors }
    }

    fn to_argb(word: u16, model: ColorModel) -> u32 {
        match model {
            ColorModel::Amiga => amiga_color(word),
            ColorModel::AtariSt => {
                let channel = |shift: u16| Self::ST_LEVELS[usize::from((word >> shift) & 0x7)];
                0xFF00_0000 | (channel(8) << 16) | (channel(4) << 8) | channel(0)
            }
        }
    }

    /// A palette of ARGB colours.
    pub fn from_colors(colors: Vec<u32>) -> Self {
        Self { colors }
    }

    pub fn colors(&self) -> &[u32] {
        &self.colors
    }

    pub fn color(&self, index: usize) -> u32 {
        self.colors.get(index).copied().unwrap_or(Self::MISSING)
    }

    /// A copy where `target` uses the colour of `source` (the game recolours
    /// team 1's red kit blue this way).
    pub fn with_color_copied(&self, source: usize, target: usize) -> Self {
        let mut copy = self.clone();
        copy.colors[target] = self.colors[source];
        copy
    }
}

/// An Amiga colour word (`0x0RGB`, 4 bits per channel) as ARGB.
pub fn amiga_color(word: u16) -> u32 {
    let channel = |shift: u16| u32::from((word >> shift) & 0xF) * 17;
    0xFF00_0000 | (channel(8) << 16) | (channel(4) << 8) | channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_both_colour_models() {
        let words = [0x07, 0x70, 0x0F, 0x0F];
        let st = Palette::read(&words, 0, 1, ColorModel::AtariSt);
        assert_eq!(st.color(0), 0xFFFF_FF00);
        let amiga = Palette::read(&words, 2, 1, ColorModel::Amiga);
        assert_eq!(amiga.color(0), 0xFFFF_00FF);
        assert_eq!(amiga.color(5), 0xFFFF_0000);
    }
}
