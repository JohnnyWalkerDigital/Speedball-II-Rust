//! Reading and writing PNG files: full-colour images with transparency,
//! and palette-indexed ones whose pixels are colour numbers.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use super::ContentError;

/// An image as ARGB pixels (`0xAARRGGBB`), as the match draws them.
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

/// An image as colour numbers, and the colours in its file.
pub struct Indexed {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    /// `[r, g, b]` for each colour number.
    pub palette: Vec<[u8; 3]>,
}

pub fn write_rgba(path: &Path, image: &Rgba) -> Result<(), ContentError> {
    let data: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|&argb| {
            let [a, r, g, b] = argb.to_be_bytes();
            [r, g, b, a]
        })
        .collect();
    write(
        path,
        image.width,
        image.height,
        png::ColorType::Rgba,
        None,
        &data,
    )
}

pub fn write_indexed(path: &Path, image: &Indexed) -> Result<(), ContentError> {
    let palette = image.palette.iter().flatten().copied().collect();
    let color_type = png::ColorType::Indexed;
    write(
        path,
        image.width,
        image.height,
        color_type,
        Some(palette),
        &image.pixels,
    )
}

fn write(
    path: &Path,
    width: usize,
    height: usize,
    color_type: png::ColorType,
    palette: Option<Vec<u8>>,
    data: &[u8],
) -> Result<(), ContentError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| ContentError::io(dir, e))?;
    }
    let file = File::create(path).map_err(|e| ContentError::io(path, e))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(color_type);
    encoder.set_depth(png::BitDepth::Eight);
    if let Some(palette) = palette {
        encoder.set_palette(palette);
    }
    let png_error = |e: png::EncodingError| ContentError::png(path, e.to_string());
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(data).map_err(png_error)
}

/// Reads any PNG as ARGB. Pixels less than half opaque become fully
/// transparent (0), the rest fully opaque: the game has nothing between.
pub fn read_rgba(path: &Path) -> Result<Rgba, ContentError> {
    let file = File::open(path).map_err(|e| ContentError::io(path, e))?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let png_error = |e: png::DecodingError| ContentError::png(path, e.to_string());
    let mut reader = decoder.read_info().map_err(png_error)?;
    let mut data = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut data).map_err(png_error)?;
    let (width, height) = (frame.width as usize, frame.height as usize);
    let channels = frame.color_type.samples();
    let pixels = data[..width * height * channels]
        .chunks_exact(channels)
        .map(|p| {
            let (r, g, b, a) = match *p {
                [v] => (v, v, v, 0xFF),
                [v, a] => (v, v, v, a),
                [r, g, b] => (r, g, b, 0xFF),
                [r, g, b, a] => (r, g, b, a),
                _ => unreachable!("PNG pixels have one to four samples"),
            };
            if a < 0x80 {
                0
            } else {
                u32::from_be_bytes([0xFF, r, g, b])
            }
        })
        .collect();
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// Reads a palette-indexed PNG, keeping the colour numbers.
pub fn read_indexed(path: &Path) -> Result<Indexed, ContentError> {
    let file = File::open(path).map_err(|e| ContentError::io(path, e))?;
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let png_error = |e: png::DecodingError| ContentError::png(path, e.to_string());
    let mut reader = decoder.read_info().map_err(png_error)?;
    let mut data = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut data).map_err(png_error)?;
    if frame.color_type != png::ColorType::Indexed {
        return Err(ContentError::png(
            path,
            "must be an indexed-colour PNG: its pixels are colour numbers".into(),
        ));
    }
    let (width, height) = (frame.width as usize, frame.height as usize);
    let bits = frame.bit_depth as usize;
    let pixels = (0..height)
        .flat_map(|y| {
            let line = &data[y * frame.line_size..(y + 1) * frame.line_size];
            (0..width).map(move |x| {
                let bit = x * bits;
                let byte = line[bit / 8];
                let shift = 8 - bits - bit % 8;
                (byte >> shift) & ((1u16 << bits) - 1) as u8
            })
        })
        .collect();
    let palette = reader
        .info()
        .palette
        .as_deref()
        .unwrap_or_default()
        .chunks_exact(3)
        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
        .collect();
    Ok(Indexed {
        width,
        height,
        pixels,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Editors save images with up to 16 colours four bits a pixel.
    #[test]
    fn indexed_images_of_any_depth_read_as_colour_numbers() {
        let dir = std::env::temp_dir().join(format!("sb2-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("four-bit.png");
        let file = File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(BufWriter::new(file), 3, 2);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Four);
        encoder.set_palette(vec![0; 16 * 3]);
        // Rows of three pixels, padded to whole bytes: 1 2 3 / 15 0 7.
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0x12, 0x30, 0xF0, 0x70]).unwrap();
        drop(writer);
        let image = read_indexed(&path).unwrap();
        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(image.pixels, [1, 2, 3, 15, 0, 7]);
        assert_eq!(image.palette.len(), 16);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn transparency_is_all_or_nothing() {
        let dir = std::env::temp_dir().join(format!("sb2-rgba-{}", std::process::id()));
        let path = dir.join("rgba.png");
        let pixels = vec![0, 0xFF12_3456, 0x7F12_3456 & 0x00FF_FFFF, 0xFFAB_CDEF];
        write_rgba(
            &path,
            &Rgba {
                width: 2,
                height: 2,
                pixels,
            },
        )
        .unwrap();
        let image = read_rgba(&path).unwrap();
        assert_eq!(image.pixels, [0, 0xFF12_3456, 0, 0xFFAB_CDEF]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
