//! A small evaluator of GID expressions, returning owned runtime values
//! with an explicit conversion back to GID. It recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use gid::{CellId, Record, Resolution, Value};
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::rc::Rc;

#[cfg(test)]
mod effect_tests;

#[cfg(test)]
mod runtime_tests;

pub mod memo;
mod reify;

pub mod vocabulary {
    use gid::CellId;

    pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
    pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
    pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
    pub const VALUE: CellId = CellId::from_u128(0x5adde9ececa6ad57c81a3b2907e75e08);
    pub const CLOSURE: CellId = CellId::from_u128(0xdb39600f3ed07398c77ac120deb108a8);
    pub const ENVIRONMENT: CellId = CellId::from_u128(0xe910025c710c25c43d0a5b296378f374);
    pub const FFI: CellId = CellId::from_u128(0x912adb7252d689659b6de9eeeb827658);
    pub const EVALUATE: CellId = CellId::from_u128(0xacfc5e50881292518dab3cec77cf43ee);
    pub const EXPRESSION: CellId = CellId::from_u128(0xccc55b0eb63b9f564ea74436094d4014);
}

pub mod absent {
    use gid::{CellId, Value};

    pub const ABSENT: CellId = CellId::from_u128(0xd9c0a7145a38859a245640d3469cbcd4);
    pub const FUEL_EXHAUSTED: CellId = CellId::from_u128(0x513628d759c04b3e7088b575e555a80e);
    pub const MISSING_CELL: CellId = CellId::from_u128(0xa5a1b4e3d0df96bd11af00f0780136ff);
    pub const CELL_CYCLE: CellId = CellId::from_u128(0x150e0fc7e38d1670f41283c3d23a9b8d);
    pub const MALFORMED_LAMBDA: CellId = CellId::from_u128(0xfbf5894d7f62b6d0048d17d26851b415);
    pub const INVALID_PARAMETER: CellId = CellId::from_u128(0x93ca0e9199372c46ee24bd4c508e178c);
    pub const NOT_CALLABLE: CellId = CellId::from_u128(0x8624488c2d10d2a4b84560dfa99a38e6);
    pub const MISSING_ARGUMENT: CellId = CellId::from_u128(0x8b2f0db36e5c3d35595eb5666cc89c78);
    pub const INVALID_ENVIRONMENT: CellId = CellId::from_u128(0x152f2cac01f072317ab5746c5befdf9c);
    pub const NO_ALTERNATIVE: CellId = CellId::from_u128(0x3bf0543fa73a0cee9036317cdb5cacd9);
    pub const EFFECTFUL_DECLINE: CellId = CellId::from_u128(0xdc2651b863aa8aaf2df25ccb0c1fef03);
    pub const DECLINED: CellId = CellId::from_u128(0x1cab38a2c8cffe5c077adc29f74c2ddf);
    pub const CAUSES: CellId = CellId::from_u128(0x2f34365ec4dce76324f482c08afe6aba);

    pub const CELL: CellId = CellId::from_u128(0x7ba69298ef92cf954456784b66e86032);
    pub const VALUE: CellId = CellId::from_u128(0xcfeaf191b71e06afef86c27ee43eb936);
    pub const CYCLE: CellId = CellId::from_u128(0x2776d28585d66ef8a747a8bc6dad7381);

    pub fn with_detail(reason: CellId, field: CellId, detail: Value) -> Value {
        Value::record([(ABSENT, Value::from(reason)), (field, detail)])
    }

    pub fn value(reason: CellId) -> Value {
        Value::record([(ABSENT, Value::from(reason))])
    }

    pub fn reason(value: &Value) -> Option<CellId> {
        value.as_record()?.get(&ABSENT).and_then(Value::as_cell)
    }

    pub fn is_absent(value: &Value) -> bool {
        reason(value).is_some()
    }

    pub fn decline() -> Value {
        value(DECLINED)
    }

    pub fn declines(value: &Value) -> bool {
        reason(value) == Some(DECLINED)
    }

    pub fn from_causes(causes: impl IntoIterator<Item = Value>) -> Value {
        match <[Value; 1]>::try_from(causes.into_iter().collect::<Vec<_>>()) {
            Ok([cause]) => cause,
            Err(causes) => Value::record([
                (ABSENT, Value::from(NO_ALTERNATIVE)),
                (CAUSES, Value::list(causes)),
            ]),
        }
    }
}

/// The f64 number convention, privileged so native numbers stay
/// unboxed through calls, containers, and environments. Privileged
/// knowledge is an accelerator only: the convention must remain
/// expressible as an ordinary external library, which would evaluate
/// correctly and merely lose the fast paths.
pub mod f64 {
    use gid::{CellId, Value};

    pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);

    pub fn value(number: f64) -> Value {
        Value::record([(F64, Value::from(number.to_le_bytes().to_vec()))])
    }

    pub fn read(value: &Value) -> Option<f64> {
        value
            .as_record()?
            .get(&F64)
            .and_then(Value::as_blob)
            .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
            .map(f64::from_le_bytes)
    }
}

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

/// Where a source expression came from before evaluation. Generated
/// runtime values have no origin; expressions read from the input or a
/// cell retain their structural route for host tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    Input(Vec<gid::Step>),
    Stored(Vec<gid::Step>),
    Cell {
        cell: CellId,
        source: Resolution,
        path: Vec<gid::Step>,
    },
}

/// Captured source-bearing calls, innermost first. Nodes share caller ancestry
/// and source locations, but retain neither environments nor evaluator storage.
#[derive(Clone, Debug)]
pub struct CallTrace(Rc<CallTraceNode>);

#[derive(Debug)]
struct CallTraceNode {
    origin: Rc<SourceOrigin>,
    caller: Option<CallTrace>,
}

impl CallTrace {
    pub fn origins(&self) -> impl Iterator<Item = &SourceOrigin> {
        std::iter::successors(Some(self), |trace| trace.0.caller.as_ref())
            .map(|trace| trace.0.origin.as_ref())
    }
}

impl PartialEq for CallTrace {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || self.origins().eq(other.origins())
    }
}

impl Eq for CallTrace {}

struct ActiveCall {
    expression: Expression,
    // The outer option means this prefix has been captured, even if none of
    // its calls has an origin (e.g. a host-generated call).
    captured: Option<Option<CallTrace>>,
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
}

/// One host definition. Native implementations retain their ordinary
/// descriptive value; reading a definition never invokes native code.
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
    pub fn foreign(value: Value, implementation: ForeignFunction) -> Self {
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

pub trait Host {
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)>;
    fn observe(&self, _observation: incremental::Observation) {}
    fn untracked(&self) {}
    fn effect(&self) {}
}

impl ForeignFunction {
    pub fn from_value(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<Value, Halt> + 'static,
    ) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Direct(Rc::new(
                move |context, expression, environment| {
                    call(context, expression, environment).map(RuntimeValue::from_value)
                },
            )),
        }
    }

    pub fn new(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt> + 'static,
    ) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Direct(Rc::new(call)),
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
        }
    }

    /// Opt in only when all changing reads use Context::read/evaluation and
    /// all observable writes use Context::effect. Captures must be immutable
    /// or evaluation-local; hidden mutable state prevents sound memoization.
    pub fn tracked(mut self) -> Self {
        self.tracked = true;
        self
    }
}

pub struct Halt(Value);

/// An owned evaluated value. Closures retain shared code and lexical captures;
/// containers retain lowered children. No creating Context or host is retained.
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
    F64(RuntimeF64),
    Record(Rc<[(CellId, RuntimeValue)]>),
    List(Rc<[RuntimeValue]>),
    Foreign(CellId),
    Closure(Closure),
}

#[derive(Clone)]
struct RuntimeF64 {
    number: f64,
    /// The exact source value, kept so metadata beside the number
    /// survives pass-through unchanged.
    original: Option<Value>,
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
    fields: Record,
    params: Rc<[Parameter]>,
    body: Expression,
    environment: Environment,
}

#[derive(Clone, Copy)]
struct Parameter {
    cell: CellId,
}

impl RuntimeValue {
    /// Conservative equality for memo reuse, including retained code identity.
    pub fn same_result(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (RuntimeValueKind::Data(a), RuntimeValueKind::Data(b)) => a == b,
            (RuntimeValueKind::F64(a), RuntimeValueKind::F64(b)) => {
                a.number.to_bits() == b.number.to_bits() && a.original == b.original
            }
            (RuntimeValueKind::Foreign(a), RuntimeValueKind::Foreign(b)) => a == b,
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

    /// Explicit borrowed GID view for consumers that have not adopted runtime values.
    pub fn as_value(&self) -> &Value {
        match &self.0 {
            RuntimeValueKind::Data(value) => value,
            _ => self.1.get_or_init(|| Rc::new(self.to_value())),
        }
    }

    pub fn from_value(value: Value) -> Self {
        Self::new(RuntimeValueKind::Data(value))
    }

    pub fn f64(number: f64) -> Self {
        Self::new(RuntimeValueKind::F64(RuntimeF64 {
            number,
            original: None,
        }))
    }

    pub fn record(fields: impl IntoIterator<Item = (CellId, RuntimeValue)>) -> Self {
        let mut fields: Vec<_> = fields.into_iter().collect();
        if fields.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            return Self::new(RuntimeValueKind::Record(fields.into()));
        }
        fields.sort_by_key(|(field, _)| *field);
        let fields = fields.into_iter().fold(Vec::new(), |mut unique, field| {
            if unique
                .last()
                .is_some_and(|(last, _): &(CellId, RuntimeValue)| *last == field.0)
            {
                unique.pop();
            }
            unique.push(field);
            unique
        });
        Self::new(RuntimeValueKind::Record(fields.into()))
    }

    pub fn list(elements: impl IntoIterator<Item = RuntimeValue>) -> Self {
        Self::new(RuntimeValueKind::List(elements.into_iter().collect()))
    }

    fn original_f64(number: f64, original: Value) -> Self {
        Self::new(RuntimeValueKind::F64(RuntimeF64 {
            number,
            original: Some(original),
        }))
    }

    pub fn as_f64(&self) -> Option<f64> {
        match &self.0 {
            RuntimeValueKind::F64(value) => Some(value.number),
            RuntimeValueKind::Data(value) => crate::f64::read(value),
            RuntimeValueKind::Record(_) => {
                let bytes = self.field(crate::f64::F64)?;
                Some(f64::from_le_bytes(bytes.as_blob()?.try_into().ok()?))
            }
            RuntimeValueKind::List(_)
            | RuntimeValueKind::Foreign(_)
            | RuntimeValueKind::Closure(_) => None,
        }
    }

    pub fn field(&self, field: CellId) -> Option<RuntimeValue> {
        match &self.0 {
            RuntimeValueKind::Data(value) => {
                value.as_record()?.get(&field).cloned().map(Self::from)
            }
            RuntimeValueKind::Record(fields) => fields
                .binary_search_by_key(&field, |(label, _)| *label)
                .ok()
                .map(|index| fields[index].1.clone()),
            // An unboxed number answers like its record form would:
            // the acceleration must not change what a value is.
            RuntimeValueKind::F64(value) => match &value.original {
                Some(original) => original.as_record()?.get(&field).cloned().map(Self::from),
                None => (field == crate::f64::F64)
                    .then(|| Self::from_value(Value::from(value.number.to_le_bytes().to_vec()))),
            },
            RuntimeValueKind::Foreign(foreign) => {
                (field == vocabulary::FFI).then(|| Self::from(Value::from(*foreign)))
            }
            RuntimeValueKind::Closure(_) if field == vocabulary::CLOSURE => self
                .to_value()
                .as_record()?
                .get(&field)
                .cloned()
                .map(Self::from),
            RuntimeValueKind::Closure(_) => None,
            RuntimeValueKind::List(_) => None,
        }
    }

    pub fn as_cell(&self) -> Option<CellId> {
        match &self.0 {
            RuntimeValueKind::Data(value) => value.as_cell(),
            RuntimeValueKind::F64(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::List(_)
            | RuntimeValueKind::Foreign(_)
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

    pub fn is_absent(&self) -> bool {
        self.field(absent::ABSENT)
            .and_then(|reason| reason.as_cell())
            .is_some()
    }

    pub fn declines(&self) -> bool {
        self.field(absent::ABSENT)
            .and_then(|reason| reason.as_cell())
            == Some(absent::DECLINED)
    }

    pub fn record_len(&self) -> Option<usize> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_record()?.len()),
            RuntimeValueKind::Record(fields) => Some(fields.len()),
            RuntimeValueKind::F64(value) => Some(match &value.original {
                Some(original) => original.as_record()?.len(),
                None => 1,
            }),
            RuntimeValueKind::Foreign(_) | RuntimeValueKind::Closure(_) => Some(1),
            RuntimeValueKind::List(_) => None,
        }
    }

