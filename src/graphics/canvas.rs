//! The 320x200 screen the match is drawn into.

use super::image::{blit, Image};

pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    /// Drawing is clipped to rows above this.
    clip_bottom: usize,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
            clip_bottom: height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Pixels as `0xAARRGGBB` (the window ignores alpha), row by row.
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }

    /// Restricts drawing to the top `rows` rows (`None` for the whole screen).
    pub fn clip_to_rows(&mut self, rows: Option<usize>) {
        self.clip_bottom = rows.unwrap_or(self.height).min(self.height);
    }

    pub fn draw(&mut self, image: &Image, x: i32, y: i32) {
        let rows = self.clip_bottom * self.width;
        blit(
            &mut self.pixels[..rows],
            self.width,
            self.clip_bottom,
            image,
            x,
            y,
        );
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        for row in y..(y + height).min(self.clip_bottom) {
            let start = row * self.width + x.min(self.width);
            let end = row * self.width + (x + width).min(self.width);
            self.pixels[start..end].fill(color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_protects_lower_rows() {
        let mut canvas = Canvas::new(4, 4);
        canvas.clip_to_rows(Some(2));
        canvas.draw(&Image::filled(4, 4, 0xFFFF_FFFF), 0, 0);
        assert_eq!(canvas.pixels()[7], 0xFFFF_FFFF);
        assert_eq!(canvas.pixels()[8], 0);
        canvas.clip_to_rows(None);
        canvas.fill_rect(1, 3, 2, 5, 7);
        assert_eq!(&canvas.pixels()[12..16], &[0, 7, 7, 0]);
    }
}
