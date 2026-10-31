//! The game's strings written so people can read and edit them, as asset
//! packs store them. Every string converts both ways exactly.
//!
//! - Capital letters are themselves, and so are spaces and punctuation.
//!   Where the font has the mark it is drawn; otherwise (in the menus) it
//!   is stored as itself and draws nothing, like a space.
//! - `0` to `9` are the fonts' digits, stored just after `Z`.
//! - In the menus, `_` is a blank the game writes a number into.
//! - The panel screens' font has a dot, a dash, lines for drawing tables
//!   (`─│┬┴┼├┤`) and arrows (`◢◣` up, `◥◤` down, `◀▶` level).
//! - `{x,y}` is a cursor code: column `x` (8 pixels each) and row `y`
//!   (4 pixels each), in decimal.
//! - `{#6E}` is any other byte, in hex.

use serde::{Deserialize, Deserializer, Serializer};

use super::text::DIGIT_ZERO;

/// The rules a set of strings follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dialect {
    /// The first two bytes are always the cursor, whatever they hold.
    leading_cursor: bool,
    /// Bytes below this are cursor codes.
    cursor_below: u8,
    /// Characters the font has after the digits.
    symbols: &'static [(char, u8)],
}

/// The menus: strings start with their cursor, and a byte below `' '`
/// moves it. A `'0'` stands where a number will be written: it draws
/// nothing.
pub const MENU: Dialect = Dialect {
    leading_cursor: true,
    cursor_below: b' ',
    symbols: &[('_', b'0')],
};

/// The panel screens, in the orange font, whose digits are followed by
/// punctuation, the table's lines and the league table's arrows (each
/// drawn in two halves).
pub const PANEL: Dialect = Dialect {
    symbols: &[
        ('_', b'0'),
        ('.', 0x65),
        ('-', 0x66),
        ('┤', 0x67),
        ('├', 0x68),
        ('┬', 0x69),
        ('│', 0x6A),
        ('┼', 0x6B),
        ('─', 0x6C),
        ('┴', 0x6D),
        ('◢', 0x6E),
        ('◣', 0x6F),
        ('◥', 0x70),
        ('◤', 0x71),
        ('◀', 0x72),
        ('▶', 0x73),
    ],
    ..MENU
};

/// The intro: any byte up to `'@'` moves the cursor, even a space.
pub const INTRO: Dialect = Dialect {
    leading_cursor: false,
    cursor_below: b'A',
    // The title font: the digits, then these, the cursor and a blank.
    symbols: &[
        ('©', 0x65),
        (',', 0x66),
        ('\'', 0x67),
        (':', 0x68),
        ('.', 0x69),
        (' ', 0x6B),
    ],
};

/// The readable form of `bytes`.
pub fn write(bytes: &[u8], dialect: Dialect) -> String {
    let mut out = String::new();
    let mut rest = bytes;
    if dialect.leading_cursor {
        if let [x, y, tail @ ..] = rest {
            out.push_str(&format!("{{{x},{y}}}"));
            rest = tail;
        }
    }
    while let Some((&byte, tail)) = rest.split_first() {
        match tail {
            [y, after @ ..] if byte < dialect.cursor_below => {
                out.push_str(&format!("{{{byte},{y}}}"));
                rest = after;
                continue;
            }
            _ => {}
        }
        match byte {
            b'A'..=b'Z' => out.push(char::from(byte)),
            b if is_digit(b) => out.push(char::from(b'0' + b - DIGIT_ZERO)),
            b => match dialect.symbols.iter().find(|&&(_, s)| s == b) {
                Some(&(c, _)) => out.push(c),
                // Unless a symbol is written the same way.
                None if b >= dialect.cursor_below
                    && is_punctuation(b)
                    && !dialect.symbols.iter().any(|&(c, _)| c == char::from(b)) =>
                {
                    out.push(char::from(b))
                }
                None => out.push_str(&format!("{{#{b:02X}}}")),
            },
        }
        rest = tail;
    }
    out
}

/// The bytes `text` stands for.
pub fn read(text: &str, dialect: Dialect) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chars = text.char_indices().peekable();
    if dialect.leading_cursor && (!text.starts_with('{') || text.starts_with("{#")) {
        return Err(format!(
            "{text:?} must start with its position, like {{8,12}}"
        ));
    }
    let mut first = true;
    while let Some((at, c)) = chars.next() {
        let leading = first && dialect.leading_cursor;
        first = false;
        match c {
            '{' => {
                let end = text[at..]
                    .find('}')
                    .map(|n| at + n)
                    .ok_or_else(|| format!("{text:?}: unclosed {{ at character {}", at + 1))?;
                bytes.extend(
                    code(&text[at + 1..end], leading, dialect).map_err(|problem| {
                        format!("{text:?}: {{{}}} {problem}", &text[at + 1..end])
                    })?,
                );
                while chars.next_if(|&(i, _)| i <= end).is_some() {}
            }
            'A'..='Z' => bytes.push(c as u8),
            '0'..='9' => bytes.push(DIGIT_ZERO + (c as u8 - b'0')),
            c => match dialect.symbols.iter().find(|&&(s, _)| s == c) {
                Some(&(_, byte)) => bytes.push(byte),
                None if c.is_ascii()
                    && c as u8 >= dialect.cursor_below
                    && is_punctuation(c as u8) =>
                {
                    bytes.push(c as u8)
                }
                None => {
                    return Err(format!(
                        "{text:?}: {c:?} is not a character the game has \
                         (write other bytes like {{#6E}})"
                    ))
                }
            },
        }
    }
    Ok(bytes)
}

