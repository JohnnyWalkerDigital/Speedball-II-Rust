//! Messages shown on the big panel at the top of the pitch.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::data::readable;
use crate::data::Rom;

/// A message in the game's own encoding: a run of letters where bytes below
/// `' '` move the cursor (`x`, then `y` as the next byte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    bytes: Vec<u8>,
}

/// Written in the [readable] form.
impl Serialize for Text {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        readable::menu::serialize(&self.bytes, s)
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        readable::menu::deserialize(d).map(Text::new)
    }
}

impl Text {
    /// A message from elsewhere (the menus' strings): the cursor, then the
    /// text.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn read(rom: &Rom, address: i32) -> Self {
        // The first two bytes are the initial cursor; the string ends at NUL.
        let mut length = 2;
        while rom.u8(address + length) != 0 {
            length += 1;
        }
        Self {
            bytes: (0..length).map(|i| rom.u8(address + i) as u8).collect(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// A copy with the initial cursor row replaced.
    pub fn with_row(&self, row: u8) -> Self {
        let mut copy = self.clone();
        if let Some(y) = copy.bytes.get_mut(1) {
            *y = row;
        }
        copy
    }

    /// Overwrites the characters starting at `pos`, staying in bounds.
    pub fn overwrite(&mut self, pos: usize, chars: &[u8]) {
        for (dst, &c) in self.bytes.iter_mut().skip(pos).zip(chars) {
            *dst = c;
        }
    }

    /// The message as plain text, without cursor codes.
    pub fn to_display_string(&self) -> String {
        let mut out = String::new();
        let mut bytes = self.bytes.iter().copied().skip(2);
        while let Some(c) = bytes.next() {
            match c {
                c if c < b' ' => {
                    bytes.next();
                    out.push(' ');
                }
                c if (b'A' + 26..b'A' + 36).contains(&c) => {
                    out.push(char::from(b'0' + c - b'A' - 26))
                }
                c => out.push(char::from(c)),
            }
        }
        out.trim().to_string()
    }

    /// Writes `value` as zero-padded digits. The font stores digits right
    /// after the 26 letters.
    pub fn write_number(&mut self, pos: usize, value: i32, width: usize) {
        let digits: Vec<u8> = format!("{value:0width$}")
            .bytes()
            .map(|c| {
                if c.is_ascii_digit() {
                    b'A' + 26 + (c - b'0')
                } else {
                    c
                }
            })
            .collect();
        self.overwrite(pos, &digits);
    }
}

/// The positions shown under "INJURED", by formation slot (0 to 8).
pub const POSITION_NAMES: [&[u8; 10]; 9] = [
    b"  GOALIE  ",
    b" LEFT DEF ",
    b"RIGHT DEF ",
    b" LEFT MID ",
    b"RIGHT MID ",
    b"CENTRE MID",
    b"LEFT WING ",
    b"RIGHT WING",
    b"CENTRE FWD",
];

/// The fixed messages. Each is numbered by the address it had in the
/// original data segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextId {
    Replay = 0xEB8,
    HalfTime = 0xEC3,
    MatchOver = 0xEDC,
    GameOver = 0xEFE,
    Team1Scored = 0xF0B,
    Team2Scored = 0xF16,
    Injured = 0xF21,
    OwnGoal = 0xF38,
    Goal = 0xF44,
    Vs = 0xF4F,
    Player1Full = 0x61B6,
    Player1Short = 0x62B4,
}

impl TextId {
    /// The message as the original held it: the cursor (column, row) and
    /// the text, where a column or row byte below `' '` moves the cursor
    /// and `0` is a blank the match writes a digit into.
    fn bytes(self) -> &'static [u8] {
        match self {
            TextId::Replay => b"\x03\x03  REPLAY",
            TextId::HalfTime => b"\x03\x03HALF  TIME\x03\x0F000 TO 000",
            TextId::MatchOver => b"\x03\x05MATCH OVER\x03\x09  SCORE\x03\x0D000 TO 000",
            TextId::GameOver => b"\x03\x09GAME  OVER",
            TextId::Team1Scored | TextId::Team2Scored => b"\x03\x0B  SCORED",
            TextId::Injured => b"\x03\x07          \x03\x0B INJURED",
            TextId::OwnGoal => b"\x03\x09 OWN GOAL",
            TextId::Goal => b"\x03\x09  GOAL 0",
            TextId::Vs => b"\x03\x09    VS",
            // `\\` is the font's "1".
            TextId::Player1Full => b"\x00\x00PLAYER \\",
            TextId::Player1Short => b"\x03\x00 PLAYER \\",
        }
    }