    /// Declared parameter order, without evaluating syntax or materializing a
    /// native closure's code and captured environment.
    pub fn callable_parameters(&self) -> Option<Vec<CellId>> {
        if let RuntimeValueKind::Closure(closure) = &self.0 {
            return Some(
                closure
                    .params
                    .iter()
                    .map(|parameter| parameter.cell)
                    .collect(),
            );
        }
        let fields = self.field(vocabulary::CLOSURE);
        let fields = fields.as_ref().unwrap_or(self);
        fields.field(vocabulary::BODY)?;
        fields
            .field(vocabulary::PARAMS)?
            .list_values()?
            .map(|parameter| parameter.as_cell())
            .collect()
    }

    pub fn list_len(&self) -> Option<usize> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_list()?.len()),
            RuntimeValueKind::List(elements) => Some(elements.len()),
            RuntimeValueKind::F64(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::Foreign(_)
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
            RuntimeValueKind::Record(fields) => fields
                .binary_search_by_key(&field, |(label, _)| *label)
                .is_ok(),
            RuntimeValueKind::F64(value) => match &value.original {
                Some(value) => value
                    .as_record()
                    .is_some_and(|fields| fields.contains_key(&field)),
                None => field == crate::f64::F64,
            },
            RuntimeValueKind::Foreign(_) => field == vocabulary::FFI,
            RuntimeValueKind::Closure(_) => field == vocabulary::CLOSURE,
            RuntimeValueKind::List(_) => false,
        }
    }

    /// The same labels as the GID representation, without materializing values.
    pub fn record_keys(&self) -> Option<Vec<CellId>> {
        Some(match &self.0 {
            RuntimeValueKind::Data(value) => value.as_record()?.keys().copied().collect(),
            RuntimeValueKind::Record(fields) => fields.iter().map(|(key, _)| *key).collect(),
            RuntimeValueKind::F64(value) => match &value.original {
                Some(value) => value.as_record()?.keys().copied().collect(),
                None => vec![crate::f64::F64],
            },
            RuntimeValueKind::Foreign(_) => vec![vocabulary::FFI],
            RuntimeValueKind::Closure(_) => vec![vocabulary::CLOSURE],
            RuntimeValueKind::List(_) => return None,
        })
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
                .values()
                .nth(index)
                .cloned()
                .map(Self::from),
            RuntimeValueKind::List(elements) => elements.get(index).cloned(),
            RuntimeValueKind::F64(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::Foreign(_)
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
            RuntimeValueKind::F64(value) => value
                .original
                .unwrap_or_else(|| crate::f64::value(value.number)),
            _ => self.to_value(),
        }
    }
}

impl From<Value> for RuntimeValue {
    fn from(value: Value) -> Self {
        Self::from_value(value)
    }
}

impl From<&Value> for RuntimeValue {
    fn from(value: &Value) -> Self {
        Self::from_value(value.clone())
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
}

impl CellIndexTable {
    fn intern(&mut self, cell: CellId) -> CellIndex {
        if let Some(index) = self.by_cell.get(&cell) {
            *index
        } else {
            let index = CellIndex(self.cells.len());
            self.cells.push(cell);
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

fn lowered_field(fields: &[(CellId, Expression)], label: CellId) -> Option<Expression> {
    fields
        .binary_search_by_key(&label, |(field, _)| *field)
        .ok()
        .map(|index| fields[index].1.clone())
}

#[derive(Debug, Clone)]
pub struct Environment {
    indices: CellIndices,
    frame: Option<Rc<EnvironmentFrame>>,
}

#[derive(Debug, Clone)]
struct EnvironmentFrame {
    parent: Option<Rc<EnvironmentFrame>>,
    bindings: Vec<(CellIndex, RuntimeValue)>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            indices: CellIndices::default(),
            frame: None,
        }
    }
}

impl PartialEq for Environment {
    fn eq(&self, other: &Self) -> bool {
        Value::from(self) == Value::from(other)
    }
}

impl Eq for Environment {}

impl Environment {
    fn with_indices(indices: CellIndices) -> Self {
        Self {
            indices,
            frame: None,
        }
    }

    pub fn get(&self, cell: CellId) -> Option<Value> {
        self.get_runtime(cell).map(RuntimeValue::to_value)
    }

    pub fn get_runtime(&self, cell: CellId) -> Option<&RuntimeValue> {
        let index = self.indices.borrow().index(cell)?;
        self.get_index(index)
    }

    fn get_index(&self, index: CellIndex) -> Option<&RuntimeValue> {
        let mut frame = self.frame.as_deref();
        while let Some(current) = frame {
            if let Some((_, value)) = current
                .bindings
                .iter()
                .rev()
                .find(|(binding, _)| *binding == index)
            {
                return Some(value);
            }
            frame = current.parent.as_deref();
        }
        None
    }

    pub fn extended(&self, bindings: impl IntoIterator<Item = (CellId, Value)>) -> Self {
        self.extended_runtime(
            bindings
                .into_iter()
                .map(|(cell, value)| (cell, RuntimeValue::from_value(value))),
        )
    }

    pub fn extended_runtime(
        &self,
        bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Self {
        self.extended_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (cell_index(&self.indices, cell), value)),
        )
    }

    /// Add bindings at the innermost level in place, copying the frame
    /// only while another environment still shares it. Lookup, equality,
    /// and reification are the same as through [`Self::extended_runtime`];
    /// sequential binders avoid a frame allocation per binding.
    pub fn push_runtime(&mut self, bindings: impl IntoIterator<Item = (CellId, RuntimeValue)>) {
        let indices = self.indices.clone();
        self.push_indexed(
            bindings
                .into_iter()
                .map(|(cell, value)| (cell_index(&indices, cell), value)),
        );
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
            return self.clone();
        }
        Self {
            indices: self.indices.clone(),
            frame: Some(Rc::new(EnvironmentFrame {
                parent: self.frame.clone(),
                bindings,
            })),
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

impl TryFrom<&Value> for Environment {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, ()> {
        let fields = value.as_record().ok_or(())?;
        let indices = CellIndices::default();
        Ok(Environment::with_indices(indices)
            .extended(fields.iter().map(|(cell, value)| (*cell, value.clone()))))
    }
}

impl TryFrom<Value> for Environment {
    type Error = ();

    fn try_from(value: Value) -> Result<Self, ()> {
        let Value::Record(fields) = value else {
            return Err(());
        };
        let indices = CellIndices::default();
        Ok(Environment::with_indices(indices).extended(fields))
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

type ScopedCall<'a, T> = dyn for<'context> Fn(CellId, &mut Context<'context>, &Expression, &Environment) -> Result<T, Halt>
    + 'a;

#[derive(Clone, Copy)]
enum OverlayCall<'a> {
    Runtime(&'a ScopedCall<'a, RuntimeValue>),
    Value(&'a ScopedCall<'a, Value>),
}

/// A synchronous, borrowed layer of foreign functions. It lets a host
/// expose state that is valid only for one evaluation without putting
/// that state in `'static` closures or rebuilding the permanent table.
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
    compiled: std::collections::HashMap<Expression, Thunk>,
    data_runtime: std::collections::HashMap<Expression, RuntimeValue>,
    calls: Vec<ActiveCall>,
    call_origins: std::collections::HashMap<Expression, Option<Rc<SourceOrigin>>>,
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
/// execution is one call. Fuel and results match the
/// per-visit interpretation these closures replaced.
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
    params: Rc<[Parameter]>,
    arguments: Rc<[Option<Expression>]>,
}

struct PreparedCallTarget {
    kind: PreparedCallTargetKind,
    plan: RefCell<Option<CallPlan>>,
    stages: RefCell<Option<(Prepare, Stage)>>,
}

enum PreparedCallTargetKind {
    Foreign(ForeignFunction),
    Value(Expression),
    Missing,
}

#[derive(Clone)]
struct OriginId(Rc<OriginNode>);

enum OriginRoot {
    Input,
    Located(SourceOrigin),
    Cell { cell: CellId, source: Resolution },
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
    },
}

#[derive(Clone)]
enum LambdaParameters {
    Valid(Rc<[Parameter]>),
    Malformed,
    Invalid(Value),
}

#[derive(Clone)]
enum CellState {
    Unknown,
    Ready(Expression),
    Evaluating { stack_index: usize },
}

#[derive(Clone)]
enum ResolvedForeign {
    Permanent(ForeignFunction),
    Scoped(CellId),
}

impl<'a> Context<'a> {
    fn conclude(mut self, run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>) -> Evaluation {
        let outcome = self.checked_call(run);
        if let Err(Halt(result)) = &outcome
            && absent::reason(result) == Some(absent::EFFECTFUL_DECLINE)
        {
            eprintln!(
                "Grap: cannot decline after performing an effect; evaluation stopped: {result:?}"
            );
        }
        let completed = outcome.is_ok();
        let result = outcome.unwrap_or_else(|Halt(result)| RuntimeValue::from(result));
        Evaluation {
            result,
            remaining_fuel: self.remaining_fuel,
            completed,
        }
    }

    /// Run an observable write to evaluation-local state. Foreign functions
    /// evaluate arguments and check applicability before entering this operation.
    pub fn effect<T>(&mut self, run: impl FnOnce() -> T) -> T {
        self.host.effect();
        self.effects += 1;
        run()
    }

    pub fn read<T: 'static>(&self, input: &incremental::Input<T>) -> Rc<T> {
        let (value, observation) = input.observed();
        self.host.observe(observation);
        value
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
            let environment = Environment::with_indices(context.indices.clone());
            context.eval(expression, &environment)
        })
    }

    fn lower(&mut self, value: &Value) -> Expression {
        self.lower_with(value, false, None)
    }

    fn lower_source(&mut self, value: &Value, root: OriginRoot) -> Expression {
        let origin = OriginId(Rc::new(OriginNode::Root(root)));
        self.lower_with(value, true, Some(origin))
    }

