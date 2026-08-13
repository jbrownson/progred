//! A small evaluator whose expressions and results are Progred graph
//! values. The evaluator recognizes only Grap forms; ordinary records
//! and lists are inert data, and each recognized form chooses its own
//! recursive evaluation.

use im::OrdMap;
use progred_graph::{CellId, Cells, Value};
use std::collections::{BTreeSet, HashMap, hash_map::Entry};
use std::fmt;

pub mod vocabulary {
    use progred_graph::CellId;

    pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
    pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
    pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
    pub const CLOSURE: CellId =
        CellId::from_u128(0xdb39600f3ed07398c77ac120deb108a8);
    pub const ENVIRONMENT: CellId =
        CellId::from_u128(0xe910025c710c25c43d0a5b296378f374);
    pub const FFI: CellId = CellId::from_u128(0x912adb7252d689659b6de9eeeb827658);
    pub const EVALUATE: CellId =
        CellId::from_u128(0xacfc5e50881292518dab3cec77cf43ee);
    pub const EXPRESSION: CellId =
        CellId::from_u128(0xccc55b0eb63b9f564ea74436094d4014);
}

pub mod absent {
    use progred_graph::CellId;

    pub const FUEL_EXHAUSTED: CellId =
        CellId::from_u128(0x513628d759c04b3e7088b575e555a80e);
    pub const MISSING_CELL: CellId =
        CellId::from_u128(0xa5a1b4e3d0df96bd11af00f0780136ff);
    pub const CELL_CYCLE: CellId =
        CellId::from_u128(0x150e0fc7e38d1670f41283c3d23a9b8d);
    pub const MALFORMED_LAMBDA: CellId =
        CellId::from_u128(0xfbf5894d7f62b6d0048d17d26851b415);
    pub const INVALID_PARAMETER: CellId =
        CellId::from_u128(0x93ca0e9199372c46ee24bd4c508e178c);
    pub const NOT_CALLABLE: CellId =
        CellId::from_u128(0x8624488c2d10d2a4b84560dfa99a38e6);
    pub const MISSING_ARGUMENT: CellId =
        CellId::from_u128(0x8b2f0db36e5c3d35595eb5666cc89c78);
    pub const INVALID_ENVIRONMENT: CellId =
        CellId::from_u128(0x152f2cac01f072317ab5746c5befdf9c);
}

pub const DEFAULT_FUEL: usize = 1_024;

struct ForeignFunction {
    params: Vec<CellId>,
    call: ForeignCall,
}

type ForeignCall =
    for<'a> fn(&mut Evaluate<'a>, &[Value], &Environment) -> Result<Value, Halt>;

pub struct Halt(Value);

pub type Evaluate<'a> =
    dyn FnMut(&Value, &Environment) -> Result<Value, Halt> + 'a;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment(OrdMap<CellId, Value>);

impl Environment {
    fn empty() -> Self {
        Self(OrdMap::new())
    }

    fn from_value(value: &Value) -> Option<Self> {
        value.as_record().cloned().map(Self)
    }

    pub fn get(&self, cell: CellId) -> Option<&Value> {
        self.0.get(&cell)
    }

    pub fn extended(
        &self,
        bindings: impl IntoIterator<Item = (CellId, Value)>,
    ) -> Self {
        Self(
            bindings
                .into_iter()
                .fold(self.0.clone(), |environment, (cell, value)| {
                    environment.update(cell, value)
                }),
        )
    }

    pub fn to_value(&self) -> Value {
        Value::Record(self.0.clone())
    }
}

pub struct ForeignFunctions {
    functions: HashMap<CellId, ForeignFunction>,
}

impl Default for ForeignFunctions {
    fn default() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    AlreadyRegistered(CellId),
}

