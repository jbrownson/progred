//! Opaque GID cell identity: construction, parsing, and stable external
//! spellings. All 128 bits are identity; there are no UUID version or
//! variant bits.

use crate::value::{hex_string, read_hex};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId([u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCellIdError;

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

    pub fn parse_str(input: &str) -> Result<Self, ParseCellIdError> {
        input.parse()
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex_string(&self.0))
    }
}

impl fmt::Debug for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CellId({self})")
    }
}

impl fmt::Display for ParseCellIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cell id must be 32 hexadecimal digits")
    }
}

impl std::error::Error for ParseCellIdError {}

impl FromStr for CellId {
    type Err = ParseCellIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        read_hex(input)
            .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
            .map(Self)
            .ok_or(ParseCellIdError)
    }
}

impl Serialize for CellId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for CellId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            String::deserialize(deserializer)?
                .parse()
                .map_err(serde::de::Error::custom)
        } else {
            let bytes = Vec::<u8>::deserialize(deserializer)?;
            <[u8; 16]>::try_from(bytes)
                .map(Self)
                .map_err(|_| serde::de::Error::custom("cell id must contain exactly 16 bytes"))
        }
    }
}

pub fn new_cell_id() -> CellId {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("no entropy source");
    CellId::from_bytes(bytes)
}

#[cfg(test)]
mod tests;
