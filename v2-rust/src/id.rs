use std::fmt;
use std::hash::{Hash, Hasher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Guid(GuidId),
    String(StringId),
    Number(NumberId),
}

impl Id {
    pub fn as_guid(&self) -> Option<&GuidId> {
        match self {
            Id::Guid(g) => Some(g),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GuidId {
    #[serde(rename = "guid")]
    pub guid: String,
}

impl GuidId {
    pub fn new(guid: String) -> Self {
        Self { guid }
    }

    pub fn generate() -> Self {
        let s4 = || {
            format!(
                "{:04x}",
                (rand::random::<u16>() as u32 + 0x10000) & 0xFFFF
            )
        };
        Self {
            guid: format!(
                "{}{}{}{}{}{}{}{}",
                s4(),
                s4(),
                s4(),
                s4(),
                s4(),
                s4(),
                s4(),
                s4()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StringId {
    #[serde(rename = "string")]
    pub value: String,
}

impl StringId {
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberId {
    #[serde(rename = "number")]
    pub value: f64,
}

impl NumberId {
    pub fn new(value: f64) -> Self {
        Self { value }
    }
}

// Manual PartialEq for NumberId to handle NaN
impl PartialEq for NumberId {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value || (self.value.is_nan() && other.value.is_nan())
    }
}

impl Eq for NumberId {}

// Manual Hash for NumberId
impl Hash for NumberId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.value.is_nan() {
            state.write_u64(0);
        } else {
            state.write_u64(self.value.to_bits());
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Id::Guid(g) => write!(f, "{}", g.guid),
            Id::String(s) => write!(f, "\"{}\"", s.value),
            Id::Number(n) => write!(f, "{}", n.value),
        }
    }
}
