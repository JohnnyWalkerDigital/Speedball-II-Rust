//! The intro (`intro_presents`, `intro` and their helpers): the publisher
//! and developer, then the title, the story and the credits typed out
//! letter by letter, round and round until fire (or space) is pressed.
//!
//! Its pictures, font, text and music come from a RAM dump taken during the
//! intro, where they sit after the intro code.

use serde::{Deserialize, Serialize};

use super::host::{Button, Control, SoundBank};
use super::ui::{Flow, Page, Ui};
use crate::data::readable;
use crate::game::events::Sound;
use crate::game::geometry::Side;
use crate::graphics::ilbm::{self, IlbmError, Picture};
use crate::graphics::indexed::IndexedImage;
use crate::graphics::planar::{decode_indexed_sprites, Layout, SpriteFormat};

mod addr {
    pub const SPLASH_TITLE_SCREEN: usize = 0x49E5E;
    pub const SPLASH_ARENA_BACKDROP: usize = 0x52D0E;
    pub const SPRITES_TITLE_FONT: usize = 0x5AE00;
    pub const PRESENTS: [usize; 2] = [0x49BCE, 0x49BE5];
    pub const STORY: [usize; 9] = [
        0x49BFF, 0x49C1D, 0x49C54, 0x49C98, 0x49CB8, 0x49CFB, 0x49D11, 0x49D24, 0x49D39,
    ];
    pub const CREDITS: [usize; 8] = [
        0x49D5A, 0x49D6C, 0x49D84, 0x49DC0, 0x49DD7, 0x49DFA, 0x49E17, 0x49E2A,
    ];
    pub const LOADING_GAME: usize = 0x49E4E;
}

/// The dump comes from a cracked copy, which says "DUAL CREW PRESENT"
/// where the original (as disassembled) says this, in as many bytes.
const ORIGINAL_PRESENTS: &[u8] = b"\x0a\x14IMAGEWORKS\x0c\x19PRESENTS";

/// The title font's letters; then the cursor and a blank.
const FONT_GLYPHS: usize = 43;
const CURSOR: usize = 41;
const GLYPH_SIZE: usize = 16;

/// `sound_play` 0x2C: the intro music.
const MUSIC: Sound = Sound {
    id: 0x2C,
    priority: 0,
};
/// 0x2D stops it.
const MUSIC_OFF: Sound = Sound {
    id: 0x2D,
    priority: 0,
};
/// The typing sounds, only in the "presents" screens. The original meant
/// 0x37 for spaces, but compared the character before converting it, so
/// letters and spaces alike get 0x36; 0x37 goes with the cursor's blinks.
const TYPE: Sound = Sound {
    id: 0x36,
    priority: 0x1E,
};
const BLINK: Sound = Sound {
    id: 0x37,
    priority: 0x1E,
};

/// `wait_100ms`: five vertical blanks.
const STEP_VBLANKS: u32 = 5;
/// The title stays up until the 8-bit frame timer wraps from 1.
const TITLE_VBLANKS: u32 = 255;

/// The intro's pictures and font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntroGraphics {
    pub title: Picture,
    pub backdrop: Picture,
    pub font: Vec<IndexedImage>,
}

impl IntroGraphics {
    /// Decodes them from the intro's RAM dump.
    pub fn load(dump: &[u8]) -> Result<Self, IlbmError> {
        let format = SpriteFormat {
            width: GLYPH_SIZE,
            height: GLYPH_SIZE,
            planes: 5,
            layout: Layout::AmigaPlanar,
        };
        let font_bytes = FONT_GLYPHS * format.bytes_per_sprite();
        Ok(Self {
            title: ilbm::decode(&dump[addr::SPLASH_TITLE_SCREEN..])?,
            backdrop: ilbm::decode(&dump[addr::SPLASH_ARENA_BACKDROP..])?,
            font: decode_indexed_sprites(
                &dump[addr::SPRITES_TITLE_FONT..addr::SPRITES_TITLE_FONT + font_bytes],
                format,
            ),
        })
    }
}

