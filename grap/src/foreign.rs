//! Foreign functions: Rust implementations under cells, the tables that hold
//! them, and overlays a caller scopes over an evaluation.

use crate::environment::Environment;
use crate::runtime::RuntimeValue;
use crate::{Context, Expression, Halt, isa, primitive, vocabulary};
use gid::{CellId, Value};
use std::rc::Rc;
/// A staged foreign function's per-visit form, returned by its
/// prepare stage with the once-parsed call structure in its captures.
pub type Stage = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

pub(crate) type Prepare = Rc<dyn Fn(&Expression) -> Stage>;

#[derive(Clone)]
pub(crate) enum ForeignImplementation {
    Direct(Rc<dyn Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt>>),
    Staged(Prepare),
}

#[derive(Clone)]
pub struct ForeignFunction {
    pub(crate) implementation: ForeignImplementation,
    pub(crate) tracked: bool,
    pub(crate) parameters: Option<Rc<[CellId]>>,
    /// The operation this function runs, when the evaluator can run it
    /// directly; behind a pointer so copying a function stays cheap.
    pub(crate) primitive: Option<Rc<primitive::Primitive>>,
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
    pub fn staged(prepare: impl Fn(&Expression) -> Stage + 'static) -> Self {
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

    pub(crate) fn as_primitive(mut self, primitive: primitive::Primitive) -> Self {
        self.primitive = Some(Rc::new(primitive));
        self
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
pub(crate) enum OverlayCall<'a> {
    Runtime(&'a OverlayFn<'a, RuntimeValue>),
    Value(&'a OverlayFn<'a, Value>),
}

/// A synchronous, borrowed layer of foreign functions. It lets a host
/// expose state that is valid only for one evaluation without putting
/// that state in `'static` closures.
pub struct ForeignOverlay<'a> {
    functions: &'a [CellId],
    pub(crate) call: OverlayCall<'a>,
    pub(crate) tracked: bool,
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

    pub(crate) fn handles(&self, function: CellId) -> bool {
        self.functions.contains(&function)
    }
}

/// A foreign function implemented under `cell`, saying nothing more.
pub fn foreign_function(cell: CellId) -> Value {
    isa::record(
        vocabulary::kind::FOREIGN_FUNCTION,
        [(vocabulary::IMPLEMENTATION, Value::from(cell))],
    )
}
