//! Read-only view of the Amiga match RAM dump, addressed like the original.

use std::sync::Arc;

#[derive(Clone)]
pub struct Memory {
    bytes: Arc<[u8]>,
}

impl Memory {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// Out-of-range reads return 0, like reading unmapped memory as silence.
    pub fn u8(&self, address: u32) -> u8 {
        self.bytes.get(address as usize).copied().unwrap_or(0)
    }

    pub fn i8(&self, address: u32) -> i8 {
        self.u8(address) as i8
    }

    pub fn u16(&self, address: u32) -> u16 {
        u16::from_be_bytes([self.u8(address), self.u8(address.wrapping_add(1))])
    }

    pub fn u32(&self, address: u32) -> u32 {
        (u32::from(self.u16(address)) << 16) | u32::from(self.u16(address.wrapping_add(2)))
    }
}