/// The intro's text, typed out in its own font.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntroText {
    /// The two screens before the title.
    #[serde(with = "readable::intro_list")]
    pub presents: Vec<Vec<u8>>,
    #[serde(with = "readable::intro_list")]
    pub story: Vec<Vec<u8>>,
    #[serde(with = "readable::intro_list")]
    pub credits: Vec<Vec<u8>>,
    #[serde(with = "readable::intro")]
    pub loading_game: Vec<u8>,
}

impl IntroText {
    /// Reads it from the intro's RAM dump.
    pub fn read(dump: &[u8]) -> Self {
        let string = |at: usize| -> Vec<u8> {
            let end = dump[at..]
                .iter()
                .position(|&b| b == 0)
                .map_or(dump.len(), |n| at + n);
            dump[at..end].to_vec()
        };
        let mut presents: Vec<Vec<u8>> = addr::PRESENTS.into_iter().map(string).collect();
        presents[0] = ORIGINAL_PRESENTS.to_vec();
        Self {
            presents,
            story: addr::STORY.into_iter().map(string).collect(),
            credits: addr::CREDITS.into_iter().map(string).collect(),
            loading_game: string(addr::LOADING_GAME),
        }
    }
}

/// The intro's graphics and text.
pub struct IntroAssets {
    pub graphics: IntroGraphics,
    pub text: IntroText,
}

/// `intro_presents`, then `intro` until it is skipped; then `start`
/// silences the music.
pub fn run(ui: &mut Ui, assets: &IntroAssets) -> Flow {
    ui.use_sound_bank(SoundBank::Intro);
    let mut intro = Intro {
        ui,
        assets,
        page: Page::blank(assets.graphics.backdrop.palette),
        presenting: true,
        skippable: true,
        fire_held: false,
    };
    intro.presents()?;
    intro.presenting = false;
    while !intro.intro()? {
        intro.ui.play_sound(MUSIC_OFF);
    }
    intro.loading()?;
    intro.ui.play_sound(MUSIC_OFF);
    intro.ui.use_sound_bank(SoundBank::Menu);
    Ok(())
}

struct Intro<'u, 'h, 'a> {
    ui: &'u mut Ui<'h>,
    assets: &'a IntroAssets,
    page: Page,
    /// `skip_flags` bit 6: the "presents" screens, which cannot be skipped
    /// and type with sounds.
    presenting: bool,
    /// `skip_flags` bit 1: fire may cut the text short ("LOADING GAME"
    /// cannot be).
    skippable: bool,
    /// Player 1's fire (or space) is down.
    fire_held: bool,
}