    fn lower_unattributed_source(&mut self, value: &Value) -> Expression {
        self.lower_with(value, true, None)
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
            RuntimeValueKind::Data(value)
            | RuntimeValueKind::F64(RuntimeF64 {
                original: Some(value),
                ..
            }) => self.lower_with(value, true, origin),
            RuntimeValueKind::Record(fields) => {
                let fields: Vec<_> = fields
                    .iter()
                    .map(|(key, value)| {
                        let child = self.child_origin(origin.clone(), gid::Step::Key(*key));
                        (*key, self.lower_runtime_code_at(value, child))
                    })
                    .collect();
                let form = if let Some(function) = lowered_field(&fields, vocabulary::FUNCTION) {
                    Form::Call { function }
                } else if let (Some(params), Some(body)) = (
                    value.field(vocabulary::PARAMS),
                    lowered_field(&fields, vocabulary::BODY),
                ) {
                    let parameters = match params.list_values() {
                        None => LambdaParameters::Malformed,
                        Some(values) => {
                            match values
                                .map(|v| {
                                    v.as_cell()
                                        .map(|cell| Parameter { cell })
                                        .ok_or_else(|| v.to_value())
                                })
                                .collect::<Result<Vec<_>, _>>()
                            {
                                Ok(params) => LambdaParameters::Valid(params.into()),
                                Err(value) => LambdaParameters::Invalid(value),
                            }
                        }
                    };
                    Form::Lambda { parameters, body }
                } else if let Some(value) = lowered_field(&fields, vocabulary::VALUE) {
                    Form::Value(value)
                } else {
                    Form::Ready(value.clone())
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
                            let child =
                                self.child_origin(origin.clone(), gid::Step::Element(position));
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

    fn child_origin(&mut self, parent: Option<OriginId>, step: gid::Step) -> Option<OriginId> {
        parent.map(|parent| OriginId(Rc::new(OriginNode::Child { parent, step })))
    }

    fn lower_with(
        &mut self,
        value: &Value,
        descend_data: bool,
        origin: Option<OriginId>,
    ) -> Expression {
        let mut lowered_fields = None;
        let mut lowered_elements = None;
        let form = match value {
            Value::Cell(cell) => Form::Cell(*cell),
            Value::Record(fields) if fields.contains_key(&vocabulary::FUNCTION) => {
                let fields: Vec<_> = fields
                    .iter()
                    .map(|(field, value)| {
                        let child = self.child_origin(origin.clone(), gid::Step::Key(*field));
                        (*field, self.lower_with(value, descend_data, child))
                    })
                    .collect();
                let function = lowered_field(&fields, vocabulary::FUNCTION)
                    .expect("the source record contains a function field");
                lowered_fields = Some(fields);
                Form::Call { function }
            }
            Value::Record(fields)
                if fields.contains_key(&vocabulary::PARAMS)
                    && fields.contains_key(&vocabulary::BODY) =>
            {
                let parameters = match fields.get(&vocabulary::PARAMS).unwrap().as_list() {
                    None => LambdaParameters::Malformed,
                    Some(parameters) => {
                        let mut parsed = Vec::with_capacity(parameters.len());
                        let mut invalid = None;
                        for parameter in parameters.values() {
                            match parameter.as_cell() {
                                Some(cell) => parsed.push(Parameter { cell }),
                                None => {
                                    invalid = Some(parameter.clone());
                                    break;
                                }
                            }
                        }
                        match invalid {
                            Some(parameter) => LambdaParameters::Invalid(parameter),
                            None => LambdaParameters::Valid(parsed.into()),
                        }
                    }
                };
                Form::Lambda {
                    parameters,
                    body: {
                        let child =
                            self.child_origin(origin.clone(), gid::Step::Key(vocabulary::BODY));
                        self.lower_with(fields.get(&vocabulary::BODY).unwrap(), descend_data, child)
                    },
                }
            }
            Value::Record(fields) if fields.contains_key(&vocabulary::VALUE) => {
                let fields: Vec<_> = fields
                    .iter()
                    .map(|(field, value)| {
                        let child = self.child_origin(origin.clone(), gid::Step::Key(*field));
                        (*field, self.lower_with(value, descend_data, child))
                    })
                    .collect();
                let value = lowered_field(&fields, vocabulary::VALUE)
                    .expect("the source record contains a value field");
                lowered_fields = Some(fields);
                Form::Value(value)
            }
            Value::Record(fields) => {
                if descend_data {
                    lowered_fields = Some(
                        fields
                            .iter()
                            .map(|(field, value)| {
                                let child =
                                    self.child_origin(origin.clone(), gid::Step::Key(*field));
                                (*field, self.lower_with(value, true, child))
                            })
                            .collect(),
                    );
                }
                Form::Data
            }
            Value::List(elements) => {
                if descend_data {
                    lowered_elements = Some(
                        elements
                            .iter()
                            .map(|(position, value)| {
                                let child = self.child_origin(
                                    origin.clone(),
                                    gid::Step::Element(position.clone()),
                                );
                                self.lower_with(value, true, child)
                            })
                            .collect(),
                    );
                }
                Form::Data
            }
            Value::Blob(_) => Form::Data,
        };
        Expression(Rc::new(Lowered {
            source: value.into(),
            origin,
            form,
            fields: lowered_fields,
            elements: lowered_elements,
        }))
    }

    pub fn value<'b>(&self, expression: &'b Expression) -> &'b Value {
        expression.value()
    }

    pub fn source_origin(&self, expression: &Expression) -> Option<SourceOrigin> {
        let mut origin = expression.0.origin.as_ref()?;
        let mut path = Vec::new();
        loop {
            match origin.0.as_ref() {
                OriginNode::Root(OriginRoot::Located(base)) => {
                    path.reverse();
                    let mut base = base.clone();
                    let prefix = match &mut base {
                        SourceOrigin::Input(path) | SourceOrigin::Stored(path) => path,
                        SourceOrigin::Cell { path, .. } => path,
                    };
                    prefix.extend(path);
                    return Some(base);
                }
                OriginNode::Root(OriginRoot::Input) => {
                    path.reverse();
                    return Some(SourceOrigin::Input(path));
                }
                OriginNode::Root(OriginRoot::Cell { cell, source }) => {
                    path.reverse();
                    return Some(SourceOrigin::Cell {
                        cell: *cell,
                        source: *source,
                        path,
                    });
                }
                OriginNode::Child { parent, step } => {
                    path.push(step.clone());
                    origin = parent;
                }
            }
        }
    }

    /// Capture the active source call chain. Calls without a source are skipped,
    /// not assigned a fabricated path. Repeated captures share live prefixes.
    pub fn call_trace(&mut self) -> Option<CallTrace> {
        let start = self.calls.iter().rposition(|call| call.captured.is_some());
        let mut caller = start.and_then(|index| self.calls[index].captured.clone().unwrap());
        for index in start.map_or(0, |index| index + 1)..self.calls.len() {
            let expression = self.calls[index].expression.clone();
            let origin = match self.call_origins.get(&expression) {
                Some(origin) => origin.clone(),
                None => {
                    let origin = self.source_origin(&expression).map(Rc::new);
                    self.call_origins.insert(expression, origin.clone());
                    origin
                }
            };
            if let Some(origin) = origin {
                caller = Some(CallTrace(Rc::new(CallTraceNode { origin, caller })));
            }
            self.calls[index].captured = Some(caller.clone());
        }
        caller
    }

    fn at_call(
        &mut self,
        expression: Expression,
        run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>,
    ) -> Result<RuntimeValue, Halt> {
        self.calls.push(ActiveCall {
            expression,
            captured: None,
        });
        let result = run(self);
        self.calls.pop();
        result
    }

    pub fn eval_to_value(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        self.eval(expression, environment)
            .map(RuntimeValue::into_value)
    }

    pub fn eval_value(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        let expression = self.lower(expression);
        self.eval_to_value(expression, environment)
    }

    pub fn eval_value_runtime(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower(expression);
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
        let params: Vec<_> = params.into_iter().collect();
        let fields = [
            (
                vocabulary::PARAMS,
                Value::list(params.iter().copied().map(Value::from)),
            ),
            (vocabulary::BODY, self.value(&body).clone()),
        ]
        .into_iter()
        .collect();
        let params = params.into_iter().map(|cell| Parameter { cell }).collect();
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

    /// Decode GID bindings into a shared lexical environment.
    pub fn environment(&self, value: &Value) -> Option<Environment> {
        let fields = value.as_record()?;
        Some(
            Environment::with_indices(self.indices.clone())
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
        match &expression.0.form {
            Form::Data => {
                if let Some(RuntimeValue(RuntimeValueKind::F64(cached), _)) =
                    self.data_runtime.get(&expression)
                {
                    return Ok(Some(cached.number));
                }
            }
            Form::Cell(cell) => {
                if let Some(RuntimeValue(RuntimeValueKind::F64(value), _)) =
                    environment.get_runtime(*cell)
                {
                    return Ok(Some(value.number));
                }
            }
            _ => {}
        }
        Ok(self.eval_burned(expression, environment)?.as_f64())
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
            Form::Data => thunk(move |context, _| {
                if let Some(cached) = context.data_runtime.get(&expression) {
                    return Ok(cached.clone());
                }
                let value = context
                    .lower_runtime(RuntimeValue::from_value(context.value(&expression).clone()));
                context
                    .data_runtime
                    .insert(expression.clone(), value.clone());
                Ok(value)
            }),
            Form::Cell(index) => {
                thunk(move |context, environment| context.eval_cell(index, environment))
            }
            Form::Value(value) => {
                thunk(move |context, environment| context.eval(value.clone(), environment))
            }
            Form::Call { function } => self.compile_call(&expression, function),
            Form::Lambda { parameters, body } => {
                let fields = self.value(&expression).as_record().unwrap().clone();
                match parameters {
                    LambdaParameters::Valid(params) => thunk(move |_, environment| {
                        Ok(RuntimeValue::new(RuntimeValueKind::Closure(Closure {
                            fields: fields.clone(),
                            params: params.clone(),
                            body: body.clone(),
                            environment: environment.clone(),
                        })))
                    }),
                    LambdaParameters::Malformed => thunk(move |_, _| {
                        Ok(RuntimeValue::from_value(absent::with_detail(
                            absent::MALFORMED_LAMBDA,
                            absent::VALUE,
                            Value::record(fields.clone()),
                        )))
                    }),
                    LambdaParameters::Invalid(parameter) => thunk(move |_, _| {
                        Ok(RuntimeValue::from_value(absent::with_detail(
                            absent::INVALID_PARAMETER,
                            absent::VALUE,
                            parameter.clone(),
                        )))
                    }),
                }
            }
        }
    }

    fn compile_call(&mut self, call: &Expression, function: Expression) -> Thunk {
        let call = call.clone();
        let function_cell = match &function.0.form {
            Form::Cell(index) => Some(*index),
            _ => None,
        };
        let plan: RefCell<Option<CallPlan>> = RefCell::new(None);
        let stages: RefCell<Option<(Prepare, Stage)>> = RefCell::new(None);
        let target: RefCell<Option<Rc<PreparedCallTarget>>> = RefCell::new(None);
        thunk(move |context, environment| {
            context.at_call(call.clone(), |context| {
                if let Some(cell) = function_cell
                    && environment.get_runtime(cell).is_none()
                    && context.transient_foreign_target(cell).is_none()
                {
                    return context.call_cell(cell, &call, environment, &target);
                }
                context.checked_call(|context| {
                    let callable = match function_cell {
                        Some(cell) => {
                            context.burn()?;
                            match environment.get_runtime(cell) {
                                Some(value) => context.lower_runtime(value.clone()),
                                None => match context.foreign_target_cell(cell) {
                                    Some(_) => RuntimeValue::new(RuntimeValueKind::Foreign(cell)),
                                    None => context.eval_cell(cell, environment)?,
                                },
                            }
                        }
                        None => context.eval(function.clone(), environment)?,
                    };
                    match context.try_call_callable(
                        callable.clone(),
                        &call,
                        environment,
                        Some(&plan),
                        Some(&stages),
                    ) {
                        Some(result) => result,
                        None => Ok(RuntimeValue::from_value(absent::with_detail(
                            absent::NOT_CALLABLE,
                            absent::VALUE,
                            callable.into_value(),
                        ))),
                    }
                })
            })
        })
    }

    fn call_cell(
        &mut self,
        cell: CellId,
        call: &Expression,
        environment: &Environment,
        cache: &RefCell<Option<Rc<PreparedCallTarget>>>,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        let cached = cache.borrow().clone();
        let target = match cached {
            Some(target) => target,
            None => {
                let kind = match self.host.resolve(cell) {
                    Some((_, Definition::Foreign(definition))) => {
                        PreparedCallTargetKind::Foreign(definition.implementation.clone())
                    }
                    Some((source, Definition::Value(value))) => PreparedCallTargetKind::Value(
                        self.lower_source(&value, OriginRoot::Cell { cell, source }),
                    ),
                    None => PreparedCallTargetKind::Missing,
                };
                let target = Rc::new(PreparedCallTarget {
                    kind,
                    plan: RefCell::new(None),
                    stages: RefCell::new(None),
                });
                *cache.borrow_mut() = Some(target.clone());
                target
            }
        };
        self.checked_call(|context| match &target.kind {
            PreparedCallTargetKind::Foreign(function) => context.call_foreign_staged(
                &ResolvedForeign::Permanent(function.clone()),
                call,
                environment,
                Some(&target.stages),
            ),
            PreparedCallTargetKind::Value(expression) => {
                let callable = context.eval_definition(cell, expression.clone(), environment)?;
                if callable.is_absent() {
                    Ok(callable)
                } else {
                    context
                        .try_call_callable(
                            callable.clone(),
                            call,
                            environment,
                            Some(&target.plan),
                            Some(&target.stages),
                        )
                        .unwrap_or_else(|| {
                            Ok(RuntimeValue::from_value(absent::with_detail(
                                absent::NOT_CALLABLE,
                                absent::VALUE,
                                callable.into_value(),
                            )))
                        })
                }
            }
            PreparedCallTargetKind::Missing => Ok(RuntimeValue::from_value(absent::with_detail(
                absent::MISSING_CELL,
                absent::CELL,
                cell.into(),
            ))),
        })
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
            Some(stack_index) => Ok(RuntimeValue::from_value(absent::with_detail(
                absent::CELL_CYCLE,
                absent::CYCLE,
                Value::list(
                    self.resolving[stack_index..]
                        .iter()
                        .copied()
                        .chain([cell])
                        .map(Value::from),
                ),
            ))),
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
        plan: Option<&RefCell<Option<CallPlan>>>,
        stages: Option<&RefCell<Option<(Prepare, Stage)>>>,
    ) -> Option<Result<RuntimeValue, Halt>> {
        match &callable.0 {
            RuntimeValueKind::Closure(closure) => {
                return Some(self.eval_grap_call(closure.clone(), call, environment, plan));
            }
            RuntimeValueKind::Foreign(cell) => {
                return Some(match self.transient_foreign_target(*cell) {
                    Some(foreign) => self.call_foreign_staged(&foreign, call, environment, stages),
                    None => self.call_cell(*cell, call, environment, &RefCell::new(None)),
                });
            }
            _ => {}
        }
        if let Some(closure) = self.runtime_closure(&callable) {
            Some(self.eval_grap_call(closure, call, environment, plan))
        } else {
            match &callable.0 {
                RuntimeValueKind::Data(_) | RuntimeValueKind::Record(_) => {
                    let cell = callable
                        .as_cell()
                        .or_else(|| callable.field(vocabulary::FFI)?.as_cell());
                    if let Some(cell) = cell
                        && self.transient_foreign_target(cell).is_none()
                    {
                        let target = RefCell::new(None);
                        return Some(self.call_cell(cell, call, environment, &target));
                    }
                    cell.and_then(|cell| self.foreign_target_cell(cell))
                        .map(|foreign| {
                            self.call_foreign_staged(&foreign, call, environment, stages)
                        })
                }
                _ => None,
            }
        }
    }

    fn lower_runtime(&self, value: RuntimeValue) -> RuntimeValue {
        match &value.0 {
            RuntimeValueKind::Data(source) => match crate::f64::read(source) {
                Some(number) => RuntimeValue::original_f64(number, source.clone()),
                None => value,
            },
            _ => value,
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

    pub fn remaining_fuel(&self) -> usize {
        self.remaining_fuel
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

    pub fn missing_argument(&self, cell: CellId) -> Value {
        absent::with_detail(absent::MISSING_ARGUMENT, absent::CELL, Value::from(cell))
    }

    pub fn missing_runtime_argument(&self, cell: CellId) -> RuntimeValue {
        self.missing_argument(cell).into()
    }

    fn eval_cell(&mut self, cell: CellId, environment: &Environment) -> Result<RuntimeValue, Halt> {
        if let Some(value) = environment.get_runtime(cell) {
            let value = value.clone();
            return Ok(self.lower_runtime(value));
        }
        let index = cell_index(&self.indices, cell);
        match self.host.resolve(cell) {
            None => Ok(RuntimeValue::from_value(absent::with_detail(
                absent::MISSING_CELL,
                absent::CELL,
                Value::from(cell),
            ))),
            Some((source, definition)) => {
                while self.cell_states.len() <= index.0 {
                    self.cell_states.push(CellState::Unknown);
                }
                match self.cell_states[index.0].clone() {
                    CellState::Ready(expression) => {
                        self.eval_resolved_cell(index, cell, expression, environment)
                    }
                    CellState::Evaluating { stack_index } => {
                        Ok(RuntimeValue::from_value(absent::with_detail(
                            absent::CELL_CYCLE,
                            absent::CYCLE,
                            Value::list(
                                self.resolving[stack_index..]
                                    .iter()
                                    .copied()
                                    .chain([cell])
                                    .map(Value::from),
                            ),
                        )))
                    }
                    CellState::Unknown => {
                        let expression = self
                            .lower_source(definition.value(), OriginRoot::Cell { cell, source });
                        self.cell_states[index.0] = CellState::Ready(expression.clone());
                        self.eval_resolved_cell(index, cell, expression, environment)
                    }
                }
            }
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
                .then_some(ResolvedForeign::Scoped(cell))
        })
    }

    fn transient_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.foreign_target_cell(cell)
    }

    fn scoped_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.foreign_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(cell))
            .map(|function| ResolvedForeign::Permanent(function.clone()))
    }

    fn call_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.call_foreign_staged(foreign, call, environment, None)
    }

