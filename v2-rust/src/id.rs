use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Id {
    Uuid(Uuid),
    String(String),
    Number(f64),
}

impl Id {
    pub fn new_uuid() -> Self {
        Id::Uuid(Uuid::new_v4())
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Id::Uuid(a), Id::Uuid(b)) => a == b,
            (Id::String(a), Id::String(b)) => a == b,
            (Id::Number(a), Id::Number(b)) => a == b || (a.is_nan() && b.is_nan()),
            _ => false,
        }
    }
}

impl Eq for Id {}

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Id::Uuid(u) => u.hash(state),
            Id::String(s) => s.hash(state),
            Id::Number(n) => {
                if n.is_nan() {
                    state.write_u64(0);
                } else {
                    state.write_u64(n.to_bits());
                }
            }
        }
    }
}

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
    fn test_guid_serialization() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = Id::Uuid(uuid);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"uuid":"550e8400-e29b-41d4-a716-446655440000"}"#);

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_string_serialization() {
        let id = Id::String("hello".into());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#"{"string":"hello"}"#);

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_number_serialization() {
        let id = Id::Number(42.5);
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
        assert_ne!(Id::String("abc".into()), Id::Number(123.0));
        assert_eq!(Id::Number(f64::NAN), Id::Number(f64::NAN));
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;

        let uuid = Uuid::new_v4();
        let mut set = HashSet::new();
        set.insert(Id::Uuid(uuid));
        set.insert(Id::String("abc".into()));
        set.insert(Id::Number(123.0));

        assert!(set.contains(&Id::Uuid(uuid)));
        assert!(set.contains(&Id::String("abc".into())));
        assert!(set.contains(&Id::Number(123.0)));
    }
}
