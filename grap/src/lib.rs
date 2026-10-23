//! A small evaluator of GID expressions, returning owned runtime values
//! with an explicit conversion back to GID. It recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use gid::{CellId, Record, Resolution, Value};
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::rc::Rc;
use unboxed::{Native, Unboxed};

pub use trace::{Answering, CallTrace, Observed, Observer, Tracer};

#[cfg(test)]
mod effect_tests;

#[cfg(test)]
mod runtime_tests;

pub mod absent;
pub mod arguments;
pub mod f32;
pub mod f64;
pub mod isa;
pub mod logic;
pub mod path;
pub mod primitive;
mod reify;
pub mod source;
pub mod text;
mod trace;
pub mod u64;
mod unboxed;
pub mod vocabulary;

pub const DEFAULT_FUEL: usize = 1_024;

/// Shared code, independent of the evaluator that executes it.
#[derive(Clone)]
pub struct Expression(Rc<Lowered>);

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Expression {}

impl std::hash::Hash for Expression {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Rc::as_ptr(&self.0), state);
    }
}

impl std::fmt::Debug for Expression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value().fmt(formatter)
    }
}

impl Expression {
    fn value(&self) -> &Value {
        self.0.source.as_value()
    }
}

/// GID's structural shapes, as a runtime value presents them.
pub enum Shape<'a> {
    Blob(&'a [u8]),
    Cell(CellId),
    List(Vec<gid::Position>),
    Record(Vec<CellId>),
}

/// Where a source expression came from before evaluation. Generated
/// runtime values have no origin; expressions read from the input or a
/// cell retain their structural route for host tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    Input(Vec<gid::Step>),
    Stored(Vec<gid::Step>),
    Cell {
        cell: CellId,
        resolution: Resolution,
        path: Vec<gid::Step>,
    },
}

/// A staged foreign function's per-visit form, returned by its
/// prepare stage with the once-parsed call structure in its captures.
pub type Stage = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

type Prepare = Rc<dyn Fn(&Context, &Expression) -> Stage>;

#[derive(Clone)]
enum ForeignImplementation {
    Direct(Rc<dyn Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt>>),
    Staged(Prepare),
}

#[derive(Clone)]
pub struct ForeignFunction {
    implementation: ForeignImplementation,
    tracked: bool,
    parameters: Option<Rc<[CellId]>>,
    /// The operation this function runs, when the evaluator can run it
    /// directly; behind a pointer so copying a function stays cheap.
    primitive: Option<Rc<primitive::Primitive>>,
}

/// One host definition. A native implementation's value is the foreign
/// function it defines, an ordinary value like any other; reading a
/// definition never invokes native code.
#[derive(Clone)]
pub enum Definition {
    Value(Value),
    Foreign(Rc<ForeignDefinition>),
}

pub struct ForeignDefinition {
    pub value: Value,
    pub implementation: ForeignFunction,
}

impl Definition {
    /// `cell`'s definition as `implementation`: its description, a record,
    /// says it's a foreign function implemented under `cell`. Declared
    /// parameters join it under `params`, as a lambda's do, unless it lists
    /// its own.
    pub fn foreign(cell: CellId, mut value: Value, implementation: ForeignFunction) -> Self {
        if let Value::Record(fields) = &mut value {
            fields.insert(isa::ISA, Value::from(vocabulary::kind::FOREIGN_FUNCTION));
            fields.insert(vocabulary::IMPLEMENTATION, Value::from(cell));
            if let Some(parameters) = &implementation.parameters
                && !fields.contains_key(&vocabulary::PARAMS)
            {
                fields.insert(
                    vocabulary::PARAMS,
                    Value::list(parameters.iter().map(|&parameter| parameter.into())),
                );
            }
        }
        Self::Foreign(Rc::new(ForeignDefinition {
            value,
            implementation,
        }))
    }

    pub fn value(&self) -> &Value {
        match self {
            Self::Value(value) => value,
            Self::Foreign(definition) => &definition.value,
        }
    }
}

/// Where an evaluation finds definitions. A host that reuses results can rely
/// on every changing read being a `resolve`, except where `untracked` says
/// otherwise.
pub trait Host {
    /// `cell`'s definition and the resolution it came through, which source
    /// origins record.
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)>;
    /// A foreign call not declared tracked ran, so the result may depend on
    /// reads the host didn't see.
    fn untracked(&self) {}
    /// An observable write ran, which a reused result would leave out.
    fn effect(&self) {}
    /// What watches the evaluations run against this host, if anything.
    fn observer(&self) -> Option<&dyn Observer> {
        None
    }
}

impl ForeignFunction {
    pub fn from_value(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<Value, Halt> + 'static,
    ) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Direct(Rc::new(
                move |context, expression, environment| {
                    call(context, expression, environment).map(RuntimeValue::from)
                },
            )),
            parameters: None,
            primitive: None,
        }
    }

    pub fn new(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt> + 'static,
    ) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Direct(Rc::new(call)),
            parameters: None,
            primitive: None,
        }
    }

    /// The two-stage form: `prepare` runs when a call site first meets
    /// this function, reads only the call's lowered structure, and
    /// returns the closure that runs per visit — call sites cache that
    /// closure, so prepare must be observation-free (no evaluation
    /// or fuel) and re-runnable. The returned stage owns
    /// every observable, including the fuel the straightforward shape
    /// would burn.
    pub fn staged(prepare: impl Fn(&Context, &Expression) -> Stage + 'static) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Staged(Rc::new(prepare)),
            parameters: None,
            primitive: None,
        }
    }

    /// Opt in only when every changing read goes through evaluation and every
    /// observable write uses Context::effect. Captures must be immutable or
    /// evaluation-local; hidden mutable state prevents sound memoization.
    pub fn tracked(mut self) -> Self {
        self.tracked = true;
        self
    }

    /// The fields a call must supply, in authoring order; arguments with
    /// defaults stay undeclared.
    pub fn parameters(mut self, parameters: impl IntoIterator<Item = CellId>) -> Self {
        self.parameters = Some(parameters.into_iter().collect());
        self
    }

    fn as_primitive(mut self, primitive: primitive::Primitive) -> Self {
        self.primitive = Some(Rc::new(primitive));
        self
    }
}

pub struct Halt(Value);

/// An owned evaluated value. Closures retain shared code and lexical captures;
/// containers retain their children as runtime values. No creating Context or host is retained.
/// [`Self::into_value`] explicitly materializes the equivalent GID data.
#[derive(Clone)]
pub struct RuntimeValue(RuntimeValueKind, OnceCell<Rc<Value>>);

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
enum RuntimeValueKind {
    Data(Value),
    Unboxed(Unboxed),
    Record(Rc<[(CellId, RuntimeValue)]>),
    List(Rc<[RuntimeValue]>),
    Closure(Closure),
}

/// An evaluated callable retained in its compact runtime form. This
/// lets a controlling FFI invoke the same Grap function repeatedly
/// without reifying and reparsing a closure Value between calls.
#[derive(Clone)]
pub struct PreparedCallable {
    value: RuntimeValue,
    environment: Environment,
}