impl ForeignFunctions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        function: CellId,
        params: impl IntoIterator<Item = CellId>,
        call: ForeignCall,
    ) -> Result<(), RegistrationError> {
        let params: Vec<_> = params.into_iter().collect();
        match self.functions.entry(function) {
            Entry::Occupied(_) => Err(RegistrationError::AlreadyRegistered(function)),
            Entry::Vacant(entry) => {
                entry.insert(ForeignFunction {
                    params,
                    call,
                });
                Ok(())
            }
        }
    }

    fn get(&self, function: CellId) -> Option<&ForeignFunction> {
        self.functions.get(&function)
    }

    pub fn merge(mut self, other: Self) -> Result<Self, RegistrationError> {
        for (cell, function) in other.functions {
            match self.functions.entry(cell) {
                Entry::Occupied(_) => return Err(RegistrationError::AlreadyRegistered(cell)),
                Entry::Vacant(entry) => {
                    entry.insert(function);
                }
            }
        }
        Ok(self)
    }

    pub fn merge_all(
        tables: impl IntoIterator<Item = Self>,
    ) -> Result<Self, RegistrationError> {
        tables.into_iter().try_fold(Self::new(), Self::merge)
    }
}

/// Core Grap's registered Rust functions. Libraries return their own
/// tables; the editor merges them.
pub fn functions() -> ForeignFunctions {
    let mut foreign = ForeignFunctions::new();
    foreign
        .register(
            vocabulary::EVALUATE,
            [vocabulary::EXPRESSION, vocabulary::ENVIRONMENT],
            evaluate_foreign,
        )
        .expect("core Grap registers one function");
    foreign
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
        let result = self
            .eval(expression, &Environment::empty())
            .unwrap_or_else(|Halt(result)| result);
        Evaluation {
            result,
            diagnostics: self.diagnostics,
            dependencies: self.dependencies,
            remaining_fuel: self.remaining_fuel,
        }
    }

    fn eval(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        self.burn()?;
        match expression {
            Value::Cell(cell) => self.eval_cell(*cell, environment),
            Value::Record(fields) => {
                // Open records may match more than one form. For now the evaluator
                // uses the simple precedence call, then lambda; an
                // ambiguous-form absent can replace it if overlaps matter in practice.
                if let Some(function) = fields.get(&vocabulary::FUNCTION) {
                    self.eval_call(function, |field| fields.get(&field), environment)
                } else {
                    match (
                        fields.get(&vocabulary::PARAMS),
                        fields.get(&vocabulary::BODY),
                    ) {
                        (Some(parameters), Some(_)) => {
                            Ok(self.eval_lambda(fields, parameters, environment))
                        }
                        _ => Ok(expression.clone()),
                    }
                }
            }
            Value::Blob(_) | Value::List(_) => {
                Ok(expression.clone())
            }
        }
    }

    fn burn(&mut self) -> Result<(), Halt> {
        self.remaining_fuel = self.remaining_fuel.saturating_sub(1);
        if self.remaining_fuel == 0 {
            Err(Halt(self.absent(
                Diagnostic::FuelExhausted,
                absent::FUEL_EXHAUSTED,
            )))
        } else {
            Ok(())
        }
    }

    fn absent(&mut self, diagnostic: Diagnostic, cell: CellId) -> Value {
        self.diagnostics.push(diagnostic);
        Value::from(cell)
    }

    fn eval_cell(
        &mut self,
        cell: CellId,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        if let Some(value) = environment.get(cell) {
            Ok(value.clone())
        } else if self.foreign.get(cell).is_some() {
            Ok(Value::record([(vocabulary::FFI, Value::from(cell))]))
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

    fn eval_lambda(
        &mut self,
        fields: &OrdMap<CellId, Value>,
        parameters: &Value,
        environment: &Environment,
    ) -> Value {
        let Some(parameters) = parameters.as_list() else {
            return self.absent(Diagnostic::MalformedLambda, absent::MALFORMED_LAMBDA);
        };
        if let Some(parameter) = parameters
            .values()
            .find(|parameter| parameter.as_cell().is_none())
        {
            return self.absent(
                Diagnostic::InvalidParameter(parameter.clone()),
                absent::INVALID_PARAMETER,
            );
        }
        let closure = Value::Record(
            fields
                .clone()
                .update(vocabulary::ENVIRONMENT, environment.to_value()),
        );
        Value::record([(vocabulary::CLOSURE, closure)])
    }

    fn eval_call<'v>(
        &mut self,
        function: &Value,
        argument: impl Fn(CellId) -> Option<&'v Value>,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        let foreign = self.foreign;
        let callable = self.eval(function, environment)?;
        match closure_target(&callable) {
            Some((params, body, closure_environment)) => self.eval_grap_call(
                params,
                body,
                closure_environment,
                argument,
                environment,
            ),
            None => match foreign_target(&callable, foreign) {
                Some(function) => self.eval_foreign_call(function, argument, environment),
                None => Ok(self.absent(
                    Diagnostic::NotCallable(callable),
                    absent::NOT_CALLABLE,
                )),
            },
        }
    }

    fn eval_grap_call<'v>(
        &mut self,
        params: Vec<CellId>,
        body: Value,
        closure_environment: Environment,
        argument: impl Fn(CellId) -> Option<&'v Value>,
        calling_environment: &Environment,
    ) -> Result<Value, Halt> {
        let mut expressions = Vec::with_capacity(params.len());
        for parameter in params {
            let Some(expression) = argument(parameter) else {
                return Ok(self.absent(
                    Diagnostic::MissingArgument(parameter),
                    absent::MISSING_ARGUMENT,
                ));
            };
            expressions.push((parameter, expression));
        }
        let arguments = expressions
            .into_iter()
            .map(|(parameter, expression)| {
                Ok((parameter, self.eval(expression, calling_environment)?))
            })
            .collect::<Result<Vec<_>, Halt>>()?;
        let body_environment = closure_environment.extended(arguments);
        self.eval(&body, &body_environment)
    }

    fn eval_foreign_call<'v>(
        &mut self,
        function: &ForeignFunction,
        argument: impl Fn(CellId) -> Option<&'v Value>,
        calling_environment: &Environment,
    ) -> Result<Value, Halt> {
        let mut arguments = Vec::with_capacity(function.params.len());
        for parameter in &function.params {
            let Some(expression) = argument(*parameter) else {
                return Ok(self.absent(
                    Diagnostic::MissingArgument(*parameter),
                    absent::MISSING_ARGUMENT,
                ));
            };
            arguments.push(expression.clone());
        }
        let mut evaluate = |expression: &Value, environment: &Environment| {
            self.eval(expression, environment)
        };
        (function.call)(&mut evaluate, &arguments, calling_environment)
    }
}

