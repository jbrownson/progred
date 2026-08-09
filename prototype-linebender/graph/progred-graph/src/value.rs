//! Values and cells: every shape — atom, list, record — is a pure
//! structural value compared by content; identity is a cell, a minted
//! 128-bit id whose current value lives in the `Cells` table. Values are
//! finite trees; the graph lives in the links. One canonical spelling
//! per value, owned by the constructors. See `docs/model.md`, Data
//! Layer v3.

use crate::cell_id::CellId;
use crate::position::Position;
use im::OrdMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};

/// A value: anything sayable — pure structure, no identity of its
/// own. Cycles are unrepresentable here; they exist only by a cell's
/// value linking back through `Value::Cell`. Positions are session-only
/// element identity — minted at load and insert, stripped at save —
/// and the hand-written Eq/Hash below IGNORE them: two occurrences of
/// `[2, 3]` are the same value.
#[derive(Debug, Clone)]
pub enum Value {
    Cell(CellId),
    Blob(Vec<u8>),
    List(OrdMap<Position, Value>),
    Record(OrdMap<CellId, Value>),
}

impl Value {
    /// Builds a list, minting evenly spread positions.
    pub fn list(elements: impl IntoIterator<Item = Value>) -> Value {
        let elements: Vec<Value> = elements.into_iter().collect();
        Value::List(
            crate::position::spread(elements.len())
                .into_iter()
                .zip(elements)
                .collect(),
        )
    }

    pub fn record(fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
        Value::Record(fields.into_iter().collect())
    }

