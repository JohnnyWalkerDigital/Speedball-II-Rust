//! The driver's sequences as text, and back.
//!
//! A sequence is bytecode (see [`super::driver`]): bytes below `0x80` are
//! notes, the rest are commands, some with a one-byte operand. In text,
//! each sequence starts `sequence N`; then come notes as plain numbers and
//! commands as words, separated by spaces or line breaks. `#` starts a
//! comment. Bytes that are no command (or a command cut short by the next
//! sequence) are written `byte 0xNN`, so every sequence reads back exactly.

use std::fmt::Write as _;

/// One sequence: its number (what sounds, calls and jumps name it by) and
/// its bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sequence {
    pub number: u8,
    pub bytes: Vec<u8>,
}

/// What a command's operand is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operand {
    None,
    Unsigned,
    /// A transposition, in quarter semitones.
    Signed,
    Sequence,
    Instrument,
    Effect,
}

/// Commands by byte, as the driver dispatches them.
const COMMANDS: &[(u8, &str, Operand)] = &[
    (0x80, "volume", Operand::Unsigned),
    (0x84, "unused_84", Operand::Unsigned),
    (0x88, "restart", Operand::None),
    (0x8C, "length", Operand::Unsigned),
    (0x90, "rest", Operand::None),
    (0x94, "tempo", Operand::Unsigned),
    (0x9C, "effect", Operand::Effect),
    (0xA0, "unused_a0", Operand::Unsigned),
    (0xA4, "unused_a4", Operand::Unsigned),
    (0xA8, "loop_effects", Operand::Unsigned),
    (0xAC, "stop", Operand::None),
    (0xB0, "call", Operand::Sequence),
    (0xB4, "return", Operand::None),
    (0xB8, "transpose", Operand::Signed),
    (0xBC, "set_transpose", Operand::Signed),
    (0xC0, "repeat", Operand::Unsigned),
    (0xC4, "next", Operand::None),
    (0xC8, "envelope", Operand::Unsigned),
    (0xCC, "no_envelope", Operand::None),
    (0xD0, "instrument", Operand::Instrument),
    (0xD4, "jump", Operand::Sequence),
];
/// Silence, whose operands are a count and that many bytes. No sequence
/// uses it, so it is written as bytes.
const SILENCE: u8 = 0x98;
const RAW: &str = "byte";
/// Commands after which a line ends.
const LINE_ENDS: &[&str] = &["restart", "stop", "return", "next", "jump"];
const WIDTH: usize = 80;

fn command(byte: u8) -> Option<(&'static str, Operand)> {
    COMMANDS
        .iter()
        .find(|(b, ..)| *b == byte)
        .map(|&(_, name, operand)| (name, operand))
}

/// What a sequence refers to by number.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct References {
    pub sequences: Vec<u8>,
    pub instruments: Vec<u8>,
    pub effects: Vec<u8>,
}

impl Sequence {
    /// The commands, one item each: notes, commands with their operands,
    /// and lone bytes.
    fn items(&self) -> Vec<Item> {
        let bytes = &self.bytes;
        let mut items = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            let item = match command(byte) {
                _ if byte < 0x80 => Item::Note(byte),
                Some((name, Operand::None)) => Item::Command(name, None),
                Some((name, operand)) if i + 1 < bytes.len() => {
                    i += 1;
                    Item::Command(name, Some((operand, bytes[i])))
                }
                _ if byte == SILENCE => {
                    // The count and its bytes follow, as far as they go.
                    let count = bytes.get(i + 1).map_or(0, |&n| match n {
                        0 => 0x1_0000,
                        n => usize::from(n),
                    });
                    let end = (i + 2 + count).min(bytes.len());
                    items.extend(bytes[i..end - 1].iter().map(|&b| Item::Byte(b)));
                    i = end - 1;
                    Item::Byte(bytes[i])
                }
                _ => Item::Byte(byte),
            };
            items.push(item);
            i += 1;
        }
        items
    }

    pub fn references(&self) -> References {
        let mut references = References::default();
        for item in self.items() {
            if let Item::Command(_, Some((operand, value))) = item {
                match operand {
                    Operand::Sequence => references.sequences.push(value),
                    Operand::Instrument => references.instruments.push(value),
                    Operand::Effect => references.effects.push(value),
                    _ => {}
                }
            }
        }
        references
    }
}

enum Item {
    Note(u8),
    Command(&'static str, Option<(Operand, u8)>),
    Byte(u8),
}

impl Item {
    fn text(&self) -> String {
        match *self {
            Item::Note(note) => note.to_string(),
            Item::Command(name, None) => name.to_owned(),
            Item::Command(name, Some((Operand::Signed, value))) => {
                format!("{name} {}", value as i8)
            }
            Item::Command(name, Some((_, value))) => format!("{name} {value}"),
            Item::Byte(byte) => format!("{RAW} 0x{byte:02X}"),
        }
    }