#[derive(Clone)]
struct Closure {
    /// The lambda's own fields, as written.
    fields: Record,
    params: Rc<[CellId]>,
    body: Expression,
    environment: Environment,
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
                Rc::ptr_eq(a, b)
                    || (a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.same_result(b)))
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

    fn new(kind: RuntimeValueKind) -> Self {
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

    pub fn list(elements: impl IntoIterator<Item = RuntimeValue>) -> Self {
        Self::new(RuntimeValueKind::List(elements.into_iter().collect()))
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
            RuntimeValueKind::List(elements) => Some(elements.len()),
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
                .is_some_and(|fields| fields.contains_key(&field)),
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
            RuntimeValueKind::List(values) => Shape::List(gid::position::spread(values.len())),
            RuntimeValueKind::Record(fields) => {
                Shape::Record(fields.iter().map(|(key, _)| *key).collect())
            }
            RuntimeValueKind::Unboxed(unboxed) => Shape::Record(unboxed.keys()),
            RuntimeValueKind::Closure(closure) => Shape::Record(closure_keys(closure)),
        }
    }

    /// Stored lists retain their positions; generated lists use the positions
    /// they receive when materialized, also used by `list_element`.
    pub fn list_positions(&self) -> Option<Vec<gid::Position>> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_list()?.keys().cloned().collect()),
            RuntimeValueKind::List(values) => Some(gid::position::spread(values.len())),
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
            RuntimeValueKind::List(elements) => elements.get(index).cloned(),
            RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn list_element(&self, position: &gid::Position) -> Option<RuntimeValue> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value.as_list()?.get(position).map(Self::from),
            RuntimeValueKind::List(values) => {
                let index = gid::position::spread(values.len())
                    .binary_search(position)
                    .ok()?;
                values.get(index).cloned()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CellIndex(usize);

/// Cell ids are minted from an OS CSPRNG, so folding their bytes is
/// already a uniform hash; SipHash would only add per-lookup cost.
#[derive(Debug, Default)]
struct FoldHasher(u64);

/// Expressions hash by node address. Addresses are aligned and clustered,
/// so a folded multiply spreads them; SipHash would only add per-lookup cost
/// to every evaluation step.
#[derive(Default)]
struct AddressHasher(u64);

impl std::hash::Hasher for AddressHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    fn write_u64(&mut self, word: u64) {
        let product = u128::from(self.0 ^ word) * 0x9e37_79b9_7f4a_7c15;
        self.0 = (product as u64) ^ ((product >> 64) as u64);
    }

    fn write_usize(&mut self, word: usize) {
        self.write_u64(word as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type ByNode<V> =
    std::collections::HashMap<Expression, V, std::hash::BuildHasherDefault<AddressHasher>>;

impl std::hash::Hasher for FoldHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0_u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0 ^= u64::from_le_bytes(word);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default)]
struct CellIndexTable {
    by_cell:
        std::collections::HashMap<CellId, CellIndex, std::hash::BuildHasherDefault<FoldHasher>>,
    cells: Vec<CellId>,
    /// Whether an environment over this table has bound each index. A
    /// reference to a cell nothing binds, a global's, misses at once
    /// instead of searching every frame.
    bound: Vec<bool>,
}

impl CellIndexTable {
    fn intern(&mut self, cell: CellId) -> CellIndex {
        if let Some(index) = self.by_cell.get(&cell) {
            *index
        } else {
            let index = CellIndex(self.cells.len());
            self.cells.push(cell);
            self.bound.push(false);
            self.by_cell.insert(cell, index);
            index
        }
    }

    fn cell(&self, index: CellIndex) -> CellId {
        self.cells[index.0]
    }

    fn index(&self, cell: CellId) -> Option<CellIndex> {
        self.by_cell.get(&cell).copied()
    }
}

type CellIndices = Rc<RefCell<CellIndexTable>>;

fn cell_index(indices: &CellIndices, cell: CellId) -> CellIndex {
    indices.borrow_mut().intern(cell)
}

/// `cell`'s index in `indices`, marked bound, for a binding.
fn binding_index(indices: &CellIndices, cell: CellId) -> CellIndex {
    let mut table = indices.borrow_mut();
    let index = table.intern(cell);
    table.bound[index.0] = true;
    index
}

/// A runtime record's field, borrowed. Runtime records keep their fields
/// sorted by label.
fn runtime_field(fields: &[(CellId, RuntimeValue)], label: CellId) -> Option<&RuntimeValue> {
    fields
        .binary_search_by_key(&label, |(field, _)| *field)
        .ok()
        .map(|index| &fields[index].1)
}

fn lowered_field(fields: &[(CellId, Expression)], label: CellId) -> Option<Expression> {
    fields
        .binary_search_by_key(&label, |(field, _)| *field)
        .ok()
        .map(|index| fields[index].1.clone())
}

#[derive(Debug, Clone, Default)]
pub struct Environment {
    indices: CellIndices,
    frame: Option<Rc<EnvironmentFrame>>,
}

#[derive(Debug, Clone)]
struct EnvironmentFrame {
    parent: Option<Rc<EnvironmentFrame>>,
    bindings: Vec<(CellIndex, RuntimeValue)>,
}

/// An environment's frames, innermost first.
fn frames(environment: &Environment) -> impl Iterator<Item = &EnvironmentFrame> {
    std::iter::successors(environment.frame.as_deref(), |frame| {
        frame.parent.as_deref()
    })
}

impl Environment {
    fn with_indices(indices: CellIndices) -> Self {
        Self {
            indices,
            frame: None,
        }
    }

    fn get_runtime(&self, cell: CellId) -> Option<&RuntimeValue> {
        let index = self.indices.borrow().index(cell)?;
        self.get_index(index)
    }

    /// `index` is `cell`'s slot in `indices`, resolved when the reference
    /// was compiled; an environment built over another table looks it up.
    fn get_at(
        &self,
        indices: &CellIndices,
        cell: CellId,
        index: CellIndex,
    ) -> Option<&RuntimeValue> {
        if Rc::ptr_eq(&self.indices, indices) {
            self.get_index(index)
        } else {
            self.get_runtime(cell)
        }
    }

    fn get_index(&self, index: CellIndex) -> Option<&RuntimeValue> {
        if self.indices.borrow().bound[index.0] {
            frames(self).find_map(|frame| {
                frame
                    .bindings
                    .iter()
                    .rev()
                    .find_map(|(binding, value)| (*binding == index).then_some(value))
            })
        } else {
            None
        }
    }

    fn extended(&self, bindings: impl IntoIterator<Item = (CellId, Value)>) -> Self {
        self.extended_runtime(
            bindings
                .into_iter()
                .map(|(cell, value)| (cell, RuntimeValue::from(value))),
        )
    }

    pub fn extended_runtime(
        &self,
        bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Self {
        self.extended_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (binding_index(&self.indices, cell), value)),
        )
    }

    /// Add bindings at the innermost level in place, copying the frame
    /// only while another environment still shares it. Lookup and
    /// reification are the same as through [`Self::extended_runtime`];
    /// sequential binders avoid a frame allocation per binding.
    pub fn push_runtime(&mut self, bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>) {
        let indices = self.indices.clone();
        self.push_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (binding_index(&indices, cell), value)),
        );
    }

    /// Make room for `additional` in-place bindings. A shared frame is
    /// copied once at its final size instead of at its current one and
    /// then grown binding by binding.
    pub fn reserve(&mut self, additional: usize) {
        if let Some(frame) = &mut self.frame {
            match Rc::get_mut(frame) {
                Some(frame) => frame.bindings.reserve(additional),
                None => {
                    let mut bindings = Vec::with_capacity(frame.bindings.len() + additional);
                    bindings.extend(frame.bindings.iter().cloned());
                    *frame = Rc::new(EnvironmentFrame {
                        parent: frame.parent.clone(),
                        bindings,
                    });
                }
            }
        }
    }

    fn push_indexed(&mut self, bindings: impl IntoIterator<Item = (CellIndex, RuntimeValue)>) {
        match &mut self.frame {
            Some(frame) => {
                let frame = Rc::make_mut(frame);
                frame.bindings.extend(bindings);
            }
            None => {
                let bindings: Vec<_> = bindings.into_iter().collect();
                if !bindings.is_empty() {
                    self.frame = Some(Rc::new(EnvironmentFrame {
                        parent: None,
                        bindings,
                    }));
                }
            }
        }
    }

    fn extended_indexed(
        &self,
        bindings: impl IntoIterator<Item = (CellIndex, RuntimeValue)>,
    ) -> Self {
        let bindings: Vec<_> = bindings.into_iter().collect();
        if bindings.is_empty() {
            self.clone()
        } else {
            Self {
                indices: self.indices.clone(),
                frame: Some(Rc::new(EnvironmentFrame {
                    parent: self.frame.clone(),
                    bindings,
                })),
            }
        }
    }
}

impl From<Environment> for Value {
    fn from(environment: Environment) -> Self {
        Value::from(&environment)
    }
}

impl From<&Environment> for Value {
    fn from(environment: &Environment) -> Self {
        reify::environment(environment)
    }
}

#[derive(Clone, Default)]
pub struct ForeignFunctions {
    functions: Vec<(CellId, ForeignFunction)>,
}

