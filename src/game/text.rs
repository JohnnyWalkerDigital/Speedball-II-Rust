//! Messages shown on the big panel at the top of the pitch.

use std::collections::HashMap;

use crate::data::AtariRom;

/// A message in the game's own encoding: a run of letters where bytes below
/// `' '` move the cursor (`x`, then `y` as the next byte).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    bytes: Vec<u8>,
}

impl Text {
    /// A message from elsewhere (the menus' strings): the cursor, then the
    /// text.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn read(rom: &AtariRom, address: i32) -> Self {
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

/// The fixed messages, by their address in the original data segment.
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
    /// `overlay_message`; `None` shows the plain "goal" panel.
    current: Option<TextId>,
    /// `text_overlay_counter`: frames (and lines) drawn since the message
    /// last changed. The C# kept it as `TextPanel._unk01`.
    counter: u8,
    texts: HashMap<TextId, Text>,
}

impl TextPanel {
    pub fn new(rom: &AtariRom) -> Self {
        Self {
            current: Some(TextId::Vs),
            counter: 0,
            texts: TextId::ALL
                .into_iter()
                .map(|id| (id, Text::read(rom, id as i32)))
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

    /// Counts a frame of the panel being built (`build_text_overlay`): the
    /// message's `lines` are drawn when the counter is 0, each counting
    /// once, then the frame counts. Returns whether the lines were drawn
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
}
