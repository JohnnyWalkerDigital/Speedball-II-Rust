//! The game's own pseudo-random generator, bit-for-bit.
//!
//! It is a pair of 32-bit registers mixed with 16-bit add-with-extend
//! operations, exactly as the 68000 code does it. Reproducing it matters:
//! item placement and every AI decision depend on the sequence.

#[derive(Clone, Debug)]
pub struct Rng {
    a: u32,
    b: u32,
}

impl Rng {
    pub fn new(a: u32, b: u32) -> Self {
        Self { a, b }
    }

    pub fn next(&mut self) -> u32 {
        let mut d0_hi = self.a >> 16;
        let mut d0_lo = self.a & 0xFFFF;
        let mut d1_hi = self.b >> 16;
        let mut d1_lo = self.b & 0xFFFF;

        // Round 1 on the low words.
        d0_lo <<= 1;
        let extend = d0_lo > 0xFFFF;
        d0_lo &= 0xFFFF;
        let saved = d0_lo;
        d0_lo += d1_lo + u32::from(extend);
        let extend = d0_lo > 0xFFFF;
        d0_lo &= 0xFFFF;
        d1_lo = saved;
        std::mem::swap(&mut d0_hi, &mut d0_lo);
        std::mem::swap(&mut d1_hi, &mut d1_lo);

        // Round 2 on the (swapped-in) high words.
        let saved = d0_lo;
        d0_lo = (d0_lo + d1_lo + u32::from(extend)) & 0xFFFF;
        d1_lo = saved;
        std::mem::swap(&mut d0_hi, &mut d0_lo);
        std::mem::swap(&mut d1_hi, &mut d1_lo);

        self.a = (d0_hi << 16) | d0_lo;
        self.b = (d1_hi << 16) | d1_lo;
        self.a
    }

    /// The top byte of the first seed (`rand_seed_1` in memory), without
    /// advancing the generator.
    pub fn high_byte(&self) -> u8 {
        (self.a >> 24) as u8
    }

    pub fn next_byte(&mut self) -> i32 {
        (self.next() & 0xFF) as i32
    }

    pub fn next_byte_masked(&mut self, mask: i32) -> i32 {
        self.next_byte() & mask
    }

    pub fn next_word_masked(&mut self, mask: i32) -> i32 {
        (self.next() & 0xFFFF) as i32 & mask
    }
}

impl Default for Rng {
    /// The seed the game boots with (RAM 0x14930).
    fn default() -> Self {
        Self::new(0x3141_5926, 0x5358_9793)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values produced by the original C# `MyRandom`.
    fn reference_next(a: &mut i64, b: &mut i64) -> i64 {
        let mut d0a = ((*a >> 16) & 0xFFFF) as i32;
        let mut d0b = (*a & 0xFFFF) as i32;
        let mut d1a = ((*b >> 16) & 0xFFFF) as i32;
        let mut d1b = (*b & 0xFFFF) as i32;
        d0b <<= 1;
        let mut extended = (d0b as u32 & 0xFFFF_0000) != 0;
        d0b &= 0xFFFF;
        let mut d2b = d0b;
        d0b += d1b + i32::from(extended);
        extended = (d0b as u32 & 0xFFFF_0000) != 0;
        d0b &= 0xFFFF;
        d1b = d2b;
        std::mem::swap(&mut d0a, &mut d0b);
        std::mem::swap(&mut d1a, &mut d1b);
        d2b = d0b;
        d0b += d1b + i32::from(extended);
        d0b &= 0xFFFF;
        d1b = d2b;
        std::mem::swap(&mut d0a, &mut d0b);
        std::mem::swap(&mut d1a, &mut d1b);
        *a = ((((d0a & 0xFFFF) << 16) | (d0b & 0xFFFF)) as i64) & 0xFFFF_FFFF;
        *b = ((((d1a & 0xFFFF) << 16) | (d1b & 0xFFFF)) as i64) & 0xFFFF_FFFF;
        *a
    }

    #[test]
    fn matches_reference_sequence() {
        let (mut a, mut b) = (0x62D9_42D0_i64, 0x1051_9DBE_i64);
        let mut rng = Rng::new(a as u32, b as u32);
        for _ in 0..10_000 {
            assert_eq!(i64::from(rng.next()), reference_next(&mut a, &mut b));
        }
    }
}