    fn ends_line(&self) -> bool {
        matches!(self, Item::Command(name, _) if LINE_ENDS.contains(name))
    }
}

/// Writes sequences in the order given.
pub fn to_text(sequences: &[Sequence]) -> String {
    let mut out = String::new();
    for sequence in sequences {
        let _ = writeln!(out, "sequence {}", sequence.number);
        let mut line = String::new();
        for item in sequence.items() {
            let text = item.text();
            if !line.is_empty() && 2 + line.len() + 1 + text.len() > WIDTH {
                let _ = writeln!(out, "  {line}");
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&text);
            if item.ends_line() {
                let _ = writeln!(out, "  {line}");
                line.clear();
            }
        }
        if !line.is_empty() {
            let _ = writeln!(out, "  {line}");
        }
        out.push('\n');
    }
    out
}

/// Reads sequences back, in the order written. Errors name the line.
pub fn parse(text: &str) -> Result<Vec<Sequence>, String> {
    let mut sequences: Vec<Sequence> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("");
        let at = |message: String| format!("line {}: {message}", number + 1);
        let mut words = line.split_whitespace();
        while let Some(word) = words.next() {
            let mut operand = |what: &str| {
                words
                    .next()
                    .ok_or_else(|| at(format!("{what} needs a number")))
            };
            if word == "sequence" {
                let value = operand(word)?;
                let n = parse_number(value, 1, 255).map_err(at)?;
                if sequences.iter().any(|s| i64::from(s.number) == n) {
                    return Err(at(format!("sequence {n} comes twice")));
                }
                sequences.push(Sequence {
                    number: n as u8,
                    bytes: Vec::new(),
                });
                continue;
            }
            let bytes = &mut sequences
                .last_mut()
                .ok_or_else(|| at(format!("\"{word}\" before the first \"sequence\"")))?
                .bytes;
            if word == RAW {
                let value = operand(word)?;
                bytes.push(parse_number(value, 0, 255).map_err(at)? as u8);
            } else if let Some(&(byte, name, kind)) = COMMANDS.iter().find(|c| c.1 == word) {
                bytes.push(byte);
                if kind != Operand::None {
                    let value = operand(name)?;
                    let (min, max) = match kind {
                        Operand::Signed => (-128, 127),
                        _ => (0, 255),
                    };
                    bytes.push(parse_number(value, min, max).map_err(at)? as u8);
                }
            } else if word.starts_with(|c: char| c.is_ascii_digit()) {
                bytes.push(parse_number(word, 0, 0x7F).map_err(at)? as u8);
            } else {
                return Err(at(format!("\"{word}\" is no note or command")));
            }
        }
    }
    Ok(sequences)
}

/// A decimal or `0x` hexadecimal number in `min..=max`.
fn parse_number(word: &str, min: i64, max: i64) -> Result<i64, String> {
    let value = match word.strip_prefix("0x").or_else(|| word.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16),
        None => word.parse(),
    }
    .map_err(|_| format!("\"{word}\" is not a number"))?;
    if !(min..=max).contains(&value) {
        return Err(format!("{value} is not in {min} to {max}"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_sequence_reads_back() {
        let sequences = vec![
            Sequence {
                number: 3,
                bytes: vec![0x94, 0x7D, 0x8C, 4, 0xD0, 9, 24, 24, 0x90, 0xB8, 0xFC, 0xAC],
            },
            // Silence and its operands, bytes that are no command, and a
            // command cut short by the end.
            Sequence {
                number: 200,
                bytes: vec![0x98, 2, 5, 6, 7, 0xFF, 0xE0, 0xD4],
            },
        ];
        let text = to_text(&sequences);
        assert!(
            text.contains("tempo 125 length 4 instrument 9 24 24 rest transpose -4 stop"),
            "{text}"
        );
        assert!(text.contains("byte 0xFF byte 0xE0 byte 0xD4"), "{text}");
        assert_eq!(parse(&text).unwrap(), sequences);
    }

    #[test]
    fn mistakes_name_their_line() {
        assert_eq!(
            parse("sequence 1\n  tempo\n").unwrap_err(),
            "line 2: tempo needs a number"
        );
        assert!(parse("sequence 1\n  128\n")
            .unwrap_err()
            .starts_with("line 2:"));
        assert!(parse("sequence 1\nsequence 1\n").is_err());
        assert!(parse("stop\n").is_err());
        assert_eq!(
            parse("sequence 1 # a comment\n  60 stop # more\n").unwrap()[0].bytes,
            [60, 0xAC]
        );
    }

    #[test]
    fn references_are_found() {
        let sequence = Sequence {
            number: 1,
            bytes: vec![0xD0, 3, 0x9C, 2, 0xB0, 7, 0xD4, 8],
        };
        assert_eq!(
            sequence.references(),
            References {
                sequences: vec![7, 8],
                instruments: vec![3],
                effects: vec![2],
            }
        );
    }
}