impl ForeignFunctions {
    pub fn register(mut self, function: CellId, definition: ForeignFunction) -> Self {
        match self
            .functions
            .binary_search_by_key(&function, |(cell, _)| *cell)
        {
            Ok(index) => self.functions[index].1 = definition,
            Err(index) => self.functions.insert(index, (function, definition)),
        }
        self
    }

    pub fn get(&self, function: CellId) -> Option<&ForeignFunction> {
        self.functions
            .binary_search_by_key(&function, |(cell, _)| *cell)
            .ok()
            .map(|index| &self.functions[index].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellId, &ForeignFunction)> {
        self.functions
            .iter()
            .map(|(cell, function)| (*cell, function))
    }

    pub fn merge(self, other: Self) -> Self {
        other
            .functions
            .into_iter()
            .fold(self, |functions, (cell, definition)| {
                functions.register(cell, definition)
            })
    }

    pub fn merge_all(tables: impl IntoIterator<Item = Self>) -> Self {
        tables.into_iter().fold(Self::default(), Self::merge)
    }
}

type OverlayFn<'a, T> = dyn for<'context> Fn(CellId, &mut Context<'context>, &Expression, &Environment) -> Result<T, Halt>
    + 'a;

#[derive(Clone, Copy)]
enum OverlayCall<'a> {
    Runtime(&'a OverlayFn<'a, RuntimeValue>),
    Value(&'a OverlayFn<'a, Value>),
}

/// A synchronous, borrowed layer of foreign functions. It lets a host
/// expose state that is valid only for one evaluation without putting
/// that state in `'static` closures.
pub struct ForeignOverlay<'a> {
    functions: &'a [CellId],
    call: OverlayCall<'a>,
    tracked: bool,
}

impl<'a> ForeignOverlay<'a> {
    pub fn new(
        functions: &'a [CellId],
        call: &'a (
                impl for<'context> Fn(
            CellId,
            &mut Context<'context>,
            &Expression,
            &Environment,
        ) -> Result<RuntimeValue, Halt>
                + 'a
            ),
    ) -> Self {
        Self {
            functions,
            call: OverlayCall::Runtime(call),
            tracked: false,
        }
    }

    pub fn from_value(
        functions: &'a [CellId],
        call: &'a (
                impl Fn(CellId, &mut Context<'_>, &Expression, &Environment) -> Result<Value, Halt> + 'a
            ),
    ) -> Self {
        Self {
            functions,
            call: OverlayCall::Value(call),
            tracked: false,
        }
    }

    /// The same observation contract as ForeignFunction::tracked. This does
    /// not make emitted effects cacheable: their owner must record the output.
    pub fn tracked(mut self) -> Self {
        self.tracked = true;
        self
    }

    fn handles(&self, function: CellId) -> bool {
        self.functions.contains(&function)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation<T = RuntimeValue> {
    pub result: T,
    pub remaining_fuel: usize,
    /// False only when evaluation halted, not when it returned an absent value.
    pub completed: bool,
}

impl Evaluation {
    pub fn into_value(self) -> Evaluation<Value> {
        Evaluation {
            result: self.result.into_value(),
            remaining_fuel: self.remaining_fuel,
            completed: self.completed,
        }
    }
}

pub struct Context<'a> {
    host: &'a dyn Host,
    overlay: Option<&'a ForeignOverlay<'a>>,
    foreign_scopes: Vec<ForeignFunctions>,
    effects: u64,
    remaining_fuel: usize,
    resolving: Vec<CellId>,
    compiled: ByNode<Thunk>,
    observer: Option<&'a dyn Observer>,
    cell_states: Vec<CellState>,
    indices: CellIndices,
}

#[derive(Clone)]
struct Lowered {
    source: RuntimeValue,
    origin: Option<OriginId>,
    form: Form,
    fields: Option<Vec<(CellId, Expression)>>,
    elements: Option<Vec<Expression>>,
}

/// A lowered node's generated form: per-node decisions — dispatch,
/// argument lookup, lambda plumbing — made once at generation, then
/// execution is one call. Generating it changes neither fuel nor results.
type Thunk = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

fn thunk(
    body: impl Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt> + 'static,
) -> Thunk {
    Rc::new(body)
}

/// A call node's memory of its last callee: the parameters slice (held
/// so its identity stays valid) and the argument expression this call
/// supplies for each parameter, in declaration order.
struct CallPlan {
    params: Rc<[CellId]>,
    arguments: Rc<[arguments::Argument]>,
    /// The parameters' indices in `table`, the callee environment's, so a
    /// call binds them without looking each one up.
    table: CellIndices,
    indices: Rc<[CellIndex]>,
}

struct PreparedCallTarget {
    kind: PreparedCallTargetKind,
    plan: RefCell<Option<CallPlan>>,
    stages: RefCell<Option<(Prepare, Stage)>>,
}

enum PreparedCallTargetKind {
    Foreign(ForeignFunction),
    /// A primitive with this call site's operands, run directly.
    Primitive(primitive::Primitive, primitive::Operands),
    Value(Expression),
    Missing,
}

#[derive(Clone)]
struct OriginId(Rc<OriginNode>);

enum OriginRoot {
    Input,
    Located(SourceOrigin),
    Cell {
        cell: CellId,
        resolution: Resolution,
    },
}

enum OriginNode {
    Root(OriginRoot),
    Child { parent: OriginId, step: gid::Step },
}

#[derive(Clone)]
enum Form {
    Data,
    Ready(RuntimeValue),
    Cell(CellId),
    Value(Expression),
    Call {
        function: Expression,
    },
    Lambda {
        parameters: LambdaParameters,
        body: Expression,
        fields: Record,
    },
}

#[derive(Clone)]
enum LambdaParameters {
    Valid(Rc<[CellId]>),
    Malformed,
    Invalid(Value),
}

impl LambdaParameters {
    /// `None` when the parameters are not a list; otherwise the parameter
    /// cells, or the first entry that is not one.
    fn parse(parameters: Option<Result<Vec<CellId>, Value>>) -> Self {
        match parameters {
            None => Self::Malformed,
            Some(Ok(parameters)) => Self::Valid(parameters.into()),
            Some(Err(parameter)) => Self::Invalid(parameter),
        }
    }
}

#[derive(Clone)]
enum CellState {
    Unknown,
    Ready(Expression),
    Evaluating { stack_index: usize },
}

#[derive(Clone)]
enum ResolvedForeign {
    Function(ForeignFunction),
    Overlay(CellId),
}

impl<'a> Context<'a> {
    fn conclude(mut self, run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>) -> Evaluation {
        let outcome = self.checked_call(run);
        Evaluation {
            completed: outcome.is_ok(),
            result: outcome.unwrap_or_else(|Halt(result)| RuntimeValue::from(result)),
            remaining_fuel: self.remaining_fuel,
        }
    }

    /// Run an observable write to evaluation-local state. Foreign functions
    /// evaluate arguments and check applicability before entering this operation.
    pub fn effect<T>(&mut self, run: impl FnOnce() -> T) -> T {
        self.host.effect();
        self.effects += 1;
        run()
    }

    fn checked_call(
        &mut self,
        run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>,
    ) -> Result<RuntimeValue, Halt> {
        let before = self.effects;
        let result = run(self)?;
        if self.effects != before && result.declines() {
            Err(Halt(absent::with_detail(
                absent::EFFECTFUL_DECLINE,
                absent::VALUE,
                result.into_value(),
            )))
        } else {
            Ok(result)
        }
    }

    fn run(self, expression: &Value) -> Evaluation {
        self.conclude(|context| {
            let expression = context.lower_source(expression, OriginRoot::Input);
            let environment = context.empty_environment();
            context.eval(expression, &environment)
        })
    }

    fn lower_source(&mut self, value: &Value, root: OriginRoot) -> Expression {
        let origin = OriginId(Rc::new(OriginNode::Root(root)));
        self.lower_with(value, Some(origin))
    }

    fn lower_unattributed_source(&mut self, value: &Value) -> Expression {
        self.lower_with(value, None)
    }

    fn lower_runtime_code(&mut self, value: &RuntimeValue) -> Expression {
        self.lower_runtime_code_at(value, None)
    }

