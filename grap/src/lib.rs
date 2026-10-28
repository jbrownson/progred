//! A small evaluator of GID expressions, returning owned runtime values
//! with an explicit conversion back to GID. It recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use crate::environment::CellIndices;
use crate::lower::{ByNode, OriginId, Thunk};
use gid::{CellId, Value};
use std::rc::Rc;

pub use call::PreparedCallable;
pub use environment::Environment;
pub use eval::{Halt, MAX_DEPTH};
pub use foreign::{
    ForeignFunction, ForeignFunctions, Stage, foreign_function, implementation_cell,
};
pub use lower::Expression;
pub use runtime::{RuntimeListValues, RuntimeValue, Shape};
pub use source::SourceOrigin;
pub use trace::{Answering, CallTrace, Observer, Tracer};

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
    fn implementation(&self, cell: CellId) -> Option<ForeignFunction<'static>>;
    /// A foreign function not declared cacheable ran, so the result may depend
    /// on reads the host didn't see, and reusing it could be wrong.
    fn uncacheable(&self) {}
    /// An observable write ran, which a reused result would leave out.
    fn effect(&self) {}
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
    /// The evaluation's own foreign functions, then those pushed while it
    /// runs: the last that has a cell answers its calls before the host.
    functions: Vec<ForeignFunctions<'a>>,
    remaining_fuel: usize,
    remaining_depth: usize,
    compiled: ByNode<Thunk>,
    observer: Option<&'a dyn Observer>,
    /// Each cell's definition, by index, lowered once this evaluation.
    definitions: Vec<Option<Expression>>,
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

/// What an evaluation runs against: the host defining its cells, its own
/// foreign functions, what watches it, and its fuel.
pub struct Evaluator<'a> {
    host: &'a dyn Host,
    functions: ForeignFunctions<'a>,
    observer: Option<&'a dyn Observer>,
    fuel: usize,
}

impl<'a> Evaluator<'a> {
    pub fn new(host: &'a dyn Host, fuel: usize) -> Self {
        Self {
            host,
            functions: ForeignFunctions::default(),
            observer: None,
            fuel,
        }
    }

    /// Foreign functions for this evaluation alone, answering calls to their
    /// cells before the host does. They may borrow what lives only as long.
    pub fn functions(self, functions: ForeignFunctions<'a>) -> Self {
        Self { functions, ..self }
    }

    /// What's told of each call the evaluation enters and leaves.
    pub fn observer(self, observer: &'a dyn Observer) -> Self {
        Self {
            observer: Some(observer),
            ..self
        }
    }

    fn context(self) -> Context<'a> {
        Context {
            host: self.host,
            functions: if self.functions.is_empty() {
                Vec::new()
            } else {
                vec![self.functions]
            },
            remaining_fuel: self.fuel,
            remaining_depth: MAX_DEPTH,
            compiled: Default::default(),
            observer: self.observer,
            definitions: Vec::new(),
            indices: CellIndices::default(),
        }
    }

    /// Evaluate syntax read from the input; its source origins are input
    /// paths.
    pub fn evaluate(self, expression: &Value) -> Evaluation {
        self.context().run(expression)
    }

    /// Interpret runtime-held syntax, rooting its source origins at `origin`;
    /// without one, it has none. Embedded native callables retain their own
    /// code origins and lexical captures.
    pub fn evaluate_runtime(
        self,
        expression: &RuntimeValue,
        origin: Option<SourceOrigin>,
    ) -> Evaluation {
        self.context().conclude(|context| {
            let origin = origin.map(OriginId::start);
            let expression = context.lower_runtime_code_at(expression, origin);
            let environment = context.empty_environment();
            context.eval(expression, &environment)
        })
    }

    /// Apply a callable expression at the host boundary: Grap parameters
    /// receive values, while foreign functions receive argument syntax to
    /// interpret. Use `apply` when the callable and all arguments have
    /// already been evaluated.
    pub fn apply_expression(
        self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Evaluation {
        self.context()
            .conclude(|context| context.apply_expression(function, arguments))
    }

    /// Apply an evaluated callable to evaluated arguments.
    pub fn apply(
        self,
        function: &RuntimeValue,
        arguments: impl IntoIterator<Item = (CellId, RuntimeValue)>,
    ) -> Evaluation {
        self.context()
            .conclude(|context| context.apply(function, arguments))
    }
}

#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
#[cfg(test)]
use test_support::{TestDefinition, TestHost};

#[cfg(test)]
#[path = "tests/lib.rs"]
mod tests;
