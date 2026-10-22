//! The goal replay: a recording of what was drawn, played back in slow
//! motion after a goal (`save_replay_origin`, `save_sprite_for_replay`,
//! `run_replay`, `replay_frame`), and saved to disk (`save_replay`,
//! `load_replay_goal`).
//!
//! The original records into a 3,060-byte ring buffer of 32-bit entries: a
//! marker with the camera position at the start of each frame, then one entry
//! per sprite drawn. A frame with many sprites uses more of the buffer, so the
//! replay's length depends on how busy the screen was. Sprites are stored
//! by their Amiga bank and index, packed as the original packed them, so
//! saved replays are the original's byte for byte.

use super::entity::Entity;
use super::geometry::{PerTeam, Point};
use super::scene;
use super::sprites::{amiga_sprite, entity_for, AmigaSprite, Bank};
use super::state::Match;

/// `replay_buf` to `replay_buf_end`, in 32-bit entries.
const CAPACITY: usize = (0x528C - 0x4698) / 4;
/// Set on a frame's first entry, which holds `x_origin` and `y_origin`.
const FRAME_MARKER: u32 = 1 << 31;
/// In a sprite's x word: a 32x32 sprite (else 16x16).
const WIDE: u32 = 1 << 9;
/// In a 32x32 sprite's x word: drawn in team 1's colours.
const TEAM1: u32 = 1 << 10;
/// In a 32x32 sprite's index byte: from the launcher's bank.
const LAUNCHER_BANK: u32 = 0x80;

pub struct Replay {
    entries: Vec<u32>,
    /// `replay_buf_tail_ptr`: oldest entry.
    tail: usize,
    /// `replay_buf_head_ptr`: next entry to write.
    head: usize,
    playback: Option<Playback>,
}

struct Playback {
    /// Read position; the buffer itself is left intact.
    cursor: usize,
    /// Ticks left showing the current frame.
    hold: u8,
    /// Ended by a key: the next tick finishes it.
    ended: bool,
}

/// One frame of a replay: the camera and the sprites, in drawing order.
pub struct ReplayFrame {
    pub scroll: Point,
    pub sprites: Vec<Entity>,
}

/// What a replay shows on a tick.
pub enum ReplayTick {
    /// The same frame again: replays run at half speed (`run_replay`
    /// doubles `frames_per_tick`).
    Same,
    Frame(ReplayFrame),
    Over,
}

impl Default for Replay {
    fn default() -> Self {
        Self {
            entries: vec![0; CAPACITY],
            tail: 0,
            head: 0,
            playback: None,
        }
    }
}

impl Replay {
    /// A replay loaded from a goal save disk.
    pub fn from_saved(saved: &SavedReplay) -> Self {
        Self {
            entries: saved.entries.clone(),
            tail: saved.tail,
            head: saved.head,
            playback: None,
        }
    }