    fn lower_runtime_code_at(
        &mut self,
        value: &RuntimeValue,
        origin: Option<OriginId>,
    ) -> Expression {
        match &value.0 {
            RuntimeValueKind::Data(value) => self.lower_with(value, origin),
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.original() {
                Some(fields) => self.lower_with(&Value::Record(fields.clone()), origin),
                None => runtime_expression(value.clone()),
            },
            RuntimeValueKind::Record(record) => {
                let fields: Vec<_> = record
                    .iter()
                    .map(|(key, value)| {
                        let child = child_origin(origin.clone(), gid::Step::Key(*key));
                        (*key, self.lower_runtime_code_at(value, child))
                    })
                    .collect();
                // A field a kind requires, or why it's missing.
                let required = |field| {
                    lowered_field(&fields, field).unwrap_or_else(|| {
                        missing_field(field, child_origin(origin.clone(), gid::Step::Key(field)))
                    })
                };
                let params = value.field(vocabulary::PARAMS);
                let form = match value.field(isa::ISA).and_then(|kind| kind.as_cell()) {
                    Some(kind) if kind == vocabulary::kind::CALL => Form::Call {
                        function: required(vocabulary::FUNCTION),
                    },
                    Some(kind) if kind == vocabulary::kind::LAMBDA => Form::Lambda {
                        parameters: LambdaParameters::parse(
                            params
                                .as_ref()
                                .and_then(RuntimeValue::list_values)
                                .map(|values| {
                                    values
                                        .map(|v| v.as_cell().ok_or_else(|| v.to_value()))
                                        .collect()
                                }),
                        ),
                        body: required(vocabulary::BODY),
                        fields: record
                            .iter()
                            .map(|(key, value)| (*key, value.to_value()))
                            .collect(),
                    },
                    Some(kind) if kind == vocabulary::kind::VALUE_WRAPPER => {
                        Form::Value(required(vocabulary::VALUE))
                    }
                    _ => Form::Ready(value.clone()),
                };
                Expression(Rc::new(Lowered {
                    source: value.clone(),
                    origin,
                    form,
                    fields: Some(fields),
                    elements: None,
                }))
            }
            RuntimeValueKind::List(values) => {
                let elements = if origin.is_some() {
                    values
                        .iter()
                        .zip(gid::position::spread(values.len()))
                        .map(|(value, position)| {
                            let child = child_origin(origin.clone(), gid::Step::Element(position));
                            self.lower_runtime_code_at(value, child)
                        })
                        .collect()
                } else {
                    values
                        .iter()
                        .map(|value| self.lower_runtime_code(value))
                        .collect()
                };
                Expression(Rc::new(Lowered {
                    source: value.clone(),
                    origin,
                    form: Form::Ready(value.clone()),
                    fields: None,
                    elements: Some(elements),
                }))
            }
            _ => runtime_expression(value.clone()),
        }
    }

    fn lower_with(&mut self, value: &Value, origin: Option<OriginId>) -> Expression {
        let (form, fields, elements) = match value {
            Value::Cell(cell) => (Form::Cell(*cell), None, None),
            Value::Record(fields) => {
                let lower_field = |this: &mut Self, field: CellId, value: &Value| {
                    let child = child_origin(origin.clone(), gid::Step::Key(field));
                    this.lower_with(value, child)
                };
                // Lower every field, reusing an already lowered head.
                let lower_all = |this: &mut Self, head: Option<(CellId, &Expression)>| {
                    fields
                        .iter()
                        .map(|(field, value)| match head {
                            Some((label, lowered)) if label == *field => (*field, lowered.clone()),
                            _ => (*field, lower_field(this, *field, value)),
                        })
                        .collect::<Vec<_>>()
                };
                // A field a kind requires, or why it's missing.
                let required = |this: &mut Self, field: CellId| match fields.get(&field) {
                    Some(value) => lower_field(this, field, value),
                    None => {
                        missing_field(field, child_origin(origin.clone(), gid::Step::Key(field)))
                    }
                };
                // A record is a call, a lambda or a value wrapper only when its
                // `isa` says so.
                match isa::of(fields) {
                    Some(kind) if kind == vocabulary::kind::CALL => {
                        let function = required(self, vocabulary::FUNCTION);
                        let fields = lower_all(self, Some((vocabulary::FUNCTION, &function)));
                        (Form::Call { function }, Some(fields), None)
                    }
                    Some(kind) if kind == vocabulary::kind::LAMBDA => (
                        Form::Lambda {
                            parameters: LambdaParameters::parse(
                                fields
                                    .get(&vocabulary::PARAMS)
                                    .and_then(Value::as_list)
                                    .map(|list| {
                                        list.values()
                                            .map(|v| v.as_cell().ok_or_else(|| v.clone()))
                                            .collect()
                                    }),
                            ),
                            body: required(self, vocabulary::BODY),
                            fields: fields.clone(),
                        },
                        None,
                        None,
                    ),
                    Some(kind) if kind == vocabulary::kind::VALUE_WRAPPER => {
                        let value = required(self, vocabulary::VALUE);
                        let fields = lower_all(self, Some((vocabulary::VALUE, &value)));
                        (Form::Value(value), Some(fields), None)
                    }
                    _ => (
                        // A number or text literal is its native value
                        // from the start, rather than read on every use.
                        match Unboxed::recognize(fields) {
                            Some(unboxed) => {
                                Form::Ready(RuntimeValue::new(RuntimeValueKind::Unboxed(unboxed)))
                            }
                            None => Form::Data,
                        },
                        Some(lower_all(self, None)),
                        None,
                    ),
                }
            }
            Value::List(elements) => (
                Form::Data,
                None,
                Some(
                    elements
                        .iter()
                        .map(|(position, value)| {
                            let child =
                                child_origin(origin.clone(), gid::Step::Element(position.clone()));
                            self.lower_with(value, child)
                        })
                        .collect(),
                ),
            ),
            Value::Blob(_) => (Form::Data, None, None),
        };
        Expression(Rc::new(Lowered {
            source: value.into(),
            origin,
            form,
            fields,
            elements,
        }))
    }