fn closure_target(value: &Value) -> Option<(Vec<CellId>, Value, Environment)> {
    let fields = value
        .as_record()?
        .get(&vocabulary::CLOSURE)?
        .as_record()?;
    let params = fields
        .get(&vocabulary::PARAMS)?
        .as_list()?
        .values()
        .map(Value::as_cell)
        .collect::<Option<Vec<_>>>()?;
    Some((
        params,
        fields.get(&vocabulary::BODY)?.clone(),
        Environment::from_value(fields.get(&vocabulary::ENVIRONMENT)?)?,
    ))
}

fn foreign_target<'a>(
    value: &Value,
    foreign: &'a ForeignFunctions,
) -> Option<&'a ForeignFunction> {
    value
        .as_record()?
        .get(&vocabulary::FFI)?
        .as_cell()
        .and_then(|cell| foreign.get(cell))
}

fn evaluate_foreign(
    evaluate: &mut Evaluate<'_>,
    arguments: &[Value],
    calling_environment: &Environment,
) -> Result<Value, Halt> {
    match arguments {
        [expression, environment] => {
            let environment = evaluate(environment, calling_environment)?;
            match Environment::from_value(&environment) {
                Some(environment) => evaluate(expression, &environment),
                None => Ok(Value::from(absent::INVALID_ENVIRONMENT)),
            }
        }
        _ => unreachable!("Grap checks foreign arity before calling"),
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
        (vocabulary::CLOSURE, "closure"),
        (vocabulary::ENVIRONMENT, "environment"),
        (vocabulary::FFI, "ffi"),
        (vocabulary::EVALUATE, "evaluate"),
        (vocabulary::EXPRESSION, "expression"),
    ] {
        cells.set_value(cell, progred_name::record(name, []));
    }
    for (cell, name) in [
        (absent::FUEL_EXHAUSTED, "fuel exhausted"),
        (absent::MISSING_CELL, "missing cell"),
        (absent::CELL_CYCLE, "cell cycle"),
        (absent::MALFORMED_LAMBDA, "malformed lambda"),
        (absent::INVALID_PARAMETER, "invalid parameter"),
        (absent::NOT_CALLABLE, "not callable"),
        (absent::MISSING_ARGUMENT, "missing argument"),
        (absent::INVALID_ENVIRONMENT, "invalid environment"),
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
        let definition = lambda([x, y], Value::from(x));
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
            lambda([parameter], Value::from(parameter)),
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
        let inner = lambda([y], Value::from(x));
        let outer = lambda([x], call(inner, [(y, blob("ignored"))]));
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
    fn rust_functions_control_evaluation_while_graph_functions_are_strict() {
        let hold = new_cell_id();
        let input = new_cell_id();
        let field = new_cell_id();
        let call_shaped_data = call(Value::from(new_cell_id()), []);
        let held = call(Value::from(hold), [(input, call_shaped_data.clone())]);
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(hold, [input], |_, arguments, _| match arguments {
                [argument] => Ok(argument.clone()),
                _ => unreachable!("Grap checks foreign arity before calling"),
            })
            .unwrap();
        let graph = lambda([input], Value::from(input));
        assert_eq!(
            evaluate(
                &held,
                |_| None,
                &foreign,
                30,
            )
            .result,
            call_shaped_data
        );
        assert_eq!(
            evaluate(
                &call(graph, [(input, held)]),
                |_| None,
                &foreign,
                30,
            )
            .result,
            call_shaped_data
        );
        let inert = Value::record([(field, call(Value::from(hold), []))]);
        assert_eq!(
            evaluate(&inert, |_| None, &foreign, 30).result,
            inert
        );
    }

    #[test]
    fn registered_cells_evaluate_to_explicit_foreign_callables() {
        let echo = new_cell_id();
        let input = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(
                echo,
                [input],
                |evaluate, arguments, environment| match arguments {
                    [argument] => evaluate(argument, environment),
                    _ => unreachable!("Grap checks foreign arity before calling"),
                },
            )
            .unwrap();

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
        let echo = new_cell_id();
        let callable = new_cell_id();
        let input = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(
                echo,
                [input],
                |evaluate, arguments, environment| match arguments {
                    [argument] => evaluate(argument, environment),
                    _ => unreachable!("Grap checks foreign arity before calling"),
                },
            )
            .unwrap();
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
        let choose = new_cell_id();
        let condition = new_cell_id();
        let yes = new_cell_id();
        let no = new_cell_id();
        let parameter = new_cell_id();
        let missing = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(
                choose,
                [condition, yes, no],
                |evaluate, arguments, environment| match arguments {
                    [condition, yes, no] => {
                        if evaluate(condition, environment)? == blob("true") {
                            evaluate(yes, environment)
                        } else {
                            evaluate(no, environment)
                        }
                    }
                    _ => unreachable!("Grap checks foreign arity before calling"),
                },
            )
            .unwrap();
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
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(inspect, [], |_, _, environment| {
                Ok(environment.to_value())
            })
            .unwrap();
        let inspect_from_body = lambda(
            [parameter],
            call(Value::from(inspect), []),
        );
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
        const BINDING: CellId =
            CellId::from_u128(0xedcd2b19cf89faf94a4a72ab1e02ec31);
        let bind = new_cell_id();
        let value = new_cell_id();
        let body = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(
                bind,
                [value, body],
                |evaluate, arguments, environment| match arguments {
                    [value, body] => {
                        let value = evaluate(value, environment)?;
                        evaluate(body, &environment.extended([(BINDING, value)]))
                    }
                    _ => unreachable!("Grap supplies the registered fields"),
                },
            )
            .unwrap();
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
    fn evaluate_is_an_ordinary_registered_rust_function() {
        let input = new_cell_id();
        let expression = call(
            lambda([input], Value::from(input)),
            [(input, blob("evaluated"))],
        );
        let evaluation = evaluate(
            &call(
                Value::from(vocabulary::EVALUATE),
                [
                    (vocabulary::EXPRESSION, expression),
                    (vocabulary::ENVIRONMENT, Value::record([])),
                ],
            ),
            |_| None,
            &functions(),
            40,
        );
        assert_eq!(evaluation.result, blob("evaluated"));
        assert!(evaluation.diagnostics.is_empty());
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
            &ForeignFunctions::new(),
            40,
        )
        .result;
        assert_eq!(
            evaluate(
                &call(closure.clone(), []),
                |_| None,
                &ForeignFunctions::new(),
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
        assert_eq!(evaluation.result, Value::from(absent::MALFORMED_LAMBDA));
    }

    #[test]
    fn incomplete_lambda_shapes_are_inert_data() {
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
        let definition = lambda(
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
            lambda([parameter], Value::from(parameter)),
            [
                (parameter, blob("result")),
                (extra, blob("still graph data")),
            ],
        );
        let evaluation = evaluate(&expression, |_| None, &ForeignFunctions::new(), 10);
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
            &ForeignFunctions::new(),
            30,
        );
        assert!(closure_target(&evaluation.result).is_some());
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn foreign_parameters_may_repeat_and_name_the_function_field() {
        let function = new_cell_id();
        let parameter = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(
                function,
                [vocabulary::FUNCTION, parameter, parameter],
                |_, arguments, _| match arguments {
                    [function, first, second] => {
                        assert!(function.as_cell().is_some());
                        assert_eq!(first, second);
                        Ok(first.clone())
                    }
                    _ => unreachable!("Grap checks foreign arity before calling"),
                },
            )
            .unwrap();
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
    fn registration_refuses_only_duplicate_implementations() {
        let function = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(function, [], |_, _, _| Ok(blob("x")))
            .unwrap();
        assert_eq!(
            foreign.register(function, [], |_, _, _| Ok(blob("x"))),
            Err(RegistrationError::AlreadyRegistered(function))
        );
    }

    #[test]
    fn merge_refuses_overlapping_tables() {
        let function = new_cell_id();
        let mut left = ForeignFunctions::new();
        left.register(function, [], |_, _, _| Ok(blob("left")))
            .unwrap();
        let mut right = ForeignFunctions::new();
        right
            .register(function, [], |_, _, _| Ok(blob("right")))
            .unwrap();
        assert_eq!(
            left.merge(right).err(),
            Some(RegistrationError::AlreadyRegistered(function))
        );
    }

    #[test]
    fn library_describes_the_grap_forms_and_absents() {
        let library = library();
        for (cell, name) in [
            (vocabulary::FUNCTION, "function"),
            (vocabulary::PARAMS, "params"),
            (vocabulary::BODY, "body"),
            (vocabulary::CLOSURE, "closure"),
            (vocabulary::ENVIRONMENT, "environment"),
            (vocabulary::FFI, "ffi"),
            (vocabulary::EVALUATE, "evaluate"),
            (vocabulary::EXPRESSION, "expression"),
        ] {
            assert_eq!(library.value(cell).and_then(progred_name::read), Some(name));
        }
        assert!(grap_absent::is_absent(
            library.value(absent::MISSING_CELL).unwrap()
        ));
        assert_eq!(library.cells().count(), 16);
    }
}
