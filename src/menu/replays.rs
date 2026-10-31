//! "REPLAY GOALS": the goals on the goal save disk, slots 1 to 9 in turn.
//! Empty slots are skipped.

use super::flow::{wait_fire, Game};
use super::host::{Button, Control, SoundBank};
use super::play::FRAME_TIME;
use super::ram::addr;
use super::ui::{Flow, Ui, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::data::save_disk::{replay_sector, REPLAY_SECTORS};
use crate::game::replay::{Replay, ReplayTick, SavedReplay};
use crate::game::text::Text;
use crate::graphics::Canvas;

/// The panel message shows while its frame counter is up to this.
const MESSAGE_FRAMES: u8 = 0x14;

/// Shows every saved goal. Escape on the first screen goes straight back
/// to the main menu.
pub fn replay_goals(ui: &mut Ui, game: &Game) -> Flow {
    let mut page = game.titled_page(&game.text.titles.replay_goals);
    page.big_text(&game.assets, &game.text.messages.insert_goal_save_disk);
    ui.transition(&page)?;
    wait_fire(ui)?;
    ui.use_sound_bank(SoundBank::Match);
    let shown = (1..=9).try_for_each(|slot| replay_goal(ui, game, slot));
    ui.use_sound_bank(SoundBank::Menu);
    shown
}

/// One slot, if it holds a replay.
fn replay_goal(ui: &mut Ui, game: &Game, slot: u8) -> Flow {
    let read = game.save_disk.read(replay_sector(slot), REPLAY_SECTORS);
    let Some(saved) = read.ok().and_then(|bytes| SavedReplay::decode(&bytes)) else {
        return Ok(());
    };
    // A blank pitch under whatever status bar was left on screen.
    let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    match &game.status_bar {
        Some(status_bar) => canvas.pixels_mut().copy_from_slice(status_bar.pixels()),
        None => game.renderer.render_blank_status_bar(&mut canvas),
    }
    game.renderer.render_blank_view(&mut canvas);
    ui.transition_to(canvas.clone(), None)?;
    run_replay(ui, game, &mut Replay::from_saved(&saved), slot, &mut canvas)
}

/// A replay from disk: fire on either joystick or Escape ends it; keys 1
/// to 9 save it again, where the flags left by the last game allow.
fn run_replay(
    ui: &mut Ui,
    game: &Game,
    replay: &mut Replay,
    slot: u8,
    canvas: &mut Canvas,
) -> Flow {
    let message = goal_message(game, slot);
    let mut counter = 0u8;
    replay.start();
    ui.clear_input();
    loop {
        while let Some(event) = ui.next_event() {
            if !event.pressed || event.repeat {
                continue;
            }
            match event.button {
                Button::Escape | Button::Joystick(_, Control::Fire) => replay.end(),
                Button::Number(to) => {
                    if let Some(saves) = game.replay_saves() {
                        let saved = replay.saved(saves.serial);
                        saves.save(ui, &game.renderer, canvas, &saved, to)?;
                    }
                }
                _ => {}
            }
        }
        match replay.tick() {
            ReplayTick::Over => return Ok(()),
            ReplayTick::Same => {}
            ReplayTick::Frame(frame) => {
                // The counter goes up for a line drawn, then for the frame.
                let shown = counter <= MESSAGE_FRAMES;
                if shown {
                    counter = counter.max(1) + 1;
                }
                game.renderer
                    .render_saved_replay(&frame, shown.then_some(&message), canvas);
            }
        }
        ui.show_frame(canvas, FRAME_TIME)?;
    }
}

/// "GOAL 1" to "GOAL 9" (the message with the slot's digit).
fn goal_message(game: &Game, slot: u8) -> Text {
    let mut text = game.text.messages.replay_goal.clone();
    // Digits follow the letters in the panel font.
    text[addr::STR_GOAL_DIGIT] = b'Z' + 1 + slot;
    Text::new(text)
}
