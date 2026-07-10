//! The typed model: `Value = Atom | List`, atoms the values that can
//! mean (every map key is one), lists inline sequences with no
//! identity of their own. One canonical spelling per value — owned by
//! the constructors — so equality stays syntactic and decidable. See
//! `docs/model.md`, Data Layer v2.

use crate::position::Position;
use im::OrdMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

/// A node identity: 16 CSPRNG bytes — the only reference. Not an RFC
/// 4122 UUID (no version/variant structure); the type and its
/// hyphenated spelling are borrowed as tooling.
pub type NodeId = Uuid;

pub fn new_node_id() -> NodeId {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("no entropy source");
    Uuid::from_bytes(bytes)
}

/// A number with one spelling per value: NaN collapses to a single
/// bit pattern and -0.0 to 0.0 at construction, so Eq/Hash by bits
/// and Ord by total_cmp agree — numbers sort numerically.
#[derive(Debug, Clone, Copy)]
pub struct Number(f64);

impl Number {
    pub fn new(n: f64) -> Self {
        Self(if n.is_nan() {
            f64::from_bits(0x7ff8_0000_0000_0000)
        } else if n == 0.0 {
            0.0
        } else {
            n
        })
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Eq for Number {}

impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The atoms: the values that can mean. Every map key is one, and a
/// label's meaning is looked up through it — nodes carry metadata,
/// strings and numbers are their own spelling.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Atom {
    Node(NodeId),
    String(String),
    Number(Number),
}

impl Atom {
    pub fn as_node(&self) -> Option<NodeId> {
        match self {
            Atom::Node(node) => Some(*node),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Atom::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Atom::Number(n) => Some(n.get()),
            _ => None,
        }
    }
}

/// A value: an atom, or an inline sequence of values. A map appears
/// in a value slot as its Node atom; the map itself lives in the
/// entity table. Positions are session-only element identity —
/// minted at load and insert, stripped at save — and the hand-written
/// Eq/Hash/Ord below IGNORE them: two occurrences of `[2, 3]` are the
/// same value.
#[derive(Debug, Clone)]
pub enum Value {
    Atom(Atom),
    List(OrdMap<Position, Value>),
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

    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            Value::Atom(atom) => Some(atom),
            Value::List(_) => None,
        }
    }

    pub fn as_node(&self) -> Option<NodeId> {
        self.as_atom()?.as_node()
    }

    pub fn as_str(&self) -> Option<&str> {
        self.as_atom()?.as_str()
    }

    pub fn as_number(&self) -> Option<f64> {
        self.as_atom()?.as_number()
    }

    pub fn as_list(&self) -> Option<&OrdMap<Position, Value>> {
        match self {
            Value::List(elements) => Some(elements),
            Value::Atom(_) => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Atom(a), Value::Atom(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len()
                    && a.values().zip(b.values()).all(|(x, y)| x == y)
            }
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
        }
    }
}

impl Ord for Value {
    /// Atoms before lists; atoms by their derived order, lists
    /// lexicographic elementwise — a deterministic order for sorted
    /// serialization, positions ignored like everywhere else.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Value::Atom(a), Value::Atom(b)) => a.cmp(b),
            (Value::Atom(_), Value::List(_)) => std::cmp::Ordering::Less,
            (Value::List(_), Value::Atom(_)) => std::cmp::Ordering::Greater,
            (Value::List(a), Value::List(b)) => a.values().cmp(b.values()),
        }
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Atom> for Value {
    fn from(atom: Atom) -> Self {
        Value::Atom(atom)
    }
}

impl From<NodeId> for Atom {
    fn from(node: NodeId) -> Self {
        Atom::Node(node)
    }
}
impl From<&str> for Atom {
    fn from(s: &str) -> Self {
        Atom::String(s.to_owned())
    }
}
impl From<String> for Atom {
    fn from(s: String) -> Self {
        Atom::String(s)
    }
}
impl From<f64> for Atom {
    fn from(n: f64) -> Self {
        Atom::Number(Number::new(n))
    }
}

impl From<NodeId> for Value {
    fn from(node: NodeId) -> Self {
        Value::Atom(Atom::Node(node))
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Atom(Atom::from(s))
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Atom(Atom::from(s))
    }
}
impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Atom(Atom::from(n))
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::Node(node) => write!(f, "{node}"),
            Atom::String(s) => write!(f, "\"{s}\""),
            Atom::Number(n) => write!(f, "{}", n.get()),
        }
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
        }
    }
}

/// A projection path step: follow a map edge, or descend into a list
/// value. A step that no longer resolves is the stale-path class the
/// editor already tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Step {
    Key(Atom),
    Element(Position),
}

