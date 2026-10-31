//! Mono WAV files of the Amiga's signed 8-bit samples.

/// A mono 8-bit WAV file. WAV 8-bit samples are unsigned, so the signed
/// Amiga data is offset by 128.
pub fn mono_8(signed_pcm: &[u8], rate: u32) -> Vec<u8> {
    let data_len = signed_pcm.len() as u32;
    let mut out = Vec::with_capacity(44 + signed_pcm.len() + 1);
    out.extend_from_slice(b"RIFF");
    // The data chunk is padded to an even length, outside its stated size.
    out.extend_from_slice(&(36 + data_len + data_len % 2).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes()); // bytes per second
    out.extend_from_slice(&1u16.to_le_bytes()); // block align
    out.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend(signed_pcm.iter().map(|&b| b.wrapping_add(128)));
    if data_len % 2 == 1 {
        out.push(0);
    }
    out
}

/// The samples of a mono PCM WAV file, as signed bytes. 16-bit samples
/// keep their top byte. The rate does not matter: the game plays samples
/// at the pitch of each note.
pub fn read_mono(file: &[u8]) -> Result<Vec<u8>, String> {
    if file.len() < 12 || &file[0..4] != b"RIFF" || &file[8..12] != b"WAVE" {
        return Err("is not a WAV file".into());
    }
    let mut bits = None;
    let mut at = 12;
    while at + 8 <= file.len() {
        let id = &file[at..at + 4];
        let size = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &file[at + 8..(at + 8 + size).min(file.len())];
        match id {
            b"fmt " if body.len() >= 16 => {
                let word = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
                let (format, channels) = (word(0), word(2));
                if format != 1 || channels != 1 {
                    return Err("must be mono PCM (uncompressed)".into());
                }
                bits = Some(word(14));
            }
            b"data" => {
                return match bits {
                    Some(8) => Ok(body.iter().map(|&b| b.wrapping_sub(128)).collect()),
                    Some(16) => Ok(body.chunks_exact(2).map(|s| s[1]).collect()),
                    Some(bits) => Err(format!("has {bits}-bit samples; use 8 or 16")),
                    None => Err("has no format before its data".into()),
                };
            }
            _ => {}
        }
        at += 8 + size + size % 2;
    }
    Err("has no data".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_sign_conversion() {
        let file = mono_8(&[0x00, 0x80, 0x7F], 8287);
        assert_eq!(&file[0..4], b"RIFF");
        assert_eq!(file.len(), 48);
        assert_eq!(&file[44..47], &[128, 0, 255]);
        assert_eq!(read_mono(&file).unwrap(), [0x00, 0x80, 0x7F]);
    }

    #[test]
    fn sixteen_bit_files_keep_their_top_byte() {
        let mut file = mono_8(&[], 8287);
        file[34] = 16;
        file.truncate(40);
        file.extend_from_slice(&4u32.to_le_bytes());
        file.extend_from_slice(&[0x34, 0x12, 0xFF, 0x80]);
        assert_eq!(read_mono(&file).unwrap(), [0x12, 0x80]);
        assert!(read_mono(b"RIFF....WAVE").is_err());
    }
}
