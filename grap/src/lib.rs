//! A small evaluator of GID expressions, returning owned runtime values
//! with an explicit conversion back to GID. It recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use crate::environment::CellIndices;
use crate::eval::CellState;
use crate::lower::{ByNode, OriginId, Thunk};
use crate::runtime::RuntimeValueKind;
use gid::{CellId, Value};
use std::rc::Rc;

pub use call::PreparedCallable;
pub use environment::Environment;
pub use eval::{Halt, MAX_DEPTH};
pub use foreign::{
    ForeignFunction, ForeignFunctions, ForeignOverlay, Stage, foreign_function, implementation_cell,
};
pub use lower::Expression;
pub use runtime::{RuntimeListValues, RuntimeValue, Shape};
pub use source::SourceOrigin;
pub use trace::{Answering, CallTrace, Observed, Observer, Tracer};

#[cfg(test)]
#[path = "tests/effect_tests.rs"]
mod effect_tests;

#[cfg(test)]
#[path = "tests/runtime_tests.rs"]
mod runtime_tests;

pub mod absent;
pub mod arguments;
mod call;
mod environment;
mod eval;
pub mod f32;
pub mod f64;
mod foreign;
pub mod isa;
pub mod logic;
mod lower;
pub mod path;
pub mod primitive;
mod reify;
mod runtime;
pub mod source;
pub mod text;
mod trace;
pub mod u64;
mod unboxed;
pub mod vocabulary;

/// Where an evaluation finds definitions, and the native code foreign
/// functions run. A host that reuses results can rely on every changing read
/// being a `definition` or an `implementation`, except where `uncacheable`
/// says otherwise.
pub trait Host {
    /// `cell`'s definition.
    fn definition(&self, cell: CellId) -> Option<Value>;
    /// The native code a foreign function implemented under `cell` runs.
    fn implementation(&self, cell: CellId) -> Option<ForeignFunction>;
    /// A foreign function not declared cacheable ran, so the result may depend
    /// on reads the host didn't see, and reusing it could be wrong.
    fn uncacheable(&self) {}
    /// An observable write ran, which a reused result would leave out.
    fn effect(&self) {}
    /// What watches the evaluations run against this host, if anything.
    fn observer(&self) -> Option<&dyn Observer> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct Evaluation {
    pub result: RuntimeValue,
    pub remaining_fuel: usize,
    /// Why it stopped before finishing, if it did; its result is then that
    /// halt's absent. Returning an absent value isn't halting.
    pub halt: Option<Halt>,
}

pub struct Context<'a> {
    host: &'a dyn Host,
    overlay: Option<&'a ForeignOverlay<'a>>,
    foreign_scopes: Vec<ForeignFunctions>,
    remaining_fuel: usize,
    depth: usize,
    resolving: Vec<CellId>,
    compiled: ByNode<Thunk>,
    observer: Option<&'a dyn Observer>,
    cell_states: Vec<CellState>,
    indices: CellIndices,
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
        remaining_fuel: fuel,
        depth: 0,
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
        let origin = origin.map(OriginId::start);
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
#[path = "tests/support.rs"]
mod test_support;
#[cfg(test)]
use test_support::{TestDefinition, TestHost};

#[cfg(test)]
#[path = "tests/lib.rs"]
mod tests;