    fn call_foreign_staged(
        &mut self,
        foreign: &ResolvedForeign,
        call: &Expression,
        environment: &Environment,
        stages: Option<&RefCell<Option<(Prepare, Stage)>>>,
    ) -> Result<RuntimeValue, Halt> {
        let tracked = match foreign {
            ResolvedForeign::Permanent(function) => function.tracked,
            ResolvedForeign::Scoped(_) => self.overlay.is_some_and(|overlay| overlay.tracked),
        };
        if !tracked {
            self.host.untracked();
        }
        let value = match foreign {
            ResolvedForeign::Permanent(function) => match &function.implementation {
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
            ResolvedForeign::Scoped(cell) => {
                let function = self
                    .overlay
                    .expect("a scoped foreign target came from the active overlay")
                    .call;
                match function {
                    OverlayCall::Runtime(function) => function(*cell, self, call, environment),
                    OverlayCall::Value(function) => {
                        function(*cell, self, call, environment).map(RuntimeValue::from)
                    }
                }
            }
        }?;
        Ok(self.lower_runtime(value))
    }

    /// Apply a callable to already-evaluated argument values inside
    /// this evaluation — the in-context form of [`apply`].
    pub fn apply_value(
        &mut self,
        function: &Value,
        arguments: impl IntoIterator<Item = (CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.apply_values(function, arguments.into_iter().collect())
    }

    pub fn apply(
        &mut self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        let callable = self.prepare_runtime_callable(function.clone(), &Environment::default());
        self.call_prepared(&callable, arguments)
    }

    /// Run synchronously with an additional foreign-function layer.
    /// Its owned closures may carry state local to this evaluation;
    /// the layer shadows host and permanent functions.
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
        let reference = match expression.0.form {
            Form::Cell(cell) if environment.get_runtime(cell).is_none() => self
                .foreign_target_cell(cell)
                .map(|_| RuntimeValue::new(RuntimeValueKind::Foreign(cell)))
                .or_else(|| match self.host.resolve(cell) {
                    Some((_, Definition::Foreign(_))) => {
                        Some(RuntimeValue::new(RuntimeValueKind::Foreign(cell)))
                    }
                    _ => None,
                }),
            _ => None,
        };
        let callable = match reference {
            Some(reference) => {
                self.burn()?;
                reference
            }
            None => self.eval(expression, environment)?,
        };
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
            RuntimeValueKind::Data(_) | RuntimeValueKind::Record(_) => self
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

    /// Invoke a prepared callable with argument VALUES. It preserves
    /// the ordinary call/function/argument fuel steps while avoiding
    /// a temporary call-shaped Value for Grap closures.
    pub fn call_prepared_value(
        &mut self,
        callable: &PreparedCallable,
        arguments: impl IntoIterator<Item = (CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.call_prepared(
            callable,
            arguments
                .into_iter()
                .map(|(cell, value)| (cell, RuntimeValue::from_value(value))),
        )
        .map(RuntimeValue::into_value)
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
                    let mut bound = Vec::with_capacity(closure.params.len());
                    for parameter in closure.params.iter() {
                        let value = if parameter.cell == vocabulary::FUNCTION {
                            callable.value.clone()
                        } else {
                            let Some((_, value)) =
                                arguments.iter().find(|(cell, _)| *cell == parameter.cell)
                            else {
                                return Ok(context.missing_runtime_argument(parameter.cell));
                            };
                            context.lower_runtime(value.clone())
                        };
                        context.burn()?;
                        bound.push((parameter.cell, value));
                    }
                    context.eval(
                        closure.body.clone(),
                        &closure.environment.extended_runtime(bound),
                    )
                }
                RuntimeValueKind::Foreign(cell) => {
                    let call = runtime_call(RuntimeValue::from(ffi(*cell)), arguments);
                    match context.transient_foreign_target(*cell) {
                        Some(foreign) => {
                            context.call_foreign(&foreign, &call, &callable.environment)
                        }
                        None => context.call_cell(
                            *cell,
                            &call,
                            &callable.environment,
                            &RefCell::new(None),
                        ),
                    }
                }
                RuntimeValueKind::Data(_) | RuntimeValueKind::Record(_) => {
                    let value = &callable.value;
                    let target = value
                        .as_cell()
                        .or_else(|| value.field(vocabulary::FFI)?.as_cell());
                    match target {
                        Some(cell) if context.transient_foreign_target(cell).is_none() => {
                            let call = runtime_call(callable.value.clone(), arguments);
                            context.call_cell(
                                cell,
                                &call,
                                &callable.environment,
                                &RefCell::new(None),
                            )
                        }
                        _ => {
                            let Some(target) =
                                target.and_then(|cell| context.foreign_target_cell(cell))
                            else {
                                return Ok(RuntimeValue::from_value(absent::with_detail(
                                    absent::NOT_CALLABLE,
                                    absent::VALUE,
                                    value.to_value(),
                                )));
                            };
                            let call = runtime_call(callable.value.clone(), arguments);
                            context.call_foreign(&target, &call, &callable.environment)
                        }
                    }
                }
                RuntimeValueKind::F64(_) | RuntimeValueKind::List(_) => {
                    Ok(RuntimeValue::from_value(absent::with_detail(
                        absent::NOT_CALLABLE,
                        absent::VALUE,
                        callable.value.to_value(),
                    )))
                }
            }
        })
    }

    fn apply_values(
        &mut self,
        function: &Value,
        arguments: Vec<(CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.apply_values_runtime(
            &function.into(),
            arguments.into_iter().map(|(k, v)| (k, v.into())).collect(),
        )
        .map(RuntimeValue::into_value)
    }

    fn apply_values_runtime(
        &mut self,
        function: &RuntimeValue,
        arguments: Vec<(CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        let environment = Environment::with_indices(self.indices.clone());
        if let Some(cell) = function.as_cell()
            && self.transient_foreign_target(cell).is_none()
        {
            return self.apply_cell(cell, function.clone(), arguments, &environment);
        }
        self.checked_call(|context| {
            let callable = if function.as_cell().is_some() {
                context.burn()?;
                function.clone()
            } else {
                let function = match &function.0 {
                    RuntimeValueKind::Data(value) => context.lower_source(value, OriginRoot::Input),
                    _ => context.lower_runtime_code(function),
                };
                context.eval(function, &environment)?
            };
            match context.try_apply_callable(callable.clone(), &arguments, &environment) {
                Some(result) => result,
                None => Ok(RuntimeValue::from_value(absent::with_detail(
                    absent::NOT_CALLABLE,
                    absent::VALUE,
                    callable.into_value(),
                ))),
            }
        })
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
            Some((_, Definition::Foreign(definition))) => {
                let call = context.lower_runtime_code(&RuntimeValue::record(
                    [(vocabulary::FUNCTION, callable)]
                        .into_iter()
                        .chain(arguments),
                ));
                context.call_foreign(
                    &ResolvedForeign::Permanent(definition.implementation.clone()),
                    &call,
                    environment,
                )
            }
            Some((source, Definition::Value(value))) => {
                let expression = context.lower_source(&value, OriginRoot::Cell { cell, source });
                let callable = context.eval_definition(cell, expression, environment)?;
                if callable.is_absent() {
                    Ok(callable)
                } else {
                    context
                        .try_apply_callable(callable.clone(), &arguments, environment)
                        .unwrap_or_else(|| {
                            Ok(RuntimeValue::from_value(absent::with_detail(
                                absent::NOT_CALLABLE,
                                absent::VALUE,
                                callable.into_value(),
                            )))
                        })
                }
            }
            None => Ok(RuntimeValue::from_value(absent::with_detail(
                absent::MISSING_CELL,
                absent::CELL,
                cell.into(),
            ))),
        })
    }

    fn try_apply_callable(
        &mut self,
        callable: RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
        environment: &Environment,
    ) -> Option<Result<RuntimeValue, Halt>> {
        if let Some(closure) = self.runtime_closure(&callable) {
            let bound: Result<Vec<_>, CellId> = closure
                .params
                .iter()
                .map(|parameter| {
                    arguments
                        .iter()
                        .find(|(cell, _)| *cell == parameter.cell)
                        .map(|(_, value)| (parameter.cell, self.lower_runtime(value.clone())))
                        .ok_or(parameter.cell)
                })
                .collect();
            return Some(match bound {
                Ok(bound) => self.eval(closure.body, &closure.environment.extended_runtime(bound)),
                Err(cell) => Ok(RuntimeValue::from_value(self.missing_argument(cell))),
            });
        }
        let foreign = match &callable.0 {
            RuntimeValueKind::Foreign(cell) => {
                if let Some(foreign) = self.transient_foreign_target(*cell) {
                    Some(foreign)
                } else {
                    return Some(self.apply_cell(
                        *cell,
                        ffi(*cell).into(),
                        arguments.to_vec(),
                        environment,
                    ));
                }
            }
            RuntimeValueKind::Data(_) | RuntimeValueKind::Record(_) => {
                let cell = callable
                    .as_cell()
                    .or_else(|| callable.field(vocabulary::FFI)?.as_cell());
                if let Some(cell) = cell
                    && self.transient_foreign_target(cell).is_none()
                {
                    return Some(self.apply_cell(
                        cell,
                        callable.clone(),
                        arguments.to_vec(),
                        environment,
                    ));
                }
                cell.and_then(|cell| self.foreign_target_cell(cell))
            }
            RuntimeValueKind::F64(_) | RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => {
                None
            }
        };
        foreign.map(|foreign| {
            let call = self.lower_runtime_code(&RuntimeValue::record(
                [(vocabulary::FUNCTION, callable)]
                    .into_iter()
                    .chain(arguments.iter().cloned()),
            ));
            self.call_foreign(&foreign, &call, environment)
        })
    }

    fn eval_grap_call(
        &mut self,
        closure: Closure,
        call: &Expression,
        calling_environment: &Environment,
        plan: Option<&RefCell<Option<CallPlan>>>,
    ) -> Result<RuntimeValue, Halt> {
        let arguments_plan = plan.and_then(|slot| {
            slot.borrow().as_ref().and_then(|cached| {
                Rc::ptr_eq(&cached.params, &closure.params).then(|| cached.arguments.clone())
            })
        });
        let arguments_plan = match arguments_plan {
            Some(arguments) => arguments,
            None => {
                let arguments: Rc<[Option<Expression>]> = closure
                    .params
                    .iter()
                    .map(|parameter| self.field(&call, parameter.cell))
                    .collect();
                if let Some(slot) = plan {
                    *slot.borrow_mut() = Some(CallPlan {
                        params: closure.params.clone(),
                        arguments: arguments.clone(),
                    });
                }
                arguments
            }
        };
        let mut arguments = Vec::with_capacity(closure.params.len());
        for (parameter, argument) in closure.params.iter().zip(arguments_plan.iter()) {
            let Some(expression) = argument else {
                return Ok(RuntimeValue::from_value(
                    self.missing_argument(parameter.cell),
                ));
            };
            arguments.push((
                parameter.cell,
                self.eval(expression.clone(), calling_environment)?,
            ));
        }
        let body_environment = closure.environment.extended_runtime(arguments);
        self.eval(closure.body, &body_environment)
    }

    fn runtime_closure(&mut self, value: &RuntimeValue) -> Option<Closure> {
        match &value.0 {
            RuntimeValueKind::Closure(closure) => Some(closure.clone()),
            RuntimeValueKind::Data(_) | RuntimeValueKind::Record(_) => {
                let fields = value.field(vocabulary::CLOSURE)?;
                let params = fields
                    .field(vocabulary::PARAMS)?
                    .list_values()?
                    .map(|value| value.as_cell().map(|cell| Parameter { cell }))
                    .collect::<Option<Vec<_>>>()?;
                let body = self.lower_runtime_code(&fields.field(vocabulary::BODY)?);
                let captured = fields.field(vocabulary::ENVIRONMENT)?;
                let environment = match &captured.0 {
                    RuntimeValueKind::Data(value) => self.environment(value)?,
                    RuntimeValueKind::Record(fields) => {
                        Environment::with_indices(self.indices.clone())
                            .extended_runtime(fields.iter().cloned())
                    }
                    _ => return None,
                };
                Some(Closure {
                    params: params.into(),
                    body,
                    environment,
                    fields: fields.as_value().as_record()?.clone(),
                })
            }
            RuntimeValueKind::F64(_) | RuntimeValueKind::List(_) | RuntimeValueKind::Foreign(_) => {
                None
            }
        }
    }
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
    let value = RuntimeValue::record(
        [(vocabulary::FUNCTION, function)]
            .into_iter()
            .chain(arguments),
    );
    let RuntimeValueKind::Record(fields) = &value.0 else {
        unreachable!()
    };
    let fields = fields
        .iter()
        .map(|(field, value)| (*field, runtime_expression(value.clone())))
        .collect();
    Expression(Rc::new(Lowered {
        source: value.clone(),
        origin: None,
        form: Form::Ready(value),
        fields: Some(fields),
        elements: None,
    }))
}

