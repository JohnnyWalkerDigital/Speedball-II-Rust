//! Laying out text the way the menu code does.
//!
//! Strings mix letters with cursor codes: a byte below `' '` sets the
//! column, and the byte after it the row, in 8-pixel columns and 4-pixel
//! rows. Letters are stored from `'A'`, with the digits straight after `'Z'`
//! (so `'['` is 0 and `'\'` is 1).

use crate::game::m68k::ext_divu_register;

/// A character to draw: its index in the font and its top-left pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub index: usize,
    pub x: i32,
    pub y: i32,
}

/// `display_string`: 16x16 letters, two columns apart, wrapping to the next
/// line of letters after column 37.
pub fn big_glyphs(text: &[u8]) -> Vec<Glyph> {
    layout(text, |x, y| {
        let (mut x, mut y) = (x + 2, y);
        if x > 0x25 {
            x = 1;
            y += 5;
        }
        (x, y)
    })
}

/// `draw_string`: 8x8 letters, one column apart, wrapping after column 39.
pub fn small_glyphs(text: &[u8]) -> Vec<Glyph> {
    layout(text, |x, y| {
        if x + 1 == 0x28 {
            (0, y + 1)
        } else {
            (x + 1, y)
        }
    })
}

fn layout(text: &[u8], advance: impl Fn(u8, u8) -> (u8, u8)) -> Vec<Glyph> {
    let mut glyphs = Vec::new();
    let (mut x, mut y) = (0u8, 0u8);
    let mut bytes = text.iter().copied();
    while let Some(c) = bytes.next() {
        match c {
            0 => break,
            c if c < b' ' => {
                x = c;
                y = bytes.next().unwrap_or(0);
            }
            b' ' => (x, y) = advance(x, y),
            c => {
                glyphs.push(Glyph {
                    // Bytes below 'A' wrap round to large indices that no
                    // font has; they draw nothing.
                    index: usize::from(c.wrapping_sub(b'A')),
                    x: i32::from(x) * 8,
                    y: i32::from(y) * 4,
                });
                (x, y) = advance(x, y);
            }
        }
    }
    glyphs
}

/// `format_N_decimal_digits`: writes `count` digits of `value` into `text`,
/// right to left, ending at `last`. Like the 68000 code, a value that does
/// not fit in 16 bits after dividing (any negative value) leaves the
/// division undone, which shows as `'Z'`s.
pub fn write_digits(text: &mut [u8], last: usize, value: i16, count: usize) {
    let mut d1 = value;
    for i in 0..count {
        let register = ext_divu_register(d1, 10);
        // The digit is the remainder, in the high word; the quotient carries on.
        text[last - i] = ((register >> 16) as u8).wrapping_add(DIGIT_ZERO);
        d1 = register as u16 as i16;
    }
}

/// Where `'0'` is in the fonts: just after `'Z'`.
pub const DIGIT_ZERO: u8 = b'A' + 26;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_text_moves_two_columns_and_follows_cursor_codes() {
        let glyphs = big_glyphs(b"\x07\x0bAB C\x0b\x1bD");
        let at: Vec<_> = glyphs.iter().map(|g| (g.index, g.x, g.y)).collect();
        assert_eq!(at, [(0, 56, 44), (1, 72, 44), (2, 104, 44), (3, 88, 108)]);
    }

    #[test]
    fn small_text_wraps_at_the_screen_edge() {
        // Cursor codes only reach column 31, so write past the edge.
        let glyphs = small_glyphs(b"\x1f\x01AAAAAAAAAB");
        assert_eq!((glyphs[8].x, glyphs[8].y), (39 * 8, 4));
        assert_eq!((glyphs[9].x, glyphs[9].y), (0, 8));
    }

    #[test]
    fn digits_follow_the_letters() {
        let mut text = *b"SCORE 000";
        write_digits(&mut text, 8, 207, 3);
        assert_eq!(&text[6..], &[DIGIT_ZERO + 2, DIGIT_ZERO, DIGIT_ZERO + 7]);
    }

    #[test]
    fn negative_numbers_overflow_the_division() {
        let mut text = [0; 3];
        write_digits(&mut text, 2, -2, 3);
        assert_eq!(&text, b"ZZZ");
    }
}
