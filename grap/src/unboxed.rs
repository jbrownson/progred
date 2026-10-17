//! Values of Grap's privileged conventions held natively. An unboxed value
//! answers every record question exactly as its record form would: the
//! acceleration must not change what a value is. Lowered values come only
//! from [`Unboxed::lowered`], so the native value always agrees with the
//! record it came from.

use gid::{CellId, Record, Value};

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
}

impl Native {
    /// The convention a record follows, if any.
    fn recognize(fields: &Record) -> Option<Self> {
        crate::f64::read_record(fields).map(Self::F64)
    }

    /// The label of the field holding the value in its record form.
    fn label(&self) -> CellId {
        match self {
            Self::F64(_) => crate::f64::F64,
        }
    }

    /// That field's value.
    fn encoded(&self) -> Value {
        match self {
            Self::F64(number) => Value::from(number.to_le_bytes().to_vec()),
        }
    }

    /// The record form of a value made natively.
    fn record(&self) -> Value {
        Value::record([(self.label(), self.encoded())])
    }

    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::F64(a), Self::F64(b)) => a.to_bits() == b.to_bits(),
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
    pub(crate) fn lowered(fields: &Record) -> Option<Self> {
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
            Some(original) => original.contains_key(&label),
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
