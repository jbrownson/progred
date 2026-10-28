//! Foreign functions: Rust implementations under cells, and the tables that
//! hold them.

use crate::environment::Environment;
use crate::runtime::RuntimeValue;
use crate::{Context, Expression, Halt, isa, primitive, vocabulary};
use gid::{CellId, Value};
use std::rc::Rc;
/// A staged foreign function's per-visit form, returned by its
/// prepare stage with the once-parsed call structure in its captures.
pub type Stage = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

pub(crate) type Prepare = Rc<dyn Fn(&Expression) -> Stage>;

type Direct<'a> =
    Rc<dyn Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt> + 'a>;

#[derive(Clone)]
pub(crate) enum ForeignImplementation<'a> {
    Direct(Direct<'a>),
    Staged(Prepare),
}

/// Native code a call runs. It may borrow what lives as long as `'a`, such as
/// state that's valid only for one evaluation; a library's functions are
/// `'static`.
#[derive(Clone)]
pub struct ForeignFunction<'a> {
    pub(crate) implementation: ForeignImplementation<'a>,
    pub(crate) cacheable: bool,
    pub(crate) parameters: Option<Rc<[CellId]>>,
    /// The operation this function runs, when the evaluator can run it
    /// directly; behind a pointer so copying a function stays cheap.
    pub(crate) primitive: Option<Rc<primitive::Primitive>>,
}

impl<'a> ForeignFunction<'a> {
    pub fn from_value(
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<Value, Halt> + 'a,
    ) -> Self {
        Self {
            cacheable: false,
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
        call: impl Fn(&mut Context, &Expression, &Environment) -> Result<RuntimeValue, Halt> + 'a,
    ) -> Self {
        Self {
            cacheable: false,
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
            cacheable: false,
            implementation: ForeignImplementation::Staged(Rc::new(prepare)),
            parameters: None,
            primitive: None,
        }
    }

    /// A host may reuse results this function helped compute. Opt in only
    /// when every changing read goes through evaluation and every observable
    /// write uses Context::effect. Captures must be immutable or
    /// evaluation-local; hidden mutable state makes a reused result wrong.
    pub fn cacheable(mut self) -> Self {
        self.cacheable = true;
        self
    }

    /// The fields a call must supply, in authoring order; arguments with
    /// defaults stay undeclared.
    pub fn parameters(mut self, parameters: impl IntoIterator<Item = CellId>) -> Self {
        self.parameters = Some(parameters.into_iter().collect());
        self
    }

    /// The parameters it declares, if any.
    pub fn declared_parameters(&self) -> Option<&[CellId]> {
        self.parameters.as_deref()
    }

    pub(crate) fn as_primitive(mut self, primitive: primitive::Primitive) -> Self {
        self.primitive = Some(Rc::new(primitive));
        self
    }
}

/// Two functions are equal when they run the same native code.
impl PartialEq for ForeignFunction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cacheable == other.cacheable
            && match (&self.implementation, &other.implementation) {
                (ForeignImplementation::Direct(left), ForeignImplementation::Direct(right)) => {
                    Rc::ptr_eq(left, right)
                }
                (ForeignImplementation::Staged(left), ForeignImplementation::Staged(right)) => {
                    Rc::ptr_eq(left, right)
                }
                _ => false,
            }
    }
}

#[derive(Clone, Default)]
pub struct ForeignFunctions<'a> {
    functions: Vec<(CellId, ForeignFunction<'a>)>,
}

impl<'a> ForeignFunctions<'a> {
    pub fn register(mut self, function: CellId, definition: ForeignFunction<'a>) -> Self {
        match self
            .functions
            .binary_search_by_key(&function, |(cell, _)| *cell)
        {
            Ok(index) => self.functions[index].1 = definition,
            Err(index) => self.functions.insert(index, (function, definition)),
        }
        self
    }

    /// `call` under each of `cells`, told which one a call named: one
    /// function sharing what it borrows across several cells.
    pub fn shared<R: Into<RuntimeValue>>(
        cells: &[CellId],
        call: impl Fn(CellId, &mut Context, &Expression, &Environment) -> Result<R, Halt> + 'a,
    ) -> Self {
        let call = Rc::new(call);
        cells.iter().fold(Self::default(), |functions, &cell| {
            let call = call.clone();
            functions.register(
                cell,
                ForeignFunction::new(move |context, expression, environment| {
                    call(cell, context, expression, environment).map(Into::into)
                }),
            )
        })
    }

    /// Every function here made [`ForeignFunction::cacheable`].
    pub fn cacheable(mut self) -> Self {
        for (_, function) in &mut self.functions {
            function.cacheable = true;
        }
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    pub fn get(&self, function: CellId) -> Option<&ForeignFunction<'a>> {
        self.functions
            .binary_search_by_key(&function, |(cell, _)| *cell)
            .ok()
            .map(|index| &self.functions[index].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (CellId, &ForeignFunction<'a>)> {
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

/// A foreign function implemented under `cell`, saying nothing more.
pub fn foreign_function(cell: CellId) -> Value {
    isa::record(
        vocabulary::kind::FOREIGN_FUNCTION,
        [(vocabulary::IMPLEMENTATION, Value::from(cell))],
    )
}

/// The cell foreign function `value` is implemented under.
pub fn implementation_cell(value: &Value) -> Option<CellId> {
    let fields = value.as_record()?;
    (isa::of(fields)? == vocabulary::kind::FOREIGN_FUNCTION).then_some(())?;
    fields.get(&vocabulary::IMPLEMENTATION)?.as_cell()
}