impl Intro<'_, '_, '_> {
    /// `display_text_presents`: two screens of text on black.
    fn presents(&mut self) -> Flow {
        let assets = self.assets;
        for (text, blinks) in assets.text.presents.iter().zip([13, 13]) {
            self.page = Page::blank(assets.graphics.backdrop.palette);
            self.ui.transition(&self.page)?;
            let (x, y) = self.type_text(text)?;
            self.blink(x, y, blinks)?;
        }
        Ok(())
    }

    /// `intro` up to its loop: the title (starting the music), the story,
    /// the title again, the credits. Returns whether fire was down at the
    /// end.
    fn intro(&mut self) -> Flow<bool> {
        let assets = self.assets;
        self.ui
            .transition_playing(&Page::picture(&assets.graphics.title), MUSIC)?;
        self.wait(TITLE_VBLANKS)?;
        self.screens(&assets.text.story, 17)?;
        self.ui.transition(&Page::picture(&assets.graphics.title))?;
        self.wait(TITLE_VBLANKS)?;
        self.screens(&assets.text.credits, 16)?;
        Ok(self.fire_held())
    }

    /// `display_text_intro` and `display_text_credits`: each text on the
    /// arena, with the cursor blinking after it. Fire skips the rest.
    fn screens(&mut self, texts: &[Vec<u8>], blinks: u32) -> Flow {
        for text in texts {
            if self.fire_held() {
                return Ok(());
            }
            self.backdrop()?;
            let (x, y) = self.type_text(text)?;
            self.blink(x, y, blinks)?;
        }
        Ok(())
    }

    /// Once fire is let go: "LOADING GAME", which cannot be skipped.
    fn loading(&mut self) -> Flow {
        while self.fire_held() {
            self.ui.vblank()?;
        }
        self.backdrop()?;
        self.skippable = false;
        let text = self.assets.text.loading_game.clone();
        self.type_text(&text)?;
        Ok(())
    }

    /// `draw_arena_backdrop`.
    fn backdrop(&mut self) -> Flow {
        self.page = Page::picture(&self.assets.graphics.backdrop);
        self.ui.transition(&self.page)
    }

    /// `display_string`: each letter after the cursor has shown there.
    /// Returns where the next letter would go.
    fn type_text(&mut self, text: &[u8]) -> Flow<(u8, u8)> {
        let (mut x, mut y) = (0, 0);
        let mut bytes = text.iter().copied();
        while let Some(c) = bytes.next() {
            if self.may_skip() && self.fire_held() {
                break;
            }
            if c <= 0x40 {
                x = c;
                y = bytes.next().unwrap_or(0);
                continue;
            }
            let under = self.save_under(x, y);
            self.cursor(x, y, &under)?;
            if self.presenting {
                self.ui.play_sound(TYPE);
            }
            self.glyph(usize::from(c - b'A'), x, y);
            x += 2;
            if x >= 40 {
                x = 2;
                y += 5;
            }
        }
        Ok((x, y))
    }

    /// The cursor blinking `count` times where the text ended.
    fn blink(&mut self, x: u8, y: u8, count: u32) -> Flow {
        let under = self.save_under(x, y);
        for _ in 0..count {
            self.cursor(x, y, &under)?;
            if self.presenting {
                self.ui.play_sound(BLINK);
            }
        }
        Ok(())
    }

    /// A wait, the cursor, a wait, then what was under it again.
    fn cursor(&mut self, x: u8, y: u8, under: &IndexedImage) -> Flow {
        self.wait(STEP_VBLANKS)?;
        self.glyph(CURSOR, x, y);
        self.wait(STEP_VBLANKS)?;
        let (px, py) = position(x, y);
        self.page.sprite_opaque(under, px, py);
        self.ui.show_page(&self.page);
        Ok(())
    }

    /// `copy_16x16_from_screen`.
    fn save_under(&self, x: u8, y: u8) -> IndexedImage {
        let (px, py) = position(x, y);
        self.page.region(px, py, GLYPH_SIZE, GLYPH_SIZE)
    }

    /// `display_title_font_char` with `mask_char_to_screen`.
    fn glyph(&mut self, index: usize, x: u8, y: u8) {
        if let Some(image) = self.assets.graphics.font.get(index) {
            let (px, py) = position(x, y);
            self.page.sprite(image, px, py);
            self.ui.show_page(&self.page);
        }
    }

    /// `wait_100ms` (or longer): cut short by fire where that is allowed.
    fn wait(&mut self, vblanks: u32) -> Flow {
        for _ in 0..vblanks {
            if self.may_skip() && self.fire_held() {
                return Ok(());
            }
            self.ui.vblank()?;
        }
        Ok(())
    }

    fn may_skip(&self) -> bool {
        self.skippable && !self.presenting
    }

    /// Whether player 1's fire (space on the Amiga too) is down.
    fn fire_held(&mut self) -> bool {
        const FIRE: Button = Button::Joystick(Side::Team1, Control::Fire);
        while let Some(event) = self.ui.next_event() {
            if event.button == FIRE {
                self.fire_held = event.pressed;
            }
        }
        self.fire_held
    }
}

/// Text positions are in 8-pixel columns (on even columns) and 4-line rows.
fn position(x: u8, y: u8) -> (i32, i32) {
    (i32::from(x & !1) * 8, i32::from(y) * 4)
}
