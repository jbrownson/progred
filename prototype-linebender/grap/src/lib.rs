//! A small evaluator whose expressions and results are Progred graph
//! values. The evaluator recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use progred_graph::{CellId, Cells, Value};
use std::collections::{BTreeSet, HashMap, hash_map::Entry};
use std::fmt;
use std::rc::Rc;

pub mod vocabulary {
    use progred_graph::CellId;

    pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
    pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
    pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
    pub const QUOTE: CellId = CellId::from_u128(0x6eb975b6080ba8f9463b8045906a9c3d);
    pub const UNQUOTE: CellId = CellId::from_u128(0x3b10378451b9eddf51cd0bc0601eb74e);
}

pub mod absent {
    use progred_graph::CellId;

    pub const FUEL_EXHAUSTED: CellId =
        CellId::from_u128(0x513628d759c04b3e7088b575e555a80e);
    pub const MISSING_CELL: CellId =
        CellId::from_u128(0xa5a1b4e3d0df96bd11af00f0780136ff);
    pub const CELL_CYCLE: CellId =
        CellId::from_u128(0x150e0fc7e38d1670f41283c3d23a9b8d);
    pub const MALFORMED_FUNCTION: CellId =
        CellId::from_u128(0xfbf5894d7f62b6d0048d17d26851b415);
    pub const INVALID_PARAMETER: CellId =
        CellId::from_u128(0x93ca0e9199372c46ee24bd4c508e178c);
    pub const NOT_CALLABLE: CellId =
        CellId::from_u128(0x8624488c2d10d2a4b84560dfa99a38e6);
    pub const MISSING_ARGUMENT: CellId =
        CellId::from_u128(0x8b2f0db36e5c3d35595eb5666cc89c78);
    pub const FOREIGN_ARGUMENT_IS_FUNCTION: CellId =
        CellId::from_u128(0x1e4c8ae55409c6c82a66a922027a8c30);
    pub const FUNCTION_IS_NOT_DATA: CellId =
        CellId::from_u128(0x030240e926485b64ea35b4aa1b13e94f);
}

pub const DEFAULT_FUEL: usize = 1_024;

#[derive(Clone)]
struct ForeignFunction {
    params: Vec<CellId>,
    call: Rc<dyn Fn(&[Value]) -> Value>,
}

#[derive(Clone, Default)]
pub struct ForeignFunctions {
    functions: HashMap<CellId, ForeignFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    AlreadyRegistered(CellId),
    ReservedParameter(CellId),
    DuplicateParameter(CellId),
}

impl ForeignFunctions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        function: CellId,
        params: impl IntoIterator<Item = CellId>,
        call: impl Fn(&[Value]) -> Value + 'static,
    ) -> Result<(), RegistrationError> {
        let params: Vec<_> = params.into_iter().collect();
        if params.contains(&vocabulary::FUNCTION) {
            Err(RegistrationError::ReservedParameter(vocabulary::FUNCTION))
        } else if let Some(duplicate) = duplicate(&params) {
            Err(RegistrationError::DuplicateParameter(duplicate))
        } else {
            match self.functions.entry(function) {
                Entry::Occupied(_) => Err(RegistrationError::AlreadyRegistered(function)),
                Entry::Vacant(entry) => {
                    entry.insert(ForeignFunction {
                        params,
                        call: Rc::new(call),
                    });
                    Ok(())
                }
            }
        }
    }

    fn get(&self, function: CellId) -> Option<&ForeignFunction> {
        self.functions.get(&function)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    FuelExhausted,
    MissingCell(CellId),
    CellCycle(Vec<CellId>),
    MalformedFunction,
    InvalidParameter(Value),
    DuplicateParameter(CellId),
    ReservedParameter(CellId),
    NotCallable(Value),
    MissingArgument(CellId),
    ForeignArgumentIsFunction(CellId),
    FunctionIsNotData,
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
            Diagnostic::MalformedFunction => {
                write!(f, "function params must be a list")
            }
            Diagnostic::InvalidParameter(value) => {
                write!(f, "function parameter is not a cell: {value:?}")
            }
            Diagnostic::DuplicateParameter(cell) => {
                write!(f, "function parameter {cell} appears more than once")
            }
            Diagnostic::ReservedParameter(cell) => {
                write!(f, "cell {cell} is reserved by the call representation")
            }
            Diagnostic::NotCallable(value) => write!(f, "value is not callable: {value:?}"),
            Diagnostic::MissingArgument(cell) => write!(f, "missing argument {cell}"),
            Diagnostic::ForeignArgumentIsFunction(cell) => {
                write!(f, "foreign argument {cell} evaluated to a function")
            }
            Diagnostic::FunctionIsNotData => write!(f, "evaluation produced a function"),
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

#[derive(Clone)]
enum RuntimeValue {
    Data(Value),
    Closure {
        params: Vec<CellId>,
        body: Value,
        environment: Environment,
    },
}

type Environment = HashMap<CellId, RuntimeValue>;
type EvalResult<T> = Result<T, Value>;

enum CallTarget {
    Graph {
        params: Vec<CellId>,
        body: Value,
        environment: Environment,
    },
    Foreign(ForeignFunction),
}

impl CallTarget {
    fn params(&self) -> &[CellId] {
        match self {
            CallTarget::Graph { params, .. } => params,
            CallTarget::Foreign(function) => &function.params,
        }
    }
}

struct Evaluator<'a, R> {
    resolve: &'a R,
    foreign: &'a ForeignFunctions,
    remaining_fuel: usize,
    diagnostics: Vec<Diagnostic>,
    dependencies: BTreeSet<CellId>,
    resolving: Vec<CellId>,
}