    pub fn value<'b>(&self, expression: &'b Expression) -> &'b Value {
        expression.value()
    }

    pub fn source_origin(&self, expression: &Expression) -> Option<SourceOrigin> {
        source::origin(expression)
    }

    /// Run `call`, telling an observer it's entered and left.
    fn at_call(
        &mut self,
        call: &Expression,
        run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>,
    ) -> Result<RuntimeValue, Halt> {
        match self.observer {
            None => run(self),
            Some(observer) => {
                observer.entered(call);
                let result = run(self);
                observer.left(result.as_ref().ok());
                result
            }
        }
    }

    pub fn eval_to_value(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        self.eval(expression, environment)
            .map(RuntimeValue::into_value)
    }

    pub fn eval_value_runtime(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower_unattributed_source(expression);
        self.eval(expression, environment)
    }

    /// Explicitly interpret runtime-held syntax in this evaluation. Embedded
    /// native closures keep their code origins and lexical captures.
    pub fn eval_runtime_code(
        &mut self,
        expression: &RuntimeValue,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower_runtime_code(expression);
        self.eval(expression, environment)
    }

    pub fn closure(
        &self,
        params: impl IntoIterator<Item = CellId>,
        body: Expression,
        environment: &Environment,
    ) -> RuntimeValue {
        let params: Rc<[CellId]> = params.into_iter().collect();
        let fields = [
            (isa::ISA, Value::from(vocabulary::kind::LAMBDA)),
            (
                vocabulary::PARAMS,
                Value::list(params.iter().copied().map(Value::from)),
            ),
            (vocabulary::BODY, self.value(&body).clone()),
        ]
        .into_iter()
        .collect();
        RuntimeValue::new(RuntimeValueKind::Closure(Closure {
            fields,
            params,
            body,
            environment: environment.clone(),
        }))
    }

    pub fn closure_value(
        &mut self,
        params: impl IntoIterator<Item = CellId>,
        body: Value,
        environment: &Environment,
    ) -> RuntimeValue {
        let body = self.lower_unattributed_source(&body);
        self.closure(params, body, environment)
    }

    /// An environment with no bindings, over this evaluation's index table.
    pub fn empty_environment(&self) -> Environment {
        Environment::with_indices(self.indices.clone())
    }

    /// Decode GID bindings into a shared lexical environment.
    pub fn environment(&self, value: &Value) -> Option<Environment> {
        let fields = value.as_record()?;
        Some(
            self.empty_environment()
                .extended(fields.iter().map(|(cell, value)| (*cell, value.clone()))),
        )
    }

    pub fn eval(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.eval_burned(expression, environment)
    }

    /// Evaluate an expression a caller will read as a number, skipping
    /// the owned `RuntimeValue` round trip when the answer is already
    /// an unboxed f64. Fuel and results match
    /// [`Self::eval`] followed by [`RuntimeValue::as_f64`] exactly.
    pub fn eval_f64(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Option<f64>, Halt> {
        self.burn()?;
        match unboxed_f64(&expression, environment) {
            Some(number) => Ok(Some(number)),
            None => self
                .eval_burned(expression, environment)
                .map(|value| value.as_f64()),
        }
    }

    /// [`Self::eval_f64`], but answering with the value itself when it isn't
    /// an f64, for a caller that reports why.
    pub fn eval_f64_or_value(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<f64, RuntimeValue>, Halt> {
        self.burn()?;
        match unboxed_f64(&expression, environment) {
            Some(number) => Ok(Ok(number)),
            None => self
                .eval_burned(expression, environment)
                .map(|value| value.as_f64().ok_or(value)),
        }
    }

    fn eval_burned(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let compiled = match self.compiled.get(&expression) {
            Some(compiled) => compiled.clone(),
            None => {
                let compiled = self.compile(expression.clone());
                self.compiled.insert(expression, compiled.clone());
                compiled
            }
        };
        compiled(self, environment)
    }

    fn compile(&mut self, expression: Expression) -> Thunk {
        match expression.0.form.clone() {
            Form::Ready(value) => thunk(move |_, _| Ok(value.clone())),
            Form::Data => {
                let value = unbox(RuntimeValue::from(expression.value().clone()));
                thunk(move |_, _| Ok(value.clone()))
            }
            Form::Cell(cell) => {
                let index = cell_index(&self.indices, cell);
                thunk(move |context, environment| context.eval_cell_at(cell, index, environment))
            }
            Form::Value(value) => {
                thunk(move |context, environment| context.eval(value.clone(), environment))
            }
            Form::Call { function } => self.compile_call(&expression, function),
            Form::Lambda {
                parameters,
                body,
                fields,
            } => match parameters {
                LambdaParameters::Valid(params) => thunk(move |_, environment| {
                    Ok(RuntimeValue::new(RuntimeValueKind::Closure(Closure {
                        fields: fields.clone(),
                        params: params.clone(),
                        body: body.clone(),
                        environment: environment.clone(),
                    })))
                }),
                LambdaParameters::Malformed => thunk(move |_, _| {
                    Ok(RuntimeValue::from(absent::with_detail(
                        absent::MALFORMED_LAMBDA,
                        absent::VALUE,
                        Value::record(fields.clone()),
                    )))
                }),
                LambdaParameters::Invalid(parameter) => thunk(move |_, _| {
                    Ok(RuntimeValue::from(absent::with_detail(
                        absent::INVALID_PARAMETER,
                        absent::VALUE,
                        parameter.clone(),
                    )))
                }),
            },
        }
    }

    fn compile_call(&mut self, call: &Expression, function: Expression) -> Thunk {
        let call = call.clone();
        let function_cell = match &function.0.form {
            Form::Cell(cell) => Some((*cell, cell_index(&self.indices, *cell))),
            _ => None,
        };
        let plan: RefCell<Option<CallPlan>> = RefCell::new(None);
        let stages: RefCell<Option<(Prepare, Stage)>> = RefCell::new(None);
        let target: RefCell<Option<Rc<PreparedCallTarget>>> = RefCell::new(None);
        thunk(move |context, environment| {
            context.at_call(&call, |context| {
                if let Some((cell, index)) = function_cell
                    && environment.get_at(&context.indices, cell, index).is_none()
                    && context.foreign_target_cell(cell).is_none()
                {
                    context.call_cell(cell, &call, environment, Some(&target))
                } else {
                    context.checked_call(|context| {
                        let callable = match function_cell {
                            Some((cell, index)) => {
                                context.burn()?;
                                match environment.get_at(&context.indices, cell, index) {
                                    Some(value) => unbox(value.clone()),
                                    // Ordinary cells took the direct path above.
                                    None => RuntimeValue::from(Value::from(cell)),
                                }
                            }
                            None => context.eval(function.clone(), environment)?,
                        };
                        match context.try_call_callable(
                            callable.clone(),
                            &call,
                            environment,
                            &plan,
                            &stages,
                        ) {
                            Some(result) => result,
                            None => Ok(not_callable(&callable)),
                        }
                    })
                }
            })
        })
    }

    fn call_cell(
        &mut self,
        cell: CellId,
        call: &Expression,
        environment: &Environment,
        cache: Option<&RefCell<Option<Rc<PreparedCallTarget>>>>,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        let cached = cache.and_then(|cache| cache.borrow().clone());
        let target = match cached {
            Some(target) => target,
            None => {
                let kind = match self.host.resolve(cell) {
                    Some((_, Definition::Foreign(definition))) => {
                        match &definition.implementation.primitive {
                            Some(primitive) => PreparedCallTargetKind::Primitive(
                                **primitive,
                                primitive.operands(self, call),
                            ),
                            None => {
                                PreparedCallTargetKind::Foreign(definition.implementation.clone())
                            }
                        }
                    }
                    Some((resolution, Definition::Value(value))) => {
                        PreparedCallTargetKind::Value(self.lowered_cell(cell, resolution, &value))
                    }
                    None => PreparedCallTargetKind::Missing,
                };
                let target = Rc::new(PreparedCallTarget {
                    kind,
                    plan: RefCell::new(None),
                    stages: RefCell::new(None),
                });
                if let Some(cache) = cache {
                    *cache.borrow_mut() = Some(target.clone());
                }
                target
            }
        };
        self.checked_call(|context| match &target.kind {
            PreparedCallTargetKind::Foreign(function) => context.call_foreign_staged(
                &ResolvedForeign::Function(function.clone()),
                call,
                environment,
                Some(&target.stages),
            ),
            PreparedCallTargetKind::Value(expression) => {
                let callable = context.eval_definition(cell, expression.clone(), environment)?;
                if callable.absent_reason().is_some() {
                    Ok(callable)
                } else {
                    context
                        .try_call_callable(
                            callable.clone(),
                            call,
                            environment,
                            &target.plan,
                            &target.stages,
                        )
                        .unwrap_or_else(|| Ok(not_callable(&callable)))
                }
            }
            PreparedCallTargetKind::Primitive(primitive, operands) => {
                primitive.run(context, call, operands, environment)
            }
            PreparedCallTargetKind::Missing => Ok(missing_cell(cell)),
        })
    }

    /// `cell`'s definition lowered once per evaluation and shared by every
    /// call site that calls it, so their compiled code and call caches are
    /// shared too.
    fn lowered_cell(&mut self, cell: CellId, resolution: Resolution, value: &Value) -> Expression {
        let index = cell_index(&self.indices, cell);
        while self.cell_states.len() <= index.0 {
            self.cell_states.push(CellState::Unknown);
        }
        match &self.cell_states[index.0] {
            CellState::Ready(expression) => expression.clone(),
            CellState::Evaluating { .. } => {
                self.lower_source(value, OriginRoot::Cell { cell, resolution })
            }
            CellState::Unknown => {
                let expression = self.lower_source(value, OriginRoot::Cell { cell, resolution });
                self.cell_states[index.0] = CellState::Ready(expression.clone());
                expression
            }
        }
    }

    fn eval_definition(
        &mut self,
        cell: CellId,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        match self
            .resolving
            .iter()
            .position(|candidate| *candidate == cell)
        {
            Some(stack_index) => Ok(cell_cycle(&self.resolving[stack_index..], cell)),
            None => {
                self.resolving.push(cell);
                let result = self.eval(expression, environment);
                self.resolving.pop();
                result
            }
        }
    }

    fn try_call_callable(
        &mut self,
        callable: RuntimeValue,
        call: &Expression,
        environment: &Environment,
        plan: &RefCell<Option<CallPlan>>,
        stages: &RefCell<Option<(Prepare, Stage)>>,
    ) -> Option<Result<RuntimeValue, Halt>> {
        match &callable.0 {
            RuntimeValueKind::Closure(closure) => {
                Some(self.eval_grap_call(closure.clone(), call, environment, plan))
            }
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => match self.runtime_closure(&callable) {
                Some(closure) => Some(self.eval_grap_call(closure, call, environment, plan)),
                None => callee_cell(&callable).map(|cell| match self.foreign_target_cell(cell) {
                    Some(foreign) => {
                        self.call_foreign_staged(&foreign, call, environment, Some(stages))
                    }
                    None => self.call_cell(cell, call, environment, None),
                }),
            },
            RuntimeValueKind::List(_) => None,
        }
    }

    /// Fuel is part of Grap's contract, not this implementation's: one
    /// burn per expression evaluation that the plain evaluator and each
    /// function's documented argument consumption would perform. An
    /// implementation shortcut that skips an evaluation must burn its
    /// fuel anyway, as the prepared-call path already does.
    pub fn burn(&mut self) -> Result<(), Halt> {
        self.remaining_fuel = self.remaining_fuel.saturating_sub(1);
        if self.remaining_fuel == 0 {
            Err(Halt(absent::value(absent::FUEL_EXHAUSTED)))
        } else {
            Ok(())
        }
    }

    pub fn field(&self, call: &Expression, label: CellId) -> Option<Expression> {
        lowered_field(call.0.fields.as_ref()?, label)
    }

    pub fn fields<'b>(&self, expression: &'b Expression) -> Option<&'b [(CellId, Expression)]> {
        expression.0.fields.as_deref()
    }

    pub fn elements<'b>(&self, expression: &'b Expression) -> Option<&'b [Expression]> {
        expression.0.elements.as_deref()
    }

    /// Why `call` has no argument under `cell`, naming where the call is.
    pub fn missing_argument(&self, call: &Expression, cell: CellId) -> Value {
        absent::missing_argument(cell, self.source_origin(call).as_ref())
    }

    pub fn missing_runtime_argument(&self, call: &Expression, cell: CellId) -> RuntimeValue {
        self.missing_argument(call, cell).into()
    }

    /// `call`'s argument under `label`, or why it has none.
    pub fn argument(&self, call: &Expression, label: CellId) -> arguments::Argument {
        self.field(call, label)
            .ok_or_else(|| self.missing_runtime_argument(call, label))
    }

    fn eval_cell_at(
        &mut self,
        cell: CellId,
        index: CellIndex,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        match environment.get_at(&self.indices, cell, index) {
            Some(value) => Ok(unbox(value.clone())),
            None => match self.host.resolve(cell) {
                // A function only a scope supplies is a foreign function
                // implemented under its cell.
                None if self.foreign_target_cell(cell).is_some() => {
                    Ok(RuntimeValue::from(foreign_function(cell)))
                }
                None => Ok(missing_cell(cell)),
                Some((resolution, definition)) => {
                    while self.cell_states.len() <= index.0 {
                        self.cell_states.push(CellState::Unknown);
                    }
                    match self.cell_states[index.0].clone() {
                        CellState::Ready(expression) => {
                            self.eval_resolved_cell(index, cell, expression, environment)
                        }
                        CellState::Evaluating { stack_index } => {
                            Ok(cell_cycle(&self.resolving[stack_index..], cell))
                        }
                        CellState::Unknown => {
                            let expression = self.lower_source(
                                definition.value(),
                                OriginRoot::Cell { cell, resolution },
                            );
                            self.cell_states[index.0] = CellState::Ready(expression.clone());
                            self.eval_resolved_cell(index, cell, expression, environment)
                        }
                    }
                }
            },
        }
    }

    fn eval_resolved_cell(
        &mut self,
        index: CellIndex,
        cell: CellId,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let stack_index = self.resolving.len();
        self.cell_states[index.0] = CellState::Evaluating { stack_index };
        self.resolving.push(cell);
        let result = self.eval(expression.clone(), environment);
        self.resolving.pop();
        self.cell_states[index.0] = CellState::Ready(expression);
        result
    }

    fn foreign_target_cell(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.scoped_foreign_target(cell).or_else(|| {
            self.overlay
                .is_some_and(|overlay| overlay.handles(cell))
                .then_some(ResolvedForeign::Overlay(cell))
        })
    }

    fn scoped_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.foreign_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(cell))
            .map(|function| ResolvedForeign::Function(function.clone()))
    }

    fn call_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.call_foreign_staged(foreign, call, environment, None)
    }

    /// Call `foreign` with evaluated arguments, which it receives as the
    /// fields of a call to `callable`.
    fn apply_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        callable: RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let call = self.lower_runtime_code(&RuntimeValue::record(
            [
                (
                    isa::ISA,
                    RuntimeValue::from(Value::from(vocabulary::kind::CALL)),
                ),
                (vocabulary::FUNCTION, callable),
            ]
            .into_iter()
            .chain(arguments),
        ));
        self.call_foreign(foreign, &call, environment)
    }

    fn call_foreign_staged(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
        stages: Option<&RefCell<Option<(Prepare, Stage)>>>,
    ) -> Result<RuntimeValue, Halt> {
        let tracked = match foreign {
            ResolvedForeign::Function(function) => function.tracked,
            ResolvedForeign::Overlay(_) => self.overlay.is_some_and(|overlay| overlay.tracked),
        };
        if !tracked {
            self.host.untracked();
        }
        let value = match foreign {
            ResolvedForeign::Function(function) => match &function.implementation {
                ForeignImplementation::Direct(call_direct) => call_direct(self, call, environment),
                ForeignImplementation::Staged(prepare) => {
                    let cached = stages.and_then(|slot| {
                        slot.borrow().as_ref().and_then(|(cached_prepare, stage)| {
                            Rc::ptr_eq(cached_prepare, prepare).then(|| stage.clone())
                        })
                    });
                    let stage = match cached {
                        Some(stage) => stage,
                        None => {
                            let stage = prepare(self, call);
                            if let Some(slot) = stages {
                                *slot.borrow_mut() = Some((prepare.clone(), stage.clone()));
                            }
                            stage
                        }
                    };
                    stage(self, environment)
                }
            },
            ResolvedForeign::Overlay(cell) => {
                let function = self
                    .overlay
                    .expect("an overlay target came from the active overlay")
                    .call;
                match function {
                    OverlayCall::Runtime(function) => function(*cell, self, call, environment),
                    OverlayCall::Value(function) => {
                        function(*cell, self, call, environment).map(RuntimeValue::from)
                    }
                }
            }
        }?;
        Ok(unbox(value))
    }

    pub fn apply(
        &mut self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        let callable = self.prepare_runtime_callable(function.clone(), &self.empty_environment());
        self.call_prepared(&callable, arguments)
    }

    /// Run synchronously with an additional foreign-function layer.
    /// Its owned closures may carry state local to this evaluation; it
    /// takes calls to its functions before the host's definitions, the
    /// overlay, and the layers it runs within.
    pub fn with_foreign_functions<T>(
        &mut self,
        functions: ForeignFunctions,
        run: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.foreign_scopes.push(functions);
        let result = run(self);
        self.foreign_scopes.pop();
        result
    }

    pub fn prepare_callable(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<PreparedCallable, Halt> {
        let callable = self.eval(expression, environment)?;
        Ok(self.prepare_runtime_callable(callable, environment))
    }

    /// Prepare an already evaluated callable without reifying its closure or
    /// evaluating the value a second time. It owns its retained code and captures.
    pub fn prepare_runtime_callable(
        &mut self,
        callable: RuntimeValue,
        environment: &Environment,
    ) -> PreparedCallable {
        let callable = match &callable.0 {
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => self
                .runtime_closure(&callable)
                .map(|closure| RuntimeValue::new(RuntimeValueKind::Closure(closure)))
                .unwrap_or(callable),
            _ => callable,
        };
        PreparedCallable {
            value: callable,
            environment: environment.clone(),
        }
    }

    pub fn call_prepared(
        &mut self,
        callable: &PreparedCallable,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        self.checked_call(|context| {
            context.burn()?;
            context.burn()?;
            let arguments: Vec<_> = arguments.into_iter().collect();
            match &callable.value.0 {
                RuntimeValueKind::Closure(closure) => {
                    context.call_closure(closure.clone(), &callable.value, &arguments)
                }
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    let value = &callable.value;
                    match callee_cell(value) {
                        Some(cell) => {
                            let call = runtime_call(value.clone(), arguments);
                            match context.foreign_target_cell(cell) {
                                Some(foreign) => {
                                    context.call_foreign(&foreign, &call, &callable.environment)
                                }
                                None => context.call_cell(cell, &call, &callable.environment, None),
                            }
                        }
                        None => Ok(not_callable(value)),
                    }
                }
                RuntimeValueKind::List(_) => Ok(not_callable(&callable.value)),
            }
        })
    }

    /// Apply runtime-held callable syntax. Grap parameters receive values;
    /// foreign functions receive argument expressions to interpret. Use
    /// `apply` when arguments must remain already-evaluated values for FFIs too.
    pub fn apply_expression(
        &mut self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        // The call itself, as a call expression would charge it.
        self.burn()?;
        let arguments = arguments.into_iter().collect();
        let environment = self.empty_environment();
        if let Some(cell) = function.as_cell()
            && self.foreign_target_cell(cell).is_none()
        {
            self.apply_cell(cell, function.clone(), arguments, &environment)
        } else {
            self.checked_call(|context| {
                let callable = if function.as_cell().is_some() {
                    context.burn()?;
                    function.clone()
                } else {
                    let function = match &function.0 {
                        RuntimeValueKind::Data(value) => {
                            context.lower_source(value, OriginRoot::Input)
                        }
                        _ => context.lower_runtime_code(function),
                    };
                    context.eval(function, &environment)?
                };
                match context.try_apply_callable(callable.clone(), &arguments, &environment) {
                    Some(result) => result,
                    None => Ok(not_callable(&callable)),
                }
            })
        }
    }

    fn apply_cell(
        &mut self,
        cell: CellId,
        callable: RuntimeValue,
        arguments: Vec<(CellId, RuntimeValue)>,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.checked_call(|context| match context.host.resolve(cell) {
            Some((_, Definition::Foreign(definition))) => context.apply_foreign(
                &ResolvedForeign::Function(definition.implementation.clone()),
                callable,
                arguments,
                environment,
            ),
            Some((resolution, Definition::Value(value))) => {
                let expression =
                    context.lower_source(&value, OriginRoot::Cell { cell, resolution });
                let callable = context.eval_definition(cell, expression, environment)?;
                if callable.absent_reason().is_some() {
                    Ok(callable)
                } else {
                    context
                        .try_apply_callable(callable.clone(), &arguments, environment)
                        .unwrap_or_else(|| Ok(not_callable(&callable)))
                }
            }
            None => Ok(missing_cell(cell)),
        })
    }

    /// Host calls bind argument values as a call expression binds its
    /// evaluated arguments: one step each, with a `function` parameter
    /// receiving the callee itself. A missing argument binds its absence
    /// without a step, naming no call: a host call has no source.
    fn call_closure(
        &mut self,
        closure: Closure,
        callable: &RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
    ) -> Result<RuntimeValue, Halt> {
        let bound = closure.params.iter().try_fold(
            Vec::with_capacity(closure.params.len()),
            |mut bound, &parameter| {
                let value = if parameter == vocabulary::FUNCTION {
                    Some(callable.clone())
                } else {
                    arguments
                        .iter()
                        .find(|(cell, _)| *cell == parameter)
                        .map(|(_, value)| unbox(value.clone()))
                };
                bound.push((
                    parameter,
                    match value {
                        Some(value) => {
                            self.burn()?;
                            value
                        }
                        None => absent::missing_argument(parameter, None).into(),
                    },
                ));
                Ok::<_, Halt>(bound)
            },
        )?;
        self.eval(closure.body, &closure.environment.extended_runtime(bound))
    }

    fn try_apply_callable(
        &mut self,
        callable: RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
        environment: &Environment,
    ) -> Option<Result<RuntimeValue, Halt>> {
        match self.runtime_closure(&callable) {
            Some(closure) => Some(self.call_closure(closure, &callable, arguments)),
            None => match callable.0 {
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    callee_cell(&callable).map(|cell| match self.foreign_target_cell(cell) {
                        Some(foreign) => self.apply_foreign(
                            &foreign,
                            callable,
                            arguments.iter().cloned(),
                            environment,
                        ),
                        None => self.apply_cell(cell, callable, arguments.to_vec(), environment),
                    })
                }
                RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
            },
        }
    }

    fn eval_grap_call(
        &mut self,
        closure: Closure,
        call: &Expression,
        calling_environment: &Environment,
        plan: &RefCell<Option<CallPlan>>,
    ) -> Result<RuntimeValue, Halt> {
        let cached = plan.borrow().as_ref().and_then(|cached| {
            (Rc::ptr_eq(&cached.params, &closure.params)
                && Rc::ptr_eq(&cached.table, &closure.environment.indices))
            .then(|| (cached.arguments.clone(), cached.indices.clone()))
        });
        let (arguments_plan, indices) = match cached {
            Some(cached) => cached,
            None => {
                let arguments: Rc<[arguments::Argument]> = closure
                    .params
                    .iter()
                    .map(|&parameter| self.argument(call, parameter))
                    .collect();
                let table = closure.environment.indices.clone();
                let indices: Rc<[CellIndex]> = closure
                    .params
                    .iter()
                    .map(|&parameter| binding_index(&table, parameter))
                    .collect();
                *plan.borrow_mut() = Some(CallPlan {
                    params: closure.params.clone(),
                    arguments: arguments.clone(),
                    table,
                    indices: indices.clone(),
                });
                (arguments, indices)
            }
        };
        // A missing argument binds its parameter to its absence, which
        // answers wherever the body uses it, so a body that doesn't still
        // has its value.
        let arguments = arguments_plan.iter().zip(indices.iter()).try_fold(
            Vec::with_capacity(closure.params.len()),
            |mut arguments, (argument, &index)| {
                arguments.push((
                    index,
                    match argument {
                        Ok(expression) => self.eval(expression.clone(), calling_environment)?,
                        Err(missing) => missing.clone(),
                    },
                ));
                Ok::<_, Halt>(arguments)
            },
        )?;
        self.eval(
            closure.body,
            &closure.environment.extended_indexed(arguments),
        )
    }

    fn runtime_closure(&mut self, value: &RuntimeValue) -> Option<Closure> {
        match &value.0 {
            RuntimeValueKind::Closure(closure) => Some(closure.clone()),
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => {
                (value.kind()? == vocabulary::kind::CLOSURE).then_some(())?;
                let lambda = value.field(vocabulary::LAMBDA)?;
                let params = lambda
                    .field(vocabulary::PARAMS)?
                    .list_values()?
                    .map(|value| value.as_cell())
                    .collect::<Option<Rc<[_]>>>()?;
                let origin = value
                    .field(source::vocabulary::BODY_ORIGIN)
                    .and_then(|value| source::read(value.as_value()))
                    .map(|origin| OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(origin)))));
                let body = self.lower_runtime_code_at(&lambda.field(vocabulary::BODY)?, origin);
                let captured = value.field(vocabulary::ENVIRONMENT)?;
                let environment = match &captured.0 {
                    RuntimeValueKind::Data(value) => self.environment(value),
                    RuntimeValueKind::Record(fields) => Some(
                        self.empty_environment()
                            .extended_runtime(fields.iter().cloned()),
                    ),
                    _ => None,
                }?;
                Some(Closure {
                    params,
                    body,
                    environment,
                    fields: lambda.as_value().as_record()?.clone(),
                })
            }
            RuntimeValueKind::List(_) => None,
        }
    }
}

