//! The message panel at the top of the pitch (`overlay_message`,
//! `build_text_overlay`).

use super::Match;
use crate::game::events::Sound;
use crate::game::text::TextId;

impl Match {
    /// Changes the top panel's message, restarting its counter as the
    /// original does.
    pub(crate) fn show_message(&mut self, message: Option<TextId>) {
        self.text_panel.show(message);
    }

    /// `match_tick` builds the text panel while the clock is stopped.
    pub(super) fn update_panel_overlay(&mut self) {
        if self.text_panel_shown() {
            self.build_panel_overlay();
        }
    }

    /// Whether `match_tick` builds a message on the panel this frame.
    pub fn text_panel_shown(&self) -> bool {
        self.clock_paused
            && if self.player_injured {
                self.injury.stage > 1
            } else {
                self.text_panel.message().is_some()
            }
    }

    /// The bookkeeping in `build_text_overlay`: the panel's lines are drawn
    /// (with a sound) when its counter is 0, and each line drawn and each
    /// frame advance the counter. Every 64 frames of a "scored" or "own
    /// goal" panel, which only shows during the goal replay, the crowd may
    /// shout "ice cream".
    pub(crate) fn build_panel_overlay(&mut self) {
        let message = self.text_panel.message();
        let team_names = if self.is_practice() { 1 } else { 2 };
        let lines = match message {
            Some(TextId::Team1Scored | TextId::Team2Scored) => 2,
            Some(TextId::Vs) => 1 + team_names,
            Some(TextId::HalfTime) => 2 + team_names,
            _ => 1,
        };
        let (drawn, counter) = self.text_panel.count_frame(lines);
        if drawn {
            self.play_sound(Sound::SILENCE_CHANNEL_0);
        }
        let goal_message = matches!(
            message,
            Some(TextId::Team1Scored | TextId::Team2Scored | TextId::OwnGoal)
        );
        if counter & 0x3F == 0 && self.rng.high_byte() & 0x20 != 0 && goal_message {
            self.play_sound(Sound::ICE_CREAM);
        }
    }
}