impl<'a, R> Evaluator<'a, R>
where
    R: Fn(CellId) -> Option<Value>,
{
    fn run(mut self, expression: &Value) -> Evaluation {
        let result = match self.eval(expression, &Environment::new()) {
            Ok(evaluated) => self.into_data(evaluated),
            Err(result) => result,
        };
        Evaluation {
            result,
            diagnostics: self.diagnostics,
            dependencies: self.dependencies,
            remaining_fuel: self.remaining_fuel,
        }
    }

    fn eval(&mut self, expression: &Value, environment: &Environment) -> EvalResult<RuntimeValue> {
        self.burn()?;
        match expression {
            Value::Cell(cell) => self.eval_cell(*cell, environment),
            Value::Record(fields) => {
                // Open records may match more than one form. For now the evaluator
                // uses the simple precedence quote, call, then definition; an
                // ambiguous-form absent can replace it if overlaps matter in practice.
                if let Some(template) = fields.get(&vocabulary::QUOTE) {
                    self.eval_quote(template, environment)
                } else if let Some(function) = fields.get(&vocabulary::FUNCTION) {
                    self.eval_call(function, |field| fields.get(&field), environment)
                } else {
                    match (
                        fields.get(&vocabulary::PARAMS),
                        fields.get(&vocabulary::BODY),
                    ) {
                        (Some(parameters), Some(body)) => {
                            Ok(self.eval_function(parameters, body, environment))
                        }
                        _ => Ok(RuntimeValue::Data(expression.clone())),
                    }
                }
            }
            Value::Blob(_) | Value::List(_) => {
                Ok(RuntimeValue::Data(expression.clone()))
            }
        }
    }

    fn burn(&mut self) -> EvalResult<()> {
        self.remaining_fuel = self.remaining_fuel.saturating_sub(1);
        if self.remaining_fuel == 0 {
            Err(self.absent_value(
                Diagnostic::FuelExhausted,
                absent::FUEL_EXHAUSTED,
            ))
        } else {
            Ok(())
        }
    }

    fn absent(&mut self, diagnostic: Diagnostic, cell: CellId) -> RuntimeValue {
        RuntimeValue::Data(self.absent_value(diagnostic, cell))
    }

    fn absent_value(&mut self, diagnostic: Diagnostic, cell: CellId) -> Value {
        self.diagnostics.push(diagnostic);
        Value::from(cell)
    }

    fn into_data(&mut self, value: RuntimeValue) -> Value {
        match value {
            RuntimeValue::Data(value) => value,
            RuntimeValue::Closure { .. } => self.absent_value(
                Diagnostic::FunctionIsNotData,
                absent::FUNCTION_IS_NOT_DATA,
            ),
        }
    }

    fn eval_cell(&mut self, cell: CellId, environment: &Environment) -> EvalResult<RuntimeValue> {
        if let Some(value) = environment.get(&cell) {
            Ok(value.clone())
        } else if self.foreign.get(cell).is_some() {
            Ok(RuntimeValue::Data(Value::from(cell)))
        } else if let Some(first) = self
            .resolving
            .iter()
            .position(|resolving| *resolving == cell)
        {
            Ok(self.absent(
                Diagnostic::CellCycle(
                    self.resolving[first..]
                        .iter()
                        .copied()
                        .chain([cell])
                        .collect(),
                ),
                absent::CELL_CYCLE,
            ))
        } else {
            self.dependencies.insert(cell);
            match (self.resolve)(cell) {
                Some(value) => {
                    self.resolving.push(cell);
                    let result = self.eval(&value, environment);
                    self.resolving.pop();
                    result
                }
                None => Ok(self.absent(Diagnostic::MissingCell(cell), absent::MISSING_CELL)),
            }
        }
    }

    fn eval_function(
        &mut self,
        parameters: &Value,
        body: &Value,
        environment: &Environment,
    ) -> RuntimeValue {
        let Some(parameters) = parameters.as_list() else {
            return self.absent(Diagnostic::MalformedFunction, absent::MALFORMED_FUNCTION);
        };
        let params = parameters
            .values()
            .map(|parameter| parameter.as_cell().ok_or_else(|| parameter.clone()))
            .collect::<Result<Vec<_>, _>>();
        let params = match params {
            Ok(params) => params,
            Err(parameter) => {
                return self.absent(
                    Diagnostic::InvalidParameter(parameter),
                    absent::INVALID_PARAMETER,
                );
            }
        };
        if params.contains(&vocabulary::FUNCTION) {
            return self.absent(
                Diagnostic::ReservedParameter(vocabulary::FUNCTION),
                absent::INVALID_PARAMETER,
            );
        }
        if let Some(duplicate) = duplicate(&params) {
            return self.absent(
                Diagnostic::DuplicateParameter(duplicate),
                absent::INVALID_PARAMETER,
            );
        }
        RuntimeValue::Closure {
            params,
            body: body.clone(),
            environment: environment.clone(),
        }
    }

    fn eval_call<'v>(
        &mut self,
        function: &Value,
        argument: impl Fn(CellId) -> Option<&'v Value>,
        environment: &Environment,
    ) -> EvalResult<RuntimeValue> {
        let callable = self.eval(function, environment)?;
        let target = match callable {
            RuntimeValue::Closure {
                params,
                body,
                environment,
            } => CallTarget::Graph {
                params,
                body,
                environment,
            },
            RuntimeValue::Data(value) => match value
                .as_cell()
                .and_then(|cell| self.foreign.get(cell).cloned())
            {
                Some(function) => CallTarget::Foreign(function),
                None => {
                    return Ok(self.absent(
                        Diagnostic::NotCallable(value.clone()),
                        absent::NOT_CALLABLE,
                    ));
                }
            },
        };
        let mut expressions = Vec::with_capacity(target.params().len());
        for parameter in target.params() {
            let Some(expression) = argument(*parameter) else {
                return Ok(self.absent(
                    Diagnostic::MissingArgument(*parameter),
                    absent::MISSING_ARGUMENT,
                ));
            };
            expressions.push((*parameter, expression));
        }
        let mut arguments = Vec::with_capacity(expressions.len());
        for (parameter, expression) in expressions {
            let value = self.eval(expression, environment)?;
            arguments.push((parameter, value));
        }
        match target {
            CallTarget::Graph {
                body,
                mut environment,
                ..
            } => {
                environment.extend(arguments);
                self.eval(&body, &environment)
            }
            CallTarget::Foreign(function) => {
                let values = arguments
                    .into_iter()
                    .map(|(parameter, argument)| match argument {
                        RuntimeValue::Data(value) => Ok(value),
                        RuntimeValue::Closure { .. } => Err(parameter),
                    })
                    .collect::<Result<Vec<_>, _>>();
                match values {
                    Ok(values) => Ok(RuntimeValue::Data((function.call)(&values))),
                    Err(parameter) => Ok(self.absent(
                        Diagnostic::ForeignArgumentIsFunction(parameter),
                        absent::FOREIGN_ARGUMENT_IS_FUNCTION,
                    )),
                }
            }
        }
    }

    fn eval_quote(
        &mut self,
        template: &Value,
        environment: &Environment,
    ) -> EvalResult<RuntimeValue> {
        Ok(RuntimeValue::Data(self.expand_quote(template, environment)?))
    }

    fn expand_quote(&mut self, template: &Value, environment: &Environment) -> EvalResult<Value> {
        self.burn()?;
        match template {
            Value::Cell(_) | Value::Blob(_) => Ok(template.clone()),
            Value::List(elements) => elements
                .iter()
                .map(|(position, value)| {
                    Ok((position.clone(), self.expand_quote(value, environment)?))
                })
                .collect::<EvalResult<_>>()
                .map(Value::List),
            Value::Record(fields) => match fields.get(&vocabulary::UNQUOTE) {
                Some(expression) => {
                    let evaluated = self.eval(expression, environment);
                    Ok(self.into_data(evaluated?))
                }
                None => fields
                    .iter()
                    .map(|(field, value)| {
                        Ok((*field, self.expand_quote(value, environment)?))
                    })
                    .collect::<EvalResult<_>>()
                    .map(Value::Record),
            },
        }
    }
}

