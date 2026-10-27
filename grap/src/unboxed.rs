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
    /// The convention a record follows, if any. A record following both
    /// unboxes as its number; a reader of its text finds it in the source
    /// record.
    fn recognize(fields: &Record) -> Option<Self> {
        // One pass over the fields rather than a lookup per convention, in
        // order of preference: f64, f32 and u64, then text, then a boolean.
        fields
            .iter()
            .filter_map(|(label, value)| {
                Some(match *label {
                    crate::f64::F64 => (0, Self::F64(crate::f64::decode(value.as_blob()?)?)),
                    crate::f32::F32 => (1, Self::F32(crate::f32::decode(value.as_blob()?)?)),
                    crate::u64::U64 => (2, Self::U64(crate::u64::decode(value.as_blob()?)?)),
                    crate::text::UTF8 => {
                        (3, Self::Text(crate::text::decode(value.as_blob()?)?.into()))
                    }
                    crate::logic::BOOL => (4, Self::Bool(crate::logic::decode(value)?)),
                    _ => return None,
                })
            })
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, native)| native)
    }

    /// The label of the field holding the value in its record form.
    fn label(&self) -> CellId {
        match self {
            Self::F64(_) => crate::f64::F64,
            Self::F32(_) => crate::f32::F32,
            Self::U64(_) => crate::u64::U64,
            Self::Text(_) => crate::text::UTF8,
            Self::Bool(_) => crate::logic::BOOL,
        }
    }

    /// That field's value.
    fn encoded(&self) -> Value {
        match self {
            Self::F64(number) => crate::f64::encode(*number),
            Self::F32(number) => crate::f32::encode(*number),
            Self::U64(number) => crate::u64::encode(*number),
            Self::Text(text) => crate::text::encode(text.as_ref()),
            Self::Bool(value) => crate::logic::encode(*value),
        }
    }

    /// The record form of a value made natively.
    fn record(&self) -> Value {
        Value::record([(self.label(), self.encoded())])
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
            None => (label == self.native.label()).then(|| self.native.encoded()),
        }
    }

    pub(crate) fn contains_field(&self, label: CellId) -> bool {
        match &self.original {
            Some(original) => original.get(&label).is_some(),
            None => label == self.native.label(),
        }
    }

    pub(crate) fn keys(&self) -> Vec<CellId> {
        match &self.original {
            Some(original) => original.keys().copied().collect(),
            None => vec![self.native.label()],
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.original.as_ref().map_or(1, Record::len)
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