/// Serialized forms. Non-finite numbers spell "nan"/"inf"/"-inf" —
/// JSON cannot spell them and the general form that used to catch
/// them is gone.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum NumberRepr {
    Finite(f64),
    Special(String),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AtomRepr {
    Node(Uuid),
    String(String),
    Number(NumberRepr),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueRepr {
    Node(Uuid),
    String(String),
    Number(NumberRepr),
    List(Vec<Value>),
}

fn number_repr(n: Number) -> NumberRepr {
    let value = n.get();
    if value.is_finite() {
        NumberRepr::Finite(value)
    } else if value.is_nan() {
        NumberRepr::Special("nan".to_string())
    } else if value > 0.0 {
        NumberRepr::Special("inf".to_string())
    } else {
        NumberRepr::Special("-inf".to_string())
    }
}

fn number_from_repr(repr: NumberRepr) -> Result<Number, String> {
    match repr {
        NumberRepr::Finite(n) if n.is_finite() => Ok(Number::new(n)),
        NumberRepr::Finite(_) => Err("non-finite numbers spell nan/inf/-inf".to_string()),
        NumberRepr::Special(s) => match s.as_str() {
            "nan" => Ok(Number::new(f64::NAN)),
            "inf" => Ok(Number::new(f64::INFINITY)),
            "-inf" => Ok(Number::new(f64::NEG_INFINITY)),
            other => Err(format!("unknown number spelling {other:?}")),
        },
    }
}

impl Serialize for Atom {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Atom::Node(node) => AtomRepr::Node(*node),
            Atom::String(s) => AtomRepr::String(s.clone()),
            Atom::Number(n) => AtomRepr::Number(number_repr(*n)),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Atom {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match AtomRepr::deserialize(deserializer)? {
            AtomRepr::Node(node) => Ok(Atom::Node(node)),
            AtomRepr::String(s) => Ok(Atom::String(s)),
            AtomRepr::Number(repr) => number_from_repr(repr)
                .map(Atom::Number)
                .map_err(serde::de::Error::custom),
        }
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let repr = match self {
            Value::Atom(Atom::Node(node)) => ValueRepr::Node(*node),
            Value::Atom(Atom::String(s)) => ValueRepr::String(s.clone()),
            Value::Atom(Atom::Number(n)) => ValueRepr::Number(number_repr(*n)),
            Value::List(elements) => ValueRepr::List(elements.values().cloned().collect()),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match ValueRepr::deserialize(deserializer)? {
            ValueRepr::Node(node) => Ok(Value::from(node)),
            ValueRepr::String(s) => Ok(Value::from(s)),
            ValueRepr::Number(repr) => number_from_repr(repr)
                .map(|n| Value::Atom(Atom::Number(n)))
                .map_err(serde::de::Error::custom),
            ValueRepr::List(elements) => Ok(Value::list(elements)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{between, spread};
    use std::collections::hash_map::DefaultHasher;

    fn hash_of(value: &Value) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn numbers_have_one_spelling_per_value() {
        assert_eq!(Value::from(-0.0), Value::from(0.0));
        assert_eq!(Value::from(f64::NAN), Value::from(f64::NAN));
        assert_eq!(
            Atom::from(f64::from_bits(0x7ff8_0000_0000_0001)),
            Atom::from(f64::NAN)
        );
        assert_eq!(Value::from(1.5).as_number(), Some(1.5));
        // Numeric order, not payload order.
        assert!(Atom::from(2.0) < Atom::from(10.0));
    }

    #[test]
    fn list_equality_ignores_positions() {
        let a = Value::list([Value::from(2.0), Value::from(3.0)]);
        // The same sequence under entirely different positions: an
        // appended-then-prepended construction.
        let first = between(None, None).unwrap();
        let second = between(Some(&first), None).unwrap();
        let b = Value::List(
            [(first, Value::from(2.0)), (second, Value::from(3.0))]
                .into_iter()
                .collect(),
        );
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));

        assert_ne!(a, Value::list([Value::from(3.0), Value::from(2.0)]));
        assert_ne!(a, Value::list([Value::from(2.0)]));
        assert_ne!(a, Value::from(2.0));
        // Nested lists compare structurally too.
        assert_eq!(
            Value::list([a.clone()]),
            Value::list([b.clone()]),
        );
        // Comparison stops at identity boundaries: equal refs, not
        // equal referents.
        let node = new_node_id();
        assert_eq!(
            Value::list([Value::from(node)]),
            Value::list([Value::from(node)]),
        );
        assert_ne!(
            Value::list([Value::from(node)]),
            Value::list([Value::from(new_node_id())]),
        );
    }

    #[test]
    fn values_round_trip_through_json() {
        let node = new_node_id();
        let cases = [
            Value::from(node),
            Value::from("hello"),
            Value::from(42.5),
            Value::from(f64::NAN),
            Value::from(f64::INFINITY),
            Value::from(f64::NEG_INFINITY),
            Value::list([]),
            Value::list([
                Value::from(1.0),
                Value::from("two"),
                Value::from(node),
                Value::list([Value::from(3.0)]),
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
            serde_json::to_string(&Value::from(f64::INFINITY)).unwrap(),
            r#"{"number":"inf"}"#
        );
        assert_eq!(
            serde_json::to_string(&Value::list([Value::from(1.0)])).unwrap(),
            r#"{"list":[{"number":1.0}]}"#
        );
    }

    #[test]
    fn a_list_refuses_to_parse_as_an_atom() {
        assert!(serde_json::from_str::<Atom>(r#"{"list":[]}"#).is_err());
        assert!(serde_json::from_str::<Atom>(r#"{"string":"k"}"#).is_ok());
        // Unknown number spellings and bare non-finite floats refuse.
        assert!(serde_json::from_str::<Value>(r#"{"number":"huge"}"#).is_err());
    }

    #[test]
    fn spread_positions_carry_list_construction() {
        let list = Value::list((0..100).map(|i| Value::from(f64::from(i))));
        let elements = list.as_list().unwrap();
        assert_eq!(elements.len(), 100);
        let positions: Vec<_> = elements.keys().cloned().collect();
        assert_eq!(positions, spread(100));
        let values: Vec<_> = elements.values().cloned().collect();
        assert_eq!(values[3], Value::from(3.0));
    }
}