fn duplicate(cells: &[CellId]) -> Option<CellId> {
    cells
        .iter()
        .enumerate()
        .find_map(|(index, cell)| cells[index + 1..].contains(cell).then_some(*cell))
}

pub fn function(params: impl IntoIterator<Item = CellId>, body: Value) -> Value {
    Value::record([
        (
            vocabulary::PARAMS,
            Value::list(params.into_iter().map(Value::from)),
        ),
        (vocabulary::BODY, body),
    ])
}

pub fn call(
    function: Value,
    arguments: impl IntoIterator<Item = (CellId, Value)>,
) -> Value {
    Value::record(
        [(vocabulary::FUNCTION, function)]
            .into_iter()
            .chain(arguments),
    )
}

pub fn quote(value: Value) -> Value {
    Value::record([(vocabulary::QUOTE, value)])
}

pub fn unquote(expression: Value) -> Value {
    Value::record([(vocabulary::UNQUOTE, expression)])
}

pub fn evaluate(
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> Evaluation {
    Evaluator {
        resolve: &resolve,
        foreign,
        remaining_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
    }
    .run(expression)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::FUNCTION, "function"),
        (vocabulary::PARAMS, "params"),
        (vocabulary::BODY, "body"),
        (vocabulary::QUOTE, "quote"),
        (vocabulary::UNQUOTE, "unquote"),
    ] {
        cells.set_value(cell, progred_name::record(name, []));
    }
    for (cell, name) in [
        (absent::FUEL_EXHAUSTED, "fuel exhausted"),
        (absent::MISSING_CELL, "missing cell"),
        (absent::CELL_CYCLE, "cell cycle"),
        (absent::MALFORMED_FUNCTION, "malformed function"),
        (absent::INVALID_PARAMETER, "invalid parameter"),
        (absent::NOT_CALLABLE, "not callable"),
        (absent::MISSING_ARGUMENT, "missing argument"),
        (
            absent::FOREIGN_ARGUMENT_IS_FUNCTION,
            "foreign argument is function",
        ),
        (absent::FUNCTION_IS_NOT_DATA, "function is not data"),
    ] {
        cells.set_value(cell, grap_absent::named(name));
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

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
            &ForeignFunctions::new(),
            10,
        );
        assert_eq!(evaluation.result, value);
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn consuming_the_entire_fuel_allowance_is_exhaustion() {
        let value = blob("one step");
        let exhausted = evaluate(&value, |_| None, &ForeignFunctions::new(), 1);
        assert_eq!(exhausted.result, Value::from(absent::FUEL_EXHAUSTED));
        assert_eq!(exhausted.diagnostics, [Diagnostic::FuelExhausted]);
        assert_eq!(exhausted.remaining_fuel, 0);
        let completed = evaluate(&value, |_| None, &ForeignFunctions::new(), 2);
        assert_eq!(completed.result, value);
        assert_eq!(completed.remaining_fuel, 1);
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
            &ForeignFunctions::new(),
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
        let definition = function([x, y], Value::from(x));
        let expression = call(
            Value::from(function_cell),
            [(x, blob("x")), (y, Value::from(new_cell_id()))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| (cell == function_cell).then(|| definition.clone()),
            &ForeignFunctions::new(),
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
            function([parameter], Value::from(parameter)),
            [(parameter, blob("local"))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| (cell == parameter).then(|| blob("document")),
            &ForeignFunctions::new(),
            20,
        );
        assert_eq!(evaluation.result, blob("local"));
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn anonymous_functions_capture_their_lexical_environment() {
        let x = new_cell_id();
        let y = new_cell_id();
        let inner = function([y], Value::from(x));
        let outer = function([x], call(inner, [(y, blob("ignored"))]));
        assert_eq!(
            evaluate(
                &call(outer, [(x, blob("captured"))]),
                |_| None,
                &ForeignFunctions::new(),
                50,
            )
            .result,
            blob("captured")
        );
    }

    #[test]
    fn foreign_and_graph_functions_return_evaluated_values() {
        let echo = new_cell_id();
        let input = new_cell_id();
        let field = new_cell_id();
        let call_shaped_data = call(Value::from(new_cell_id()), []);
        let argument = quote(call_shaped_data.clone());
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(echo, [input], |arguments| arguments[0].clone())
            .unwrap();
        let graph = function([input], Value::from(input));
        assert_eq!(
            evaluate(
                &call(Value::from(echo), [(input, argument.clone())]),
                |_| None,
                &foreign,
                30,
            )
            .result,
            call_shaped_data
        );
        assert_eq!(
            evaluate(
                &call(graph, [(input, argument)]),
                |_| None,
                &foreign,
                30,
            )
            .result,
            call_shaped_data
        );
        let inert = Value::record([(field, call(Value::from(echo), []))]);
        assert_eq!(
            evaluate(&inert, |_| None, &foreign, 30).result,
            inert
        );
    }

    #[test]
    fn foreign_functions_are_cell_values_until_called() {
        let echo = new_cell_id();
        let input = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(echo, [input], |arguments| arguments[0].clone())
            .unwrap();

        let evaluation = evaluate(
            &Value::from(echo),
            |_| Some(blob("foreign cells do not resolve as data")),
            &foreign,
            10,
        );
        assert_eq!(evaluation.result, Value::from(echo));
        assert!(evaluation.dependencies.is_empty());
        assert_eq!(
            evaluate(
                &call(Value::from(echo), [(input, Value::from(echo))]),
                |_| None,
                &foreign,
                10,
            )
            .result,
            Value::from(echo)
        );
    }

    #[test]
    fn quote_interpolates_explicit_unquotes_once_without_following_cells() {
        let field = new_cell_id();
        let cell = new_cell_id();
        let quoted = quote(Value::record([
            (field, unquote(Value::from(cell))),
            (new_cell_id(), Value::from(cell)),
        ]));
        let evaluation = evaluate(
            &quoted,
            |candidate| (candidate == cell).then(|| blob("copied")),
            &ForeignFunctions::new(),
            30,
        );
        let fields = evaluation.result.as_record().unwrap();
        assert_eq!(fields.get(&field), Some(&blob("copied")));
        assert!(fields.values().any(|value| value == &Value::from(cell)));
        assert_eq!(evaluation.dependencies, BTreeSet::from([cell]));

        let literal_unquote = unquote(blob("literal"));
        assert_eq!(
            evaluate(
                &quote(unquote(literal_unquote.clone())),
                |_| None,
                &ForeignFunctions::new(),
                20,
            )
            .result,
            literal_unquote
        );
        assert_eq!(
            evaluate(
                &unquote(blob("outside")),
                |_| None,
                &ForeignFunctions::new(),
                10,
            )
            .result,
            unquote(blob("outside"))
        );
    }

    #[test]
    fn absents_are_stable_values_with_diagnostics() {
        let missing = new_cell_id();
        let evaluation = evaluate(
            &Value::from(missing),
            |_| None,
            &ForeignFunctions::new(),
            10,
        );
        assert_eq!(evaluation.result, Value::from(absent::MISSING_CELL));
        assert_eq!(evaluation.diagnostics, [Diagnostic::MissingCell(missing)]);

        let malformed = Value::record([
            (vocabulary::PARAMS, blob("not a list")),
            (vocabulary::BODY, blob("body")),
        ]);
        let evaluation = evaluate(&malformed, |_| None, &ForeignFunctions::new(), 10);
        assert_eq!(evaluation.result, Value::from(absent::MALFORMED_FUNCTION));
    }

    #[test]
    fn incomplete_function_shapes_are_inert_data() {
        for incomplete in [
            Value::record([(
                vocabulary::PARAMS,
                Value::list(Vec::<Value>::new()),
            )]),
            Value::record([(vocabulary::BODY, blob("body"))]),
        ] {
            let evaluation = evaluate(
                &incomplete,
                |_| None,
                &ForeignFunctions::new(),
                10,
            );
            assert_eq!(evaluation.result, incomplete);
            assert!(evaluation.diagnostics.is_empty());
        }
    }

    #[test]
    fn recursion_and_cell_cycles_are_values() {
        let recurse = new_cell_id();
        let parameter = new_cell_id();
        let definition = function(
            [parameter],
            call(Value::from(recurse), [(parameter, Value::from(parameter))]),
        );
        let evaluation = evaluate(
            &call(Value::from(recurse), [(parameter, blob("again"))]),
            |cell| (cell == recurse).then(|| definition.clone()),
            &ForeignFunctions::new(),
            30,
        );
        assert_eq!(evaluation.result, Value::from(absent::FUEL_EXHAUSTED));

        let a = new_cell_id();
        let b = new_cell_id();
        let evaluation = evaluate(
            &Value::from(a),
            |cell| match cell {
                cell if cell == a => Some(Value::from(b)),
                cell if cell == b => Some(Value::from(a)),
                _ => None,
            },
            &ForeignFunctions::new(),
            20,
        );
        assert_eq!(evaluation.result, Value::from(absent::CELL_CYCLE));
    }

    #[test]
    fn extra_call_fields_do_not_prevent_calling() {
        let parameter = new_cell_id();
        let extra = new_cell_id();
        let expression = call(
            function([parameter], Value::from(parameter)),
            [
                (parameter, blob("result")),
                (extra, blob("still graph data")),
            ],
        );
        let evaluation = evaluate(&expression, |_| None, &ForeignFunctions::new(), 10);
        assert_eq!(evaluation.result, blob("result"));
    }

    #[test]
    fn registration_refuses_ambiguous_function_shapes() {
        let function = new_cell_id();
        let parameter = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        assert_eq!(
            foreign.register(function, [vocabulary::FUNCTION], |_| blob("x")),
            Err(RegistrationError::ReservedParameter(vocabulary::FUNCTION))
        );
        assert_eq!(
            foreign.register(function, [parameter, parameter], |_| blob("x")),
            Err(RegistrationError::DuplicateParameter(parameter))
        );
        foreign
            .register(function, [parameter], |_| blob("x"))
            .unwrap();
        assert_eq!(
            foreign.register(function, [parameter], |_| blob("x")),
            Err(RegistrationError::AlreadyRegistered(function))
        );
    }

    #[test]
    fn library_describes_the_grap_forms_and_absents() {
        let library = library();
        for (cell, name) in [
            (vocabulary::FUNCTION, "function"),
            (vocabulary::PARAMS, "params"),
            (vocabulary::BODY, "body"),
            (vocabulary::QUOTE, "quote"),
            (vocabulary::UNQUOTE, "unquote"),
        ] {
            assert_eq!(library.value(cell).and_then(progred_name::read), Some(name));
        }
        assert!(grap_absent::is_absent(
            library.value(absent::MISSING_CELL).unwrap()
        ));
        assert_eq!(library.cells().count(), 14);
    }
}