    pub fn as_cell(&self) -> Option<CellId> {
        match self {
            Value::Cell(cell) => Some(*cell),
            _ => None,
        }
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&OrdMap<Position, Value>> {
        match self {
            Value::List(elements) => Some(elements),
            _ => None,
        }
    }

    pub fn as_record(&self) -> Option<&OrdMap<CellId, Value>> {
        match self {
            Value::Record(fields) => Some(fields),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Cell(a), Value::Cell(b)) => a == b,
            (Value::Blob(a), Value::Blob(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.values().zip(b.values()).all(|(x, y)| x == y)
            }
            (Value::Record(a), Value::Record(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Cell(cell) => {
                0_u8.hash(state);
                cell.hash(state);
            }
            Value::Blob(bytes) => {
                1_u8.hash(state);
                bytes.hash(state);
            }
            Value::List(elements) => {
                2_u8.hash(state);
                elements.len().hash(state);
                for element in elements.values() {
                    element.hash(state);
                }
            }
            Value::Record(fields) => {
                3_u8.hash(state);
                fields.len().hash(state);
                for (label, value) in fields {
                    label.hash(state);
                    value.hash(state);
                }
            }
        }
    }
}

impl From<CellId> for Value {
    fn from(cell: CellId) -> Self {
        Value::Cell(cell)
    }
}
impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Blob(bytes)
    }
}

pub fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Strict reads: lowercase pairs only, so every blob has exactly one
/// spelled form.
fn hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let digit = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(format!(
            "blob hex must be lowercase hex, got {:?}",
            c as char
        )),
    };
    if !s.len().is_multiple_of(2) {
        return Err("blob hex must have even length".to_string());
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| Ok(digit(pair[0])? << 4 | digit(pair[1])?))
        .collect()
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Cell(cell) => write!(f, "{cell}"),
            Value::Blob(bytes) => write!(f, "0x{}", hex_string(bytes)),
            Value::List(elements) => {
                write!(f, "[")?;
                for (index, element) in elements.values().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    element.fmt(f)?;
                }
                write!(f, "]")
            }
            Value::Record(fields) => {
                write!(f, "{{")?;
                for (index, (label, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{label}: {value}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// A projection path step: into a record field, into a list element,
/// or through a link to the cell's current value. A step that no
/// longer resolves is the stale-path class the editor already
/// tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Key(CellId),
    Element(Position),
    Follow,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueRepr {
    Cell(CellId),
    Blob(String),
    List(Vec<Value>),
    Record(Vec<(CellId, Value)>),
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Value::Cell(cell) => ValueRepr::Cell(*cell),
            Value::Blob(bytes) => ValueRepr::Blob(hex_string(bytes)),
            Value::List(elements) => ValueRepr::List(elements.values().cloned().collect()),
            // OrdMap iterates in label order, so the file's pair
            // order is canonical without an explicit sort.
            Value::Record(fields) => {
                ValueRepr::Record(fields.iter().map(|(k, v)| (*k, v.clone())).collect())
            }
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ValueRepr::deserialize(deserializer)? {
            ValueRepr::Cell(cell) => Ok(Value::from(cell)),
            ValueRepr::Blob(hex) => hex_bytes(&hex)
                .map(Value::from)
                .map_err(serde::de::Error::custom),
            ValueRepr::List(elements) => Ok(Value::list(elements)),
            ValueRepr::Record(fields) => Ok(Value::record(fields)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{between, spread};
    use crate::{CellId, new_cell_id};
    use std::collections::hash_map::DefaultHasher;

    fn relation(name: &str) -> CellId {
        CellId::from_u128(match name {
            "x" => 0xebbb03b25e12960d230b25badc553723,
            "y" => 0x7824623804db8467097bdddbf3518394,
            "name" => 0xc19a573c2534703d0797bb163528547c,
            "at" => 0xd6448992e5df9057033a15ff4396f43e,
            "row" => 0x811d61c56fb3341c9247363be492d680,
            "k" => 0x2be2251626ad23562583b8af8649e746,
            "a" => 0x1ca0184130075343f0d121acc948215f,
            "b" => 0xc882fd0d8c251a0b6f0d97306bd87886,
            _ => unreachable!("fixture relation"),
        })
    }

    fn label(name: &str) -> CellId {
        relation(name)
    }

    fn blob(text: &str) -> Value {
        Value::from(text.as_bytes().to_vec())
    }

    fn hash_of(value: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn list_equality_ignores_positions() {
        let a = Value::list([blob("x"), blob("y")]);
        // The same sequence under entirely different positions: an
        // appended-then-prepended construction.
        let first = between(None, None).unwrap();
        let second = between(Some(&first), None).unwrap();
        let b = Value::List(
            [(first, blob("x")), (second, blob("y"))]
                .into_iter()
                .collect(),
        );
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));

        assert_ne!(a, Value::list([blob("y"), blob("x")]));
        assert_ne!(a, Value::list([blob("x")]));
        assert_ne!(a, blob("x"));
        // Nested lists compare structurally too.
        assert_eq!(Value::list([a.clone()]), Value::list([b.clone()]));
        // Comparison stops at links: equal links, not equal linked
        // values.
        let cell = new_cell_id();
        assert_eq!(
            Value::list([Value::from(cell)]),
            Value::list([Value::from(cell)]),
        );
        assert_ne!(
            Value::list([Value::from(cell)]),
            Value::list([Value::from(new_cell_id())]),
        );
    }

    #[test]
    fn records_are_content_compared_values() {
        let a = Value::record([(label("x"), blob("1")), (label("y"), blob("2"))]);
        let b = Value::record([(label("y"), blob("2")), (label("x"), blob("1"))]);
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        assert_ne!(a, Value::record([(label("x"), blob("1"))]));
        assert_ne!(a, Value::record([]));
        assert_ne!(Value::record([]), Value::list([]));
        // Equal inline records nest equally.
        assert_eq!(Value::list([a.clone()]), Value::list([b.clone()]));
    }

    #[test]
    fn blobs_are_their_bytes() {
        assert_eq!(Value::from(vec![0xde, 0xad]), Value::from(vec![0xde, 0xad]));
        assert_ne!(Value::from(vec![0xde, 0xad]), Value::from(vec![0xad, 0xde]));
        assert_eq!(Value::from(vec![0xde]).as_blob(), Some(&[0xde_u8][..]));
    }

    #[test]
    fn values_round_trip_through_json() {
        let cell = new_cell_id();
        let cases = [
            Value::from(cell),
            blob("hello"),
            Value::from(vec![0x89, 0x50, 0x4e, 0x47]),
            Value::from(Vec::<u8>::new()),
            Value::list([]),
            Value::record([]),
            Value::record([
                (label("name"), blob("roof")),
                (cell, Value::list([blob("a")])),
                (label("at"), Value::record([(label("row"), blob("top"))])),
            ]),
        ];
        for value in cases {
            let json = serde_json::to_string(&value).unwrap();
            let parsed: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, value, "{json}");
            // Save → load → save is a fixed point even though minted
            // positions differ: only the order is the data.
            assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        }
        assert_eq!(
            serde_json::to_string(&Value::from(vec![0xde, 0xad])).unwrap(),
            r#"{"blob":"dead"}"#
        );
        assert_eq!(
            serde_json::to_string(&Value::from(CellId::from_u128(
                0x00112233445566778899aabbccddeeff,
            )))
            .unwrap(),
            r#"{"cell":"00112233-4455-6677-8899-aabbccddeeff"}"#
        );
        let record_json = serde_json::to_string(&Value::record([(label("k"), blob("v"))])).unwrap();
        assert!(record_json.contains(&label("k").to_string()));
        assert!(!record_json.contains("\"string\""));
    }

    #[test]
    fn record_pairs_serialize_in_label_order() {
        let cell = new_cell_id();
        let value = Value::record([
            (label("b"), blob("2")),
            (label("a"), blob("1")),
            (cell, blob("0")),
        ]);
        let json = serde_json::to_value(&value).unwrap();
        let labels: Vec<String> = json["record"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| serde_json::to_string(&pair[0]).unwrap())
            .collect();
        let mut sorted = labels.clone();
        sorted.sort();
        assert_eq!(labels, sorted);
    }

    #[test]
    fn malformed_spellings_refuse() {
        // Blob hex is strict: lowercase, even length.
        assert!(serde_json::from_str::<Value>(r#"{"blob":"DEAD"}"#).is_err());
        assert!(serde_json::from_str::<Value>(r#"{"blob":"abc"}"#).is_err());
        assert!(serde_json::from_str::<Value>(r#"{"blob":"zz"}"#).is_err());
        // The removed string representation is rejected for values,
        // and record keys must deserialize as cell ids.
        assert!(serde_json::from_str::<Value>(r#"{"string":"v"}"#).is_err());
        assert!(
            serde_json::from_str::<Value>(r#"{"record":[[{"blob":"00"},{"string":"v"}]]}"#)
                .is_err()
        );
        assert!(
            serde_json::from_str::<Value>(r#"{"record":[[{"list":[]},{"string":"v"}]]}"#).is_err()
        );
        // Numbers left the data model.
        assert!(serde_json::from_str::<Value>(r#"{"number":1.0}"#).is_err());
        assert!(serde_json::from_str::<CellId>(r#"{"number":1.0}"#).is_err());
    }

    #[test]
    fn spread_positions_carry_list_construction() {
        let list = Value::list((0..100).map(|i| blob(&i.to_string())));
        let elements = list.as_list().unwrap();
        assert_eq!(elements.len(), 100);
        let positions: Vec<_> = elements.keys().cloned().collect();
        assert_eq!(positions, spread(100));
        let values: Vec<_> = elements.values().cloned().collect();
        assert_eq!(values[3], blob("3"));
    }
}
