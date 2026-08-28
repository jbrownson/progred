//! A small evaluator whose expressions and results are GID
//! values. The evaluator recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use gid::{CellId, Record, Value};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

pub mod vocabulary {
    use gid::CellId;

    pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
    pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
    pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
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

    pub fn value(reason: CellId) -> Value {
        Value::record([(ABSENT, Value::from(reason))])
    }

    pub fn reason(value: &Value) -> Option<CellId> {
        value.as_record()?.get(&ABSENT).and_then(Value::as_cell)
    }

    pub fn is_absent(value: &Value) -> bool {
        reason(value).is_some()
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

/// A source expression lowered into the current evaluation's arena.
/// Handles never escape that synchronous evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Expression(usize);

/// Where a source expression came from before evaluation. Generated
/// runtime values have no origin; expressions read from the input or a
/// cell retain their structural route for host tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    Input(Vec<gid::Step>),
    Cell { cell: CellId, path: Vec<gid::Step> },
}

/// A staged foreign function's per-visit form, returned by its
/// prepare stage with the once-parsed call structure in its captures.
pub type Stage = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

type Prepare = Rc<dyn Fn(&Context, Expression) -> Stage>;

#[derive(Clone)]
enum ForeignImplementation {
    Direct(Rc<dyn Fn(&mut Context, Expression, &Environment) -> Result<RuntimeValue, Halt>>),
    Staged(Prepare),
}

#[derive(Clone)]
pub struct ForeignFunction {
    implementation: ForeignImplementation,
}

impl ForeignFunction {
    pub fn new(
        call: impl Fn(&mut Context, Expression, &Environment) -> Result<Value, Halt> + 'static,
    ) -> Self {
        Self {
            implementation: ForeignImplementation::Direct(Rc::new(
                move |context, expression, environment| {
                    call(context, expression, environment).map(RuntimeValue::from_value)
                },
            )),
        }
    }

    pub fn runtime(
        call: impl Fn(&mut Context, Expression, &Environment) -> Result<RuntimeValue, Halt> + 'static,
    ) -> Self {
        Self {
            implementation: ForeignImplementation::Direct(Rc::new(call)),
        }
    }

    /// The two-stage form: `prepare` runs when a call site first meets
    /// this function, reads only the call's lowered structure, and
    /// returns the closure that runs per visit — call sites cache that
    /// closure, so prepare must be observation-free (no evaluation,
    /// diagnostics, or fuel) and re-runnable. The returned stage owns
    /// every observable, including the fuel the straightforward shape
    /// would burn.
    pub fn staged(prepare: impl Fn(&Context, Expression) -> Stage + 'static) -> Self {
        Self {
            implementation: ForeignImplementation::Staged(Rc::new(prepare)),
        }
    }
}

pub struct Halt(Value);

/// Values remain Grap's source and result language. During one
/// evaluation, callables stay parsed and bundled library values may
/// use equivalent host representations such as an unboxed f64.
///
/// This enum is several words wide and moves through every argument,
/// binding, and list slot; profiles of drawing-program evaluation put
/// a low-double-digit share of interpreter time in those moves, drop
/// glue, and slot sizes. The known next representation, once the model
/// settles, is a NaN-boxed word: bare f64s, cells, and small
/// immediates inline in 8 bytes, everything else behind a pointer.
/// One current design stands in the way and would need rethinking:
/// enriched f64 records (the open representation keeps metadata beside
/// the number, so only bare numbers can inline).
///
/// Deliberately deferred (2026-08): staged foreign functions and typed
/// per-node channels capture much of the win first, leaving boxing's
/// residual too small for its footprint while the model still moves.
/// When attempted: documents may legitimately carry non-canonical NaN
/// bit patterns in f64 blobs, so decoding must canonicalize NaNs —
/// never treat stray patterns as undefined behavior.
#[derive(Clone)]
pub struct RuntimeValue(RuntimeValueKind);

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
    Foreign(ResolvedForeign),
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
    index: CellIndex,
}

impl RuntimeValue {
    pub fn from_value(value: Value) -> Self {
        Self(RuntimeValueKind::Data(value))
    }

    pub fn f64(number: f64) -> Self {
        Self(RuntimeValueKind::F64(RuntimeF64 {
            number,
            original: None,
        }))
    }

    pub fn record(fields: impl IntoIterator<Item = (CellId, RuntimeValue)>) -> Self {
        let mut fields: Vec<_> = fields.into_iter().collect();
        if fields.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            return Self(RuntimeValueKind::Record(fields.into()));
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
        Self(RuntimeValueKind::Record(fields.into()))
    }

    pub fn list(elements: impl IntoIterator<Item = RuntimeValue>) -> Self {
        Self(RuntimeValueKind::List(elements.into_iter().collect()))
    }

    fn original_f64(number: f64, original: Value) -> Self {
        Self(RuntimeValueKind::F64(RuntimeF64 {
            number,
            original: Some(original),
        }))
    }

