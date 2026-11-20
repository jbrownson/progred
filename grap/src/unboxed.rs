//! Values of Grap's privileged conventions held natively. An unboxed value
//! answers every record question exactly as its record form would: the
//! acceleration must not change what a value is. A record unboxes only
//! through [`Unboxed::recognize`], so the native value always agrees with
//! the record it came from.

use gid::{CellId, Record, Value};
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct Unboxed {
    native: Native,
    /// The exact source record, kept so metadata beside the native value
    /// survives pass-through unchanged.
    original: Option<Record>,
}

/// One privileged convention's native value. A new convention adds a
/// variant here and its arms below.
#[derive(Clone)]
pub(crate) enum Native {
    F64(f64),
    F32(f32),
    U64(u64),
    Text(Rc<str>),
    Bool(bool),
}

impl Native {
    /// The convention a record follows, if any: a literal of a number's or
    /// text's kind, or a boolean.
    fn recognize(fields: &Record) -> Option<Self> {
        let bytes = |kind| crate::literal::of(fields, kind);
        match crate::isa::of(fields) {
            Some(crate::f64::F64) => Some(Self::F64(crate::f64::decode(bytes(crate::f64::F64)?)?)),
            Some(crate::f32::F32) => Some(Self::F32(crate::f32::decode(bytes(crate::f32::F32)?)?)),
            Some(crate::u64::U64) => Some(Self::U64(crate::u64::decode(bytes(crate::u64::U64)?)?)),
            Some(crate::text::TEXT) => Some(Self::Text(
                crate::text::decode(bytes(crate::text::TEXT)?)?.into(),
            )),
            _ => Some(Self::Bool(crate::logic::decode(
                fields.get(&crate::logic::BOOL)?,
            )?)),
        }
    }

    /// Its fields in its record form, in order.
    fn fields(&self) -> Vec<(CellId, Value)> {
        let literal = |kind, bytes: Vec<u8>| {
            vec![
                (crate::isa::ISA, Value::from(kind)),
                (crate::vocabulary::BYTES, Value::from(bytes)),
            ]
        };
        match self {
            Self::F64(number) => literal(crate::f64::F64, crate::f64::encode(*number)),
            Self::F32(number) => literal(crate::f32::F32, crate::f32::encode(*number)),
            Self::U64(number) => literal(crate::u64::U64, crate::u64::encode(*number)),
            Self::Text(text) => literal(crate::text::TEXT, crate::text::encode(text.as_ref())),
            Self::Bool(value) => vec![(crate::logic::BOOL, crate::logic::encode(*value))],
        }
    }

    /// The record form of a value made natively.
    fn record(&self) -> Value {
        Value::record(self.fields())
    }

    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::F64(a), Self::F64(b)) => a.to_bits() == b.to_bits(),
            (Self::F32(a), Self::F32(b)) => a.to_bits() == b.to_bits(),
            (Self::U64(a), Self::U64(b)) => a == b,
            (Self::Text(a), Self::Text(b)) => Rc::ptr_eq(a, b) || a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            _ => false,
        }
    }
}

impl Unboxed {
    /// A value made natively, with no source record.
    pub(crate) fn new(native: Native) -> Self {
        Self {
            native,
            original: None,
        }
    }

    /// The native form of a record following a privileged convention.
    pub(crate) fn recognize(fields: &Record) -> Option<Self> {
        Native::recognize(fields).map(|native| Self {
            native,
            original: Some(fields.clone()),
        })
    }

    pub(crate) fn native(&self) -> &Native {
        &self.native
    }

    pub(crate) fn original(&self) -> Option<&Record> {
        self.original.as_ref()
    }

    pub(crate) fn field(&self, label: CellId) -> Option<Value> {
        match &self.original {
            Some(original) => original.get(&label).cloned(),
            None => self
                .native
                .fields()
                .into_iter()
                .find_map(|(key, value)| (key == label).then_some(value)),
        }
    }

    pub(crate) fn contains_field(&self, label: CellId) -> bool {
        match &self.original {
            Some(original) => original.get(&label).is_some(),
            None => self.native.fields().iter().any(|(key, _)| *key == label),
        }
    }

    pub(crate) fn keys(&self) -> Vec<CellId> {
        match &self.original {
            Some(original) => original.keys().copied().collect(),
            None => self
                .native
                .fields()
                .into_iter()
                .map(|(key, _)| key)
                .collect(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match &self.original {
            Some(original) => original.len(),
            None => self.native.fields().len(),
        }
    }

    /// Conservative equality for memo reuse: the same native value from the
    /// same source record.
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.native.same(&other.native) && self.original == other.original
    }

    pub(crate) fn to_value(&self) -> Value {
        match &self.original {
            Some(original) => Value::Record(original.clone()),
            None => self.native.record(),
        }
    }

    pub(crate) fn into_value(self) -> Value {
        match self.original {
            Some(original) => Value::Record(original),
            None => self.native.record(),
        }
    }
}
