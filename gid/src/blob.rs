use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, ops::Deref, sync::Arc};

/// Bytes with value semantics: clones share storage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Blob(Arc<Vec<u8>>);

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(Arc::new(bytes))
    }
}

impl Deref for Blob {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

/// Spelled as lowercase hex, its one spelling.
impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex_string(self))
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(deserializer)?;
        hex_bytes(&hex)
            .map(Blob::from)
            .map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex_string(self))
    }
}

pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The bytes that hex digit pairs spell, in either case; [`hex_string`]
/// writes them.
pub fn read_hex(hex: &str) -> Option<Vec<u8>> {
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    hex.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => Some(digit(*high)? << 4 | digit(*low)?),
            _ => None,
        })
        .collect()
}

/// Strict reads: lowercase pairs only, so every blob has exactly one
/// spelled form.
fn hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    match s.chars().find(|c| !matches!(c, '0'..='9' | 'a'..='f')) {
        Some(c) => Err(format!("blob hex must be lowercase hex, got {c:?}")),
        None => read_hex(s).ok_or_else(|| "blob hex must have even length".to_string()),
    }
}

#[cfg(test)]
#[path = "tests/blob.rs"]
mod tests;
