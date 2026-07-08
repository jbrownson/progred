//! An identity is a space plus a payload: `(space: Uuid, payload:
//! bytes)`. node ids are the payload discipline of one well-known space;
//! strings and numbers are two more; further spaces are
//! library-definable, including ones with non-minted payload
//! disciplines (content hashes, external identifier systems). See
//! `docs/model.md`.
//!
//! Equality is derived from the pair, so a payload must have exactly
//! one canonical form per value; construction owns that. Numbers
//! collapse NaN to a single bit pattern and -0.0 to 0.0. Strings are
//! the exact UTF-8 sequence (normalizing input is an editor
//! convention, not a substrate rule).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use uuid::Uuid;

/// A node identity: 16 CSPRNG bytes. Not an RFC 4122 UUID — the node
/// space's payload discipline is its own — though the type and its
/// hyphenated spelling are borrowed as tooling.
pub type NodeId = Uuid;

pub const NODE_SPACE: Uuid = Uuid::from_u128(0xf02b_45d2_23e1_43b5_ba14_77ef_534b_c9a9);
pub const STRING_SPACE: Uuid = Uuid::from_u128(0x11d2_4563_f7fc_48da_873f_d97d_0838_1b97);
pub const NUMBER_SPACE: Uuid = Uuid::from_u128(0xae81_64cc_f488_4089_b5ba_a041_086c_98ff);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    space: Uuid,
    payload: Vec<u8>,
}

/// Mint a fresh node identity: 16 CSPRNG bytes, full
/// 128-bit entropy. The space's payload discipline is ours, so RFC
/// 4122's version/variant structure is deliberately not used — it
/// exists to let different generation schemes share a namespace, a
/// context this space doesn't have.
pub fn new_node_id() -> Uuid {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("no entropy source");
    Uuid::from_bytes(bytes)
}

impl Id {
    pub fn new_node_id() -> Self {
        new_node_id().into()
    }

    /// Escape hatch for library-defined spaces; the caller owns the
    /// payload's canonical form.
    pub fn in_space(space: Uuid, payload: Vec<u8>) -> Self {
        Self { space, payload }
    }

