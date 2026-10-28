//! Lowering: stored and runtime code turned into expressions that remember
//! their origins, and expressions compiled into thunks.

use crate::call::{CallPlan, PreparedCallTarget, not_callable};
use crate::environment::{Environment, cell_index};
use crate::eval::{CellState, unbox};
use crate::foreign::{Prepare, Stage};
use crate::runtime::{Closure, RuntimeValue, RuntimeValueKind};
use crate::unboxed::Unboxed;
use crate::{Context, Halt, SourceOrigin, absent, arguments, isa, source, vocabulary};
use gid::{CellId, Record, Value};
use std::cell::{OnceCell, RefCell};
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

/// A node of lowered code: its source, where it came from, how it evaluates,
/// and its lowered children. It's equal only to itself, so each node is its
/// own call site; clones share the node.
#[derive(Clone)]
pub struct Expression(pub(crate) Rc<Lowered>);

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
    /// The code as written.
    pub fn value(&self) -> &Value {
        self.0.source.as_value()
    }

    pub fn source_origin(&self) -> Option<SourceOrigin> {
        source::origin(self)
    }

    pub fn field(&self, label: CellId) -> Option<Expression> {
        lowered_field(self.0.fields.as_ref()?, label)
    }

    pub fn fields(&self) -> Option<&[(CellId, Expression)]> {
        self.0.fields.as_deref()
    }

    pub fn elements(&self) -> Option<&[Expression]> {
        Some(&self.0.elements.as_ref()?.expressions)
    }

    /// The positions of this list's elements, in order, gathered once and
    /// shared, so a list built from them keeps them without a copy.
    pub fn element_positions(&self) -> Option<Rc<[gid::Position]>> {
        let elements = self.0.elements.as_ref()?;
        Some(
            elements
                .positions
                .get_or_init(|| {
                    (self.0.source.shared_list_positions())
                        .expect("a node with elements lowers a list")
                })
                .clone(),
        )
    }

    /// Why this call has no argument under `cell`, naming where the call is.
    pub fn missing_argument(&self, cell: CellId) -> Value {
        absent::missing_argument(cell, self.source_origin().as_ref())
    }

    pub fn missing_runtime_argument(&self, cell: CellId) -> RuntimeValue {
        self.missing_argument(cell).into()
    }

    /// This call's argument under `label`, or why it has none.
    pub fn argument(&self, label: CellId) -> arguments::Argument {
        self.field(label)
            .ok_or_else(|| self.missing_runtime_argument(label))
    }
}

pub(crate) struct Lowered {
    pub(crate) source: RuntimeValue,
    pub(crate) origin: Option<OriginId>,
    pub(crate) form: Form,
    fields: Option<Vec<(CellId, Expression)>>,
    elements: Option<Elements>,
}

/// A list's lowered elements, and their positions once something asks.
struct Elements {
    expressions: Vec<Expression>,
    positions: OnceCell<Rc<[gid::Position]>>,
}

impl From<Vec<Expression>> for Elements {
    fn from(expressions: Vec<Expression>) -> Self {
        Self {
            expressions,
            positions: OnceCell::new(),
        }
    }
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
                let form = construct(value, &fields, &origin)
                    .unwrap_or_else(|| Form::Ready(value.clone()));
                Expression(Rc::new(Lowered {
                    source: value.clone(),
                    origin,
                    form,
                    fields: Some(fields),
                    elements: None,
                }))
            }
            RuntimeValueKind::List(list) => {
                let elements: Vec<_> = if origin.is_some() {
                    list.values
                        .iter()
                        .zip(list.positions().iter())
                        .map(|(value, position)| {
                            let child =
                                child_origin(origin.clone(), gid::Step::Element(position.clone()));
                            self.lower_runtime_code_at(value, child)
                        })
                        .collect()
                } else {
                    list.values
                        .iter()
                        .map(|value| self.lower_runtime_code(value))
                        .collect()
                };
                Expression(Rc::new(Lowered {
                    source: value.clone(),
                    origin,
                    form: Form::Ready(value.clone()),
                    fields: None,
                    elements: Some(elements.into()),
                }))
            }
            _ => runtime_expression(value.clone()),
        }
    }

    fn lower_with(&mut self, value: &Value, origin: Option<OriginId>) -> Expression {
        let source = RuntimeValue::from(value);
        let (form, fields, elements) = match value {
            Value::Cell(cell) => (Form::Cell(*cell), None, None),
            Value::Record(record) => {
                let fields: Vec<_> = record
                    .iter()
                    .map(|(field, value)| {
                        let child = child_origin(origin.clone(), gid::Step::Key(*field));
                        (*field, self.lower_with(value, child))
                    })
                    .collect();
                // A number or text literal is its native value from the
                // start, rather than read on every use.
                let form =
                    construct(&source, &fields, &origin).unwrap_or_else(
                        || match Unboxed::recognize(record) {
                            Some(unboxed) => {
                                Form::Ready(RuntimeValue::new(RuntimeValueKind::Unboxed(unboxed)))
                            }
                            None => Form::Data,
                        },
                    );
                (form, Some(fields), None)
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
                        .collect::<Vec<_>>()
                        .into(),
                ),
            ),
            Value::Blob(_) => (Form::Data, None, None),
        };
        Expression(Rc::new(Lowered {
            source,
            origin,
            form,
            fields,
            elements,
        }))
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
}

/// What a record's `isa` makes it, read from its lowered `fields`: a call,
/// a lambda or a value wrapper, or `None` for any other record. A field the
/// kind requires but the record lacks lowers to why it's missing.
fn construct(
    source: &RuntimeValue,
    fields: &[(CellId, Expression)],
    origin: &Option<OriginId>,
) -> Option<Form> {
    let required = |field| {
        lowered_field(fields, field).unwrap_or_else(|| {
            missing_field(field, child_origin(origin.clone(), gid::Step::Key(field)))
        })
    };
    Some(match source.field(isa::ISA)?.as_cell()? {
        vocabulary::kind::CALL => Form::Call {
            function: required(vocabulary::FUNCTION),
        },
        vocabulary::kind::LAMBDA => Form::Lambda {
            parameters: LambdaParameters::parse(
                source
                    .field(vocabulary::PARAMS)
                    .as_ref()
                    .and_then(RuntimeValue::list_values)
                    .map(|values| {
                        values
                            .map(|v| v.as_cell().ok_or_else(|| v.to_value()))
                            .collect()
                    }),
            ),
            body: required(vocabulary::BODY),
            fields: source.as_value().as_record()?.clone(),
        },
        vocabulary::kind::VALUE_WRAPPER => Form::Value(required(vocabulary::VALUE)),
        _ => return None,
    })
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