pub fn lambda(params: impl IntoIterator<Item = CellId>, body: Value) -> Value {
    Value::record([
        (
            vocabulary::PARAMS,
            Value::list(params.into_iter().map(Value::from)),
        ),
        (vocabulary::BODY, body),
    ])
}

/// A callable naming a foreign function — the FFI bridge as a value.
pub fn ffi(cell: CellId) -> Value {
    Value::record([(vocabulary::FFI, Value::from(cell))])
}

pub fn call(function: Value, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(
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
        data_runtime: Default::default(),
        calls: Vec::new(),
        call_origins: Default::default(),
        cell_states: Vec::new(),
        indices: CellIndices::default(),
    }
}

/// The host supplies each definition with its stable source in the current
/// resolution context. Source origins preserve it independently of load order.
pub fn evaluate(expression: &Value, host: &dyn Host, fuel: usize) -> Evaluation {
    context(host, None, fuel).run(expression)
}

pub fn evaluate_at(
    expression: &Value,
    origin: Option<SourceOrigin>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation {
    context(host, None, fuel).conclude(|context| {
        let expression = match origin {
            Some(origin) => context.lower_source(expression, OriginRoot::Located(origin)),
            None => context.lower_unattributed_source(expression),
        };
        context.eval(expression, &Environment::default())
    })
}

/// Interpret runtime-held syntax at an optional source location. Embedded
/// native callables retain their own code origins and lexical captures.
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
        context.eval(expression, &Environment::default())
    })
}

/// Apply a callable expression at the existing host boundary: Grap parameters
/// receive values, while foreign functions receive argument syntax to interpret.
/// Use `apply` when the callable and all arguments have already been evaluated.
pub fn apply_expression(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation {
    context(host, None, fuel).conclude(|context| {
        context.apply_values_runtime(&function.into(), arguments.into_iter().collect())
    })
}

/// Apply runtime-held callable syntax at an expression-facing host boundary.
/// Grap parameters receive values; foreign functions receive argument syntax,
/// just as in the GID-facing `apply_value_scoped` adapter.
pub fn apply_expression_scoped(
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    overlay: &ForeignOverlay<'_>,
    fuel: usize,
) -> Evaluation {
    context(host, Some(overlay), fuel)
        .conclude(|context| context.apply_values_runtime(function, arguments.into_iter().collect()))
}

/// Evaluate with a borrowed foreign-function layer that exists only for
/// this synchronous evaluation.
pub fn evaluate_scoped<'a>(
    expression: &Value,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    context(host, Some(overlay), fuel).run(expression)
}

/// Apply a callable to already-evaluated argument VALUES. This is the
/// host boundary: a code-shaped value (a stored lambda or call
/// record) binds as data, where `call` + [`evaluate`] would evaluate
/// it as an expression. A foreign target still receives the
/// arguments as call fields and evaluates them itself; every value
/// but a code-shaped one self-quotes through that.
pub fn apply_value(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation<Value> {
    context(host, None, fuel)
        .conclude(|context| {
            context.apply_values_runtime(
                &function.into(),
                arguments.into_iter().map(|(k, v)| (k, v.into())).collect(),
            )
        })
        .into_value()
}

/// Apply with a borrowed foreign-function layer that exists only for
/// this synchronous evaluation.
pub fn apply_value_scoped<'a>(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation<Value> {
    context(host, Some(overlay), fuel)
        .conclude(|context| {
            context.apply_values_runtime(
                &function.into(),
                arguments.into_iter().map(|(k, v)| (k, v.into())).collect(),
            )
        })
        .into_value()
}

pub fn evaluate_value(expression: &Value, host: &dyn Host, fuel: usize) -> Evaluation<Value> {
    evaluate(expression, host, fuel).into_value()
}

/// Explicitly interpret runtime-held syntax without serializing its embedded callables.
pub fn evaluate_runtime_scoped<'a>(
    expression: &RuntimeValue,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    match &expression.0 {
        RuntimeValueKind::Data(value) => context(host, Some(overlay), fuel).run(value),
        _ => context(host, Some(overlay), fuel)
            .conclude(|context| context.eval_runtime_code(expression, &Environment::default())),
    }
}

pub fn evaluate_value_scoped<'a>(
    expression: &Value,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation<Value> {
    evaluate_scoped(expression, host, overlay, fuel).into_value()
}

pub fn apply(
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation {
    apply_in(context(host, None, fuel), function, arguments)
}

pub fn apply_scoped<'a>(
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    apply_in(context(host, Some(overlay), fuel), function, arguments)
}

