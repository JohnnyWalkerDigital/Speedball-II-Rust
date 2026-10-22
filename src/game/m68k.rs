//! 68000 arithmetic the original relies on, quirks included.

/// `ext.l` then `divu.w #divisor`: the 16-bit `value` is sign-extended and
/// divided as unsigned. Returns `(quotient, remainder)`, or `None` when the
/// quotient does not fit in 16 bits (any negative value), in which case the
/// 68000 leaves the register as it was.
pub fn ext_divu(value: i16, divisor: u16) -> Option<(u16, u16)> {
    let dividend = value as i32 as u32;
    let quotient = dividend / u32::from(divisor);
    (quotient <= 0xFFFF).then(|| (quotient as u16, (dividend % u32::from(divisor)) as u16))
}

/// The register after `ext.l` and `divu.w`: remainder in the high word and
/// quotient in the low word, or the untouched sign-extended value.
pub fn ext_divu_register(value: i16, divisor: u16) -> u32 {
    match ext_divu(value, divisor) {
        Some((quotient, remainder)) => (u32::from(remainder) << 16) | u32::from(quotient),
        None => value as i32 as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divides_positive_values() {
        assert_eq!(ext_divu(207, 10), Some((20, 7)));
        assert_eq!(ext_divu_register(207, 10), 0x0007_0014);
    }

    #[test]
    fn negative_values_overflow_and_are_left_alone() {
        assert_eq!(ext_divu(-2, 10), None);
        assert_eq!(ext_divu_register(-2, 10), 0xFFFF_FFFE);
    }
}
