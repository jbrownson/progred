//! Opaque GID cell identity: construction, parsing, and stable external
//! spellings. All 128 bits are identity; there are no UUID version or
//! variant bits.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId([u8; 16]);

impl CellId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(value.to_be_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// The id that 32 hex digits spell, in either case, as Display writes
    /// it; `from_str_radix` alone would also take fewer digits or a leading
    /// `+`.
    pub fn read(text: &str) -> Option<Self> {
        if text.len() != 32 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        u128::from_str_radix(text, 16).ok().map(Self::from_u128)
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", u128::from_be_bytes(self.0))
    }
}

impl fmt::Debug for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CellId({self})")
    }
}

pub fn new_cell_id() -> CellId {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("no entropy source");
    CellId::from_bytes(bytes)
}

#[cfg(test)]
#[path = "tests/cell_id.rs"]
mod tests;
