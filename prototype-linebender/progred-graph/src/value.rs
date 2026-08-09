//! Values and cells: every shape — atom, list, record — is a pure
//! structural value compared by content; identity is a cell, a minted
//! 128-bit id whose current value lives in the `Cells` table. Values are
//! finite trees; the graph lives in the links. One canonical spelling
//! per value, owned by the constructors. See `docs/model.md`, Data
//! Layer v3.

use crate::position::Position;
use im::OrdMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

/// A cell identity: 16 opaque CSPRNG bytes — the only reference. All
/// 128 bits are identity; there are no UUID version or variant bits.
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

/// The leaves. A link is followed to its cell's current value; a
/// blob is uninterpreted bytes. Text, numbers, and every other
/// semantic scalar are library conventions over these primitives.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Atom {
    Cell(CellId),
    Blob(Vec<u8>),
}

impl Atom {
    pub fn as_cell(&self) -> Option<CellId> {
        match self {
            Atom::Cell(cell) => Some(*cell),
            _ => None,
        }
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Atom::Blob(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The label this atom can serve as: every record relation has
    /// cell identity, while blobs decline.
    pub fn as_label(&self) -> Option<Label> {
        match self {
            Atom::Cell(cell) => Some(Label::from(*cell)),
            Atom::Blob(_) => None,
        }
    }
}

/// A record field's relation identity. Its display name, if any, is
/// an ordinary graph fact about this cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Label(CellId);

impl Label {
    pub const fn cell(&self) -> CellId {
        self.0
    }
}

impl From<Label> for Atom {
    fn from(label: Label) -> Self {
        Atom::Cell(label.0)
    }
}

/// A value: anything sayable — pure structure, no identity of its
/// own. Cycles are unrepresentable here; they exist only by a cell's
/// value linking back through `Atom::Cell`. Positions are session-only
/// element identity — minted at load and insert, stripped at save —
/// and the hand-written Eq/Hash below IGNORE them: two occurrences of
/// `[2, 3]` are the same value.
#[derive(Debug, Clone)]
pub enum Value {
    Atom(Atom),
    List(OrdMap<Position, Value>),
    Record(OrdMap<Label, Value>),
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

    pub fn record(fields: impl IntoIterator<Item = (Label, Value)>) -> Value {
        Value::Record(fields.into_iter().collect())
    }

    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            Value::Atom(atom) => Some(atom),
            _ => None,
        }
    }

    pub fn as_cell(&self) -> Option<CellId> {
        self.as_atom()?.as_cell()
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        self.as_atom()?.as_blob()
    }

    pub fn as_list(&self) -> Option<&OrdMap<Position, Value>> {
        match self {
            Value::List(elements) => Some(elements),
            _ => None,
        }
    }

    pub fn as_record(&self) -> Option<&OrdMap<Label, Value>> {
        match self {
            Value::Record(fields) => Some(fields),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Atom(a), Value::Atom(b)) => a == b,
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
            Value::Atom(atom) => {
                0_u8.hash(state);
                atom.hash(state);
            }
            Value::List(elements) => {
                1_u8.hash(state);
                elements.len().hash(state);
                for element in elements.values() {
                    element.hash(state);
                }
            }
            Value::Record(fields) => {
                2_u8.hash(state);
                fields.len().hash(state);
                for (label, value) in fields {
                    label.hash(state);
                    value.hash(state);
                }
            }
        }
    }
}

impl From<Atom> for Value {
    fn from(atom: Atom) -> Self {
        Value::Atom(atom)
    }
}

impl From<CellId> for Atom {
    fn from(cell: CellId) -> Self {
        Atom::Cell(cell)
    }
}
impl From<Vec<u8>> for Atom {
    fn from(bytes: Vec<u8>) -> Self {
        Atom::Blob(bytes)
    }
}

impl From<CellId> for Value {
    fn from(cell: CellId) -> Self {
        Value::Atom(Atom::Cell(cell))
    }
}
impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Atom(Atom::from(bytes))
    }
}

impl From<CellId> for Label {
    fn from(cell: CellId) -> Self {
        Label(cell)
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

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::Cell(cell) => write!(f, "{cell}"),
            Atom::Blob(bytes) => write!(f, "0x{}", hex_string(bytes)),
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Atom(atom) => atom.fmt(f),
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
    Key(Label),
    Element(Position),
    Follow,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LabelRepr {
    Cell(CellId),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueRepr {
    Cell(CellId),
    Blob(String),
    List(Vec<Value>),
    Record(Vec<(Label, Value)>),
}

impl Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Label(cell) => LabelRepr::Cell(*cell),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match LabelRepr::deserialize(deserializer)? {
            LabelRepr::Cell(cell) => Ok(Label(cell)),
        }
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Value::Atom(Atom::Cell(cell)) => ValueRepr::Cell(*cell),
            Value::Atom(Atom::Blob(bytes)) => ValueRepr::Blob(hex_string(bytes)),
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

    fn label(name: &str) -> Label {
        Label::from(relation(name))
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
        assert_eq!(
            serde_json::to_string(&Value::from(id)).unwrap(),
            r#"{"cell":"00112233-4455-6677-8899-aabbccddeeff"}"#
        );
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
    fn only_cells_can_be_labels() {
        let cell = new_cell_id();
        assert_eq!(Atom::from(cell).as_label(), Some(Label::from(cell)));
        assert_eq!(Atom::from(vec![1_u8]).as_label(), None);
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
                (Label::from(cell), Value::list([blob("a")])),
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
        let record_json = serde_json::to_string(&Value::record([(label("k"), blob("v"))])).unwrap();
        assert!(record_json.contains("\"cell\""));
        assert!(!record_json.contains("\"string\""));
    }

    #[test]
    fn record_pairs_serialize_in_label_order() {
        let cell = new_cell_id();
        let value = Value::record([
            (label("b"), blob("2")),
            (label("a"), blob("1")),
            (Label::from(cell), blob("0")),
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
        // Only cells label; the removed string representation is
        // rejected for both labels and values.
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
        assert!(serde_json::from_str::<Label>(r#"{"number":1.0}"#).is_err());
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
