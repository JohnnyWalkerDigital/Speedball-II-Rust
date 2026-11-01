//! Big-endian reads over byte slices (both the ST and the Amiga run on a 68000).

pub trait BigEndian {
    fn i8_at(&self, offset: usize) -> i8;
    fn u16_at(&self, offset: usize) -> u16;
    fn i16_at(&self, offset: usize) -> i16;
    fn u32_at(&self, offset: usize) -> u32;
}

impl BigEndian for [u8] {
    fn i8_at(&self, offset: usize) -> i8 {
        self[offset] as i8
    }

    fn u16_at(&self, offset: usize) -> u16 {
        u16::from_be_bytes([self[offset], self[offset + 1]])
    }

    fn i16_at(&self, offset: usize) -> i16 {
        self.u16_at(offset) as i16
    }

    fn u32_at(&self, offset: usize) -> u32 {
        u32::from_be_bytes([
            self[offset],
            self[offset + 1],
            self[offset + 2],
            self[offset + 3],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_big_endian_values() {
        let bytes = [0x12, 0x34, 0xFF, 0xFE];
        assert_eq!(bytes.u16_at(0), 0x1234);
        assert_eq!(bytes.i16_at(2), -2);
        assert_eq!(bytes.u32_at(0), 0x1234_FFFE);
        assert_eq!(bytes.i8_at(3), -2);
    }
}
