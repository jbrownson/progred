//! Lowering: stored and runtime code turned into expressions that remember
//! their origins, and expressions compiled into thunks.

use crate::call::{CallPlan, PreparedCallTarget, not_callable};
use crate::environment::{Environment, cell_index};
use crate::eval::{CellState, unbox};
use crate::foreign::{Prepare, Stage};
use crate::runtime::{Closure, RuntimeValue, RuntimeValueKind};
use crate::unboxed::Unboxed;
use crate::{Context, Expression, Halt, SourceOrigin, absent, arguments, isa, source, vocabulary};
use gid::{CellId, Record, Value};
use std::cell::RefCell;
use std::rc::Rc;
/// Expressions hash by node address. Addresses are aligned and clustered,
/// so a folded multiply spreads them; SipHash would only add per-lookup cost
/// to every evaluation step.
#[derive(Default)]
pub(crate) struct AddressHasher(u64);

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

pub(crate) type ByNode<V> =
    std::collections::HashMap<Expression, V, std::hash::BuildHasherDefault<AddressHasher>>;

fn lowered_field(fields: &[(CellId, Expression)], label: CellId) -> Option<Expression> {
    fields
        .binary_search_by_key(&label, |(field, _)| *field)
        .ok()
        .map(|index| fields[index].1.clone())
}

#[derive(Clone)]
pub(crate) struct Lowered {
    pub(crate) source: RuntimeValue,
    pub(crate) origin: Option<OriginId>,
    pub(crate) form: Form,
    fields: Option<Vec<(CellId, Expression)>>,
    elements: Option<Vec<Expression>>,
}

/// A lowered node's generated form: per-node decisions — dispatch,
/// argument lookup, lambda plumbing — made once at generation, then
/// execution is one call. Generating it changes neither fuel nor results.
pub(crate) type Thunk = Rc<dyn Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt>>;

fn thunk(
    body: impl Fn(&mut Context, &Environment) -> Result<RuntimeValue, Halt> + 'static,
) -> Thunk {
    Rc::new(body)
}

#[derive(Clone)]
pub(crate) struct OriginId(pub(crate) Rc<OriginNode>);

pub(crate) enum OriginRoot {
    Input,
    Located(SourceOrigin),
    Cell(CellId),
}

pub(crate) enum OriginNode {
    Root(OriginRoot),
    Child { parent: OriginId, step: gid::Step },
}

#[derive(Clone)]
pub(crate) enum Form {
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
pub(crate) enum LambdaParameters {
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

impl<'a> Context<'a> {
    pub(crate) fn lower_source(&mut self, value: &Value, root: OriginRoot) -> Expression {
        let origin = OriginId(Rc::new(OriginNode::Root(root)));
        self.lower_with(value, Some(origin))
    }

    pub(crate) fn lower_unattributed_source(&mut self, value: &Value) -> Expression {
        self.lower_with(value, None)
    }

    pub(crate) fn lower_runtime_code(&mut self, value: &RuntimeValue) -> Expression {
        self.lower_runtime_code_at(value, None)
    }

    pub(crate) fn lower_runtime_code_at(
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

    pub(crate) fn compile(&mut self, expression: Expression) -> Thunk {
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

    /// `cell`'s definition lowered once per evaluation and shared by every
    /// call site that calls it, so their compiled code and call caches are
    /// shared too.
    pub(crate) fn lowered_cell(&mut self, cell: CellId, value: &Value) -> Expression {
        let index = cell_index(&self.indices, cell);
        while self.cell_states.len() <= index.0 {
            self.cell_states.push(CellState::Unknown);
        }
        match &self.cell_states[index.0] {
            CellState::Ready(expression) => expression.clone(),
            CellState::Evaluating { .. } => self.lower_source(value, OriginRoot::Cell(cell)),
            CellState::Unknown => {
                let expression = self.lower_source(value, OriginRoot::Cell(cell));
                self.cell_states[index.0] = CellState::Ready(expression.clone());
                expression
            }
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
}

fn child_origin(parent: Option<OriginId>, step: gid::Step) -> Option<OriginId> {
    parent.map(|parent| OriginId(Rc::new(OriginNode::Child { parent, step })))
}

/// A field a construct's kind requires, while it's missing: found where the
/// field would be, answering why it can't be used yet.
pub(crate) fn missing_field(field: CellId, origin: Option<OriginId>) -> Expression {
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

pub(crate) fn runtime_call(
    function: RuntimeValue,
    arguments: Vec<(CellId, RuntimeValue)>,
) -> Expression {
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
