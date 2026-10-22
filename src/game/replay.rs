//! The goal replay: a recording of what was drawn, played back in slow
//! motion after a goal (`save_replay_origin`, `save_sprite_for_replay`,
//! `run_replay`, `replay_frame`).
//!
//! The original records into a 3,060-byte ring buffer of 32-bit entries: a
//! marker with the camera position at the start of each frame, then one entry
//! per sprite drawn. A frame with many sprites uses more of the buffer, so the
//! replay's length depends on how busy the screen was. This port stores the
//! drawn entities themselves, but counts entries exactly like the original so
//! the replay covers the same frames.

use super::entity::Entity;
use super::geometry::{PerTeam, Point};
use super::scene;
use super::state::Match;

/// `replay_buf` to `replay_buf_end`, in 32-bit entries.
const CAPACITY: usize = (0x528C - 0x4698) / 4;

#[derive(Clone)]
enum Entry {
    /// Start of a frame, with the camera position.
    Frame(Point),
    Sprite(Box<Entity>),
}

pub struct Replay {
    slots: Vec<Option<Entry>>,
    /// `replay_buf_tail_ptr`: oldest entry.
    tail: usize,
    /// `replay_buf_head_ptr`: next entry to write.
    head: usize,
    playback: Option<Playback>,
}

struct Playback {
    /// Read position; the buffer itself is left intact.
    cursor: usize,
    /// Ticks left showing the current frame (replays run at half speed).
    hold: u8,
}

impl Default for Replay {
    fn default() -> Self {
        Self {
            slots: vec![None; CAPACITY],
            tail: 0,
            head: 0,
            playback: None,
        }
    }
}

impl Replay {
    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }

    /// `write_replay_buf`: when the buffer is full, the oldest entry goes.
    fn write(&mut self, entry: Entry) {
        self.slots[self.head] = Some(entry);
        self.head = (self.head + 1) % CAPACITY;
        if self.head == self.tail {
            self.tail = (self.tail + 1) % CAPACITY;
        }
    }

    fn entry(&self, at: usize) -> &Entry {
        self.slots[at].as_ref().expect("entries between tail and head are written")
    }
}

impl Match {
    /// Records one frame while the clock runs (`save_replay_origin` and
    /// `save_sprite_for_replay`): only sprites `draw_sprite` would draw.
    pub(crate) fn record_replay_frame(&mut self) {
        if self.clock_paused {
            return;
        }
        let scroll = self.scroll;
        self.replay.write(Entry::Frame(scroll));
        for sprite in &self.scene {
            if scene::on_screen(sprite, scroll) {
                self.replay.write(Entry::Sprite(Box::new(sprite.clone())));
            }
        }
    }

    /// Starts `run_replay`. Returns `false` if there is nothing to show.
    pub(crate) fn start_replay(&mut self) -> bool {
        self.clock_paused = true;
        // Fire skips the replay, so stale presses are cleared first.
        self.inputs = PerTeam::default();
        if self.replay.tail == self.replay.head {
            self.clock_paused = false;
            return false;
        }
        self.replay.playback = Some(Playback {
            cursor: self.replay.tail,
            hold: 0,
        });
        true
    }

    /// Shows the next replay frame. Returns `false` once the replay is over.
    pub(crate) fn step_replay(&mut self) -> bool {
        let Some(playback) = self.replay.playback.as_mut() else {
            return false;
        };
        if playback.hold > 0 {
            playback.hold -= 1;
            return true;
        }
        let mut cursor = playback.cursor;
        loop {
            let skipped = self.inputs.team1.fire() || self.inputs.team2.fire();
            if skipped || cursor == self.replay.head {
                self.replay.playback = None;
                self.clock_paused = false;
                return false;
            }
            let entry = self.replay.entry(cursor).clone();
            cursor = (cursor + 1) % CAPACITY;
            // A frame whose marker was overwritten is skipped.
            let Entry::Frame(scroll) = entry else { continue };
            self.scroll = scroll;
            self.scene.clear();
            while cursor != self.replay.head {
                let Entry::Sprite(sprite) = self.replay.entry(cursor) else { break };
                self.scene.push((**sprite).clone());
                cursor = (cursor + 1) % CAPACITY;
            }
            break;
        }
        self.build_panel_overlay();
        // `run_replay` doubles `frames_per_tick`: each frame shows twice as long.
        self.replay.playback = Some(Playback { cursor, hold: 1 });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_buffer_drops_the_oldest_entry() {
        let mut replay = Replay::default();
        for i in 0..CAPACITY as i32 {
            replay.write(Entry::Frame(Point::new(i, 0)));
        }
        // One slot always stays free: 764 entries fit.
        assert_eq!(replay.head, 0);
        assert_eq!(replay.tail, 1);
        let Entry::Frame(oldest) = replay.entry(replay.tail) else { panic!() };
        assert_eq!(oldest.x, 1);
    }
}
