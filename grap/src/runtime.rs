//! Runtime values: GID data, closures and unboxed numbers as evaluation
//! produces them, with the conversion back to GID.

use crate::environment::Environment;
use crate::unboxed::{Native, Unboxed};
use crate::{Expression, Shape, absent, isa, reify, source, vocabulary};
use gid::{CellId, Record, Value};
use std::cell::OnceCell;
use std::fmt;
use std::rc::Rc;
/// An owned evaluated value. Closures retain shared code and lexical captures;
/// containers retain their children as runtime values. No creating Context or host is retained.
/// [`Self::into_value`] explicitly materializes the equivalent GID data.
#[derive(Clone)]
pub struct RuntimeValue(pub(crate) RuntimeValueKind, pub(crate) OnceCell<Rc<Value>>);

pub struct RuntimeListValues<'a> {
    value: &'a RuntimeValue,
    index: usize,
    len: usize,
}

impl Iterator for RuntimeListValues<'_> {
    type Item = RuntimeValue;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.len {
            None
        } else {
            let index = self.index;
            self.index += 1;
            self.value.list_get(index)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RuntimeListValues<'_> {}

#[derive(Clone)]
pub(crate) enum RuntimeValueKind {
    Data(Value),
    Unboxed(Unboxed),
    Record(Rc<[(CellId, RuntimeValue)]>),
    List(RuntimeList),
    Closure(Closure),
}

/// A list's elements and their positions: the ones it was given, or ones
/// minted the first time something asks, as converting it to GID would.
#[derive(Clone)]
pub(crate) struct RuntimeList {
    pub(crate) values: Rc<[RuntimeValue]>,
    positions: OnceCell<Rc<[gid::Position]>>,
}

impl RuntimeList {
    pub(crate) fn positions(&self) -> &Rc<[gid::Position]> {
        self.positions
            .get_or_init(|| gid::position::spread(self.values.len()).into())
    }

    /// Whether both lists' elements sit at the same positions, without
    /// minting any: two lists of one length that were given none agree.
    fn same_positions(&self, other: &Self) -> bool {
        match (self.positions.get(), other.positions.get()) {
            (None, None) => true,
            _ => {
                Rc::ptr_eq(self.positions(), other.positions())
                    || self.positions() == other.positions()
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Closure {
    /// The lambda's own fields, as written.
    pub(crate) fields: Record,
    pub(crate) params: Rc<[CellId]>,
    pub(crate) body: Expression,
    pub(crate) environment: Environment,
}

/// A closure's fields as data, in its record's order: its lambda and
/// environment, and its body's origin when it has one.
fn closure_keys(closure: &Closure) -> Vec<CellId> {
    let mut keys: Vec<_> = [isa::ISA, vocabulary::LAMBDA, vocabulary::ENVIRONMENT]
        .into_iter()
        .chain(source::origin(&closure.body).map(|_| source::vocabulary::BODY_ORIGIN))
        .collect();
    keys.sort();
    keys
}

impl RuntimeValue {
    /// Conservative equality for memo reuse, including retained code identity.
    pub fn same_result(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (RuntimeValueKind::Data(a), RuntimeValueKind::Data(b)) => a == b,
            (RuntimeValueKind::Unboxed(a), RuntimeValueKind::Unboxed(b)) => a.same(b),
            (RuntimeValueKind::Record(a), RuntimeValueKind::Record(b)) => {
                Rc::ptr_eq(a, b)
                    || (a.len() == b.len()
                        && a.iter()
                            .zip(b.iter())
                            .all(|((ak, av), (bk, bv))| ak == bk && av.same_result(bv)))
            }
            (RuntimeValueKind::List(a), RuntimeValueKind::List(b)) => {
                Rc::ptr_eq(&a.values, &b.values)
                    || (a.values.len() == b.values.len()
                        && a.same_positions(b)
                        && a.values
                            .iter()
                            .zip(b.values.iter())
                            .all(|(a, b)| a.same_result(b)))
            }
            (RuntimeValueKind::Closure(a), RuntimeValueKind::Closure(b)) => {
                a.body == b.body
                    && a.fields == b.fields
                    && match (&a.environment.frame, &b.environment.frame) {
                        (None, None) => true,
                        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                        _ => false,
                    }
            }
            _ => false,
        }
    }

    pub(crate) fn new(kind: RuntimeValueKind) -> Self {
        Self(kind, OnceCell::new())
    }

    /// This value as GID data, borrowed: stored data as it is, anything
    /// else materialized on first request and kept.
    pub fn as_value(&self) -> &Value {
        match &self.0 {
            RuntimeValueKind::Data(value) => value,
            _ => self.1.get_or_init(|| Rc::new(self.to_value())),
        }
    }

    pub fn f64(number: f64) -> Self {
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::F64(number))))
    }

    pub fn bool(value: bool) -> Self {
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::Bool(value))))
    }

    pub fn f32(number: f32) -> Self {
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::F32(number))))
    }

    pub fn u64(number: u64) -> Self {
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::U64(number))))
    }

    /// The f32 this value holds under the f32 convention.
    pub fn as_f32(&self) -> Option<f32> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::F32(number) => Some(*number),
                _ => crate::f32::read_record(unboxed.original()?),
            },
            RuntimeValueKind::Data(value) => crate::f32::read(value),
            RuntimeValueKind::Record(fields) => runtime_field(fields, crate::f32::F32)?
                .as_blob()
                .and_then(crate::f32::decode),
            RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
        }
    }

    /// The u64 this value holds under the u64 convention.
    pub fn as_u64(&self) -> Option<u64> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::U64(number) => Some(*number),
                _ => crate::u64::read_record(unboxed.original()?),
            },
            RuntimeValueKind::Data(value) => crate::u64::read(value),
            RuntimeValueKind::Record(fields) => runtime_field(fields, crate::u64::U64)?
                .as_blob()
                .and_then(crate::u64::decode),
            RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn record(fields: impl IntoIterator<Item = (CellId, RuntimeValue)>) -> Self {
        Self::new(RuntimeValueKind::Record(gid::sorted_entries(fields).into()))
    }

    /// A new list, whose positions are minted when something first asks.
    pub fn list(elements: impl IntoIterator<Item = RuntimeValue>) -> Self {
        Self::new(RuntimeValueKind::List(RuntimeList {
            values: elements.into_iter().collect(),
            positions: OnceCell::new(),
        }))
    }

    /// A list whose elements sit at `positions`, in increasing order and one
    /// for each, as a list derived from another keeps its elements' positions
    /// there. Shared positions are taken as they are, without a copy.
    pub fn list_at(
        positions: Rc<[gid::Position]>,
        elements: impl IntoIterator<Item = RuntimeValue>,
    ) -> Self {
        let values: Rc<[RuntimeValue]> = elements.into_iter().collect();
        assert_eq!(values.len(), positions.len(), "a position for each element");
        debug_assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "positions in increasing order"
        );
        Self::new(RuntimeValueKind::List(RuntimeList {
            values,
            positions: OnceCell::from(positions),
        }))
    }

    pub fn as_f64(&self) -> Option<f64> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::F64(number) => Some(*number),
                _ => crate::f64::read_record(unboxed.original()?),
            },
            RuntimeValueKind::Data(value) => crate::f64::read(value),
            RuntimeValueKind::Record(fields) => runtime_field(fields, crate::f64::F64)?
                .as_blob()
                .and_then(crate::f64::decode),
            RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
        }
    }

    /// The text this value holds under the text convention, borrowed
    /// without materializing it.
    pub fn as_text(&self) -> Option<&str> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::Text(text) => Some(text.as_ref()),
                _ => crate::text::read_record(unboxed.original()?),
            },
            RuntimeValueKind::Data(value) => crate::text::read(value),
            RuntimeValueKind::Record(fields) => runtime_field(fields, crate::text::UTF8)?
                .as_blob()
                .and_then(crate::text::decode),
            RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn field(&self, field: CellId) -> Option<RuntimeValue> {
        match &self.0 {
            RuntimeValueKind::Data(value) => {
                value.as_record()?.get(&field).cloned().map(Self::from)
            }
            RuntimeValueKind::Record(fields) => runtime_field(fields, field).cloned(),
            RuntimeValueKind::Unboxed(unboxed) => unboxed.field(field).map(Self::from),
            RuntimeValueKind::Closure(_) if field == isa::ISA => {
                Some(Self::from(Value::from(vocabulary::kind::CLOSURE)))
            }
            RuntimeValueKind::Closure(closure) => closure_keys(closure)
                .contains(&field)
                .then(|| self.to_value().as_record()?.get(&field).cloned())
                .flatten()
                .map(Self::from),
            RuntimeValueKind::List(_) => None,
        }
    }

    pub fn as_cell(&self) -> Option<CellId> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value.as_cell(),
            RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::List(_)
            | RuntimeValueKind::Closure(_) => None,
        }
    }

    /// Inspect an atom without materializing enclosing runtime containers.
    pub fn as_blob(&self) -> Option<&[u8]> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value.as_blob(),
            _ => None,
        }
    }

    /// The kind its `isa` says it is.
    pub fn kind(&self) -> Option<CellId> {
        match &self.0 {
            RuntimeValueKind::Closure(_) => Some(vocabulary::kind::CLOSURE),
            _ => self.field(isa::ISA)?.as_cell(),
        }
    }

    /// Why this value is absent, if it is.
    pub fn absent_reason(&self) -> Option<CellId> {
        (self.kind()? == vocabulary::kind::ABSENT).then_some(())?;
        self.field(absent::REASON)?.as_cell()
    }

    pub fn declines(&self) -> bool {
        self.absent_reason() == Some(absent::DECLINED)
    }

    pub fn record_len(&self) -> Option<usize> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_record()?.len()),
            RuntimeValueKind::Record(fields) => Some(fields.len()),
            RuntimeValueKind::Unboxed(unboxed) => Some(unboxed.len()),
            RuntimeValueKind::Closure(closure) => Some(closure_keys(closure).len()),
            RuntimeValueKind::List(_) => None,
        }
    }

    /// Declared parameter order, without evaluating syntax or materializing a
    /// native closure's code and captured environment.
    pub fn callable_parameters(&self) -> Option<Vec<CellId>> {
        match &self.0 {
            RuntimeValueKind::Closure(closure) => Some(closure.params.to_vec()),
            // A closure's lambda, as it's called; or a lambda, which has its
            // parameters before its body is written.
            _ => {
                let lambda = match self.kind()? {
                    kind if kind == vocabulary::kind::CLOSURE => self.field(vocabulary::LAMBDA)?,
                    kind if kind == vocabulary::kind::LAMBDA => self.clone(),
                    _ => return None,
                };
                lambda
                    .field(vocabulary::PARAMS)?
                    .list_values()?
                    .map(|parameter| parameter.as_cell())
                    .collect()
            }
        }
    }

    pub fn list_len(&self) -> Option<usize> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_list()?.len()),
            RuntimeValueKind::List(list) => Some(list.values.len()),
            RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn list_values(&self) -> Option<RuntimeListValues<'_>> {
        Some(RuntimeListValues {
            value: self,
            index: 0,
            len: self.list_len()?,
        })
    }

    /// Inspect record membership without converting any of its children.
    pub fn contains_field(&self, field: CellId) -> bool {
        match &self.0 {
            RuntimeValueKind::Data(value) => value
                .as_record()
                .and_then(|fields| fields.get(&field))
                .is_some(),
            RuntimeValueKind::Record(fields) => runtime_field(fields, field).is_some(),
            RuntimeValueKind::Unboxed(unboxed) => unboxed.contains_field(field),
            RuntimeValueKind::Closure(closure) => closure_keys(closure).contains(&field),
            RuntimeValueKind::List(_) => false,
        }
    }

    /// The same labels as the GID representation, without materializing values.
    pub fn record_keys(&self) -> Option<Vec<CellId>> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_record()?.keys().copied().collect()),
            RuntimeValueKind::Record(fields) => Some(fields.iter().map(|(key, _)| *key).collect()),
            RuntimeValueKind::Unboxed(unboxed) => Some(unboxed.keys()),
            RuntimeValueKind::Closure(closure) => Some(closure_keys(closure)),
            RuntimeValueKind::List(_) => None,
        }
    }

    /// Which of GID's four structural shapes this value has, with its labels
    /// or positions. Agrees with `as_blob`, `as_cell`, `list_positions`, and
    /// `record_keys`.
    pub fn shape(&self) -> Shape<'_> {
        match &self.0 {
            RuntimeValueKind::Data(Value::Blob(bytes)) => Shape::Blob(bytes),
            RuntimeValueKind::Data(Value::Cell(cell)) => Shape::Cell(*cell),
            RuntimeValueKind::Data(Value::List(elements)) => {
                Shape::List(elements.keys().cloned().collect())
            }
            RuntimeValueKind::Data(Value::Record(fields)) => {
                Shape::Record(fields.keys().copied().collect())
            }
            RuntimeValueKind::List(list) => Shape::List(list.positions().to_vec()),
            RuntimeValueKind::Record(fields) => {
                Shape::Record(fields.iter().map(|(key, _)| *key).collect())
            }
            RuntimeValueKind::Unboxed(unboxed) => Shape::Record(unboxed.keys()),
            RuntimeValueKind::Closure(closure) => Shape::Record(closure_keys(closure)),
        }
    }

    /// The positions of the list's elements, in order: stored ones, the ones
    /// a built list was given, or those minted for it once.
    pub fn list_positions(&self) -> Option<Vec<gid::Position>> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_list()?.keys().cloned().collect()),
            RuntimeValueKind::List(list) => Some(list.positions().to_vec()),
            _ => None,
        }
    }

    /// [`Self::list_positions`], shared: a built list's own, or a stored
    /// list's gathered once.
    pub(crate) fn shared_list_positions(&self) -> Option<Rc<[gid::Position]>> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_list()?.keys().cloned().collect()),
            RuntimeValueKind::List(list) => Some(list.positions().clone()),
            _ => None,
        }
    }

    pub fn list_get(&self, index: usize) -> Option<RuntimeValue> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value
                .as_list()?
                .iter()
                .nth(index)
                .map(|(_, value)| Self::from(value)),
            RuntimeValueKind::List(list) => list.values.get(index).cloned(),
            RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn list_element(&self, position: &gid::Position) -> Option<RuntimeValue> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value.as_list()?.get(position).map(Self::from),
            RuntimeValueKind::List(list) => {
                let index = list.positions().binary_search(position).ok()?;
                list.values.get(index).cloned()
            }
            _ => None,
        }
    }

    pub fn to_value(&self) -> Value {
        reify::value(self)
    }

    pub fn into_value(self) -> Value {
        match self.0 {
            RuntimeValueKind::Data(value) => value,
            RuntimeValueKind::Unboxed(unboxed) => unboxed.into_value(),
            _ => self.to_value(),
        }
    }
}

impl From<Value> for RuntimeValue {
    fn from(value: Value) -> Self {
        Self::new(RuntimeValueKind::Data(value))
    }
}

impl From<&Value> for RuntimeValue {
    fn from(value: &Value) -> Self {
        Self::from(value.clone())
    }
}

impl From<&RuntimeValue> for RuntimeValue {
    fn from(value: &RuntimeValue) -> Self {
        value.clone()
    }
}

impl fmt::Debug for RuntimeValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_value().fmt(formatter)
    }
}

/// A runtime record's field, borrowed. Runtime records keep their fields
/// sorted by label.
fn runtime_field(fields: &[(CellId, RuntimeValue)], label: CellId) -> Option<&RuntimeValue> {
    fields
        .binary_search_by_key(&label, |(field, _)| *field)
        .ok()
        .map(|index| &fields[index].1)
}

#[cfg(test)]
#[path = "tests/runtime_properties.rs"]
mod properties;
