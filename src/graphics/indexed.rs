//! Palette-indexed images, for screens whose colours fade.
//!
//! The menus fade by stepping the hardware palette, so they are drawn as
//! colour indices and only turned into RGB when shown, as on the Amiga.

use super::canvas::Canvas;
use super::palette::amiga_color;

/// The 32 colour registers as 12-bit Amiga colour words (`0x0RGB`).
pub type ColorWords = [u16; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedImage {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl IndexedImage {
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Self {
        assert_eq!(pixels.len(), width * height, "pixel count must match size");
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> u8 {
        self.pixels[y * self.width + x]
    }

    pub fn row(&self, y: usize) -> &[u8] {
        &self.pixels[y * self.width..(y + 1) * self.width]
    }
}

/// A screen of colour indices.
#[derive(Clone)]
pub struct IndexedScreen {
    image: IndexedImage,
}

impl IndexedScreen {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            image: IndexedImage::new(width, height, vec![0; width * height]),
        }
    }

    /// Copies `src` over the screen, colour 0 included, from the top left.
    pub fn fill_from(&mut self, src: &IndexedImage) {
        let width = self.image.width.min(src.width);
        for y in 0..self.image.height.min(src.height) {
            let start = y * self.image.width;
            self.image.pixels[start..start + width].copy_from_slice(&src.row(y)[..width]);
        }
    }

    /// Draws a masked sprite: colour 0 is see-through, as the blitter's
    /// mask is made from all the planes together.
    pub fn draw(&mut self, src: &IndexedImage, x: i32, y: i32) {
        for sy in 0..src.height {
            let dy = y + sy as i32;
            if dy < 0 || dy >= self.image.height as i32 {
                continue;
            }
            for sx in 0..src.width {
                let dx = x + sx as i32;
                let color = src.pixel(sx, sy);
                if color != 0 && dx >= 0 && dx < self.image.width as i32 {
                    self.image.pixels[dy as usize * self.image.width + dx as usize] = color;
                }
            }
        }
    }

    /// Shows the screen through `palette`.
    pub fn render(&self, palette: &ColorWords, canvas: &mut Canvas) {
        let colors: Vec<u32> = palette.iter().map(|&word| amiga_color(word)).collect();
        let width = canvas.width().min(self.image.width);
        let height = canvas.height().min(self.image.height);
        let canvas_width = canvas.width();
        let pixels = canvas.pixels_mut();
        for y in 0..height {
            let row = self.image.row(y);
            for x in 0..width {
                pixels[y * canvas_width + x] = colors[usize::from(row[x]) % colors.len()];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_zero_is_see_through() {
        let mut screen = IndexedScreen::new(3, 1);
        screen.fill_from(&IndexedImage::new(3, 1, vec![5, 5, 5]));
        screen.draw(&IndexedImage::new(2, 1, vec![0, 7]), 1, 0);
        assert_eq!(screen.image.row(0), &[5, 5, 7]);
    }

    #[test]
    fn renders_through_the_palette() {
        let mut screen = IndexedScreen::new(2, 1);
        screen.fill_from(&IndexedImage::new(2, 1, vec![0, 1]));
        let mut palette = [0; 32];
        palette[1] = 0x0F80;
        let mut canvas = Canvas::new(2, 1);
        screen.render(&palette, &mut canvas);
        assert_eq!(canvas.pixels(), &[0xFF00_0000, 0xFFFF_8800]);
    }
}
