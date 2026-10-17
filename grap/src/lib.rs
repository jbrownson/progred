//! A small evaluator of GID expressions, returning owned runtime values
//! with an explicit conversion back to GID. It recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use gid::{CellId, Record, Resolution, Value};
use std::cell::{OnceCell, RefCell};
use std::fmt;
use std::rc::Rc;
use unboxed::{Native, Unboxed};

#[cfg(test)]
mod effect_tests;

#[cfg(test)]
mod runtime_tests;

pub mod path;
mod reify;
pub mod source;
mod unboxed;

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
    use gid::{CellId, Record, Value};

    pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);

    pub fn value(number: f64) -> Value {
        Value::record([(F64, Value::from(number.to_le_bytes().to_vec()))])
    }

    pub fn read(value: &Value) -> Option<f64> {
        read_record(value.as_record()?)
    }

    pub fn read_record(fields: &Record) -> Option<f64> {
        fields
            .get(&F64)
            .and_then(Value::as_blob)
            .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
            .map(f64::from_le_bytes)
    }
}

/// The UTF-8 text convention: a record whose `utf8` field holds the text's
/// bytes. Privileged like f64's, so text stays unboxed through calls,
/// containers, and environments, and readers borrow it without decoding;
/// it remains an ordinary convention an external library could own.
pub mod text {
    use gid::{CellId, Record, Value};

    pub const UTF8: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);

    pub fn value(text: impl Into<String>) -> Value {
        Value::record([(UTF8, Value::from(text.into().into_bytes()))])
    }

    pub fn read(value: &Value) -> Option<&str> {
        read_record(value.as_record()?)
    }

    pub fn read_record(fields: &Record) -> Option<&str> {
        fields
            .get(&UTF8)?
            .as_blob()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
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
    parameters: Option<Rc<[CellId]>>,
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
    /// Declared parameters join the description under `params`, as a
    /// lambda's do, unless the description already lists its own.
    pub fn foreign(mut value: Value, implementation: ForeignFunction) -> Self {
        if let (Some(parameters), Value::Record(fields)) = (&implementation.parameters, &mut value)
            && !fields.contains_key(&vocabulary::PARAMS)
        {
            fields.insert(
                vocabulary::PARAMS,
                Value::list(parameters.iter().map(|&parameter| parameter.into())),
            );
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
    fn resolve(&self, cell: CellId) -> Option<(Resolution, Definition)>;
    /// A foreign call not declared tracked ran, so the result may depend on
    /// reads the host didn't see.
    fn untracked(&self) {}
    /// An observable write ran, which a reused result would leave out.
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
            parameters: None,
        }
    }

    pub fn new(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt> + 'static,
    ) -> Self {
        Self {
            tracked: false,
            implementation: ForeignImplementation::Direct(Rc::new(call)),
            parameters: None,
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
    Unboxed(Unboxed),
    Record(Rc<[(CellId, RuntimeValue)]>),
    List(Rc<[RuntimeValue]>),
    Foreign(CellId),
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
            (RuntimeValueKind::Unboxed(a), RuntimeValueKind::Unboxed(b)) => a.same(b),
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
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::F64(number))))
    }

    pub fn text(text: impl Into<Rc<str>>) -> Self {
        Self::new(RuntimeValueKind::Unboxed(Unboxed::new(Native::Text(
            text.into(),
        ))))
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

    pub fn as_f64(&self) -> Option<f64> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::F64(number) => Some(*number),
                Native::Text(_) => crate::f64::read_record(unboxed.original()?),
            },
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

    /// The text this value holds under the text convention, borrowed
    /// without materializing it.
    pub fn as_text(&self) -> Option<&str> {
        match &self.0 {
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
                Native::Text(text) => Some(text.as_ref()),
                Native::F64(_) => crate::text::read_record(unboxed.original()?),
            },
            RuntimeValueKind::Data(value) => crate::text::read(value),
            RuntimeValueKind::Record(fields) => runtime_field(fields, crate::text::UTF8)?
                .as_blob()
                .and_then(|bytes| std::str::from_utf8(bytes).ok()),
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
            RuntimeValueKind::Record(fields) => runtime_field(fields, field).cloned(),
            RuntimeValueKind::Unboxed(unboxed) => unboxed.field(field).map(Self::from_value),
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
            RuntimeValueKind::Unboxed(_)
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
            RuntimeValueKind::Unboxed(unboxed) => Some(unboxed.len()),
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
            RuntimeValueKind::Unboxed(_)
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
            RuntimeValueKind::Record(fields) => runtime_field(fields, field).is_some(),
            RuntimeValueKind::Unboxed(unboxed) => unboxed.contains_field(field),
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
            RuntimeValueKind::Unboxed(unboxed) => unboxed.keys(),
            RuntimeValueKind::Foreign(_) => vec![vocabulary::FFI],
            RuntimeValueKind::Closure(_) => vec![vocabulary::CLOSURE],
            RuntimeValueKind::List(_) => return None,
        })
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
            RuntimeValueKind::Foreign(_) => Shape::Record(vec![vocabulary::FFI]),
            RuntimeValueKind::Closure(_) => Shape::Record(vec![vocabulary::CLOSURE]),
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
                .values()
                .nth(index)
                .cloned()
                .map(Self::from),
            RuntimeValueKind::List(elements) => elements.get(index).cloned(),
            RuntimeValueKind::Unboxed(_)
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
            RuntimeValueKind::Unboxed(unboxed) => unboxed.into_value(),
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
    compiled: ByNode<Thunk>,
    data_runtime: ByNode<RuntimeValue>,
    calls: Vec<ActiveCall>,
    call_origins: ByNode<Option<Rc<SourceOrigin>>>,
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
    Valid(Rc<[Parameter]>),
    Malformed,
    Invalid(Value),
}

