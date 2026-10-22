//! A simple ARGB pixel buffer.

/// Pixels are `0xAARRGGBB`; alpha is either 0 (transparent) or 0xFF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
}

impl Image {
    pub const TRANSPARENT: u32 = 0;

    pub fn new(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        assert_eq!(pixels.len(), width * height, "pixel count must match size");
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn filled(width: usize, height: usize, color: u32) -> Self {
        Self::new(width, height, vec![color; width * height])
    }

    pub fn row(&self, y: usize) -> &[u32] {
        &self.pixels[y * self.width..(y + 1) * self.width]
    }

    pub fn is_opaque(pixel: u32) -> bool {
        pixel >> 24 != 0
    }

    pub fn flip_vertically(&mut self) {
        for y in 0..self.height / 2 {
            let (top, bottom) = self.pixels.split_at_mut((self.height - 1 - y) * self.width);
            top[y * self.width..(y + 1) * self.width].swap_with_slice(&mut bottom[..self.width]);
        }
    }

    pub fn flipped_vertically(&self) -> Self {
        let mut copy = self.clone();
        copy.flip_vertically();
        copy
    }

    pub fn sub_image(&self, x: usize, y: usize, width: usize, height: usize) -> Self {
        let pixels = (y..y + height)
            .flat_map(|row| self.row(row)[x..x + width].iter().copied())
            .collect();
        Self::new(width, height, pixels)
    }

    /// Draws `src` with its top-left corner at (`x`, `y`), skipping
    /// transparent pixels and anything outside this image.
    pub fn draw(&mut self, src: &Image, x: i32, y: i32) {
        let width = self.width;
        blit(&mut self.pixels, width, self.height, src, x, y);
    }
}

/// Copies `src` onto a `width` x `height` buffer, clipped and alpha-tested.
pub(crate) fn blit(dst: &mut [u32], width: usize, height: usize, src: &Image, x: i32, y: i32) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + src.width as i32).min(width as i32);
    let y1 = (y + src.height as i32).min(height as i32);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for dy in y0..y1 {
        let src_row = src.row((dy - y) as usize);
        let src_span = &src_row[(x0 - x) as usize..(x1 - x) as usize];
        let dst_start = dy as usize * width + x0 as usize;
        let dst_span = &mut dst[dst_start..dst_start + src_span.len()];
        for (d, &s) in dst_span.iter_mut().zip(src_span) {
            if Image::is_opaque(s) {
                *d = s;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_and_crops() {
        let image = Image::new(2, 3, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(image.flipped_vertically().row(0), &[5, 6]);
        assert_eq!(image.sub_image(1, 1, 1, 2), Image::new(1, 2, vec![4, 6]));
    }

    #[test]
    fn draw_clips_and_skips_transparent_pixels() {
        let mut dst = Image::filled(3, 2, 0xFF00_0000);
        let src = Image::new(
            2,
            2,
            vec![0xFF11_1111, Image::TRANSPARENT, 0xFF22_2222, 0xFF33_3333],
        );
        dst.draw(&src, 2, -1);
        assert_eq!(dst.row(0), &[0xFF00_0000, 0xFF00_0000, 0xFF22_2222]);
        assert_eq!(dst.row(1), &[0xFF00_0000; 3]);
    }
}