    pub fn space(&self) -> Uuid {
        self.space
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn as_node_id(&self) -> Option<Uuid> {
        (self.space == NODE_SPACE)
            .then(|| Uuid::from_slice(&self.payload).ok())
            .flatten()
    }

    pub fn as_str(&self) -> Option<&str> {
        (self.space == STRING_SPACE)
            .then(|| std::str::from_utf8(&self.payload).ok())
            .flatten()
    }

    pub fn as_number(&self) -> Option<f64> {
        (self.space == NUMBER_SPACE)
            .then(|| {
                let bytes: [u8; 8] = self.payload.as_slice().try_into().ok()?;
                let n = f64::from_le_bytes(bytes);
                // Strict read: only the canonical spelling parses, so
                // near-miss bytes render as bytes instead of
                // impersonating the value.
                (canonical(n).to_le_bytes() == bytes).then_some(n)
            })
            .flatten()
    }
}

impl From<Uuid> for Id {
    fn from(uuid: Uuid) -> Self {
        Self {
            space: NODE_SPACE,
            payload: uuid.as_bytes().to_vec(),
        }
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Self {
            space: STRING_SPACE,
            payload: s.as_bytes().to_vec(),
        }
    }
}

impl From<String> for Id {
    fn from(s: String) -> Self {
        Self {
            space: STRING_SPACE,
            payload: s.into_bytes(),
        }
    }
}

/// The one spelling per value: NaN collapses to a single bit pattern,
/// -0.0 to 0.0.
fn canonical(n: f64) -> f64 {
    if n.is_nan() {
        f64::from_bits(0x7ff8_0000_0000_0000)
    } else if n == 0.0 {
        0.0
    } else {
        n
    }
}

impl From<f64> for Id {
    fn from(n: f64) -> Self {
        Self {
            space: NUMBER_SPACE,
            payload: canonical(n).to_le_bytes().to_vec(),
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(uuid) = self.as_node_id() {
            write!(f, "{uuid}")
        } else if let Some(s) = self.as_str() {
            write!(f, "\"{s}\"")
        } else if let Some(n) = self.as_number() {
            write!(f, "{n}")
        } else {
            write!(f, "{}:{}", self.space, to_hex(&self.payload))
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    // ASCII-only keeps the two-byte slices below on char boundaries.
    (s.is_ascii() && s.len().is_multiple_of(2))
        .then(|| {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
                .collect()
        })
        .flatten()
}

/// Serialized forms: the well-known spaces keep their privileged
/// spellings; any other space uses the general `value` form, as do
/// non-finite numbers, which JSON cannot spell (serde_json writes
/// null).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IdRepr {
    Node(Uuid),
    String(String),
    Number(f64),
    Value(Uuid, String),
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = if let Some(uuid) = self.as_node_id() {
            IdRepr::Node(uuid)
        } else if let Some(s) = self.as_str() {
            IdRepr::String(s.to_owned())
        } else if let Some(n) = self.as_number().filter(|n| n.is_finite()) {
            IdRepr::Number(n)
        } else {
            IdRepr::Value(self.space, to_hex(&self.payload))
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match IdRepr::deserialize(deserializer)? {
            IdRepr::Node(uuid) => Ok(uuid.into()),
            IdRepr::String(s) => Ok(s.into()),
            IdRepr::Number(n) => Ok(n.into()),
            IdRepr::Value(space, hex) => from_hex(&hex)
                .map(|payload| Id::in_space(space, payload))
                .ok_or_else(|| serde::de::Error::custom("invalid hex payload")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unparsable_payloads_are_identities_without_a_reading() {
        let sneaky_nan = Id::in_space(
            NUMBER_SPACE,
            f64::from_bits(0x7ff8_0000_0000_0001).to_le_bytes().to_vec(),
        );
        assert_eq!(sneaky_nan.as_number(), None);
        assert_ne!(sneaky_nan, Id::from(f64::NAN));

        let negative_zero = Id::in_space(NUMBER_SPACE, (-0.0_f64).to_le_bytes().to_vec());
        assert_eq!(negative_zero.as_number(), None);

        let not_utf8 = Id::in_space(STRING_SPACE, vec![0xff, 0xfe]);
        assert_eq!(not_utf8.as_str(), None);

        // Identity stays total: they equal themselves, hash, and
        // round-trip through the general serialized form.
        let json = serde_json::to_string(&not_utf8).unwrap();
        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, not_utf8);
    }

    #[test]
    fn test_uuid_serialization() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = Id::from(uuid);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"node":"550e8400-e29b-41d4-a716-446655440000"}"#);

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_string_serialization() {
        let id: Id = "hello".into();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"string":"hello"}"#);

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_number_serialization() {
        let id: Id = 42.5.into();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"number":42.5}"#);

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn non_finite_numbers_take_the_general_serialized_form() {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let id = Id::from(n);
            let json = serde_json::to_string(&id).unwrap();
            assert!(json.starts_with(r#"{"value":"#), "{json}");
            assert_eq!(serde_json::from_str::<Id>(&json).unwrap(), id);
        }
    }

    #[test]
    fn malformed_hex_payloads_error_instead_of_panicking() {
        // Multi-byte chars once panicked the byte slicing; odd length
        // and non-hex are plain rejections.
        for hex in ["aαb", "abc", "zz"] {
            let json =
                format!(r#"{{"value":["0e7b9a2f-5f3d-4c1e-9a4b-0f2f4bfa7c11","{hex}"]}}"#);
            assert!(serde_json::from_str::<Id>(&json).is_err(), "{hex}");
        }
    }

    #[test]
    fn unknown_space_roundtrips_through_the_general_form() {
        let space = Uuid::parse_str("0e7b9a2f-5f3d-4c1e-9a4b-0f2f4bfa7c11").unwrap();
        let id = Id::in_space(space, vec![0xde, 0xad, 0xbe, 0xef]);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(
            json,
            r#"{"value":["0e7b9a2f-5f3d-4c1e-9a4b-0f2f4bfa7c11","deadbeef"]}"#
        );

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_equality() {
        let uuid = new_node_id();
        assert_eq!(Id::from(uuid), Id::from(uuid));
        assert_ne!(Id::from(new_node_id()), Id::from(new_node_id()));
        assert_ne!(Id::from("abc"), Id::from(123.0));
        assert_eq!(Id::from(f64::NAN), Id::from(f64::NAN));
    }

    #[test]
    fn number_payloads_are_canonical() {
        assert_eq!(Id::from(-0.0), Id::from(0.0));
        assert_eq!(
            Id::from(f64::from_bits(0x7ff8_0000_0000_0001)),
            Id::from(f64::NAN)
        );
        assert_eq!(Id::from(1.5).as_number(), Some(1.5));
    }

    #[test]
    fn uuid_payload_is_the_uuid_bytes() {
        let uuid = new_node_id();
        let id = Id::from(uuid);
        assert_eq!(id.payload(), uuid.as_bytes());
        assert_eq!(id.as_node_id(), Some(uuid));
        assert_eq!(id.as_str(), None);
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;

        let uuid = new_node_id();
        let mut set = HashSet::new();
        set.insert(Id::from(uuid));
        set.insert(Id::from("abc"));
        set.insert(Id::from(123.0));

        assert!(set.contains(&Id::from(uuid)));
        assert!(set.contains(&Id::from("abc")));
        assert!(set.contains(&Id::from(123.0)));
    }
}