    /// The buffer as `save_replay` writes it: the whole replay, however
    /// far it has played.
    pub fn saved(&self, serial: u32) -> SavedReplay {
        SavedReplay {
            entries: self.entries.clone(),
            serial,
            tail: self.tail,
            head: self.head,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }

    /// Starts playing from the oldest entry. Returns `false` if there is
    /// nothing to show.
    pub fn start(&mut self) -> bool {
        if self.tail == self.head {
            return false;
        }
        self.playback = Some(Playback {
            cursor: self.tail,
            hold: 0,
            ended: false,
        });
        true
    }

    /// Ends the replay on the next tick (Escape, or F to speed up).
    pub fn end(&mut self) {
        if let Some(playback) = &mut self.playback {
            playback.ended = true;
        }
    }

    /// Advances playback by a tick (`run_replay`'s loop and
    /// `replay_frame`).
    pub fn tick(&mut self) -> ReplayTick {
        let Some(playback) = self.playback.as_mut() else {
            return ReplayTick::Over;
        };
        if playback.ended {
            self.playback = None;
            return ReplayTick::Over;
        }
        if playback.hold > 0 {
            playback.hold -= 1;
            return ReplayTick::Same;
        }
        let mut cursor = playback.cursor;
        // A frame whose marker was overwritten is skipped.
        let origin = loop {
            if cursor == self.head {
                self.playback = None;
                return ReplayTick::Over;
            }
            let entry = self.entries[cursor];
            cursor = (cursor + 1) % CAPACITY;
            if entry & FRAME_MARKER != 0 {
                break entry;
            }
        };
        let scroll = Point::new((origin >> 16 & 0x7FFF) as i32, (origin & 0xFFFF) as i32);
        let mut sprites = Vec::new();
        while cursor != self.head && self.entries[cursor] & FRAME_MARKER == 0 {
            sprites.push(decode_sprite(self.entries[cursor], scroll));
            cursor = (cursor + 1) % CAPACITY;
        }
        self.playback = Some(Playback {
            cursor,
            hold: 1,
            ended: false,
        });
        ReplayTick::Frame(ReplayFrame { scroll, sprites })
    }

    /// `write_replay_buf`: when the buffer is full, the oldest entry goes.
    fn write(&mut self, entry: u32) {
        self.entries[self.head] = entry;
        self.head = (self.head + 1) % CAPACITY;
        if self.head == self.tail {
            self.tail = (self.tail + 1) % CAPACITY;
        }
    }

    /// `save_replay_origin`.
    fn record_frame(&mut self, scroll: Point) {
        self.write(FRAME_MARKER | (scroll.x as u32 & 0xFFFF) << 16 | scroll.y as u32 & 0xFFFF);
    }

    /// `save_sprite_for_replay`, for a sprite `draw_sprite` draws.
    fn record_sprite(&mut self, sprite: &Entity, scroll: Point) {
        if let Some(entry) = encode_sprite(sprite, scroll) {
            self.write(entry);
        }
    }
}

/// `save_sprite_for_replay`: the sprite's screen position plus its size
/// (so that it is positive), its index and which bank and colours.
fn encode_sprite(e: &Entity, scroll: Point) -> Option<u32> {
    let AmigaSprite { bank, index } = amiga_sprite(e)?;
    let size = e.width;
    let x = e.pos.x + e.origin.x - size / 2 - (scroll.x & !0xF) + size;
    // The width again, not the height: the sprites are square.
    let y = e.pos.y + e.origin.y - e.height / 2 - scroll.y + size;
    let mut index = index as u32 & 0xFF;
    let mut x_word = x as u32 & 0xFFFF;
    if size != 16 {
        if bank == Bank::Launcher {
            index |= LAUNCHER_BANK;
        }
        x_word |= WIDE;
        if e.flags.bit3() {
            x_word |= TEAM1;
        }
    }
    Some(x_word << 16 | index << 8 | y as u32 & 0xFF)
}

/// `replay_frame`'s decoding. A 32x32 sprite from the players' bank has
/// no record of whether it was a player: the original drew them all with
/// the first player of one team or the other, so anything else from that
/// bank (the big ball) comes back as a player frame.
fn decode_sprite(entry: u32, scroll: Point) -> Entity {
    let x_word = entry >> 16;
    let size = if x_word & WIDE != 0 { 32 } else { 16 };
    let x = (x_word & 0x1FF) as i32 - size + (scroll.x & !0xF);
    let y = scroll.y + (entry & 0xFF) as i32 - size;
    let mut index = (entry >> 8 & 0xFF) as i32;
    let bank = if size == 16 {
        Bank::Entities
    } else if index as u32 & LAUNCHER_BANK != 0 {
        index &= !(LAUNCHER_BANK as i32);
        Bank::Launcher
    } else {
        Bank::Players
    };
    entity_for(
        AmigaSprite { bank, index },
        x_word & TEAM1 != 0,
        Point::new(x, y),
        size,
    )
}

/// A replay as written to a goal save disk: the whole buffer, then
/// `maybe_serial`, the tail and head as byte offsets, and "goal".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedReplay {
    entries: Vec<u32>,
    serial: u32,
    tail: usize,
    head: usize,
}

