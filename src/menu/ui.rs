//! The menus' screen: pages drawn off screen, then faded in.
//!
//! As on the Amiga, menu code draws into a back buffer (a [`Page`]) and
//! `display_fade_transition` brings it to the front: straight away if it
//! uses the colours already showing, otherwise by fading the old screen to
//! black and the new one up from black, one palette step every 40 ms.

use std::collections::VecDeque;
use std::time::Duration;

use super::assets::{MenuAssets, SmallFont};
use super::host::{Button, ButtonEvent, Closed, Host, SoundBank, VBLANK};
use super::ram::{Arrows, Title};
use super::text::{big_glyphs, small_glyphs};
use crate::game::events::Sound;
use crate::graphics::ilbm::Picture;
use crate::graphics::indexed::{ColorWords, IndexedImage, IndexedScreen};
use crate::graphics::Canvas;

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;

/// Why the menu code stopped early.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// Escape: back to the main menu (`misc_flags` bit 6).
    Abort,
    /// The window closed, or Escape on the main menu.
    Quit,
}

impl From<Closed> for Stop {
    fn from(_: Closed) -> Self {
        Stop::Quit
    }
}

pub type Flow<T = ()> = Result<T, Stop>;

/// A screen being drawn, and the colours it will be shown with.
#[derive(Clone)]
pub struct Page {
    screen: IndexedScreen,
    palette: ColorWords,
}

impl Page {
    /// A screen drawn elsewhere, shown in `palette`.
    pub fn from_screen(screen: IndexedScreen, palette: ColorWords) -> Self {
        Self { screen, palette }
    }

    /// `display_splash`: a full-screen picture and its colours.
    pub fn picture(picture: &Picture) -> Self {
        let mut screen = IndexedScreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        screen.fill_from(&picture.image);
        Self {
            screen,
            palette: picture.palette,
        }
    }

    /// `show_title`: the backdrop with a title at the top and bottom.
    pub fn titled(assets: &MenuAssets, title: &Title) -> Self {
        let mut page = Self::picture(&assets.backdrop);
        // The top title sits a pixel higher (`sprite_char_1x1.y_offset`).
        page.small_text(assets, &title.top, SmallFont::TitleTop, -1);
        page.small_text(assets, &title.bottom, SmallFont::TitleBottom, 0);
        page
    }

    /// `display_string`: text in the big menu font.
    pub fn big_text(&mut self, assets: &MenuAssets, text: &[u8]) {
        for glyph in big_glyphs(text) {
            draw_glyph(
                &mut self.screen,
                &assets.big_font,
                glyph.index,
                glyph.x,
                glyph.y,
            );
        }
    }

    /// `draw_string`: text in one of the small fonts.
    pub fn small_text(&mut self, assets: &MenuAssets, text: &[u8], font: SmallFont, y_offset: i32) {
        let images = assets.small_font(font);
        for glyph in small_glyphs(text) {
            draw_glyph(
                &mut self.screen,
                images,
                glyph.index,
                glyph.x,
                glyph.y + y_offset,
            );
        }
    }

    /// `display_screen_block`: the plain panel behind the table screens.
    pub fn panel(&mut self) {
        self.fill_box(17, 286, 17, 166, PANEL_COLOUR);
    }

    /// `draw_box_colour`: a filled rectangle in one palette colour.
    pub fn fill_box(&mut self, x: usize, width: usize, y: usize, height: usize, colour: u8) {
        self.screen.fill(x, y, width, height, colour);
    }

    /// `draw_string_alt`: small text without cursor codes, starting at the
    /// pixel column in its first byte and the row in its second, six
    /// pixels a letter.
    pub fn alt_text(&mut self, assets: &MenuAssets, text: &[u8], font: SmallFont) {
        let images = assets.small_font(font);
        let (mut x, y) = (i32::from(text[0]), i32::from(text[1]) * 4);
        for &c in &text[2..] {
            if c != b' ' {
                draw_glyph(
                    &mut self.screen,
                    images,
                    usize::from(c.wrapping_sub(b'A')),
                    x,
                    y,
                );
            }
            x += 6;
        }
    }

    /// `draw_string_list`: lines of orange text, two rows apart, ending at
    /// row 44.
    pub fn string_list(&mut self, assets: &MenuAssets, lines: &[Vec<u8>]) {
        for (i, line) in lines.iter().enumerate() {
            let mut line = line.clone();
            line[1] = ((0x16 - (lines.len() - 1 - i)) * 2) as u8;
            self.alt_text(assets, &line, SmallFont::Orange);
        }
    }

    /// `draw_menu_arrows`: the arrows either side of the chosen item.
    pub fn arrows(&mut self, assets: &MenuAssets, arrows: Arrows) {
        const RIGHT_ARROW: usize = 0x24;
        const LEFT_ARROW: usize = 0x25;
        for (index, (x, y)) in [(RIGHT_ARROW, arrows.left), (LEFT_ARROW, arrows.right)] {
            let (x, y) = (i32::from(x) * 8, i32::from(y) * 4);
            draw_glyph(&mut self.screen, &assets.big_font, index, x, y);
        }
    }
}

/// The panel's colour, also used to blank things out on it.
pub const PANEL_COLOUR: u8 = 6;