/// `expression`'s f64 when it's already one without evaluating: a number
/// literal, which lowering made native, or a variable bound to one.
fn unboxed_f64(expression: &Expression, environment: &Environment) -> Option<f64> {
    match &expression.0.form {
        // Literals are lowered to their native values.
        Form::Ready(value) => native_f64(value),
        Form::Cell(cell) => environment.get_runtime(*cell).and_then(native_f64),
        _ => None,
    }
}

fn native_f64(value: &RuntimeValue) -> Option<f64> {
    match &value.0 {
        RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
            Native::F64(number) => Some(*number),
            _ => None,
        },
        _ => None,
    }
}

/// A foreign function implemented under `cell`, saying nothing more, as one
/// a scope supplies reads.
pub fn foreign_function(cell: CellId) -> Value {
    isa::record(
        vocabulary::kind::FOREIGN_FUNCTION,
        [(vocabulary::IMPLEMENTATION, Value::from(cell))],
    )
}

/// The cell calling `callable` calls: the cell it is, or the one a foreign
/// function is implemented under.
fn callee_cell(callable: &RuntimeValue) -> Option<CellId> {
    callable.as_cell().or_else(|| {
        (callable.kind()? == vocabulary::kind::FOREIGN_FUNCTION)
            .then(|| callable.field(vocabulary::IMPLEMENTATION)?.as_cell())?
    })
}