fn apply_in(
    context: Context<'_>,
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
) -> Evaluation {
    context.conclude(|context| context.apply(function, arguments))
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
mod tests {
    use super::*;
    use gid::new_cell_id;
    use std::cell::Cell;

    fn definitions_from_parts<'a>(
        resolve: impl Fn(CellId) -> Option<Value> + 'a,
        foreign: &'a ForeignFunctions,
    ) -> impl Host + 'a {
        TestHost(move |cell| {
            match (resolve(cell), foreign.get(cell)) {
                (value, Some(function)) => Some(Definition::foreign(
                    value.unwrap_or_else(|| Value::record([])),
                    function.clone(),
                )),
                (Some(value), None) => Some(Definition::Value(value)),
                (None, None) => None,
            }
            .map(|definition| (Resolution::Document, definition))
            .into_iter()
            .collect()
        })
    }

    fn evaluate(
        expression: &Value,
        resolve: impl Fn(CellId) -> Option<Value>,
        foreign: &ForeignFunctions,
        fuel: usize,
    ) -> Evaluation<Value> {
        super::evaluate_value(expression, &definitions_from_parts(resolve, foreign), fuel)
    }

    fn apply(
        function: &Value,
        arguments: impl IntoIterator<Item = (CellId, Value)>,
        resolve: impl Fn(CellId) -> Option<Value>,
        foreign: &ForeignFunctions,
        fuel: usize,
    ) -> Evaluation<Value> {
        super::apply_value(
            function,
            arguments,
            &definitions_from_parts(resolve, foreign),
            fuel,
        )
    }

    #[test]
    fn direct_cell_calls_return_decline_without_trying_another_definition() {
        let function = new_cell_id();
        let library = Resolution::Library(new_cell_id());
        let expression = call(Value::from(function), []);
        let evaluation = super::evaluate_value(
            &expression,
            &TestHost(|cell| {
                assert_eq!(cell, function);
                vec![
                    (
                        Resolution::Document,
                        Definition::foreign(
                            gid::Value::record([]),
                            ForeignFunction::from_value(|_, _, _| Ok(absent::decline())),
                        ),
                    ),
                    (Resolution::Document, Definition::Value(Value::record([]))),
                    (
                        library,
                        Definition::Value(lambda([], Value::from(b"grap".to_vec()))),
                    ),
                    (
                        library,
                        Definition::foreign(
                            gid::Value::record([]),
                            ForeignFunction::from_value(|_, _, _| {
                                Ok(Value::from(b"too late".to_vec()))
                            }),
                        ),
                    ),
                ]
            }),
            20,
        );

        assert_eq!(evaluation.result, absent::decline());
    }

    #[test]
    fn an_ordinary_absent_stops_dispatch_with_its_details() {
        let function = new_cell_id();
        let library = Resolution::Library(new_cell_id());
        let first_missing = new_cell_id();
        let second_missing = new_cell_id();
        let evaluation = super::evaluate_value(
            &call(Value::from(function), []),
            &TestHost(|cell| match cell {
                cell if cell == function => vec![
                    (
                        Resolution::Document,
                        Definition::Value(Value::from(first_missing)),
                    ),
                    (library, Definition::Value(Value::from(second_missing))),
                ],
                cell if cell == first_missing => Vec::new(),
                _ => unreachable!(),
            }),
            20,
        );

        assert_eq!(
            evaluation.result,
            absent::with_detail(absent::MISSING_CELL, absent::CELL, first_missing.into()),
        );
    }

    #[test]
    fn host_apply_returns_the_selected_definitions_decline() {
        let function = new_cell_id();
        let evaluation = super::apply_value(
            &Value::from(function),
            [],
            &TestHost(|_| {
                vec![
                    (
                        Resolution::Document,
                        Definition::foreign(
                            gid::Value::record([]),
                            ForeignFunction::from_value(|_, _, _| Ok(absent::decline())),
                        ),
                    ),
                    (
                        Resolution::Document,
                        Definition::Value(lambda([], Value::from(b"grap".to_vec()))),
                    ),
                ]
            }),
            20,
        );

        assert_eq!(evaluation.result, absent::decline());
    }

    fn blob(text: &str) -> Value {
        Value::from(text.as_bytes().to_vec())
    }

    #[test]
    fn ordinary_structures_are_inert_data() {
        let field = new_cell_id();
        let cell = new_cell_id();
        let value = Value::record([(field, Value::list([Value::from(cell)]))]);
        let evaluation = evaluate(
            &value,
            |_| panic!("inert data must not resolve cells"),
            &ForeignFunctions::default(),
            10,
        );
        assert_eq!(evaluation.result, value);
    }

    #[test]
    fn value_wrappers_evaluate_only_their_expression_in_the_calling_environment() {
        let parameter = new_cell_id();
        let metadata = new_cell_id();
        let wrapper = Value::record([
            (vocabulary::VALUE, parameter.into()),
            (metadata, new_cell_id().into()),
        ]);
        let expression = call(lambda([parameter], wrapper), [(parameter, blob("local"))]);
        let evaluation = evaluate(
            &expression,
            |_| panic!("neither the bound parameter nor wrapper metadata needs resolving"),
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(evaluation.result, blob("local"));
        assert!(evaluation.completed);
    }

    #[test]
    fn value_wrappers_can_be_anonymous_nested_and_callable() {
        let parameter = new_cell_id();
        let cell = new_cell_id();
        let wrapper = Value::record([(
            vocabulary::VALUE,
            Value::record([(vocabulary::VALUE, lambda([parameter], parameter.into()))]),
        )]);
        let evaluation = evaluate(
            &call(cell.into(), [(parameter, blob("argument"))]),
            |queried| (queried == cell).then(|| wrapper.clone()),
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(evaluation.result, blob("argument"));
    }

    #[test]
    fn referencing_a_value_wrapper_again_reruns_its_expression() {
        let cell = new_cell_id();
        let function = new_cell_id();
        let first = new_cell_id();
        let second = new_cell_id();
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(move |context, _, _| {
                Ok(context.effect(|| {
                    count.set(count.get() + 1);
                    blob("result")
                }))
            }),
        );
        let wrapper = Value::record([(vocabulary::VALUE, call(function.into(), []))]);
        let evaluation = evaluate(
            &call(
                lambda([first, second], first.into()),
                [(first, cell.into()), (second, cell.into())],
            ),
            |queried| (queried == cell).then(|| wrapper.clone()),
            &foreign,
            40,
        );
        assert_eq!(evaluation.result, blob("result"));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn value_wrappers_do_not_evaluate_inside_data_or_foreign_results() {
        let wrapper = Value::record([(vocabulary::VALUE, new_cell_id().into())]);
        let foreign_cell = new_cell_id();
        let result = wrapper.clone();
        let foreign = ForeignFunctions::default().register(
            foreign_cell,
            ForeignFunction::from_value(move |_, _, _| Ok(result.clone())),
        );
        for data in [
            Value::list([wrapper.clone()]),
            Value::record([(new_cell_id(), wrapper.clone())]),
        ] {
            assert_eq!(
                evaluate(&data, |_| panic!("inert data"), &foreign, 10).result,
                data,
            );
        }
        assert_eq!(
            evaluate(&call(foreign_cell.into(), []), |_| None, &foreign, 10).result,
            wrapper,
        );
    }

    #[test]
    fn value_wrappers_propagate_absence_cycles_and_fuel_exhaustion() {
        let foreign = ForeignFunctions::default();
        let absent = absent::value(absent::MISSING_ARGUMENT);
        let wrapper = Value::record([(vocabulary::VALUE, absent.clone())]);
        let evaluation = evaluate(&wrapper, |_| None, &foreign, 10);
        assert_eq!(evaluation.result, absent);
        assert!(evaluation.completed);
        let exhausted = evaluate(&wrapper, |_| None, &foreign, 2);
        assert_eq!(exhausted.result, absent::value(absent::FUEL_EXHAUSTED));
        assert!(!exhausted.completed);

        let cell = new_cell_id();
        let wrapper = Value::record([(vocabulary::VALUE, cell.into())]);
        let cycle = evaluate(&cell.into(), |_| Some(wrapper.clone()), &foreign, 20);
        assert_eq!(absent::reason(&cycle.result), Some(absent::CELL_CYCLE));
    }

    #[test]
    fn value_wrappers_preserve_the_inner_expression_source() {
        let cell = new_cell_id();
        let function = new_cell_id();
        let functions = [function];
        let origin = RefCell::new(None);
        let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
            *origin.borrow_mut() = context.source_origin(&call);
            Ok(blob("result"))
        };
        let wrapper = Value::record([(vocabulary::VALUE, call(function.into(), []))]);
        let evaluation = super::evaluate_value_scoped(
            &cell.into(),
            &definitions_from_parts(
                |queried| (queried == cell).then(|| wrapper.clone()),
                &ForeignFunctions::default(),
            ),
            &ForeignOverlay::from_value(&functions, &scoped),
            20,
        );
        assert_eq!(evaluation.result, blob("result"));
        assert_eq!(
            origin.into_inner(),
            Some(SourceOrigin::Cell {
                cell,
                source: Resolution::Document,
                path: vec![gid::Step::Key(vocabulary::VALUE)],
            }),
        );
    }

    #[test]
    fn consuming_the_entire_fuel_allowance_is_exhaustion() {
        let value = blob("one step");
        let exhausted = evaluate(&value, |_| None, &ForeignFunctions::default(), 1);
        assert_eq!(exhausted.result, absent::value(absent::FUEL_EXHAUSTED));
        assert_eq!(exhausted.remaining_fuel, 0);
        let completed = evaluate(&value, |_| None, &ForeignFunctions::default(), 2);
        assert_eq!(completed.result, value);
        assert_eq!(completed.remaining_fuel, 1);
    }

    #[test]
    fn a_staged_foreign_prepares_once_per_compiled_call_site() {
        let staged = new_cell_id();
        let repeat = new_cell_id();
        let argument = new_cell_id();
        let preparations = Rc::new(Cell::new(0));
        let count = preparations.clone();
        let foreign = ForeignFunctions::default()
            .register(
                staged,
                ForeignFunction::staged(move |_, _| {
                    count.set(count.get() + 1);
                    Rc::new(|_, _| Ok(RuntimeValue::from_value(blob("staged"))))
                }),
            )
            .register(
                repeat,
                ForeignFunction::new(move |context, call, environment| {
                    let Some(argument) = context.field(&call, argument) else {
                        return Ok(context.missing_runtime_argument(argument));
                    };
                    context.eval(argument.clone(), environment)?;
                    context.eval(argument.clone(), environment)
                }),
            );
        let expression = call(
            Value::from(repeat),
            [(argument, call(Value::from(staged), []))],
        );

        assert_eq!(
            evaluate(&expression, |_| None, &foreign, 20).result,
            blob("staged"),
        );
        assert_eq!(preparations.get(), 1);
    }

    #[test]
    fn a_staged_call_reprepares_when_a_scoped_function_changes() {
        let staged = new_cell_id();
        let run_scoped = new_cell_id();
        let argument = new_cell_id();
        let base_preparations = Rc::new(Cell::new(0));
        let scoped_preparations = Rc::new(Cell::new(0));
        let base_count = base_preparations.clone();
        let scoped_count = scoped_preparations.clone();
        let scoped = ForeignFunctions::default().register(
            staged,
            ForeignFunction::staged(move |_, _| {
                scoped_count.set(scoped_count.get() + 1);
                Rc::new(|_, _| Ok(RuntimeValue::from_value(blob("scoped"))))
            }),
        );
        let foreign = ForeignFunctions::default()
            .register(
                staged,
                ForeignFunction::staged(move |_, _| {
                    base_count.set(base_count.get() + 1);
                    Rc::new(|_, _| Ok(RuntimeValue::from_value(blob("base"))))
                }),
            )
            .register(
                run_scoped,
                ForeignFunction::new(move |context, call, environment| {
                    let Some(argument) = context.field(&call, argument) else {
                        return Ok(context.missing_runtime_argument(argument));
                    };
                    let before = context.eval(argument.clone(), environment)?;
                    let during = context.with_foreign_functions(scoped.clone(), |context| {
                        context.eval(argument.clone(), environment)
                    })?;
                    let after = context.eval(argument.clone(), environment)?;
                    Ok(RuntimeValue::list([before, during, after]))
                }),
            );
        let expression = call(
            Value::from(run_scoped),
            [(argument, call(Value::from(staged), []))],
        );

        assert_eq!(
            evaluate(&expression, |_| None, &foreign, 30).result,
            Value::list([blob("base"), blob("scoped"), blob("base")]),
        );
        assert_eq!(base_preparations.get(), 1);
        assert_eq!(scoped_preparations.get(), 1);
    }

    #[test]
    fn cells_evaluate_when_they_are_the_expression() {
        let first = new_cell_id();
        let second = new_cell_id();
        let evaluation = evaluate(
            &Value::from(first),
            |cell| match cell {
                cell if cell == first => Some(Value::from(second)),
                cell if cell == second => Some(blob("done")),
                _ => None,
            },
            &ForeignFunctions::default(),
            10,
        );
        assert_eq!(evaluation.result, blob("done"));
    }

    #[test]
    fn graph_functions_match_parameter_labelled_fields() {
        let function_cell = new_cell_id();
        let x = new_cell_id();
        let y = new_cell_id();
        let definition = lambda([x, y], Value::from(x));
        let argument = new_cell_id();
        let reads = std::cell::RefCell::new(Vec::new());
        let expression = call(
            Value::from(function_cell),
            [(x, blob("x")), (y, Value::from(argument))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| {
                reads.borrow_mut().push(cell);
                (cell == function_cell).then(|| definition.clone())
            },
            &ForeignFunctions::default(),
            30,
        );
        assert_eq!(evaluation.result, blob("x"));
        assert_eq!(*reads.borrow(), [function_cell, argument]);
    }

    #[test]
    fn parameters_shadow_document_cells() {
        let parameter = new_cell_id();
        let expression = call(
            lambda([parameter], Value::from(parameter)),
            [(parameter, blob("local"))],
        );
        let evaluation = evaluate(
            &expression,
            |_| panic!("a lexical binding must not consult the document"),
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(evaluation.result, blob("local"));
    }

    #[test]
    fn parameters_shadow_foreign_functions() {
        let function = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|_, _, _| Ok(blob("foreign"))),
        );
        let shadowed = call(
            lambda([function], Value::from(function)),
            [(function, blob("bound"))],
        );
        assert_eq!(
            evaluate(&shadowed, |_| None, &foreign, 30).result,
            blob("bound"),
        );
        let unshadowed = evaluate(&call(Value::from(function), []), |_| None, &foreign, 30);
        assert_eq!(unshadowed.result, blob("foreign"));
    }

    #[test]
    fn a_foreign_call_outside_a_shadowing_scope_still_uses_the_registry() {
        let function = new_cell_id();
        let ignored = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|_, _, _| Ok(blob("foreign"))),
        );
        let expression = call(
            lambda([ignored], call(function.into(), [])),
            [(
                ignored,
                call(
                    lambda([function], Value::from(function)),
                    [(function, blob("local"))],
                ),
            )],
        );
        assert_eq!(
            evaluate(&expression, |_| Some(blob("ordinary data")), &foreign, 100).result,
            blob("foreign")
        );
    }

    #[test]
    fn cell_reads_use_descriptions_without_invoking_native_code_or_scoped_capabilities() {
        struct DataHost(CellId, Vec<(Resolution, Definition)>);
        impl Host for DataHost {
            fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)> {
                assert_eq!(cell, self.0);
                self.1.first().cloned()
            }
        }
        let cell = new_cell_id();
        let functions = [cell];
        let invoke = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
            panic!("reading data must not invoke a scoped capability")
        };
        let overlay = ForeignOverlay::from_value(&functions, &invoke);
        let first = (Resolution::Document, Definition::Value(blob("document")));
        let second = (
            Resolution::Library(new_cell_id()),
            Definition::foreign(
                blob("library"),
                ForeignFunction::from_value(|_, _, _| {
                    panic!("reading a native definition must not invoke it")
                }),
            ),
        );
        for (values, expected) in [
            (
                vec![],
                absent::with_detail(absent::MISSING_CELL, absent::CELL, cell.into()),
            ),
            (vec![first.clone()], first.1.value().clone()),
            (vec![second.clone()], second.1.value().clone()),
            (vec![first.clone(), second], first.1.value().clone()),
        ] {
            let result =
                super::evaluate_value_scoped(&cell.into(), &DataHost(cell, values), &overlay, 20);
            assert!(result.completed);
            assert_eq!(result.result, expected);
        }
    }

    #[test]
    fn anonymous_functions_capture_their_lexical_environment() {
        let x = new_cell_id();
        let y = new_cell_id();
        let inner = lambda([y], Value::from(x));
        let outer = lambda([x], call(inner, [(y, blob("ignored"))]));
        assert_eq!(
            evaluate(
                &call(outer, [(x, blob("captured"))]),
                |_| None,
                &ForeignFunctions::default(),
                50,
            )
            .result,
            blob("captured")
        );
    }

    #[test]
    fn rust_functions_control_evaluation_while_graph_functions_are_strict() {
        const INPUT: CellId = CellId::from_u128(0x6a0d2c8e1f934b70a5c14e8d2b07f391);
        let hold = new_cell_id();
        let input = INPUT;
        let field = new_cell_id();
        let call_shaped_data = call(Value::from(new_cell_id()), []);
        let held = call(Value::from(hold), [(input, call_shaped_data.clone())]);
        let foreign = ForeignFunctions::default().register(
            hold,
            ForeignFunction::from_value(|context, call, _| match context.field(&call, INPUT) {
                Some(value) => Ok(context.value(&value).clone()),
                None => Ok(context.missing_argument(INPUT)),
            }),
        );
        let graph = lambda([input], Value::from(input));
        assert_eq!(
            evaluate(&held, |_| None, &foreign, 30,).result,
            call_shaped_data
        );
        assert_eq!(
            evaluate(&call(graph, [(input, held)]), |_| None, &foreign, 30,).result,
            call_shaped_data
        );
        let inert = Value::record([(field, call(Value::from(hold), []))]);
        assert_eq!(evaluate(&inert, |_| None, &foreign, 30).result, inert);
    }

    #[test]
    fn foreign_registrations_do_not_change_cell_values() {
        const INPUT: CellId = CellId::from_u128(0x2e9c4a71b8d0563f91a0c7e4d15b6820);
        let echo = new_cell_id();
        let input = INPUT;
        let foreign = ForeignFunctions::default().register(
            echo,
            ForeignFunction::from_value(|context, call, environment| {
                match context.field(&call, INPUT) {
                    Some(value) => context.eval_to_value(value, environment),
                    None => Ok(context.missing_argument(INPUT)),
                }
            }),
        );

        let evaluation = evaluate(
            &Value::from(echo),
            |_| Some(blob("ordinary data")),
            &foreign,
            10,
        );
        assert_eq!(evaluation.result, blob("ordinary data"));
        assert_eq!(
            evaluate(
                &call(Value::from(echo), [(input, ffi(echo))]),
                |_| None,
                &foreign,
                10,
            )
            .result,
            Value::record([(vocabulary::FFI, Value::from(echo))])
        );
    }

    #[test]
    fn foreign_callables_pass_through_grap_bindings_as_values() {
        const INPUT: CellId = CellId::from_u128(0x94b7e20c5d1a836f4e09c2a7b6d3581f);
        let echo = new_cell_id();
        let callable = new_cell_id();
        let input = INPUT;
        let foreign = ForeignFunctions::default().register(
            echo,
            ForeignFunction::from_value(|context, call, environment| {
                match context.field(&call, INPUT) {
                    Some(value) => context.eval_to_value(value, environment),
                    None => Ok(context.missing_argument(INPUT)),
                }
            }),
        );
        let apply = lambda(
            [callable, input],
            call(Value::from(callable), [(input, Value::from(input))]),
        );
        let evaluation = evaluate(
            &call(apply, [(callable, ffi(echo)), (input, blob("passed"))]),
            |_| None,
            &foreign,
            40,
        );
        assert_eq!(evaluation.result, blob("passed"));
    }

    #[test]
    fn rust_functions_choose_which_raw_operands_to_evaluate() {
        const CONDITION: CellId = CellId::from_u128(0x0c8f3e5a7192b4d6e1a047c59b83d20e);
        const YES: CellId = CellId::from_u128(0x5d21a9c0e8473f6b1a4c80d2e59f37b6);
        const NO: CellId = CellId::from_u128(0x81e4b07c3a952d6f4c10e8a7b5d6392a);
        let choose = new_cell_id();
        let condition = CONDITION;
        let yes = YES;
        let no = NO;
        let parameter = new_cell_id();
        let missing = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            choose,
            ForeignFunction::from_value(|context, call, environment| {
                let Some(condition) = context.field(&call, CONDITION) else {
                    return Ok(context.missing_argument(CONDITION));
                };
                let Some(yes) = context.field(&call, YES) else {
                    return Ok(context.missing_argument(YES));
                };
                let Some(no) = context.field(&call, NO) else {
                    return Ok(context.missing_argument(NO));
                };
                if context.eval_to_value(condition, environment)? == blob("true") {
                    context.eval_to_value(yes, environment)
                } else {
                    context.eval_to_value(no, environment)
                }
            }),
        );
        let select_parameter = lambda(
            [parameter],
            call(
                Value::from(choose),
                [
                    (condition, blob("true")),
                    (yes, Value::from(parameter)),
                    (no, Value::from(missing)),
                ],
            ),
        );
        let evaluation = evaluate(
            &call(select_parameter, [(parameter, blob("selected"))]),
            |cell| {
                assert_eq!(cell, choose);
                None
            },
            &foreign,
            50,
        );
        assert_eq!(evaluation.result, blob("selected"));
    }

    #[test]
    fn rust_functions_can_expose_the_calling_environment_as_graph_data() {
        let inspect = new_cell_id();
        let parameter = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            inspect,
            ForeignFunction::from_value(|_, _, environment| Ok(Value::from(environment))),
        );
        let inspect_from_body = lambda([parameter], call(Value::from(inspect), []));
        let evaluation = evaluate(
            &call(inspect_from_body, [(parameter, blob("bound"))]),
            |_| None,
            &foreign,
            40,
        );
        assert_eq!(
            evaluation
                .result
                .as_record()
                .and_then(|environment| environment.get(&parameter)),
            Some(&blob("bound"))
        );
    }

    #[test]
    fn rust_functions_evaluate_raw_operands_in_extended_environments() {
        const BINDING: CellId = CellId::from_u128(0xedcd2b19cf89faf94a4a72ab1e02ec31);
        const VALUE: CellId = CellId::from_u128(0x3f7a1c90d2e84b65a0c19e4d7b5826f3);
        const BODY: CellId = CellId::from_u128(0x70d4e8a1c5b2936f4a1e07c8d5b64920);
        let bind = new_cell_id();
        let value = VALUE;
        let body = BODY;
        let foreign = ForeignFunctions::default().register(
            bind,
            ForeignFunction::from_value(|context, call, environment| {
                let Some(value) = context.field(&call, VALUE) else {
                    return Ok(context.missing_argument(VALUE));
                };
                let Some(body) = context.field(&call, BODY) else {
                    return Ok(context.missing_argument(BODY));
                };
                let value = context.eval_to_value(value, environment)?;
                context.eval_to_value(body, &environment.extended([(BINDING, value)]))
            }),
        );
        let evaluation = evaluate(
            &call(
                Value::from(bind),
                [(value, blob("locally bound")), (body, Value::from(BINDING))],
            ),
            |_| None,
            &foreign,
            30,
        );
        assert_eq!(evaluation.result, blob("locally bound"));
    }

    #[test]
    fn returned_closures_are_graph_values_with_graph_environments() {
        let captured = new_cell_id();
        let make_constant = lambda(
            [captured],
            lambda(Vec::<CellId>::new(), Value::from(captured)),
        );
        let closure = evaluate(
            &call(make_constant, [(captured, blob("remembered"))]),
            |_| None,
            &ForeignFunctions::default(),
            40,
        )
        .result;
        assert_eq!(
            evaluate(
                &call(closure.clone(), []),
                |_| None,
                &ForeignFunctions::default(),
                40,
            )
            .result,
            blob("remembered")
        );
        assert_eq!(
            closure
                .as_record()
                .and_then(|value| value.get(&vocabulary::CLOSURE))
                .and_then(Value::as_record)
                .and_then(|closure| closure.get(&vocabulary::ENVIRONMENT))
                .and_then(Value::as_record)
                .and_then(|environment| environment.get(&captured)),
            Some(&blob("remembered"))
        );
    }

    #[test]
    fn absents_carry_stable_reasons_and_occurrence_details() {
        let missing = new_cell_id();
        let evaluation = evaluate(
            &Value::from(missing),
            |_| None,
            &ForeignFunctions::default(),
            10,
        );
        assert_eq!(
            evaluation.result,
            absent::with_detail(absent::MISSING_CELL, absent::CELL, missing.into())
        );

        let malformed = Value::record([
            (vocabulary::PARAMS, blob("not a list")),
            (vocabulary::BODY, blob("body")),
        ]);
        let evaluation = evaluate(&malformed, |_| None, &ForeignFunctions::default(), 10);
        assert_eq!(
            evaluation.result,
            absent::with_detail(absent::MALFORMED_LAMBDA, absent::VALUE, malformed)
        );

        let invalid = blob("not a cell");
        let lambda = Value::record([
            (vocabulary::PARAMS, Value::list([invalid.clone()])),
            (vocabulary::BODY, blob("body")),
        ]);
        let evaluation = evaluate(&lambda, |_| None, &ForeignFunctions::default(), 10);
        assert_eq!(
            evaluation.result,
            absent::with_detail(absent::INVALID_PARAMETER, absent::VALUE, invalid)
        );
    }

    #[test]
    fn incomplete_lambda_shapes_are_inert_data() {
        for incomplete in [
            Value::record([(vocabulary::PARAMS, Value::list(Vec::<Value>::new()))]),
            Value::record([(vocabulary::BODY, blob("body"))]),
        ] {
            let evaluation = evaluate(&incomplete, |_| None, &ForeignFunctions::default(), 10);
            assert_eq!(evaluation.result, incomplete);
        }
    }

    #[test]
    fn recursion_and_cell_cycles_are_values() {
        let recurse = new_cell_id();
        let parameter = new_cell_id();
        let definition = lambda(
            [parameter],
            call(Value::from(recurse), [(parameter, Value::from(parameter))]),
        );
        let evaluation = evaluate(
            &call(Value::from(recurse), [(parameter, blob("again"))]),
            |cell| (cell == recurse).then(|| definition.clone()),
            &ForeignFunctions::default(),
            30,
        );
        assert_eq!(evaluation.result, absent::value(absent::FUEL_EXHAUSTED));

        let a = new_cell_id();
        let b = new_cell_id();
        let evaluation = evaluate(
            &Value::from(a),
            |cell| match cell {
                cell if cell == a => Some(Value::from(b)),
                cell if cell == b => Some(Value::from(a)),
                _ => None,
            },
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(
            evaluation.result,
            absent::with_detail(
                absent::CELL_CYCLE,
                absent::CYCLE,
                Value::list([a, b, a].map(Value::from))
            )
        );
    }

    #[test]
    fn extra_call_fields_do_not_prevent_calling() {
        let parameter = new_cell_id();
        let extra = new_cell_id();
        let expression = call(
            lambda([parameter], Value::from(parameter)),
            [(parameter, blob("result")), (extra, blob("still GID data"))],
        );
        let evaluation = evaluate(&expression, |_| None, &ForeignFunctions::default(), 10);
        assert_eq!(evaluation.result, blob("result"));
    }

    #[test]
    fn grap_parameters_may_repeat_and_name_the_function_field() {
        let parameter = new_cell_id();
        let definition = lambda(
            [vocabulary::FUNCTION, parameter, parameter],
            Value::from(vocabulary::FUNCTION),
        );
        let evaluation = evaluate(
            &call(definition, [(parameter, blob("argument"))]),
            |_| None,
            &ForeignFunctions::default(),
            30,
        );
        assert!(
            evaluation
                .result
                .as_record()
                .and_then(|fields| fields.get(&vocabulary::CLOSURE))
                .is_some()
        );
    }

    #[test]
    fn foreign_functions_may_consume_the_function_field() {
        const PARAMETER: CellId = CellId::from_u128(0x4b8e0d27c1a9563f80e2c4a7d6b1359e);
        let function = new_cell_id();
        let parameter = PARAMETER;
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|context, call, _| {
                let Some(function) = context.field(&call, vocabulary::FUNCTION) else {
                    return Ok(context.missing_argument(vocabulary::FUNCTION));
                };
                let Some(value) = context.field(&call, PARAMETER) else {
                    return Ok(context.missing_argument(PARAMETER));
                };
                assert!(context.value(&function).as_cell().is_some());
                Ok(context.value(&value).clone())
            }),
        );
        assert_eq!(
            evaluate(
                &call(Value::from(function), [(parameter, blob("argument"))]),
                |_| None,
                &foreign,
                20,
            )
            .result,
            blob("argument")
        );
    }

    #[test]
    fn a_later_table_overrides_a_shared_cell() {
        let function = new_cell_id();
        let left = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|_, _, _| Ok(blob("left"))),
        );
        let right = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|_, _, _| Ok(blob("right"))),
        );
        assert_eq!(
            evaluate(
                &call(Value::from(function), []),
                |_| None,
                &left.merge(right),
                10,
            )
            .result,
            blob("right")
        );
    }

    #[test]
    fn apply_binds_code_shaped_arguments_as_data() {
        let parameter = new_cell_id();
        let identity = lambda([parameter], Value::from(parameter));
        let code_shaped = lambda([new_cell_id()], blob("body"));
        // `call` + `evaluate` closes over the argument; `apply`
        // hands it through untouched.
        assert!(
            evaluate(
                &call(identity.clone(), [(parameter, code_shaped.clone())]),
                |_| None,
                &ForeignFunctions::default(),
                20,
            )
            .result
                != code_shaped
        );
        let applied = apply(
            &identity,
            [(parameter, code_shaped.clone())],
            |_| None,
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(applied.result, code_shaped);
    }

    #[test]
    fn apply_resolves_a_cell_to_its_stored_function() {
        let function = new_cell_id();
        let parameter = new_cell_id();
        let stored = lambda([parameter], Value::from(parameter));
        let applied = apply(
            &Value::from(function),
            [(parameter, blob("argument"))],
            |cell| (cell == function).then(|| stored.clone()),
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(applied.result, blob("argument"));
    }

    #[test]
    fn apply_reaches_foreign_targets_with_the_arguments_as_fields() {
        let function = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::from_value(|context, call, environment| {
                let argument = context.field(&call, CellId::from_u128(7)).unwrap();
                context.eval_to_value(argument, environment)
            }),
        );
        let applied = apply(
            &ffi(function),
            [(CellId::from_u128(7), blob("passed"))],
            |_| None,
            &foreign,
            20,
        );
        assert_eq!(applied.result, blob("passed"));
    }

    #[test]
    fn prepared_calls_preserve_explicit_foreign_values() {
        let control = new_cell_id();
        let foreign = new_cell_id();
        let callable = new_cell_id();
        let argument = new_cell_id();
        let decoration = new_cell_id();
        let functions = ForeignFunctions::default()
            .register(
                foreign,
                ForeignFunction::from_value(move |context, call, environment| {
                    let function = context.field(&call, vocabulary::FUNCTION).unwrap();
                    assert_eq!(
                        context
                            .value(&function)
                            .as_record()
                            .and_then(|fields| fields.get(&decoration)),
                        Some(&blob("retained")),
                    );
                    let argument = context.field(&call, argument).unwrap();
                    context.eval_to_value(argument, environment)
                }),
            )
            .register(
                control,
                ForeignFunction::from_value(move |context, call, environment| {
                    let callable = context.field(&call, callable).unwrap();
                    let callable = context.prepare_callable(callable, environment)?;
                    context.call_prepared_value(&callable, [(argument, blob("passed"))])
                }),
            );
        let explicit_foreign = Value::record([
            (vocabulary::FFI, Value::from(foreign)),
            (decoration, blob("retained")),
        ]);
        let evaluation = evaluate(
            &call(Value::from(control), [(callable, explicit_foreign)]),
            |_| None,
            &functions,
            20,
        );
        assert_eq!(evaluation.result, blob("passed"));
    }

    #[test]
    fn a_scoped_foreign_layer_borrows_state_and_remains_reentrant() {
        let outer = new_cell_id();
        let inner = new_cell_id();
        let calls = std::cell::Cell::new(0);
        let functions = [outer, inner];
        let scoped =
            |function, context: &mut Context<'_>, _: &Expression, environment: &Environment| {
                calls.set(calls.get() + 1);
                if function == outer {
                    context.eval_value(&call(Value::from(inner), []), environment)
                } else {
                    Ok(blob("scoped"))
                }
            };
        let overlay = ForeignOverlay::from_value(&functions, &scoped);
        let foreign = ForeignFunctions::default();
        let applied = super::apply_value_scoped(
            &Value::from(outer),
            [],
            &definitions_from_parts(|_| None, &foreign),
            &overlay,
            20,
        );
        assert_eq!(applied.result, blob("scoped"));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn a_scoped_foreign_layer_can_evaluate_a_source_call() {
        let function = new_cell_id();
        let calls = std::cell::Cell::new(0);
        let functions = [function];
        let scoped = |_, _: &mut Context<'_>, _: &Expression, _: &Environment| {
            calls.set(calls.get() + 1);
            Ok(blob("drawn"))
        };
        let overlay = ForeignOverlay::from_value(&functions, &scoped);
        let foreign = ForeignFunctions::default();
        let evaluation = super::evaluate_value_scoped(
            &call(Value::from(function), []),
            &definitions_from_parts(|_| None, &foreign),
            &overlay,
            10,
        );
        assert_eq!(evaluation.result, blob("drawn"));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn captured_calls_share_ancestry_and_source_locations() {
        let repeat = new_cell_id();
        let body = new_cell_id();
        let leaf = new_cell_id();
        let captured = Rc::new(RefCell::new(Vec::new()));
        let output = captured.clone();
        let functions = ForeignFunctions::default()
            .register(
                repeat,
                ForeignFunction::from_value(move |context, call, environment| {
                    let body = context.field(&call, body).unwrap();
                    context.eval_to_value(body.clone(), environment)?;
                    context.eval_to_value(body.clone(), environment)
                }),
            )
            .register(
                leaf,
                ForeignFunction::from_value(move |context, _, _| {
                    let trace = context.call_trace().unwrap();
                    let again = context.call_trace().unwrap();
                    assert!(Rc::ptr_eq(&trace.0, &again.0));
                    output.borrow_mut().push(trace);
                    Ok(blob("done"))
                }),
            );
        let expression = call(repeat.into(), [(body, call(leaf.into(), []))]);
        let result = evaluate(&expression, |_| None, &functions, 100);
        assert!(result.completed);
        let captured = captured.borrow();
        assert_eq!(captured.len(), 2);
        assert_eq!(
            captured[0].origins().cloned().collect::<Vec<_>>(),
            vec![
                SourceOrigin::Input(vec![gid::Step::Key(body)]),
                SourceOrigin::Input(vec![]),
            ]
        );
        assert!(Rc::ptr_eq(&captured[0].0.origin, &captured[1].0.origin));
        assert!(Rc::ptr_eq(
            &captured[0].0.caller.as_ref().unwrap().0,
            &captured[1].0.caller.as_ref().unwrap().0,
        ));
    }

    #[test]
    fn generated_prepared_calls_keep_their_source_callers() {
        let invoke = new_cell_id();
        let leaf = new_cell_id();
        let function = new_cell_id();
        let captured = Rc::new(RefCell::new(None));
        let output = captured.clone();
        let functions = ForeignFunctions::default()
            .register(
                invoke,
                ForeignFunction::from_value(move |context, call, environment| {
                    let function = context.field(&call, function).unwrap();
                    let function = context.prepare_callable(function, environment)?;
                    context.call_prepared_value(&function, [])
                }),
            )
            .register(
                leaf,
                ForeignFunction::staged(move |_, _| {
                    let output = output.clone();
                    Rc::new(move |context, _| {
                        *output.borrow_mut() = context.call_trace();
                        Ok(RuntimeValue::from_value(blob("done")))
                    })
                }),
            );
        let expression = call(
            invoke.into(),
            [(function, lambda([], call(leaf.into(), [])))],
        );
        let result = evaluate(&expression, |_| None, &functions, 100);
        assert!(result.completed);
        assert_eq!(
            captured
                .borrow()
                .as_ref()
                .unwrap()
                .origins()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                SourceOrigin::Input(vec![
                    gid::Step::Key(function),
                    gid::Step::Key(vocabulary::BODY)
                ]),
                SourceOrigin::Input(vec![]),
            ]
        );
    }

    #[test]
    fn a_foreign_call_retains_its_cell_relative_source_origin() {
        let function = new_cell_id();
        let foreign = new_cell_id();
        let source = RefCell::new(None);
        let functions = [foreign];
        let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
            *source.borrow_mut() = context.source_origin(&call);
            Ok(blob("drawn"))
        };
        let overlay = ForeignOverlay::from_value(&functions, &scoped);
        let stored = lambda([], call(Value::from(foreign), []));
        let foreign_functions = ForeignFunctions::default();

        let evaluation = super::evaluate_value_scoped(
            &call(Value::from(function), []),
            &definitions_from_parts(
                |cell| (cell == function).then(|| stored.clone()),
                &foreign_functions,
            ),
            &overlay,
            20,
        );

        assert_eq!(evaluation.result, blob("drawn"));
        assert_eq!(
            *source.borrow(),
            Some(SourceOrigin::Cell {
                cell: function,
                source: Resolution::Document,
                path: vec![gid::Step::Key(vocabulary::BODY)],
            }),
        );
    }

    #[test]
    fn a_single_value_definition_retains_its_resolution_source() {
        let cell = new_cell_id();
        let foreign = new_cell_id();
        let functions = [foreign];
        for source in [Resolution::Document, Resolution::Library(new_cell_id())] {
            let origin = RefCell::new(None);
            let scoped = |_, context: &mut Context<'_>, call: &Expression, _: &Environment| {
                *origin.borrow_mut() = context.source_origin(&call);
                Ok(blob("drawn"))
            };
            let overlay = ForeignOverlay::from_value(&functions, &scoped);
            let evaluation = super::evaluate_value_scoped(
                &Value::from(cell),
                &TestHost(|queried| {
                    assert_eq!(queried, cell);
                    vec![(source, Definition::Value(call(Value::from(foreign), [])))]
                }),
                &overlay,
                20,
            );
            assert_eq!(evaluation.result, blob("drawn"));
            assert_eq!(
                origin.into_inner(),
                Some(SourceOrigin::Cell {
                    cell,
                    source,
                    path: vec![]
                })
            );
        }
    }

    #[test]
    fn apply_classifies_missing_arguments_and_uncallable_targets() {
        let parameter = new_cell_id();
        let missing = apply(
            &lambda([parameter], Value::from(parameter)),
            [],
            |_| None,
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(
            missing.result,
            absent::with_detail(absent::MISSING_ARGUMENT, absent::CELL, parameter.into())
        );

        let uncallable = apply(
            &blob("not a function"),
            [],
            |_| None,
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(
            uncallable.result,
            absent::with_detail(absent::NOT_CALLABLE, absent::VALUE, blob("not a function"))
        );
    }
}
