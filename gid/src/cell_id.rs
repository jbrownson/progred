//! Opaque GID cell identity: construction, parsing, and stable external
//! spellings. All 128 bits are identity; there are no UUID version or
//! variant bits.

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

    pub const fn simple(self) -> SimpleCellId {
        SimpleCellId(self)
    }

    pub fn parse_str(input: &str) -> Result<Self, ParseCellIdError> {
        input.parse()
    }
}

pub struct SimpleCellId(CellId);

impl fmt::Display for SimpleCellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                write!(f, "-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CellId({self})")
    }
}

impl fmt::Display for ParseCellIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cell id must be 32 hexadecimal digits, optionally in canonical hyphenated form"
        )
    }
}

impl std::error::Error for ParseCellIdError {}

impl FromStr for CellId {
    type Err = ParseCellIdError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let bytes = input.as_bytes();
        let simple = bytes.len() == 32;
        let hyphenated = bytes.len() == 36
            && bytes.get(8) == Some(&b'-')
            && bytes.get(13) == Some(&b'-')
            && bytes.get(18) == Some(&b'-')
            && bytes.get(23) == Some(&b'-');
        if simple || hyphenated {
            let mut digits = bytes.iter().copied().filter(|byte| *byte != b'-');
            let mut id = [0_u8; 16];
            for out in &mut id {
                let high = digits.next().ok_or(ParseCellIdError)?;
                let low = digits.next().ok_or(ParseCellIdError)?;
                *out = hex_digit(high)? << 4 | hex_digit(low)?;
            }
            Ok(Self(id))
        } else {
            Err(ParseCellIdError)
        }
    }
}

fn hex_digit(byte: u8) -> Result<u8, ParseCellIdError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(ParseCellIdError),
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
mod tests {
    use super::*;

    #[test]
    fn cell_ids_are_opaque_128_bit_values_with_stable_spellings() {
        let id = CellId::from_u128(0x00112233445566778899aabbccddeeff);
        assert_eq!(
            id.as_bytes(),
            &[
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ]
        );
        assert_eq!(id.simple().to_string(), "00112233445566778899aabbccddeeff");
        assert_eq!(id.to_string(), "00112233-4455-6677-8899-aabbccddeeff");
        assert_eq!(
            CellId::parse_str("00112233445566778899AABBCCDDEEFF"),
            Ok(id)
        );
        assert_eq!(
            CellId::parse_str("00112233-4455-6677-8899-aabbccddeeff"),
            Ok(id)
        );
        assert!(CellId::parse_str("00112233-44556677-8899-aabbccddeeff").is_err());
        assert!(CellId::parse_str("00112233445566778899aabbccddeefg").is_err());
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            r#""00112233-4455-6677-8899-aabbccddeeff""#
        );
        assert_eq!(
            serde_json::from_str::<CellId>(r#""00112233-4455-6677-8899-aabbccddeeff""#).unwrap(),
            id
        );
    }
}