/// The absence a call answers when `callable` can't be called: an absent
/// `callable` as it is, since what failed before it is the reason.
fn not_callable(callable: &RuntimeValue) -> RuntimeValue {
    arguments::not(absent::NOT_CALLABLE, callable.clone())
}

/// The absence a reference answers when `cell` has no definition.
fn missing_cell(cell: CellId) -> RuntimeValue {
    RuntimeValue::from(absent::with_detail(
        absent::MISSING_CELL,
        absent::CELL,
        cell.into(),
    ))
}

/// The absence `cell` answers when its own definition reaches it again;
/// `resolving` holds the cells under resolution from `cell` on.
fn cell_cycle(resolving: &[CellId], cell: CellId) -> RuntimeValue {
    RuntimeValue::from(absent::with_detail(
        absent::CELL_CYCLE,
        absent::CYCLE,
        Value::list(resolving.iter().copied().chain([cell]).map(Value::from)),
    ))
}

/// `value` in its unboxed form when it is a record following a
/// privileged convention.
fn unbox(value: RuntimeValue) -> RuntimeValue {
    match &value.0 {
        RuntimeValueKind::Data(Value::Record(fields)) => match Unboxed::recognize(fields) {
            Some(unboxed) => RuntimeValue::new(RuntimeValueKind::Unboxed(unboxed)),
            None => value,
        },
        _ => value,
    }
}