impl SavedReplay {
    pub const SIZE: usize = 0xC00;
    const MAGIC: &'static [u8; 4] = b"goal";
    const SERIAL_AT: usize = CAPACITY * 4;

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = self.entries.iter().flat_map(|e| e.to_be_bytes()).collect();
        bytes.extend(self.serial.to_be_bytes());
        bytes.extend((self.tail as u16 * 4).to_be_bytes());
        bytes.extend((self.head as u16 * 4).to_be_bytes());
        bytes.extend(Self::MAGIC);
        bytes
    }

    /// `None` unless the bytes end with "goal" (`load_replay_goal` skips
    /// the slot). Offsets that are not a whole entry inside the buffer
    /// never come from the original; they are refused too.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE || &bytes[Self::SIZE - 4..Self::SIZE] != Self::MAGIC {
            return None;
        }
        let word = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]) as usize;
        let entry =
            |offset: usize| (offset % 4 == 0 && offset / 4 < CAPACITY).then_some(offset / 4);
        let at = Self::SERIAL_AT;
        Some(Self {
            entries: bytes[..at]
                .chunks_exact(4)
                .map(|e| u32::from_be_bytes(e.try_into().expect("four bytes")))
                .collect(),
            serial: u32::from_be_bytes(bytes[at..at + 4].try_into().expect("four bytes")),
            tail: entry(word(at + 4))?,
            head: entry(word(at + 6))?,
        })
    }
}

impl Match {
    /// Records one frame while the clock runs (`save_replay_origin` and
    /// `save_sprite_for_replay`): only sprites `draw_sprite` would draw.
    /// Nothing is drawn, so nothing recorded, while the match is sped up.
    pub(crate) fn record_replay_frame(&mut self) {
        if self.clock_paused || self.accelerated {
            return;
        }
        let scroll = self.scroll;
        self.replay.record_frame(scroll);
        for sprite in &self.scene {
            if scene::on_screen(sprite, scroll) {
                self.replay.record_sprite(sprite, scroll);
            }
        }
    }

    /// Starts `run_replay`. Returns `false` if there is nothing to show, or
    /// the match is sped up.
    pub(crate) fn start_replay(&mut self) -> bool {
        if self.accelerated {
            return false;
        }
        self.clock_paused = true;
        // Fire skips the replay, so stale presses are cleared first.
        self.inputs = PerTeam::default();
        if !self.replay.start() {
            self.clock_paused = false;
            return false;
        }
        true
    }

    /// Shows the next replay frame. Returns `false` once the replay is over.
    pub(crate) fn step_replay(&mut self) -> bool {
        if self.inputs.team1.fire() || self.inputs.team2.fire() {
            self.replay.end();
        }
        match self.replay.tick() {
            ReplayTick::Same => true,
            ReplayTick::Frame(frame) => {
                self.scroll = frame.scroll;
                self.scene = frame.sprites;
                self.build_panel_overlay();
                true
            }
            ReplayTick::Over => {
                self.clock_paused = false;
                false
            }
        }
    }

    pub fn is_replaying(&self) -> bool {
        self.replay.is_playing()
    }

    /// Escape during a replay ends it, not the match.
    pub fn end_replay(&mut self) {
        self.replay.end();
    }