fn draw_glyph(screen: &mut IndexedScreen, font: &[IndexedImage], index: usize, x: i32, y: i32) {
    if let Some(image) = font.get(index) {
        screen.draw(image, x, y);
    }
}

/// The steps of a fade: how far each colour channel is dimmed.
const FADE_STEPS: u8 = 15;
/// `wait_40ms` between steps.
const FADE_STEP_VBLANKS: u32 = 2;

/// Dims an ARGB colour made of 4-bit Amiga channels by `steps`, as the fade
/// routines step each channel of each palette entry down by one.
pub fn darken(argb: u32, steps: u8) -> u32 {
    if steps == 0 {
        return argb;
    }
    let channel = |shift: u32| {
        let level = ((argb >> shift) & 0xFF) / 17;
        level.saturating_sub(u32::from(steps)) * 17
    };
    0xFF00_0000 | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

/// The screen as the player sees it, and the input since the last look.
pub struct Ui<'h> {
    host: &'h mut dyn Host,
    /// What is showing, at full brightness.
    front: Canvas,
    /// The colours of an indexed screen showing; `None` for a match frame.
    front_palette: Option<ColorWords>,
    darkness: u8,
    events: VecDeque<ButtonEvent>,
    shown: Canvas,
}

impl<'h> Ui<'h> {
    pub fn new(host: &'h mut dyn Host) -> Self {
        Self {
            host,
            front: Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            front_palette: None,
            darkness: 0,
            events: VecDeque::new(),
            shown: Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT),
        }
    }

    pub fn host(&mut self) -> &mut dyn Host {
        self.host
    }

    /// Waits for the next vertical blank, showing the front screen.
    pub fn vblank(&mut self) -> Flow {
        self.wait(VBLANK)
    }

    pub fn vblanks(&mut self, count: u32) -> Flow {
        (0..count).try_for_each(|_| self.vblank())
    }

    fn wait(&mut self, duration: Duration) -> Flow {
        let pixels = self.shown.pixels_mut();
        for (shown, &front) in pixels.iter_mut().zip(self.front.pixels()) {
            *shown = darken(front, self.darkness);
        }
        let events = self.host.show(&self.shown, duration)?;
        self.events.extend(events);
        Ok(())
    }

    /// `display_fade_transition`: brings `page` to the front.
    pub fn transition(&mut self, page: &Page) -> Flow {
        let mut canvas = Canvas::new(SCREEN_WIDTH, SCREEN_HEIGHT);
        page.screen.render(&page.palette, &mut canvas);
        self.transition_to(canvas, Some(page.palette))
    }

    /// Brings a frame to the front; a frame with no palette (a match) always
    /// fades.
    pub fn transition_to(&mut self, frame: Canvas, palette: Option<ColorWords>) -> Flow {
        let same_colours = palette.is_some() && palette == self.front_palette;
        if same_colours && self.darkness == 0 {
            self.front = frame;
            return self.vblank();
        }
        self.fade_out()?;
        self.front = frame;
        self.front_palette = palette;
        self.vblank()?;
        self.fade_in()
    }

    /// `display_fade_out`: fifteen steps down from the current colours.
    pub fn fade_out(&mut self) -> Flow {
        while self.darkness < FADE_STEPS {
            self.darkness += 1;
            self.vblanks(FADE_STEP_VBLANKS)?;
        }
        Ok(())
    }

    /// `display_fade_in`: fifteen steps up to the new colours.
    fn fade_in(&mut self) -> Flow {
        for darkness in (0..FADE_STEPS).rev() {
            self.darkness = darkness;
            self.vblanks(FADE_STEP_VBLANKS)?;
        }
        Ok(())
    }

    /// Swaps `page` in straight away (for redraws in the same colours).
    pub fn show_page(&mut self, page: &Page) {
        page.screen.render(&page.palette, &mut self.front);
    }

    /// Puts a match frame on screen as it is, at full brightness.
    pub fn show_frame(&mut self, frame: &Canvas, wait: Duration) -> Flow {
        self.front.pixels_mut().copy_from_slice(frame.pixels());
        self.front_palette = None;
        self.darkness = 0;
        self.wait(wait)
    }

    /// Forgets anything pressed so far (`clr.b controller_1_cooked`).
    pub fn clear_input(&mut self) {
        self.events.clear();
    }

    /// The next button event, if any arrived.
    pub fn next_event(&mut self) -> Option<ButtonEvent> {
        self.events.pop_front()
    }

    /// Waits for the next fresh press satisfying `wanted`.
    pub fn next_press(&mut self, wanted: impl Fn(Button) -> bool) -> Option<Button> {
        while let Some(event) = self.events.pop_front() {
            if event.pressed && !event.repeat && wanted(event.button) {
                return Some(event.button);
            }
        }
        None
    }

    pub fn play_sound(&mut self, sound: Sound) {
        self.host.play_sound(sound);
    }

    pub fn use_sound_bank(&mut self, bank: SoundBank) {
        self.host.use_sound_bank(bank);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darkening_steps_each_channel_down() {
        assert_eq!(darken(0xFF11_22FF, 1), 0xFF00_11EE);
        assert_eq!(darken(0xFFFF_FFFF, 15), 0xFF00_0000);
        assert_eq!(darken(0xFF88_4422, 0), 0xFF88_4422);
    }
}