fn child_origin(parent: Option<OriginId>, step: gid::Step) -> Option<OriginId> {
    parent.map(|parent| OriginId(Rc::new(OriginNode::Child { parent, step })))
}

/// A field a construct's kind requires, while it's missing: found where the
/// field would be, answering why it can't be used yet.
fn missing_field(field: CellId, origin: Option<OriginId>) -> Expression {
    let value = RuntimeValue::from(absent::missing_field(field));
    Expression(Rc::new(Lowered {
        source: value.clone(),
        origin,
        form: Form::Ready(value),
        fields: None,
        elements: None,
    }))
}

fn runtime_expression(value: RuntimeValue) -> Expression {
    Expression(Rc::new(Lowered {
        source: value.clone(),
        origin: None,
        form: Form::Ready(value),
        fields: None,
        elements: None,
    }))
}

fn runtime_call(function: RuntimeValue, arguments: Vec<(CellId, RuntimeValue)>) -> Expression {
    let fields: Rc<[_]> = gid::sorted_entries(
        [
            (
                isa::ISA,
                RuntimeValue::from(Value::from(vocabulary::kind::CALL)),
            ),
            (vocabulary::FUNCTION, function),
        ]
        .into_iter()
        .chain(arguments),
    )
    .into();
    let lowered = fields
        .iter()
        .map(|(field, value)| (*field, runtime_expression(value.clone())))
        .collect();
    let value = RuntimeValue::new(RuntimeValueKind::Record(fields));
    Expression(Rc::new(Lowered {
        source: value.clone(),
        origin: None,
        form: Form::Ready(value),
        fields: Some(lowered),
        elements: None,
    }))
}

pub fn lambda(params: impl IntoIterator<Item = CellId>, body: Value) -> Value {
    isa::record(
        vocabulary::kind::LAMBDA,
        [
            (
                vocabulary::PARAMS,
                Value::list(params.into_iter().map(Value::from)),
            ),
            (vocabulary::BODY, body),
        ],
    )
}

pub fn call(function: Value, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    isa::record(
        vocabulary::kind::CALL,
        [(vocabulary::FUNCTION, function)]
            .into_iter()
            .chain(arguments),
    )
}

fn context<'a>(
    host: &'a dyn Host,
    overlay: Option<&'a ForeignOverlay<'a>>,
    fuel: usize,
) -> Context<'a> {
    Context {
        host,
        overlay,
        foreign_scopes: Vec::new(),
        effects: 0,
        remaining_fuel: fuel,
        resolving: Vec::new(),
        compiled: Default::default(),
        observer: host.observer(),
        cell_states: Vec::new(),
        indices: CellIndices::default(),
    }
}

/// Evaluate syntax read from the input; its source origins are input paths.
pub fn evaluate(expression: &Value, host: &dyn Host, fuel: usize) -> Evaluation {
    context(host, None, fuel).run(expression)
}

/// Interpret runtime-held syntax, rooting its source origins at `origin`;
/// without one, it has none. Embedded native callables retain their own
/// code origins and lexical captures.
pub fn evaluate_runtime_at(
    expression: &RuntimeValue,
    origin: Option<SourceOrigin>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation {
    context(host, None, fuel).conclude(|context| {
        let origin =
            origin.map(|origin| OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(origin)))));
        let expression = context.lower_runtime_code_at(expression, origin);
        let environment = context.empty_environment();
        context.eval(expression, &environment)
    })
}

/// Apply a callable expression at the host boundary: Grap parameters
/// receive values, while foreign functions receive argument syntax to
/// interpret. An `overlay` lends foreign functions for this synchronous
/// evaluation only. Use `apply` when the callable and all arguments have
/// already been evaluated.
pub fn apply_expression(
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    overlay: Option<&ForeignOverlay<'_>>,
    fuel: usize,
) -> Evaluation {
    context(host, overlay, fuel).conclude(|context| context.apply_expression(function, arguments))
}

/// Apply a callable to already-evaluated argument VALUES. This is the
/// host boundary: a code-shaped value (a stored lambda or call
/// record) binds as data, where `call` + [`evaluate`] would evaluate
/// it as an expression. A foreign target still receives the
/// arguments as call fields and evaluates them itself; every value
/// but a code-shaped one self-quotes through that. An `overlay` lends
/// foreign functions for this synchronous evaluation only.
pub fn apply_value(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    host: &dyn Host,
    overlay: Option<&ForeignOverlay<'_>>,
    fuel: usize,
) -> Evaluation<Value> {
    context(host, overlay, fuel)
        .conclude(|context| {
            context.apply_expression(
                &function.into(),
                arguments.into_iter().map(|(k, v)| (k, v.into())),
            )
        })
        .into_value()
}

pub fn evaluate_value(expression: &Value, host: &dyn Host, fuel: usize) -> Evaluation<Value> {
    evaluate(expression, host, fuel).into_value()
}

/// Interpret runtime-held syntax without serializing its embedded
/// callables. Syntax held as GID data has input paths as its source
/// origins, as through [`evaluate`]; generated syntax has none. `overlay`
/// lends foreign functions for this synchronous evaluation only.
pub fn evaluate_runtime_scoped<'a>(
    expression: &RuntimeValue,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    match &expression.0 {
        RuntimeValueKind::Data(value) => context(host, Some(overlay), fuel).run(value),
        _ => context(host, Some(overlay), fuel).conclude(|context| {
            let environment = context.empty_environment();
            context.eval_runtime_code(expression, &environment)
        }),
    }
}

/// Apply an evaluated callable to evaluated arguments. An `overlay`
/// lends foreign functions for this synchronous evaluation only.
pub fn apply(
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    overlay: Option<&ForeignOverlay<'_>>,
    fuel: usize,
) -> Evaluation {
    context(host, overlay, fuel).conclude(|context| context.apply(function, arguments))
}

#[cfg(test)]
struct TestHost<F>(F);

#[cfg(test)]
impl<F: Fn(CellId) -> Vec<(Resolution, Definition)>> Host for TestHost<F> {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)> {
        (self.0)(cell).into_iter().next()
    }
}

#[cfg(test)]
mod tests;
