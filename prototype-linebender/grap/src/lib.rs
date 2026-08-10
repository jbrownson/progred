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

pub mod error {
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

type ForeignCall = Rc<dyn Fn(&[Value]) -> Value>;

#[derive(Clone)]
struct ForeignFunction {
    params: Vec<CellId>,
    call: ForeignCall,
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
                write!(f, "a function needs params and body fields")
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
    /// Top-level call fields that the chosen function did not consume.
    /// Projections use this to avoid hiding unrelated graph data.
    pub unconsumed: BTreeSet<CellId>,
    pub steps: usize,
}

#[derive(Clone)]
enum RuntimeValue {
    Data(Value),
    Closure {
        params: Vec<CellId>,
        body: Value,
        environment: Environment,
    },
    Foreign(CellId),
}

type Environment = HashMap<CellId, RuntimeValue>;

struct Evaluator<'a, R> {
    resolve: &'a R,
    foreign: &'a ForeignFunctions,
    remaining: usize,
    initial_fuel: usize,
    diagnostics: Vec<Diagnostic>,
    dependencies: BTreeSet<CellId>,
    resolving: Vec<CellId>,
    depth: usize,
    unconsumed: BTreeSet<CellId>,
    halted: bool,
}

impl<'a, R> Evaluator<'a, R>
where
    R: Fn(CellId) -> Option<Value>,
{
    fn evaluate(mut self, expression: &Value) -> Evaluation {
        let evaluated = self.eval(expression, &Environment::new());
        let result = if self.halted {
            Value::from(error::FUEL_EXHAUSTED)
        } else {
            self.into_data(evaluated)
        };
        Evaluation {
            result,
            diagnostics: self.diagnostics,
            dependencies: self.dependencies,
            unconsumed: self.unconsumed,
            steps: self.initial_fuel - self.remaining,
        }
    }

    fn eval(&mut self, expression: &Value, environment: &Environment) -> RuntimeValue {
        if !self.burn() {
            return RuntimeValue::Data(Value::from(error::FUEL_EXHAUSTED));
        }
        let root = self.depth == 0;
        self.depth += 1;
        let result = match expression {
            Value::Cell(cell) => self.eval_cell(*cell, environment),
            Value::Record(fields) if fields.contains_key(&vocabulary::QUOTE) => {
                self.eval_quote(expression, environment)
            }
            Value::Record(fields) if fields.contains_key(&vocabulary::FUNCTION) => {
                self.eval_call(expression, environment, root)
            }
            Value::Record(fields)
                if fields.contains_key(&vocabulary::PARAMS)
                    || fields.contains_key(&vocabulary::BODY) =>
            {
                self.eval_function(expression, environment)
            }
            Value::Blob(_) | Value::List(_) | Value::Record(_) => {
                RuntimeValue::Data(expression.clone())
            }
        };
        self.depth -= 1;
        result
    }

    fn burn(&mut self) -> bool {
        if self.remaining == 0 {
            if !self.halted {
                self.diagnostics.push(Diagnostic::FuelExhausted);
                self.halted = true;
            }
            false
        } else {
            self.remaining -= 1;
            true
        }
    }

    fn failure(&mut self, diagnostic: Diagnostic, cell: CellId) -> RuntimeValue {
        RuntimeValue::Data(self.failure_value(diagnostic, cell))
    }

    fn failure_value(&mut self, diagnostic: Diagnostic, cell: CellId) -> Value {
        self.diagnostics.push(diagnostic);
        Value::from(cell)
    }

    fn into_data(&mut self, value: RuntimeValue) -> Value {
        match value {
            RuntimeValue::Data(value) => value,
            RuntimeValue::Closure { .. } | RuntimeValue::Foreign(_) => self.failure_value(
                Diagnostic::FunctionIsNotData,
                error::FUNCTION_IS_NOT_DATA,
            ),
        }
    }

    fn eval_cell(&mut self, cell: CellId, environment: &Environment) -> RuntimeValue {
        if let Some(value) = environment.get(&cell) {
            value.clone()
        } else if self.foreign.get(cell).is_some() {
            RuntimeValue::Foreign(cell)
        } else if let Some(first) = self
            .resolving
            .iter()
            .position(|resolving| *resolving == cell)
        {
            self.failure(
                Diagnostic::CellCycle(
                    self.resolving[first..]
                        .iter()
                        .copied()
                        .chain([cell])
                        .collect(),
                ),
                error::CELL_CYCLE,
            )
        } else {
            self.dependencies.insert(cell);
            match (self.resolve)(cell) {
                Some(value) => {
                    self.resolving.push(cell);
                    let result = self.eval(&value, environment);
                    self.resolving.pop();
                    result
                }
                None => self.failure(Diagnostic::MissingCell(cell), error::MISSING_CELL),
            }
        }
    }

    fn eval_function(&mut self, expression: &Value, environment: &Environment) -> RuntimeValue {
        let fields = expression.as_record().expect("matched record");
        match (
            fields.get(&vocabulary::PARAMS).and_then(Value::as_list),
            fields.get(&vocabulary::BODY),
        ) {
            (Some(parameters), Some(body)) => {
                let params = parameters
                    .values()
                    .map(|parameter| parameter.as_cell().ok_or_else(|| parameter.clone()))
                    .collect::<Result<Vec<_>, _>>();
                match params {
                    Err(parameter) => self.failure(
                        Diagnostic::InvalidParameter(parameter),
                        error::INVALID_PARAMETER,
                    ),
                    Ok(params) if params.contains(&vocabulary::FUNCTION) => self.failure(
                        Diagnostic::ReservedParameter(vocabulary::FUNCTION),
                        error::INVALID_PARAMETER,
                    ),
                    Ok(params) if duplicate(&params).is_some() => {
                        let duplicate = duplicate(&params).expect("matched duplicate");
                        self.failure(
                            Diagnostic::DuplicateParameter(duplicate),
                            error::INVALID_PARAMETER,
                        )
                    }
                    Ok(params) => RuntimeValue::Closure {
                        params,
                        body: body.clone(),
                        environment: environment.clone(),
                    },
                }
            }
            _ => self.failure(Diagnostic::MalformedFunction, error::MALFORMED_FUNCTION),
        }
    }

    fn eval_call(
        &mut self,
        expression: &Value,
        environment: &Environment,
        root: bool,
    ) -> RuntimeValue {
        let fields = expression.as_record().expect("matched record");
        let callable = self.eval(
            fields
                .get(&vocabulary::FUNCTION)
                .expect("matched function field"),
            environment,
        );
        if self.halted {
            return callable;
        }
        let params = match &callable {
            RuntimeValue::Closure { params, .. } => Some(params.clone()),
            RuntimeValue::Foreign(function) => Some(
                self.foreign
                    .get(*function)
                    .expect("foreign function was resolved")
                    .params
                    .clone(),
            ),
            RuntimeValue::Data(value) => {
                return self.failure(
                    Diagnostic::NotCallable(value.clone()),
                    error::NOT_CALLABLE,
                );
            }
        }
        .expect("all callable variants provide params");
        if root {
            self.unconsumed.extend(
                fields
                    .keys()
                    .filter(|field| {
                        **field != vocabulary::FUNCTION && !params.contains(field)
                    })
                    .copied(),
            );
        }
        if let Some(missing) = params
            .iter()
            .find(|parameter| !fields.contains_key(*parameter))
        {
            return self.failure(
                Diagnostic::MissingArgument(*missing),
                error::MISSING_ARGUMENT,
            );
        }
        let mut arguments = Environment::new();
        for parameter in &params {
            let argument = self.eval(
                fields.get(parameter).expect("checked argument"),
                environment,
            );
            if self.halted {
                return argument;
            }
            arguments.insert(*parameter, argument);
        }
        match callable {
            RuntimeValue::Closure {
                body,
                mut environment,
                ..
            } => {
                environment.extend(arguments);
                self.eval(&body, &environment)
            }
            RuntimeValue::Foreign(function) => {
                let values = params
                    .iter()
                    .map(|parameter| match arguments.get(parameter) {
                        Some(RuntimeValue::Data(value)) => Ok(value.clone()),
                        Some(RuntimeValue::Closure { .. } | RuntimeValue::Foreign(_)) => {
                            Err(*parameter)
                        }
                        None => unreachable!("checked argument"),
                    })
                    .collect::<Result<Vec<_>, _>>();
                match values {
                    Ok(values) => RuntimeValue::Data(
                        (self
                            .foreign
                            .get(function)
                            .expect("foreign function was resolved")
                            .call)(&values),
                    ),
                    Err(parameter) => self.failure(
                        Diagnostic::ForeignArgumentIsFunction(parameter),
                        error::FOREIGN_ARGUMENT_IS_FUNCTION,
                    ),
                }
            }
            RuntimeValue::Data(_) => unreachable!("checked callable"),
        }
    }

    fn eval_quote(&mut self, expression: &Value, environment: &Environment) -> RuntimeValue {
        let template = expression
            .as_record()
            .expect("matched record")
            .get(&vocabulary::QUOTE)
            .expect("matched quote field");
        RuntimeValue::Data(self.expand_quote(template, environment))
    }

    fn expand_quote(&mut self, template: &Value, environment: &Environment) -> Value {
        if !self.burn() {
            return Value::from(error::FUEL_EXHAUSTED);
        }
        match template {
            Value::Cell(_) | Value::Blob(_) => template.clone(),
            Value::List(elements) => Value::List(
                elements
                    .iter()
                    .map(|(position, value)| {
                        (position.clone(), self.expand_quote(value, environment))
                    })
                    .collect(),
            ),
            Value::Record(fields) if fields.contains_key(&vocabulary::UNQUOTE) => {
                let evaluated = self.eval(
                    fields
                        .get(&vocabulary::UNQUOTE)
                        .expect("matched unquote field"),
                    environment,
                );
                self.into_data(evaluated)
            }
            Value::Record(fields) => Value::Record(
                fields
                    .iter()
                    .map(|(field, value)| (*field, self.expand_quote(value, environment)))
                    .collect(),
            ),
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
        remaining: fuel,
        initial_fuel: fuel,
        diagnostics: Vec::new(),
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
        depth: 0,
        unconsumed: BTreeSet::new(),
        halted: false,
    }
    .evaluate(expression)
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
        (error::FUEL_EXHAUSTED, "fuel exhausted"),
        (error::MISSING_CELL, "missing cell"),
        (error::CELL_CYCLE, "cell cycle"),
        (error::MALFORMED_FUNCTION, "malformed function"),
        (error::INVALID_PARAMETER, "invalid parameter"),
        (error::NOT_CALLABLE, "not callable"),
        (error::MISSING_ARGUMENT, "missing argument"),
        (
            error::FOREIGN_ARGUMENT_IS_FUNCTION,
            "foreign argument is function",
        ),
        (error::FUNCTION_IS_NOT_DATA, "function is not data"),
    ] {
        cells.set_value(cell, grap_error::named(name));
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
        assert_eq!(evaluation.steps, 1);
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
    fn failures_are_stable_error_values_with_diagnostics() {
        let missing = new_cell_id();
        let evaluation = evaluate(
            &Value::from(missing),
            |_| None,
            &ForeignFunctions::new(),
            10,
        );
        assert_eq!(evaluation.result, Value::from(error::MISSING_CELL));
        assert_eq!(evaluation.diagnostics, [Diagnostic::MissingCell(missing)]);

        let malformed = Value::record([(
            vocabulary::PARAMS,
            Value::list(Vec::<Value>::new()),
        )]);
        let evaluation = evaluate(&malformed, |_| None, &ForeignFunctions::new(), 10);
        assert_eq!(evaluation.result, Value::from(error::MALFORMED_FUNCTION));
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
        assert_eq!(evaluation.result, Value::from(error::FUEL_EXHAUSTED));

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
        assert_eq!(evaluation.result, Value::from(error::CELL_CYCLE));
    }

    #[test]
    fn malformed_calls_report_unconsumed_fields() {
        let parameter = new_cell_id();
        let extra = new_cell_id();
        let expression = call(
            function([parameter], Value::from(parameter)),
            [(extra, blob("still data"))],
        );
        let evaluation = evaluate(&expression, |_| None, &ForeignFunctions::new(), 10);
        assert_eq!(evaluation.result, Value::from(error::MISSING_ARGUMENT));
        assert_eq!(evaluation.unconsumed, BTreeSet::from([extra]));
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
    fn library_describes_the_grap_forms_and_errors() {
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
        assert!(grap_error::is_error(
            library.value(error::MISSING_CELL).unwrap()
        ));
        assert_eq!(library.cells().count(), 14);
    }
}