    /// The replay as `save_replay` writes it.
    pub fn saved_replay(&self, serial: u32) -> SavedReplay {
        self.replay.saved(serial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::addr;
    use crate::game::sprites::entity_for;

    fn sprite(bank: Bank, index: i32, team1: bool, at: Point, size: i32) -> Entity {
        entity_for(AmigaSprite { bank, index }, team1, at, size)
    }

    #[test]
    fn a_full_buffer_drops_the_oldest_entry() {
        let mut replay = Replay::default();
        for i in 0..CAPACITY as i32 {
            replay.record_frame(Point::new(i, 0));
        }
        // One slot always stays free: 764 entries fit.
        assert_eq!(replay.head, 0);
        assert_eq!(replay.tail, 1);
        assert_eq!(replay.entries[replay.tail], FRAME_MARKER | 1 << 16);
    }

    #[test]
    fn sprites_are_packed_as_the_original_packed_them() {
        let scroll = Point::new(0xA5, 484);
        // A team 1 player whose top left is 10 pixels right of and 20 below
        // the screen's (scrolled to a 16-pixel boundary).
        let player = sprite(Bank::Players, 5, true, Point::new(0xA0 + 10, 504), 32);
        assert_eq!(
            encode_sprite(&player, scroll),
            Some((42 | WIDE | TEAM1) << 16 | 5 << 8 | 52)
        );
        let launcher = sprite(Bank::Launcher, 9, false, Point::new(0xA0, 484), 32);
        assert_eq!(
            encode_sprite(&launcher, scroll),
            Some((32 | WIDE) << 16 | (9 | LAUNCHER_BANK) << 8 | 32)
        );
        let coin = sprite(Bank::Entities, 70, false, Point::new(0xA0 - 15, 470), 16);
        assert_eq!(encode_sprite(&coin, scroll), Some(1 << 16 | 70 << 8 | 2));
    }

    #[test]
    fn replayed_sprites_come_back_where_they_were() {
        let scroll = Point::new(0x123, 300);
        for (bank, index, team1, size) in [
            (Bank::Players, 17, true, 32),
            (Bank::Players, 17, false, 32),
            (Bank::Launcher, 2, false, 32),
            (Bank::Launcher, 9, false, 32),
            (Bank::Entities, 41, false, 16),
        ] {
            let e = sprite(bank, index, team1, Point::new(0x120 + 50, 330), size);
            let back = decode_sprite(encode_sprite(&e, scroll).unwrap(), scroll);
            assert_eq!(amiga_sprite(&back), amiga_sprite(&e));
            assert_eq!((back.pos, back.flags.bit3()), (e.pos, team1));
        }
    }

    #[test]
    fn the_big_ball_replays_as_a_player_frame_as_in_the_original() {
        let scroll = Point::new(0, 0);
        let big_ball = sprite(Bank::BigBall, 3, false, Point::new(100, 100), 32);
        let back = decode_sprite(encode_sprite(&big_ball, scroll).unwrap(), scroll);
        assert_eq!(
            amiga_sprite(&back),
            Some(AmigaSprite {
                bank: Bank::Players,
                index: 3
            })
        );
        assert_eq!(back.address, addr::PLAYERS_TEAM2);
    }

    #[test]
    fn playback_shows_each_frame_twice_then_ends() {
        let mut replay = Replay::default();
        let coin = sprite(Bank::Entities, 3, false, Point::new(40, 50), 16);
        for x in [16, 32] {
            let scroll = Point::new(x, 0);
            replay.record_frame(scroll);
            replay.record_sprite(&coin, scroll);
        }
        assert!(replay.start());
        let ReplayTick::Frame(first) = replay.tick() else {
            panic!("a frame")
        };
        assert_eq!(first.scroll, Point::new(16, 0));
        assert_eq!(first.sprites.len(), 1);
        assert_eq!(first.sprites[0].pos, coin.pos);
        assert!(matches!(replay.tick(), ReplayTick::Same));
        assert!(matches!(replay.tick(), ReplayTick::Frame(_)));
        assert!(matches!(replay.tick(), ReplayTick::Same));
        assert!(matches!(replay.tick(), ReplayTick::Over));
        assert!(!replay.is_playing());
    }

    #[test]
    fn a_saved_replay_is_the_buffer_then_serial_offsets_and_magic() {
        let mut replay = Replay::default();
        replay.record_frame(Point::new(0xA0, 0x1E4));
        let saved = replay.saved(0x167F);
        let bytes = saved.encode();
        assert_eq!(bytes.len(), SavedReplay::SIZE);
        assert_eq!(&bytes[..4], &[0x80, 0xA0, 0x01, 0xE4]);
        assert_eq!(&bytes[0xBF4..], b"\0\0\x16\x7f\0\0\0\x04goal");
        assert_eq!(SavedReplay::decode(&bytes), Some(saved));
        assert_eq!(SavedReplay::decode(&[0; SavedReplay::SIZE]), None);
    }
}
