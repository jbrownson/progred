//! A small evaluator whose syntax is Progred's existing graph data.
//! Grap adds no value variants or identity space: three fixed library
//! cells describe functions and calls, while all data and operations
//! beyond those primitives arrive through ordinary Grap libraries.

use progred_graph::{Atom, CellId, Cells, Label, Value};
use std::collections::{BTreeSet, HashMap, hash_map::Entry};
use std::fmt;
use std::rc::Rc;

pub mod vocabulary {
    use progred_graph::CellId;

    pub const FUNCTION: CellId = CellId::from_u128(0x652a661e44ea44bd48bce4306e6e5995);
    pub const PARAMS: CellId = CellId::from_u128(0xa4e2f4e064fe700d4dd5e55404839382);
    pub const BODY: CellId = CellId::from_u128(0xfcf003c00ec4144cb748e8bbc7bb568e);
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
pub enum Error {
    FuelExhausted,
    MissingCell(CellId),
    CellCycle(Vec<CellId>),
    MalformedFunction,
    InvalidParameter(Value),
    DuplicateParameter(CellId),
    ReservedParameter(CellId),
    NotCallable(Value),
    MissingArgument(CellId),
    UnexpectedArgument(Label),
    ForeignArgumentIsFunction(CellId),
    FunctionIsNotData,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FuelExhausted => write!(f, "evaluation ran out of fuel"),
            Error::MissingCell(cell) => write!(f, "cell {cell} has no value"),
            Error::CellCycle(cells) => write!(
                f,
                "cell resolution cycle: {}",
                cells
                    .iter()
                    .map(CellId::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            Error::MalformedFunction => {
                write!(f, "a function needs exactly params and body fields")
            }
            Error::InvalidParameter(value) => {
                write!(f, "function parameter is not a cell: {value:?}")
            }
            Error::DuplicateParameter(cell) => {
                write!(f, "function parameter {cell} appears more than once")
            }
            Error::ReservedParameter(cell) => {
                write!(f, "cell {cell} is reserved by the call representation")
            }
            Error::NotCallable(value) => write!(f, "value is not callable: {value:?}"),
            Error::MissingArgument(cell) => write!(f, "missing argument {cell}"),
            Error::UnexpectedArgument(label) => write!(f, "unexpected argument {label}"),
            Error::ForeignArgumentIsFunction(cell) => {
                write!(f, "foreign argument {cell} evaluated to a function")
            }
            Error::FunctionIsNotData => write!(f, "evaluation produced a function"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub result: Result<Value, Error>,
    pub dependencies: BTreeSet<CellId>,
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
    dependencies: BTreeSet<CellId>,
    resolving: Vec<CellId>,
}

impl<'a, R> Evaluator<'a, R>
where
    R: Fn(CellId) -> Option<Value>,
{
    fn evaluate(mut self, expression: &Value) -> Evaluation {
        let result = self
            .eval(expression, &Environment::new())
            .and_then(|value| match value {
                RuntimeValue::Data(value) => Ok(value),
                RuntimeValue::Closure { .. } | RuntimeValue::Foreign(_) => {
                    Err(Error::FunctionIsNotData)
                }
            });
        Evaluation {
            result,
            dependencies: self.dependencies,
            steps: self.initial_fuel - self.remaining,
        }
    }

    fn eval(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Error> {
        self.burn()?;
        match expression {
            Value::Atom(Atom::Cell(cell)) => self.eval_cell(*cell, environment),
            Value::Record(fields) if fields.contains_key(&Label::Cell(vocabulary::FUNCTION)) => {
                self.eval_call(expression, environment)
            }
            Value::Record(fields)
                if fields.contains_key(&Label::Cell(vocabulary::PARAMS))
                    || fields.contains_key(&Label::Cell(vocabulary::BODY)) =>
            {
                self.eval_function(expression, environment)
            }
            Value::Atom(_) | Value::List(_) | Value::Record(_) => {
                Ok(RuntimeValue::Data(expression.clone()))
            }
        }
    }

    fn burn(&mut self) -> Result<(), Error> {
        if self.remaining == 0 {
            Err(Error::FuelExhausted)
        } else {
            self.remaining -= 1;
            Ok(())
        }
    }

    fn eval_cell(
        &mut self,
        cell: CellId,
        environment: &Environment,
    ) -> Result<RuntimeValue, Error> {
        if let Some(value) = environment.get(&cell) {
            Ok(value.clone())
        } else if self.foreign.get(cell).is_some() {
            Ok(RuntimeValue::Foreign(cell))
        } else if let Some(first) = self
            .resolving
            .iter()
            .position(|resolving| *resolving == cell)
        {
            Err(Error::CellCycle(
                self.resolving[first..]
                    .iter()
                    .copied()
                    .chain([cell])
                    .collect(),
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
                None => Err(Error::MissingCell(cell)),
            }
        }
    }

    fn eval_function(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Error> {
        let fields = expression.as_record().expect("matched record");
        if fields.len() != 2 {
            Err(Error::MalformedFunction)
        } else {
            match (
                fields
                    .get(&Label::Cell(vocabulary::PARAMS))
                    .and_then(Value::as_list),
                fields.get(&Label::Cell(vocabulary::BODY)),
            ) {
                (Some(parameters), Some(body)) => {
                    let params = parameters
                        .values()
                        .map(|parameter| {
                            parameter
                                .as_cell()
                                .ok_or_else(|| Error::InvalidParameter(parameter.clone()))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    match (params.contains(&vocabulary::FUNCTION), duplicate(&params)) {
                        (true, _) => Err(Error::ReservedParameter(vocabulary::FUNCTION)),
                        (false, Some(duplicate)) => Err(Error::DuplicateParameter(duplicate)),
                        (false, None) => Ok(RuntimeValue::Closure {
                            params,
                            body: body.clone(),
                            environment: environment.clone(),
                        }),
                    }
                }
                _ => Err(Error::MalformedFunction),
            }
        }
    }

    fn eval_call(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Error> {
        let fields = expression.as_record().expect("matched record");
        let callable = self.eval(
            fields
                .get(&Label::Cell(vocabulary::FUNCTION))
                .expect("matched function field"),
            environment,
        )?;
        let params = match &callable {
            RuntimeValue::Closure { params, .. } => Ok(params.clone()),
            RuntimeValue::Foreign(function) => Ok(self
                .foreign
                .get(*function)
                .expect("foreign function was resolved")
                .params
                .clone()),
            RuntimeValue::Data(value) => Err(Error::NotCallable(value.clone())),
        }?;
        if let Some(unexpected) = fields
            .keys()
            .filter(|label| **label != Label::Cell(vocabulary::FUNCTION))
            .find(|label| !label.as_cell().is_some_and(|cell| params.contains(&cell)))
        {
            Err(Error::UnexpectedArgument(unexpected.clone()))
        } else if let Some(missing) = params
            .iter()
            .find(|parameter| !fields.contains_key(&Label::Cell(**parameter)))
        {
            Err(Error::MissingArgument(*missing))
        } else {
            let arguments =
                params
                    .iter()
                    .try_fold(Environment::new(), |mut arguments, parameter| {
                        self.eval(
                            fields
                                .get(&Label::Cell(*parameter))
                                .expect("checked argument"),
                            environment,
                        )
                        .map(|argument| {
                            arguments.insert(*parameter, argument);
                            arguments
                        })
                    })?;
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
                                Err(Error::ForeignArgumentIsFunction(*parameter))
                            }
                            None => unreachable!("checked argument"),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let foreign = self
                        .foreign
                        .get(function)
                        .expect("foreign function was resolved");
                    Ok(RuntimeValue::Data((foreign.call)(&values)))
                }
                RuntimeValue::Data(_) => unreachable!("checked callable"),
            }
        }
    }
}

fn duplicate(cells: &[CellId]) -> Option<CellId> {
    cells
        .iter()
        .enumerate()
        .find_map(|(index, cell)| cells[index + 1..].contains(cell).then_some(*cell))
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
        dependencies: BTreeSet::new(),
        resolving: Vec::new(),
    }
    .evaluate(expression)
}

pub fn library() -> Cells {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::FUNCTION, "function"),
        (vocabulary::PARAMS, "params"),
        (vocabulary::BODY, "body"),
    ] {
        cells.set_name(cell, name);
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

    fn call(function: Value, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
        Value::record(
            [(Label::Cell(vocabulary::FUNCTION), function)]
                .into_iter()
                .chain(
                    arguments
                        .into_iter()
                        .map(|(parameter, value)| (Label::Cell(parameter), value)),
                ),
        )
    }

    fn function(params: impl IntoIterator<Item = CellId>, body: Value) -> Value {
        Value::record([
            (
                Label::Cell(vocabulary::PARAMS),
                Value::list(params.into_iter().map(Value::from)),
            ),
            (Label::Cell(vocabulary::BODY), body),
        ])
    }

    #[test]
    fn ordinary_values_are_data() {
        let value = Value::record([(Label::from("x"), Value::list([Value::from("y")]))]);
        let evaluation = evaluate(&value, |_| None, &ForeignFunctions::new(), 10);
        assert_eq!(evaluation.result, Ok(value));
        assert!(evaluation.dependencies.is_empty());
        assert_eq!(evaluation.steps, 1);
    }

    #[test]
    fn cells_resolve_transparently_and_report_dependencies() {
        let first = new_cell_id();
        let second = new_cell_id();
        let evaluation = evaluate(
            &Value::from(first),
            |cell| match cell {
                cell if cell == first => Some(Value::from(second)),
                cell if cell == second => Some(Value::from("done")),
                _ => None,
            },
            &ForeignFunctions::new(),
            10,
        );
        assert_eq!(evaluation.result, Ok(Value::from("done")));
        assert_eq!(evaluation.dependencies, BTreeSet::from([first, second]));
        assert_eq!(evaluation.steps, 3);
    }

    #[test]
    fn foreign_functions_are_supplied_by_libraries() {
        let echo = new_cell_id();
        let input = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(echo, [input], |args| args[0].clone())
            .unwrap();
        let expression = call(Value::from(echo), [(input, Value::from("hello"))]);
        assert_eq!(
            evaluate(&expression, |_| None, &foreign, 10).result,
            Ok(Value::from("hello"))
        );
    }

    #[test]
    fn functions_have_explicit_ordered_parameters() {
        let first = new_cell_id();
        let x = new_cell_id();
        let y = new_cell_id();
        let definition = function([x, y], Value::from(x));
        let expression = call(
            Value::from(first),
            [(x, Value::from("x")), (y, Value::from("y"))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| (cell == first).then(|| definition.clone()),
            &ForeignFunctions::new(),
            50,
        );
        assert_eq!(evaluation.result, Ok(Value::from("x")));
        assert_eq!(evaluation.dependencies, BTreeSet::from([first]));
    }

    #[test]
    fn the_call_marker_cannot_also_be_a_parameter() {
        let definition = function([vocabulary::FUNCTION], Value::from("body"));
        assert_eq!(
            evaluate(&definition, |_| None, &ForeignFunctions::new(), 10).result,
            Err(Error::ReservedParameter(vocabulary::FUNCTION))
        );
    }

    #[test]
    fn a_parameter_shadows_the_document_cell_with_the_same_identity() {
        let parameter = new_cell_id();
        let expression = call(
            function([parameter], Value::from(parameter)),
            [(parameter, Value::from("local"))],
        );
        let evaluation = evaluate(
            &expression,
            |cell| (cell == parameter).then(|| Value::from("document")),
            &ForeignFunctions::new(),
            20,
        );
        assert_eq!(evaluation.result, Ok(Value::from("local")));
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn anonymous_functions_capture_their_lexical_environment() {
        let x = new_cell_id();
        let y = new_cell_id();
        let inner = function([y], Value::from(x));
        let outer = function([x], call(inner, [(y, Value::from("ignored"))]));
        let expression = call(outer, [(x, Value::from("captured"))]);
        assert_eq!(
            evaluate(&expression, |_| None, &ForeignFunctions::new(), 50).result,
            Ok(Value::from("captured"))
        );
    }

    #[test]
    fn recursion_is_bounded_by_fuel_not_mistaken_for_an_alias_cycle() {
        let recurse = new_cell_id();
        let x = new_cell_id();
        let definition = function([x], call(Value::from(recurse), [(x, Value::from(x))]));
        let expression = call(Value::from(recurse), [(x, Value::from("again"))]);
        let evaluation = evaluate(
            &expression,
            |cell| (cell == recurse).then(|| definition.clone()),
            &ForeignFunctions::new(),
            30,
        );
        assert_eq!(evaluation.result, Err(Error::FuelExhausted));
        assert_eq!(evaluation.dependencies, BTreeSet::from([recurse]));
    }

    #[test]
    fn malformed_calls_fail_without_hiding_the_underlying_data() {
        let echo = new_cell_id();
        let input = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        foreign
            .register(echo, [input], |args| args[0].clone())
            .unwrap();
        let expression = call(Value::from(echo), []);
        assert_eq!(
            evaluate(&expression, |_| None, &foreign, 10).result,
            Err(Error::MissingArgument(input))
        );
    }

    #[test]
    fn cell_alias_cycles_are_reported() {
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
        assert_eq!(evaluation.result, Err(Error::CellCycle(vec![a, b, a])));
    }

    #[test]
    fn fuel_bounds_evaluation() {
        let cell = new_cell_id();
        let evaluation = evaluate(
            &Value::from(cell),
            |candidate| (candidate == cell).then(|| Value::from("done")),
            &ForeignFunctions::new(),
            1,
        );
        assert_eq!(evaluation.result, Err(Error::FuelExhausted));
        assert_eq!(evaluation.steps, 1);
    }

    #[test]
    fn foreign_registration_refuses_ambiguous_calls() {
        let function = new_cell_id();
        let parameter = new_cell_id();
        let mut foreign = ForeignFunctions::new();
        assert_eq!(
            foreign.register(function, [vocabulary::FUNCTION], |_| Value::from("x")),
            Err(RegistrationError::ReservedParameter(vocabulary::FUNCTION))
        );
        assert_eq!(
            foreign.register(function, [parameter, parameter], |_| Value::from("x")),
            Err(RegistrationError::DuplicateParameter(parameter))
        );
        foreign
            .register(function, [parameter], |_| Value::from("x"))
            .unwrap();
        assert_eq!(
            foreign.register(function, [parameter], |_| Value::from("x")),
            Err(RegistrationError::AlreadyRegistered(function))
        );
    }

    #[test]
    fn vocabulary_is_only_graps_function_protocol() {
        let library = library();
        assert_eq!(library.name(vocabulary::FUNCTION), Some("function"));
        assert_eq!(library.name(vocabulary::PARAMS), Some("params"));
        assert_eq!(library.name(vocabulary::BODY), Some("body"));
        assert_eq!(library.cells().count(), 3);
    }
}
