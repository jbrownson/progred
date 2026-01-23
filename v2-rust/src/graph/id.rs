use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Id {
    Uuid(Uuid),
    String(String),
    Number(OrderedFloat<f64>),
}

impl Id { pub fn new_uuid() -> Self { Id::Uuid(Uuid::new_v4()) } }
impl From<Uuid> for Id { fn from(u: Uuid) -> Self { Id::Uuid(u) } }
impl From<String> for Id { fn from(s: String) -> Self { Id::String(s) } }
impl From<&str> for Id { fn from(s: &str) -> Self { Id::String(s.to_string()) } }
impl From<f64> for Id { fn from(n: f64) -> Self { Id::Number(OrderedFloat(n)) } }

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Uuid(u) => write!(f, "{}", u),
            Id::String(s) => write!(f, "\"{}\"", s),
            Id::Number(n) => write!(f, "{}", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_serialization() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = Id::Uuid(uuid);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"uuid":"550e8400-e29b-41d4-a716-446655440000"}"#);

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
    fn test_equality() {
        let uuid = Uuid::new_v4();
        assert_eq!(Id::Uuid(uuid), Id::Uuid(uuid));
        assert_ne!(Id::Uuid(Uuid::new_v4()), Id::Uuid(Uuid::new_v4()));
        assert_ne!(Id::from("abc"), Id::from(123.0));
        assert_eq!(Id::from(f64::NAN), Id::from(f64::NAN));
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;

        let uuid = Uuid::new_v4();
        let mut set = HashSet::new();
        set.insert(Id::Uuid(uuid));
        set.insert(Id::from("abc"));
        set.insert(Id::from(123.0));

        assert!(set.contains(&Id::Uuid(uuid)));
        assert!(set.contains(&Id::from("abc")));
        assert!(set.contains(&Id::from(123.0)));
    }
}