    pub fn as_f64(&self) -> Option<f64> {
        match &self.0 {
            RuntimeValueKind::F64(value) => Some(value.number),
            RuntimeValueKind::Data(value) => crate::f64::read(value),
            RuntimeValueKind::Record(_) => crate::f64::read(&self.to_value()),
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
            RuntimeValueKind::List(_)
            | RuntimeValueKind::Foreign(_)
            | RuntimeValueKind::Closure(_) => None,
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

    pub fn is_absent(&self) -> bool {
        self.field(absent::ABSENT)
            .and_then(|reason| reason.as_cell())
            .is_some()
    }

    pub fn record_len(&self) -> Option<usize> {
        match &self.0 {
            RuntimeValueKind::Data(value) => Some(value.as_record()?.len()),
            RuntimeValueKind::Record(fields) => Some(fields.len()),
            RuntimeValueKind::F64(value) => Some(match &value.original {
                Some(original) => original.as_record()?.len(),
                None => 1,
            }),
            RuntimeValueKind::List(_)
            | RuntimeValueKind::Foreign(_)
            | RuntimeValueKind::Closure(_) => None,
        }
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

    pub fn to_value(&self) -> Value {
        self.clone().into_value()
    }

    pub fn into_value(self) -> Value {
        match self.0 {
            RuntimeValueKind::Data(value) => value,
            RuntimeValueKind::F64(value) => value
                .original
                .unwrap_or_else(|| crate::f64::value(value.number)),
            RuntimeValueKind::Record(fields) => Value::record(
                fields
                    .iter()
                    .map(|(field, value)| (*field, value.to_value())),
            ),
            RuntimeValueKind::List(elements) => {
                Value::list(elements.iter().map(RuntimeValue::to_value))
            }
            RuntimeValueKind::Foreign(foreign) => ffi(foreign.cell()),
            RuntimeValueKind::Closure(closure) => Value::record([(
                vocabulary::CLOSURE,
                Value::Record(
                    closure
                        .fields
                        .update(vocabulary::ENVIRONMENT, Value::from(closure.environment)),
                ),
            )]),
        }
    }
}

impl From<Value> for RuntimeValue {
    fn from(value: Value) -> Self {
        Self::from_value(value)
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
    /// Cells ever bound in any environment of this evaluation. Function
    /// references are typically never bound, so they skip the
    /// environment walk that must otherwise prove absence frame by frame.
    bound: Vec<u64>,
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

    fn mark_bound(&mut self, index: CellIndex) {
        let word = index.0 / 64;
        if self.bound.len() <= word {
            self.bound.resize(word + 1, 0);
        }
        self.bound[word] |= 1 << (index.0 & 63);
    }

    fn is_bound(&self, index: CellIndex) -> bool {
        self.bound
            .get(index.0 / 64)
            .is_some_and(|word| word & (1 << (index.0 & 63)) != 0)
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
        .map(|index| fields[index].1)
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
        let index = self.indices.borrow().index(cell)?;
        self.get_index(index).map(RuntimeValue::to_value)
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
                for (index, value) in bindings {
                    self.indices.borrow_mut().mark_bound(index);
                    frame.bindings.push((index, value));
                }
            }
            None => {
                let bindings: Vec<_> = bindings.into_iter().collect();
                if !bindings.is_empty() {
                    let mut table = self.indices.borrow_mut();
                    for (index, _) in &bindings {
                        table.mark_bound(*index);
                    }
                    drop(table);
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
        {
            let mut table = self.indices.borrow_mut();
            for (index, _) in &bindings {
                table.mark_bound(*index);
            }
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
        let indices = environment.indices.borrow();
        let mut frames = Vec::new();
        let mut frame = environment.frame.as_deref();
        while let Some(current) = frame {
            frames.push(current);
            frame = current.parent.as_deref();
        }
        Value::record(frames.into_iter().rev().flat_map(|frame| {
            frame
                .bindings
                .iter()
                .map(|(index, value)| (indices.cell(*index), value.to_value()))
        }))
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

    fn get(&self, function: CellId) -> Option<&ForeignFunction> {
        self.functions
            .binary_search_by_key(&function, |(cell, _)| *cell)
            .ok()
            .map(|index| &self.functions[index].1)
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

type ScopedCall<'a> = dyn for<'context> Fn(
        CellId,
        &mut Context<'context>,
        Expression,
        &Environment,
    ) -> Result<Value, Halt>
    + 'a;

/// A synchronous, borrowed layer of foreign functions. It lets a host
/// expose state that is valid only for one evaluation without putting
/// that state in `'static` closures or rebuilding the permanent table.
pub struct ForeignOverlay<'a> {
    functions: &'a [CellId],
    call: &'a ScopedCall<'a>,
}

impl<'a> ForeignOverlay<'a> {
    pub fn new(
        functions: &'a [CellId],
        call: &'a (
                impl for<'context> Fn(
            CellId,
            &mut Context<'context>,
            Expression,
            &Environment,
        ) -> Result<Value, Halt>
                + 'a
            ),
    ) -> Self {
        Self { functions, call }
    }

    fn handles(&self, function: CellId) -> bool {
        self.functions.contains(&function)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    FuelExhausted,
    MissingCell(CellId),
    CellCycle(Vec<CellId>),
    MalformedLambda,
    InvalidParameter(Value),
    NotCallable(Value),
    MissingArgument(CellId),
    InvalidEnvironment(Value),
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Diagnostic::FuelExhausted => write!(f, "evaluation ran out of fuel"),
            Diagnostic::MissingCell(cell) => write!(f, "cell {cell} has no value"),
            Diagnostic::CellCycle(cells) => write!(
                f,
                "cell resolution cycle: {}",
                cells
                    .iter()
                    .map(CellId::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            Diagnostic::MalformedLambda => {
                write!(f, "lambda params must be a list")
            }
            Diagnostic::InvalidParameter(value) => {
                write!(f, "parameter is not a cell: {value:?}")
            }
            Diagnostic::NotCallable(value) => write!(f, "value is not callable: {value:?}"),
            Diagnostic::MissingArgument(cell) => write!(f, "missing argument {cell}"),
            Diagnostic::InvalidEnvironment(value) => {
                write!(f, "evaluation environment is not a record: {value:?}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub result: Value,
    pub diagnostics: Vec<Diagnostic>,
    pub dependencies: BTreeSet<CellId>,
    pub remaining_fuel: usize,
}

pub struct Context<'a> {
    resolve: &'a dyn Fn(CellId) -> Option<Value>,
    foreign: &'a ForeignFunctions,
    overlay: Option<&'a ForeignOverlay<'a>>,
    foreign_scopes: Vec<ForeignFunctions>,
    remaining_fuel: usize,
    diagnostics: Vec<Diagnostic>,
    dependencies: BTreeSet<CellId>,
    resolving: Vec<CellId>,
    expressions: Vec<Lowered>,
    origins: Vec<OriginNode>,
    cell_states: Vec<CellState>,
    /// Per cell index, the remembered overlay/permanent resolution
    /// (`None` = not resolved yet); scoped tables are never cached.
    foreign_cache: Vec<Option<Option<ResolvedForeign>>>,
    indices: CellIndices,
}

#[derive(Clone)]
struct Lowered {
    source: Value,
    origin: Option<OriginId>,
    form: Form,
    fields: Option<Vec<(CellId, Expression)>>,
    elements: Option<Vec<Expression>>,
    /// The lowered runtime form of a data expression, reused across
    /// evaluations of the same node.
    data_runtime: Option<RuntimeValue>,
    /// The node generated into a host closure on first evaluation; the
    /// arena stays the authoritative representation the closure runs.
    compiled: Option<Thunk>,
}

/// A lowered node's generated form: per-node decisions — dispatch,
/// argument lookup, lambda plumbing — made once at generation, then
/// execution is one call. Fuel, diagnostics, and results match the
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

#[derive(Clone, Copy)]
struct OriginId(usize);

enum OriginRoot {
    Input,
    Cell(CellId),
}

enum OriginNode {
    Root(OriginRoot),
    Child { parent: OriginId, step: gid::Step },
}

#[derive(Clone)]
enum Form {
    Data,
    Cell(CellIndex),
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

#[derive(Clone, Copy)]
enum CellState {
    Unknown,
    Ready(Expression),
    Evaluating { stack_index: usize },
}

#[derive(Clone)]
enum ResolvedForeign {
    Permanent {
        cell: CellId,
        function: ForeignFunction,
    },
    Scoped(CellId),
}

impl ResolvedForeign {
    fn cell(&self) -> CellId {
        match self {
            Self::Permanent { cell, .. } | Self::Scoped(cell) => *cell,
        }
    }
}

impl<'a> Context<'a> {
    fn conclude(mut self, run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>) -> Evaluation {
        let result = run(&mut self)
            .map(RuntimeValue::into_value)
            .unwrap_or_else(|Halt(result)| result);
        Evaluation {
            result,
            diagnostics: self.diagnostics,
            dependencies: self.dependencies,
            remaining_fuel: self.remaining_fuel,
        }
    }

    fn run(self, expression: &Value) -> Evaluation {
        self.conclude(|context| {
            let expression = context.lower_source(expression, OriginRoot::Input);
            let environment = Environment::with_indices(context.indices.clone());
            context.eval_runtime(expression, &environment)
        })
    }

    fn lower(&mut self, value: &Value) -> Expression {
        self.lower_with(value, false, None)
    }

    fn lower_source(&mut self, value: &Value, root: OriginRoot) -> Expression {
        let origin = OriginId(self.origins.len());
        self.origins.push(OriginNode::Root(root));
        self.lower_with(value, true, Some(origin))
    }

    fn lower_unattributed_source(&mut self, value: &Value) -> Expression {
        self.lower_with(value, true, None)
    }

    fn child_origin(&mut self, parent: Option<OriginId>, step: gid::Step) -> Option<OriginId> {
        parent.map(|parent| {
            let origin = OriginId(self.origins.len());
            self.origins.push(OriginNode::Child { parent, step });
            origin
        })
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
            Value::Cell(cell) => Form::Cell(cell_index(&self.indices, *cell)),
            Value::Record(fields) if fields.contains_key(&vocabulary::FUNCTION) => {
                let fields: Vec<_> = fields
                    .iter()
                    .map(|(field, value)| {
                        let child = self.child_origin(origin, gid::Step::Key(*field));
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
                                Some(cell) => parsed.push(Parameter {
                                    cell,
                                    index: cell_index(&self.indices, cell),
                                }),
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
                        let child = self.child_origin(origin, gid::Step::Key(vocabulary::BODY));
                        self.lower_with(fields.get(&vocabulary::BODY).unwrap(), descend_data, child)
                    },
                }
            }
            Value::Record(fields) => {
                if descend_data {
                    lowered_fields = Some(
                        fields
                            .iter()
                            .map(|(field, value)| {
                                let child = self.child_origin(origin, gid::Step::Key(*field));
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
                                let child =
                                    self.child_origin(origin, gid::Step::Element(position.clone()));
                                self.lower_with(value, true, child)
                            })
                            .collect(),
                    );
                }
                Form::Data
            }
            Value::Blob(_) => Form::Data,
        };
        let expression = Expression(self.expressions.len());
        self.expressions.push(Lowered {
            source: value.clone(),
            origin,
            form,
            fields: lowered_fields,
            elements: lowered_elements,
            data_runtime: None,
            compiled: None,
        });
        expression
    }

    pub fn value(&self, expression: Expression) -> &Value {
        &self.expressions[expression.0].source
    }

    pub fn source_origin(&self, expression: Expression) -> Option<SourceOrigin> {
        let mut origin = self.expressions[expression.0].origin?;
        let mut path = Vec::new();
        loop {
            match &self.origins[origin.0] {
                OriginNode::Root(OriginRoot::Input) => {
                    path.reverse();
                    return Some(SourceOrigin::Input(path));
                }
                OriginNode::Root(OriginRoot::Cell(cell)) => {
                    path.reverse();
                    return Some(SourceOrigin::Cell { cell: *cell, path });
                }
                OriginNode::Child { parent, step } => {
                    path.push(step.clone());
                    origin = *parent;
                }
            }
        }
    }

    pub fn eval(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        debug_assert!(
            Rc::ptr_eq(&self.indices, &environment.indices),
            "use Context::environment to decode an environment for this evaluation",
        );
        self.eval_runtime(expression, environment)
            .map(RuntimeValue::into_value)
    }

    pub fn eval_value(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        let expression = self.lower(expression);
        self.eval(expression, environment)
    }

    pub fn eval_value_runtime(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower(expression);
        self.eval_runtime(expression, environment)
    }

    /// Decode a Grap environment using this evaluation's cell-index table.
    /// Environments passed back into this Context must share that table
    /// with its lowered cell references.
    pub fn environment(&self, value: &Value) -> Option<Environment> {
        let fields = value.as_record()?;
        Some(
            Environment::with_indices(self.indices.clone())
                .extended(fields.iter().map(|(cell, value)| (*cell, value.clone()))),
        )
    }

    pub fn eval_runtime(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.eval_burned(expression, environment)
    }

    /// Evaluate an expression a caller will read as a number, skipping
    /// the owned `RuntimeValue` round trip when the answer is already
    /// an unboxed f64. Fuel, diagnostics, and results match
    /// `eval_runtime` followed by [`RuntimeValue::as_f64`] exactly.
    pub fn eval_f64(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Option<f64>, Halt> {
        self.burn()?;
        match &self.expressions[expression.0].form {
            Form::Data => {
                if let Some(RuntimeValue(RuntimeValueKind::F64(cached))) =
                    self.expressions[expression.0].data_runtime.as_ref()
                {
                    return Ok(Some(cached.number));
                }
            }
            Form::Cell(index) => {
                if self.indices.borrow().is_bound(*index)
                    && let Some(RuntimeValue(RuntimeValueKind::F64(value))) =
                        environment.get_index(*index)
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
        let compiled = match &self.expressions[expression.0].compiled {
            Some(compiled) => compiled.clone(),
            None => {
                let compiled = self.compile(expression);
                self.expressions[expression.0].compiled = Some(compiled.clone());
                compiled
            }
        };
        compiled(self, environment)
    }

    fn compile(&mut self, expression: Expression) -> Thunk {
        match self.expressions[expression.0].form.clone() {
            Form::Data => thunk(move |context, _| {
                if let Some(cached) = &context.expressions[expression.0].data_runtime {
                    return Ok(cached.clone());
                }
                let value = context
                    .lower_runtime(RuntimeValue::from_value(context.value(expression).clone()));
                context.expressions[expression.0].data_runtime = Some(value.clone());
                Ok(value)
            }),
            Form::Cell(index) => {
                thunk(move |context, environment| context.eval_cell(index, environment))
            }
            Form::Call { function } => self.compile_call(expression, function),
            Form::Lambda { parameters, body } => {
                let fields = self.value(expression).as_record().unwrap().clone();
                match parameters {
                    LambdaParameters::Valid(params) => thunk(move |_, environment| {
                        Ok(RuntimeValue(RuntimeValueKind::Closure(Closure {
                            fields: fields.clone(),
                            params: params.clone(),
                            body,
                            environment: environment.clone(),
                        })))
                    }),
                    LambdaParameters::Malformed => thunk(move |context, _| {
                        Ok(RuntimeValue::from_value(context.absent(
                            Diagnostic::MalformedLambda,
                            absent::MALFORMED_LAMBDA,
                        )))
                    }),
                    LambdaParameters::Invalid(parameter) => thunk(move |context, _| {
                        Ok(RuntimeValue::from_value(context.absent(
                            Diagnostic::InvalidParameter(parameter.clone()),
                            absent::INVALID_PARAMETER,
                        )))
                    }),
                }
            }
        }
    }

    fn compile_call(&mut self, call: Expression, function: Expression) -> Thunk {
        let function_cell = match &self.expressions[function.0].form {
            Form::Cell(index) => Some(*index),
            _ => None,
        };
        let plan: RefCell<Option<CallPlan>> = RefCell::new(None);
        let stages: RefCell<Option<(Prepare, Stage)>> = RefCell::new(None);
        thunk(move |context, environment| {
            let callable = match function_cell {
                Some(index) => {
                    context.burn()?;
                    context.eval_cell(index, environment)?
                }
                None => context.eval_runtime(function, environment)?,
            };
            let callable = match callable.0 {
                RuntimeValueKind::Closure(closure) => {
                    return context.eval_grap_call(closure, call, environment, Some(&plan));
                }
                RuntimeValueKind::Foreign(foreign) => {
                    return context.call_foreign_staged(&foreign, call, environment, Some(&stages));
                }
                other => RuntimeValue(other),
            };
            if let Some(closure) = context.runtime_closure(&callable) {
                return context.eval_grap_call(closure, call, environment, Some(&plan));
            }
            let foreign = match &callable.0 {
                RuntimeValueKind::Data(value) => context.foreign_target(value),
                _ => None,
            };
            match foreign {
                Some(foreign) => context.call_foreign(&foreign, call, environment),
                None => Ok(RuntimeValue::from_value(context.absent(
                    Diagnostic::NotCallable(callable.into_value()),
                    absent::NOT_CALLABLE,
                ))),
            }
        })
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
            Err(Halt(
                self.absent(Diagnostic::FuelExhausted, absent::FUEL_EXHAUSTED),
            ))
        } else {
            Ok(())
        }
    }

    pub fn field(&self, call: Expression, label: CellId) -> Option<Expression> {
        lowered_field(self.expressions[call.0].fields.as_ref()?, label)
    }

    pub fn fields(&self, expression: Expression) -> Option<&[(CellId, Expression)]> {
        self.expressions[expression.0].fields.as_deref()
    }

    pub fn elements(&self, expression: Expression) -> Option<&[Expression]> {
        self.expressions[expression.0].elements.as_deref()
    }

    pub fn missing_argument(&mut self, cell: CellId) -> Value {
        self.absent(Diagnostic::MissingArgument(cell), absent::MISSING_ARGUMENT)
    }

    pub fn missing_runtime_argument(&mut self, cell: CellId) -> RuntimeValue {
        self.missing_argument(cell).into()
    }

    fn absent(&mut self, diagnostic: Diagnostic, cell: CellId) -> Value {
        self.diagnostics.push(diagnostic);
        absent::value(cell)
    }

    fn eval_cell(
        &mut self,
        index: CellIndex,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        if self.indices.borrow().is_bound(index)
            && let Some(value) = environment.get_index(index)
        {
            let value = value.clone();
            return Ok(self.lower_runtime(value));
        }
        let cell = self.indices.borrow().cell(index);
        if let Some(foreign) = self.cached_foreign_target(index, cell) {
            Ok(RuntimeValue(RuntimeValueKind::Foreign(foreign)))
        } else {
            while self.cell_states.len() <= index.0 {
                self.cell_states.push(CellState::Unknown);
            }
            match self.cell_states[index.0] {
                CellState::Ready(expression) => {
                    self.eval_resolved_cell(index, cell, expression, environment)
                }
                CellState::Evaluating { stack_index } => Ok(RuntimeValue::from_value(
                    self.absent(
                        Diagnostic::CellCycle(
                            self.resolving[stack_index..]
                                .iter()
                                .copied()
                                .chain([cell])
                                .collect(),
                        ),
                        absent::CELL_CYCLE,
                    ),
                )),
                CellState::Unknown => {
                    self.dependencies.insert(cell);
                    let Some(value) = (self.resolve)(cell) else {
                        return Ok(RuntimeValue::from_value(
                            self.absent(Diagnostic::MissingCell(cell), absent::MISSING_CELL),
                        ));
                    };
                    let expression = self.lower_source(&value, OriginRoot::Cell(cell));
                    self.cell_states[index.0] = CellState::Ready(expression);
                    self.eval_resolved_cell(index, cell, expression, environment)
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
        let result = self.eval_runtime(expression, environment);
        self.resolving.pop();
        self.cell_states[index.0] = CellState::Ready(expression);
        result
    }

    fn cached_foreign_target(&mut self, index: CellIndex, cell: CellId) -> Option<ResolvedForeign> {
        if let Some(resolved) = self.scoped_foreign_target(cell) {
            return Some(resolved);
        }
        if self.foreign_cache.len() <= index.0 {
            self.foreign_cache.resize(index.0 + 1, None);
        }
        if let Some(resolved) = &self.foreign_cache[index.0] {
            return resolved.clone();
        }
        let resolved = self.fixed_foreign_target(cell);
        self.foreign_cache[index.0] = Some(resolved.clone());
        resolved
    }

    fn foreign_target_cell(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.scoped_foreign_target(cell)
            .or_else(|| self.fixed_foreign_target(cell))
    }

    fn scoped_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        self.foreign_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(cell))
            .map(|function| ResolvedForeign::Permanent {
                cell,
                function: function.clone(),
            })
    }

    /// Resolution through the borrowed overlay and permanent table only.
    /// Both are fixed for the whole evaluation, so this answer may be
    /// remembered per cell; scoped tables are always consulted live.
    fn fixed_foreign_target(&self, cell: CellId) -> Option<ResolvedForeign> {
        if self.overlay.is_some_and(|overlay| overlay.handles(cell)) {
            Some(ResolvedForeign::Scoped(cell))
        } else {
            self.foreign
                .get(cell)
                .cloned()
                .map(|function| ResolvedForeign::Permanent { cell, function })
        }
    }

    fn foreign_target(&self, callable: &Value) -> Option<ResolvedForeign> {
        let cell = callable.as_record()?.get(&vocabulary::FFI)?.as_cell()?;
        self.foreign_target_cell(cell)
    }

    fn call_foreign(
        &mut self,
        foreign: &ResolvedForeign,
        call: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.call_foreign_staged(foreign, call, environment, None)
    }

    fn call_foreign_staged(
        &mut self,
        foreign: &ResolvedForeign,
        call: Expression,
        environment: &Environment,
        stages: Option<&RefCell<Option<(Prepare, Stage)>>>,
    ) -> Result<RuntimeValue, Halt> {
        let value = match foreign {
            ResolvedForeign::Permanent { function, .. } => match &function.implementation {
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
                function(*cell, self, call, environment).map(RuntimeValue::from_value)
            }
        }?;
        Ok(self.lower_runtime(value))
    }

    /// Apply a callable to already-evaluated argument values inside
    /// this evaluation — the in-context form of [`apply`].
    pub fn apply(
        &mut self,
        function: &Value,
        arguments: impl IntoIterator<Item = (CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.apply_values(function, arguments.into_iter().collect())
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
        debug_assert!(
            Rc::ptr_eq(&self.indices, &environment.indices),
            "a prepared callable must use this evaluation's environment",
        );
        let callable = self.eval_runtime(expression, environment)?;
        let callable = match &callable.0 {
            RuntimeValueKind::Data(_) => self
                .runtime_closure(&callable)
                .map(|closure| RuntimeValue(RuntimeValueKind::Closure(closure)))
                .unwrap_or(callable),
            _ => callable,
        };
        Ok(PreparedCallable {
            value: callable,
            environment: environment.clone(),
        })
    }

    /// Invoke a prepared callable with argument VALUES. It preserves
    /// the ordinary call/function/argument fuel steps while avoiding
    /// a temporary call-shaped Value for Grap closures.
    pub fn call_prepared(
        &mut self,
        callable: &PreparedCallable,
        arguments: impl IntoIterator<Item = (CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.call_prepared_runtime(
            callable,
            arguments
                .into_iter()
                .map(|(cell, value)| (cell, RuntimeValue::from_value(value))),
        )
        .map(RuntimeValue::into_value)
    }

    pub fn call_prepared_runtime(
        &mut self,
        callable: &PreparedCallable,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.burn()?;
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
                            return Ok(self.missing_runtime_argument(parameter.cell));
                        };
                        self.lower_runtime(value.clone())
                    };
                    self.burn()?;
                    bound.push((parameter.index, value));
                }
                self.eval_runtime(closure.body, &closure.environment.extended_indexed(bound))
            }
            RuntimeValueKind::Foreign(foreign) => {
                let call = call(
                    callable.value.clone().into_value(),
                    arguments
                        .into_iter()
                        .map(|(cell, value)| (cell, value.into_value())),
                );
                let call = self.lower(&call);
                self.call_foreign(foreign, call, &callable.environment)
            }
            RuntimeValueKind::Data(value) => {
                let Some(target) = self.foreign_target(value) else {
                    return Ok(RuntimeValue::from_value(self.absent(
                        Diagnostic::NotCallable(value.clone()),
                        absent::NOT_CALLABLE,
                    )));
                };
                let call = call(
                    value.clone(),
                    arguments
                        .into_iter()
                        .map(|(cell, value)| (cell, value.into_value())),
                );
                let call = self.lower(&call);
                self.call_foreign(&target, call, &callable.environment)
            }
            RuntimeValueKind::F64(_) | RuntimeValueKind::Record(_) | RuntimeValueKind::List(_) => {
                Ok(RuntimeValue::from_value(self.absent(
                    Diagnostic::NotCallable(callable.value.to_value()),
                    absent::NOT_CALLABLE,
                )))
            }
        }
    }

    fn apply_values(
        &mut self,
        function: &Value,
        arguments: Vec<(CellId, Value)>,
    ) -> Result<Value, Halt> {
        self.apply_values_runtime(function, arguments)
            .map(RuntimeValue::into_value)
    }

    fn apply_values_runtime(
        &mut self,
        function: &Value,
        arguments: Vec<(CellId, Value)>,
    ) -> Result<RuntimeValue, Halt> {
        let environment = Environment::with_indices(self.indices.clone());
        let function = self.lower_source(function, OriginRoot::Input);
        let callable = self.eval_runtime(function, &environment)?;
        if let Some(closure) = self.runtime_closure(&callable) {
            let mut bound = Vec::with_capacity(closure.params.len());
            for parameter in closure.params.iter() {
                match arguments.iter().find(|(cell, _)| *cell == parameter.cell) {
                    Some((_, value)) => bound.push((
                        parameter.index,
                        self.lower_runtime(RuntimeValue::from_value(value.clone())),
                    )),
                    None => {
                        return Ok(RuntimeValue::from_value(
                            self.missing_argument(parameter.cell),
                        ));
                    }
                }
            }
            return self.eval_runtime(closure.body, &closure.environment.extended_indexed(bound));
        }
        let foreign = match &callable.0 {
            RuntimeValueKind::Foreign(foreign) => Some(foreign.clone()),
            RuntimeValueKind::Data(value) => self.foreign_target(value),
            RuntimeValueKind::F64(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::List(_)
            | RuntimeValueKind::Closure(_) => None,
        };
        let callable_value = callable.clone().into_value();
        match foreign {
            Some(foreign) => {
                let call = self.lower(&call(callable_value, arguments));
                self.call_foreign(&foreign, call, &environment)
            }
            None => Ok(RuntimeValue::from_value(self.absent(
                Diagnostic::NotCallable(callable.into_value()),
                absent::NOT_CALLABLE,
            ))),
        }
    }

    fn eval_grap_call(
        &mut self,
        closure: Closure,
        call: Expression,
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
                parameter.index,
                self.eval_runtime(*expression, calling_environment)?,
            ));
        }
        let body_environment = closure.environment.extended_indexed(arguments);
        self.eval_runtime(closure.body, &body_environment)
    }

    fn runtime_closure(&mut self, value: &RuntimeValue) -> Option<Closure> {
        match &value.0 {
            RuntimeValueKind::Closure(closure) => Some(closure.clone()),
            RuntimeValueKind::Data(value) => {
                let fields = value
                    .as_record()?
                    .get(&vocabulary::CLOSURE)?
                    .as_record()?
                    .clone();
                let params = fields
                    .get(&vocabulary::PARAMS)?
                    .as_list()?
                    .values()
                    .map(Value::as_cell)
                    .map(|cell| {
                        cell.map(|cell| Parameter {
                            cell,
                            index: cell_index(&self.indices, cell),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let body = self.lower_unattributed_source(fields.get(&vocabulary::BODY)?);
                Some(Closure {
                    params: params.into(),
                    body,
                    environment: self.environment(fields.get(&vocabulary::ENVIRONMENT)?)?,
                    fields,
                })
            }
            RuntimeValueKind::F64(_)
            | RuntimeValueKind::Record(_)
            | RuntimeValueKind::List(_)
            | RuntimeValueKind::Foreign(_) => None,
        }
    }
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

pub fn evaluate(
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> Evaluation {
    Context {
        resolve: &resolve,
        foreign,
        overlay: None,
        foreign_scopes: Vec::new(),
        remaining_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
        expressions: Vec::new(),
        origins: Vec::new(),
        cell_states: Vec::new(),
        foreign_cache: Vec::new(),
        indices: CellIndices::default(),
    }
    .run(expression)
}

/// Evaluate with a borrowed foreign-function layer that exists only for
/// this synchronous evaluation.
pub fn evaluate_scoped<'a>(
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &'a ForeignFunctions,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    Context {
        resolve: &resolve,
        foreign,
        overlay: Some(overlay),
        foreign_scopes: Vec::new(),
        remaining_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
        expressions: Vec::new(),
        origins: Vec::new(),
        cell_states: Vec::new(),
        foreign_cache: Vec::new(),
        indices: CellIndices::default(),
    }
    .run(expression)
}

/// Apply a callable to already-evaluated argument VALUES. This is the
/// host boundary: a code-shaped value (a stored lambda or call
/// record) binds as data, where `call` + [`evaluate`] would evaluate
/// it as an expression. A foreign target still receives the
/// arguments as call fields and evaluates them itself; every value
/// but a code-shaped one self-quotes through that.
pub fn apply(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> Evaluation {
    Context {
        resolve: &resolve,
        foreign,
        overlay: None,
        foreign_scopes: Vec::new(),
        remaining_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
        expressions: Vec::new(),
        origins: Vec::new(),
        cell_states: Vec::new(),
        foreign_cache: Vec::new(),
        indices: CellIndices::default(),
    }
    .conclude(|context| context.apply_values_runtime(function, arguments.into_iter().collect()))
}

/// Apply with a borrowed foreign-function layer that exists only for
/// this synchronous evaluation.
pub fn apply_scoped<'a>(
    function: &Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &'a ForeignFunctions,
    overlay: &'a ForeignOverlay<'a>,
    fuel: usize,
) -> Evaluation {
    Context {
        resolve: &resolve,
        foreign,
        overlay: Some(overlay),
        foreign_scopes: Vec::new(),
        remaining_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
        expressions: Vec::new(),
        origins: Vec::new(),
        cell_states: Vec::new(),
        foreign_cache: Vec::new(),
        indices: CellIndices::default(),
    }
    .conclude(|context| context.apply_values_runtime(function, arguments.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use std::cell::Cell;

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
            |candidate| (candidate == cell).then(|| blob("not followed")),
            &ForeignFunctions::default(),
            10,
        );
        assert_eq!(evaluation.result, value);
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn consuming_the_entire_fuel_allowance_is_exhaustion() {
        let value = blob("one step");
        let exhausted = evaluate(&value, |_| None, &ForeignFunctions::default(), 1);
        assert_eq!(exhausted.result, absent::value(absent::FUEL_EXHAUSTED));
        assert_eq!(exhausted.diagnostics, [Diagnostic::FuelExhausted]);
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
                ForeignFunction::runtime(move |context, call, environment| {
                    let Some(argument) = context.field(call, argument) else {
                        return Ok(context.missing_runtime_argument(argument));
                    };
                    context.eval_runtime(argument, environment)?;
                    context.eval_runtime(argument, environment)
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
                ForeignFunction::runtime(move |context, call, environment| {
                    let Some(argument) = context.field(call, argument) else {
                        return Ok(context.missing_runtime_argument(argument));
                    };
                    let before = context.eval_runtime(argument, environment)?;
                    let during = context.with_foreign_functions(scoped.clone(), |context| {
                        context.eval_runtime(argument, environment)
                    })?;
                    let after = context.eval_runtime(argument, environment)?;
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
        assert_eq!(base_preparations.get(), 2);
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
        assert_eq!(evaluation.dependencies, BTreeSet::from([first, second]));
    }

    #[test]
    fn graph_functions_match_parameter_labelled_fields() {
        let function_cell = new_cell_id();
        let x = new_cell_id();
        let y = new_cell_id();
        let definition = lambda([x, y], Value::from(x));
        let expression = call(
            Value::from(function_cell),
            [(x, blob("x")), (y, Value::from(new_cell_id()))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| (cell == function_cell).then(|| definition.clone()),
            &ForeignFunctions::default(),
            30,
        );
        assert_eq!(evaluation.result, blob("x"));
        assert_eq!(evaluation.dependencies.len(), 2);
        assert!(matches!(
            evaluation.diagnostics.as_slice(),
            [Diagnostic::MissingCell(_)]
        ));
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
            |cell| (cell == parameter).then(|| blob("document")),
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(evaluation.result, blob("local"));
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn parameters_shadow_foreign_functions() {
        let function = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::new(|_, _, _| Ok(blob("foreign"))),
        );
        let shadowed = call(
            lambda([function], Value::from(function)),
            [(function, blob("bound"))],
        );
        assert_eq!(
            evaluate(&shadowed, |_| None, &foreign, 30).result,
            blob("bound"),
        );
        let unshadowed = evaluate(&Value::from(function), |_| None, &foreign, 30);
        assert_eq!(unshadowed.result, ffi(function));
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
            ForeignFunction::new(|context, call, _| match context.field(call, INPUT) {
                Some(value) => Ok(context.value(value).clone()),
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
    fn registered_cells_evaluate_to_explicit_foreign_callables() {
        const INPUT: CellId = CellId::from_u128(0x2e9c4a71b8d0563f91a0c7e4d15b6820);
        let echo = new_cell_id();
        let input = INPUT;
        let foreign = ForeignFunctions::default().register(
            echo,
            ForeignFunction::new(
                |context, call, environment| match context.field(call, INPUT) {
                    Some(value) => context.eval(value, environment),
                    None => Ok(context.missing_argument(INPUT)),
                },
            ),
        );

        let evaluation = evaluate(
            &Value::from(echo),
            |_| Some(blob("foreign cells do not resolve as data")),
            &foreign,
            10,
        );
        let callable = Value::record([(vocabulary::FFI, Value::from(echo))]);
        assert_eq!(evaluation.result, callable);
        assert!(evaluation.dependencies.is_empty());
        assert_eq!(
            evaluate(
                &call(Value::from(echo), [(input, Value::from(echo))]),
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
            ForeignFunction::new(
                |context, call, environment| match context.field(call, INPUT) {
                    Some(value) => context.eval(value, environment),
                    None => Ok(context.missing_argument(INPUT)),
                },
            ),
        );
        let apply = lambda(
            [callable, input],
            call(Value::from(callable), [(input, Value::from(input))]),
        );
        let evaluation = evaluate(
            &call(
                apply,
                [(callable, Value::from(echo)), (input, blob("passed"))],
            ),
            |_| None,
            &foreign,
            40,
        );
        assert_eq!(evaluation.result, blob("passed"));
        assert!(evaluation.diagnostics.is_empty());
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
            ForeignFunction::new(|context, call, environment| {
                let Some(condition) = context.field(call, CONDITION) else {
                    return Ok(context.missing_argument(CONDITION));
                };
                let Some(yes) = context.field(call, YES) else {
                    return Ok(context.missing_argument(YES));
                };
                let Some(no) = context.field(call, NO) else {
                    return Ok(context.missing_argument(NO));
                };
                if context.eval(condition, environment)? == blob("true") {
                    context.eval(yes, environment)
                } else {
                    context.eval(no, environment)
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
            |_| None,
            &foreign,
            50,
        );
        assert_eq!(evaluation.result, blob("selected"));
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn rust_functions_can_expose_the_calling_environment_as_graph_data() {
        let inspect = new_cell_id();
        let parameter = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            inspect,
            ForeignFunction::new(|_, _, environment| Ok(Value::from(environment))),
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
            ForeignFunction::new(|context, call, environment| {
                let Some(value) = context.field(call, VALUE) else {
                    return Ok(context.missing_argument(VALUE));
                };
                let Some(body) = context.field(call, BODY) else {
                    return Ok(context.missing_argument(BODY));
                };
                let value = context.eval(value, environment)?;
                context.eval(body, &environment.extended([(BINDING, value)]))
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
        assert!(evaluation.dependencies.is_empty());
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
    fn absents_are_stable_values_with_diagnostics() {
        let missing = new_cell_id();
        let evaluation = evaluate(
            &Value::from(missing),
            |_| None,
            &ForeignFunctions::default(),
            10,
        );
        assert_eq!(evaluation.result, absent::value(absent::MISSING_CELL));
        assert_eq!(evaluation.diagnostics, [Diagnostic::MissingCell(missing)]);

        let malformed = Value::record([
            (vocabulary::PARAMS, blob("not a list")),
            (vocabulary::BODY, blob("body")),
        ]);
        let evaluation = evaluate(&malformed, |_| None, &ForeignFunctions::default(), 10);
        assert_eq!(evaluation.result, absent::value(absent::MALFORMED_LAMBDA));
    }

    #[test]
    fn incomplete_lambda_shapes_are_inert_data() {
        for incomplete in [
            Value::record([(vocabulary::PARAMS, Value::list(Vec::<Value>::new()))]),
            Value::record([(vocabulary::BODY, blob("body"))]),
        ] {
            let evaluation = evaluate(&incomplete, |_| None, &ForeignFunctions::default(), 10);
            assert_eq!(evaluation.result, incomplete);
            assert!(evaluation.diagnostics.is_empty());
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
        assert_eq!(evaluation.result, absent::value(absent::CELL_CYCLE));
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
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn foreign_functions_may_consume_the_function_field() {
        const PARAMETER: CellId = CellId::from_u128(0x4b8e0d27c1a9563f80e2c4a7d6b1359e);
        let function = new_cell_id();
        let parameter = PARAMETER;
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::new(|context, call, _| {
                let Some(function) = context.field(call, vocabulary::FUNCTION) else {
                    return Ok(context.missing_argument(vocabulary::FUNCTION));
                };
                let Some(value) = context.field(call, PARAMETER) else {
                    return Ok(context.missing_argument(PARAMETER));
                };
                assert!(context.value(function).as_cell().is_some());
                Ok(context.value(value).clone())
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
        let left = ForeignFunctions::default()
            .register(function, ForeignFunction::new(|_, _, _| Ok(blob("left"))));
        let right = ForeignFunctions::default()
            .register(function, ForeignFunction::new(|_, _, _| Ok(blob("right"))));
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
        assert!(applied.diagnostics.is_empty());
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
        assert_eq!(applied.dependencies, BTreeSet::from([function]));
    }

    #[test]
    fn apply_reaches_foreign_targets_with_the_arguments_as_fields() {
        let function = new_cell_id();
        let foreign = ForeignFunctions::default().register(
            function,
            ForeignFunction::new(|context, call, environment| {
                let argument = context.field(call, CellId::from_u128(7)).unwrap();
                context.eval(argument, environment)
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
                ForeignFunction::new(move |context, call, environment| {
                    let function = context.field(call, vocabulary::FUNCTION).unwrap();
                    assert_eq!(
                        context
                            .value(function)
                            .as_record()
                            .and_then(|fields| fields.get(&decoration)),
                        Some(&blob("retained")),
                    );
                    let argument = context.field(call, argument).unwrap();
                    context.eval(argument, environment)
                }),
            )
            .register(
                control,
                ForeignFunction::new(move |context, call, environment| {
                    let callable = context.field(call, callable).unwrap();
                    let callable = context.prepare_callable(callable, environment)?;
                    context.call_prepared(&callable, [(argument, blob("passed"))])
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
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn a_scoped_foreign_layer_borrows_state_and_remains_reentrant() {
        let outer = new_cell_id();
        let inner = new_cell_id();
        let calls = std::cell::Cell::new(0);
        let functions = [outer, inner];
        let scoped =
            |function, context: &mut Context<'_>, _: Expression, environment: &Environment| {
                calls.set(calls.get() + 1);
                if function == outer {
                    context.eval_value(&call(Value::from(inner), []), environment)
                } else {
                    Ok(blob("scoped"))
                }
            };
        let overlay = ForeignOverlay::new(&functions, &scoped);
        let applied = apply_scoped(
            &Value::from(outer),
            [],
            |_| None,
            &ForeignFunctions::default(),
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
        let scoped = |_, _: &mut Context<'_>, _: Expression, _: &Environment| {
            calls.set(calls.get() + 1);
            Ok(blob("drawn"))
        };
        let overlay = ForeignOverlay::new(&functions, &scoped);
        let evaluation = evaluate_scoped(
            &call(Value::from(function), []),
            |_| None,
            &ForeignFunctions::default(),
            &overlay,
            10,
        );
        assert_eq!(evaluation.result, blob("drawn"));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_foreign_call_retains_its_cell_relative_source_origin() {
        let function = new_cell_id();
        let foreign = new_cell_id();
        let source = RefCell::new(None);
        let functions = [foreign];
        let scoped = |_, context: &mut Context<'_>, call: Expression, _: &Environment| {
            *source.borrow_mut() = context.source_origin(call);
            Ok(blob("drawn"))
        };
        let overlay = ForeignOverlay::new(&functions, &scoped);
        let stored = lambda([], call(Value::from(foreign), []));

        let evaluation = evaluate_scoped(
            &call(Value::from(function), []),
            |cell| (cell == function).then(|| stored.clone()),
            &ForeignFunctions::default(),
            &overlay,
            20,
        );

        assert_eq!(evaluation.result, blob("drawn"));
        assert_eq!(
            *source.borrow(),
            Some(SourceOrigin::Cell {
                cell: function,
                path: vec![gid::Step::Key(vocabulary::BODY)],
            }),
        );
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
        assert_eq!(missing.result, absent::value(absent::MISSING_ARGUMENT));
        assert!(!missing.diagnostics.is_empty());

        let uncallable = apply(
            &blob("not a function"),
            [],
            |_| None,
            &ForeignFunctions::default(),
            20,
        );
        assert_eq!(uncallable.result, absent::value(absent::NOT_CALLABLE));
    }
}