/// What is inside `{...}`: `#XX`, or `x,y`.
fn code(inside: &str, leading: bool, dialect: Dialect) -> Result<Vec<u8>, String> {
    if let Some(hex) = inside.strip_prefix('#') {
        let byte = u8::from_str_radix(hex, 16).map_err(|_| "is not a byte in hex")?;
        return Ok(vec![byte]);
    }
    let (x, y) = inside
        .split_once(',')
        .ok_or("should be a position, x,y, or a byte, #XX")?;
    let number = |n: &str| {
        n.trim()
            .parse::<u8>()
            .map_err(|_| "has a number out of range")
    };
    let (x, y) = (number(x)?, number(y)?);
    if !leading && x >= dialect.cursor_below {
        return Err(format!(
            "has a column past {}, the most a cursor code can say",
            dialect.cursor_below - 1
        ));
    }
    Ok(vec![x, y])
}

fn is_digit(byte: u8) -> bool {
    (DIGIT_ZERO..DIGIT_ZERO + 10).contains(&byte)
}

/// Spaces and the punctuation before `'A'`, other than the digits.
fn is_punctuation(byte: u8) -> bool {
    matches!(byte, b' '..=b'/' | b':'..=b'@')
}

/// `#[serde(with = ...)]` helpers for strings and lists of them.
macro_rules! serde_with {
    ($name:ident, $list:ident, $dialect:expr) => {
        pub mod $name {
            use super::*;

            pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&write(bytes, $dialect))
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
                read(&String::deserialize(d)?, $dialect).map_err(serde::de::Error::custom)
            }
        }

        pub mod $list {
            use super::*;

            pub fn serialize<S: Serializer>(list: &[Vec<u8>], s: S) -> Result<S::Ok, S::Error> {
                s.collect_seq(list.iter().map(|bytes| write(bytes, $dialect)))
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<u8>>, D::Error> {
                Vec::<String>::deserialize(d)?
                    .iter()
                    .map(|text| read(text, $dialect).map_err(serde::de::Error::custom))
                    .collect()
            }
        }
    };
}

serde_with!(menu, menu_list, MENU);
serde_with!(panel, panel_list, PANEL);
serde_with!(intro, intro_list, INTRO);

/// A text cell's column and row, as `"x,y"`.
pub mod cell {
    use super::*;

    pub fn serialize<S: Serializer>(&(x, y): &(u8, u8), s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{x},{y}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<(u8, u8), D::Error> {
        let text = String::deserialize(d)?;
        let number = |n: &str| n.trim().parse::<u8>().ok();
        text.split_once(',')
            .and_then(|(x, y)| Some((number(x)?, number(y)?)))
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "{text:?} should be a column and row, like \"5,11\""
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_strings_read_naturally() {
        let bytes = b"\x07\x0bSCORE [\\] TO \x0b\x1bno";
        let text = write(bytes, MENU);
        assert_eq!(text, "{7,11}SCORE 012 TO {11,27}{#6E}{#6F}");
        assert_eq!(write(b"\x01\x02MATCH 0", MENU), "{1,2}MATCH _");
        assert_eq!(read(&text, MENU).unwrap(), bytes);
    }

    #[test]
    fn the_leading_cursor_can_hold_anything() {
        let bytes = b"A\x00B";
        assert_eq!(write(bytes, MENU), "{65,0}B");
        assert_eq!(read("{65,0}B", MENU).unwrap(), bytes);
        assert!(read("B", MENU).is_err(), "the position is required");
    }

    #[test]
    fn intro_punctuation_is_the_fonts_own() {
        // 2095 in the font's digits.
        let bytes = b"\x02\x0fYEAR\x68\x6b][d`\x02\x14A";
        let text = write(bytes, INTRO);
        assert_eq!(text, "{2,15}YEAR: 2095{2,20}A");
        assert_eq!(read(&text, INTRO).unwrap(), bytes);
        assert!(read("{2,1}A!", INTRO).is_err(), "the font has no '!'");
    }

    #[test]
    fn panels_draw_tables() {
        let bytes = b"\x10\x04\x6a\x6c\x6bPL.\x65no";
        let text = write(bytes, PANEL);
        assert_eq!(text, "{16,4}│─┼PL{#2E}.◢◣");
        assert_eq!(read(&text, PANEL).unwrap(), bytes);
    }

    #[test]
    fn every_byte_survives() {
        for dialect in [MENU, PANEL, INTRO] {
            for a in 0..=255u8 {
                for b in [0, 1, 0x20, 0x41, 0x5B, 0xFF] {
                    let bytes = [3, 4, a, b, a];
                    let text = write(&bytes, dialect);
                    assert_eq!(read(&text, dialect).unwrap(), bytes, "{text}");
                }
            }
        }
    }

    #[test]
    fn mistakes_are_explained() {
        let error = read("{1,2}hello", MENU).unwrap_err();
        assert!(error.contains("'h'"), "{error}");
        let error = read("{1,2}A{40,2}", MENU).unwrap_err();
        assert!(error.contains("column"), "{error}");
        assert!(read("{1,2}A{#1G}", MENU).is_err());
        assert!(read("{1,2", MENU).is_err());
    }
}