    const ALL: [TextId; 12] = [
        TextId::Replay,
        TextId::HalfTime,
        TextId::MatchOver,
        TextId::GameOver,
        TextId::Team1Scored,
        TextId::Team2Scored,
        TextId::Injured,
        TextId::OwnGoal,
        TextId::Goal,
        TextId::Vs,
        TextId::Player1Full,
        TextId::Player1Short,
    ];
}

/// What the top panel shows while the clock is stopped.
#[derive(Clone, Debug)]
pub struct TextPanel {
    /// The message shown; `None` shows the plain "goal" panel.
    current: Option<TextId>,
    /// Frames (and lines) drawn since the message last changed.
    counter: u8,
    texts: HashMap<TextId, Text>,
}

impl TextPanel {
    pub fn new() -> Self {
        Self {
            current: Some(TextId::Vs),
            counter: 0,
            texts: TextId::ALL
                .into_iter()
                .map(|id| (id, Text::new(id.bytes().to_vec())))
                .collect(),
        }
    }

    /// The message showing, if any.
    pub fn message(&self) -> Option<TextId> {
        self.current
    }

    /// Changes the message, restarting the counter as the original does.
    pub fn show(&mut self, message: Option<TextId>) {
        self.current = message;
        self.counter = 0;
    }

    /// Counts a frame of the panel being built: the message's `lines` are
    /// drawn when the counter is 0, each counting once, then the frame
    /// counts. Returns whether the lines were drawn
    /// this frame, and the counter.
    pub fn count_frame(&mut self, lines: u8) -> (bool, u8) {
        let drawn = self.counter == 0;
        if drawn {
            self.counter = lines;
        }
        self.counter = self.counter.wrapping_add(1);
        (drawn, self.counter)
    }

    pub fn text(&self, id: TextId) -> &Text {
        &self.texts[&id]
    }

    /// Some messages have blanks the match fills in (scores, player names).
    pub fn text_mut(&mut self, id: TextId) -> &mut Text {
        self.texts.get_mut(&id).expect("all texts are loaded")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_use_the_font_digit_range() {
        let mut text = Text {
            bytes: vec![1, 2, b'A', b'B', b'C', b'D'],
        };
        text.write_number(2, 7, 3);
        assert_eq!(text.bytes(), &[1, 2, b'A' + 26, b'A' + 26, b'A' + 33, b'D']);
        assert_eq!(text.with_row(9).bytes()[1], 9);
    }

    #[test]
    fn messages_are_the_originals() {
        let config = crate::config::Config::default();
        let Ok(amiga) = crate::data::files::read_amiga_files(&config.data_dir) else {
            eprintln!("skipping: no originals");
            return;
        };
        let rom = Rom::new(&amiga.intro_memory);
        for id in TextId::ALL {
            assert_eq!(id.bytes(), Text::read(&rom, id as i32).bytes(), "{id:?}");
        }
        for (slot, name) in POSITION_NAMES.iter().enumerate() {
            // Where the original held them.
            let at = 0xFA2 + slot as i32 * 10;
            let original: Vec<u8> = (0..10).map(|i| rom.u8(at + i) as u8).collect();
            assert_eq!(name.as_slice(), original, "position {slot}");
        }
    }
}