impl LambdaParameters {
    /// `None` when the parameters are not a list; otherwise the parameter
    /// cells, or the first entry that is not one.
    fn parse(parameters: Option<Result<Vec<Parameter>, Value>>) -> Self {
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
            RuntimeValueKind::Data(value) => self.lower_with(value, true, origin),
            RuntimeValueKind::Unboxed(unboxed) => match unboxed.original() {
                Some(fields) => self.lower_with(&Value::Record(fields.clone()), true, origin),
                None => runtime_expression(value.clone()),
            },
            RuntimeValueKind::Record(record) => {
                let fields: Vec<_> = record
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
                    Form::Lambda {
                        parameters: LambdaParameters::parse(params.list_values().map(|values| {
                            values
                                .map(|v| {
                                    v.as_cell()
                                        .map(|cell| Parameter { cell })
                                        .ok_or_else(|| v.to_value())
                                })
                                .collect()
                        })),
                        body,
                        fields: record
                            .iter()
                            .map(|(key, value)| (*key, value.to_value()))
                            .collect(),
                    }
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
            Value::Record(fields) => {
                let lower_field = |this: &mut Self, field: CellId, value: &Value| {
                    let child = this.child_origin(origin.clone(), gid::Step::Key(field));
                    this.lower_with(value, descend_data, child)
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
                match (
                    fields.get(&vocabulary::FUNCTION),
                    fields
                        .get(&vocabulary::PARAMS)
                        .zip(fields.get(&vocabulary::BODY)),
                    fields.get(&vocabulary::VALUE),
                ) {
                    (Some(function), _, _) => {
                        let function = lower_field(self, vocabulary::FUNCTION, function);
                        lowered_fields =
                            Some(lower_all(self, Some((vocabulary::FUNCTION, &function))));
                        Form::Call { function }
                    }
                    (None, Some((params, body)), _) => Form::Lambda {
                        parameters: LambdaParameters::parse(params.as_list().map(|list| {
                            list.values()
                                .map(|v| {
                                    v.as_cell()
                                        .map(|cell| Parameter { cell })
                                        .ok_or_else(|| v.clone())
                                })
                                .collect()
                        })),
                        body: lower_field(self, vocabulary::BODY, body),
                        fields: fields.clone(),
                    },
                    (None, None, Some(value)) => {
                        let value = lower_field(self, vocabulary::VALUE, value);
                        lowered_fields = Some(lower_all(self, Some((vocabulary::VALUE, &value))));
                        Form::Value(value)
                    }
                    // Data records keep their children only when data is descended.
                    (None, None, None) => {
                        if descend_data {
                            lowered_fields = Some(lower_all(self, None));
                        }
                        Form::Data
                    }
                }
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
        source::origin(expression)
    }

    /// Capture the active source call chain. Calls without a source are skipped,
    /// not assigned a fabricated path. Repeated captures share live prefixes.
    pub fn call_trace(&mut self) -> Option<CallTrace> {
        let (start, mut caller) = self
            .calls
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, call)| Some((index + 1, call.captured.clone()?)))
            .unwrap_or((0, None));
        for index in start..self.calls.len() {
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
        let expression = self.lower_unattributed_source(expression);
        self.eval_to_value(expression, environment)
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
    pub fn empty_environment(&self) -> Environment {
        Environment::with_indices(self.indices.clone())
    }

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
                if let Some(RuntimeValue(RuntimeValueKind::Unboxed(cached), _)) =
                    self.data_runtime.get(&expression)
                    && let Native::F64(number) = cached.native()
                {
                    return Ok(Some(*number));
                }
            }
            Form::Cell(cell) => {
                if let Some(RuntimeValue(RuntimeValueKind::Unboxed(value), _)) =
                    environment.get_runtime(*cell)
                    && let Native::F64(number) = value.native()
                {
                    return Ok(Some(*number));
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
            context.at_call(call.clone(), |context| {
                if let Some((cell, index)) = function_cell
                    && environment.get_at(&context.indices, cell, index).is_none()
                    && context.foreign_target_cell(cell).is_none()
                {
                    return context.call_cell(cell, &call, environment, &target);
                }
                context.checked_call(|context| {
                    let callable = match function_cell {
                        Some((cell, index)) => {
                            context.burn()?;
                            match environment.get_at(&context.indices, cell, index) {
                                Some(value) => context.lower_runtime(value.clone()),
                                // Ordinary cells took the direct path above.
                                None => RuntimeValue::new(RuntimeValueKind::Foreign(cell)),
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
                    Some((resolution, Definition::Value(value))) => PreparedCallTargetKind::Value(
                        self.lower_source(&value, OriginRoot::Cell { cell, resolution }),
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
                return Some(match self.foreign_target_cell(*cell) {
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
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    let cell = callable
                        .as_cell()
                        .or_else(|| callable.field(vocabulary::FFI)?.as_cell());
                    if let Some(cell) = cell
                        && self.foreign_target_cell(cell).is_none()
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
            RuntimeValueKind::Data(Value::Record(fields)) => match Unboxed::lowered(fields) {
                Some(unboxed) => RuntimeValue::new(RuntimeValueKind::Unboxed(unboxed)),
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

    fn eval_cell_at(
        &mut self,
        cell: CellId,
        index: CellIndex,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        if let Some(value) = environment.get_at(&self.indices, cell, index) {
            let value = value.clone();
            return Ok(self.lower_runtime(value));
        }
        match self.host.resolve(cell) {
            None => Ok(RuntimeValue::from_value(absent::with_detail(
                absent::MISSING_CELL,
                absent::CELL,
                Value::from(cell),
            ))),
            Some((resolution, definition)) => {
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
                        let expression = self.lower_source(
                            definition.value(),
                            OriginRoot::Cell { cell, resolution },
                        );
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
                    context.call_closure(closure.clone(), &callable.value, &arguments)
                }
                RuntimeValueKind::Foreign(cell) => {
                    let call = runtime_call(RuntimeValue::from(ffi(*cell)), arguments);
                    match context.foreign_target_cell(*cell) {
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
                RuntimeValueKind::Data(_)
                | RuntimeValueKind::Unboxed(_)
                | RuntimeValueKind::Record(_) => {
                    let value = &callable.value;
                    let target = value
                        .as_cell()
                        .or_else(|| value.field(vocabulary::FFI)?.as_cell());
                    match target {
                        Some(cell) if context.foreign_target_cell(cell).is_none() => {
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
                RuntimeValueKind::List(_) => Ok(RuntimeValue::from_value(absent::with_detail(
                    absent::NOT_CALLABLE,
                    absent::VALUE,
                    callable.value.to_value(),
                ))),
            }
        })
    }

    fn apply_values(
        &mut self,
        function: &Value,
        arguments: Vec<(CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.apply_expression(
            &function.into(),
            arguments.into_iter().map(|(k, v)| (k, v.into())),
        )
        .map(RuntimeValue::into_value)
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
        let environment = Environment::with_indices(self.indices.clone());
        if let Some(cell) = function.as_cell()
            && self.foreign_target_cell(cell).is_none()
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
            Some((resolution, Definition::Value(value))) => {
                let expression =
                    context.lower_source(&value, OriginRoot::Cell { cell, resolution });
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

    /// Host calls bind argument values as a call expression binds its
    /// evaluated arguments: one step each, and a `function` parameter
    /// receives the callee itself.
    fn call_closure(
        &mut self,
        closure: Closure,
        callable: &RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
    ) -> Result<RuntimeValue, Halt> {
        let mut bound = Vec::with_capacity(closure.params.len());
        for parameter in closure.params.iter() {
            let value = if parameter.cell == vocabulary::FUNCTION {
                callable.clone()
            } else {
                let Some((_, value)) = arguments.iter().find(|(cell, _)| *cell == parameter.cell)
                else {
                    return Ok(self.missing_runtime_argument(parameter.cell));
                };
                self.lower_runtime(value.clone())
            };
            self.burn()?;
            bound.push((parameter.cell, value));
        }
        self.eval(closure.body, &closure.environment.extended_runtime(bound))
    }

    fn try_apply_callable(
        &mut self,
        callable: RuntimeValue,
        arguments: &[(CellId, RuntimeValue)],
        environment: &Environment,
    ) -> Option<Result<RuntimeValue, Halt>> {
        if let Some(closure) = self.runtime_closure(&callable) {
            return Some(self.call_closure(closure, &callable, arguments));
        }
        let foreign = match &callable.0 {
            RuntimeValueKind::Foreign(cell) => {
                if let Some(foreign) = self.foreign_target_cell(*cell) {
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
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => {
                let cell = callable
                    .as_cell()
                    .or_else(|| callable.field(vocabulary::FFI)?.as_cell());
                if let Some(cell) = cell
                    && self.foreign_target_cell(cell).is_none()
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
            RuntimeValueKind::List(_) | RuntimeValueKind::Closure(_) => None,
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
                    .map(|parameter| self.field(call, parameter.cell))
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
            RuntimeValueKind::Data(_)
            | RuntimeValueKind::Unboxed(_)
            | RuntimeValueKind::Record(_) => {
                let fields = value.field(vocabulary::CLOSURE)?;
                let params = fields
                    .field(vocabulary::PARAMS)?
                    .list_values()?
                    .map(|value| value.as_cell().map(|cell| Parameter { cell }))
                    .collect::<Option<Vec<_>>>()?;
                let origin = fields
                    .field(source::vocabulary::BODY_ORIGIN)
                    .and_then(|value| source::read(value.as_value()))
                    .map(|origin| OriginId(Rc::new(OriginNode::Root(OriginRoot::Located(origin)))));
                let body = self.lower_runtime_code_at(&fields.field(vocabulary::BODY)?, origin);
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
            RuntimeValueKind::List(_) | RuntimeValueKind::Foreign(_) => None,
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
    function: &RuntimeValue,
    arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    host: &dyn Host,
    fuel: usize,
) -> Evaluation {
    context(host, None, fuel).conclude(|context| context.apply_expression(function, arguments))
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
        .conclude(|context| context.apply_expression(function, arguments))
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
            context.apply_expression(
                &function.into(),
                arguments.into_iter().map(|(k, v)| (k, v.into())),
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
mod tests;
