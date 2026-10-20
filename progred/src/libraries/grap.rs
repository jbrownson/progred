//! Grap vocabulary and source projections, including named value wrappers.
//! The live `evaluate` projection is separate from evaluator syntax: a host
//! that never loads it never gives that field evaluation behavior.

use crate::computations::memo;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::libraries::{Library, name};
use gid::{CellId, Step, Value};

pub const ID: CellId = CellId::from_u128(0xf7735b90f6826b25c350a8fd83af8c47);
use crate::display::structure::{field_label, field_order};
use crate::display::{
    Completion, CompletionKind, Delim, Face, Pending, Picker, RecordField, ResolvedCell,
    activatable, alternatives, col, descend_local, dim, faced, indent, picker, ranked,
    record_fragment_with, row, selectable_bracket, shared,
};
use crate::display::{group, group_hug};
use ::grap::vocabulary::{BODY, EVALUATE, FFI, FUNCTION, PARAMS, VALUE};
use ::grap::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};

pub mod numbers;
mod suggestions;

pub(crate) fn expression<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    match input.value {
        Some(_) => shallow_cell(input),
        None => (input.default_projection)(input).or_else(|| {
            Some(picker(Picker::new(
                CompletionKind::Value,
                ranked(std::rc::Rc::new(suggestions::offers)),
            )))
        }),
    }
}

pub mod vocabulary {
    use gid::CellId;

    /// Source whose contents belong to the Grap domain. This is an
    /// ordinary field and requests no evaluation.
    pub const GRAP: CellId = CellId::from_u128(0x315ca8459cfc64a210d518da1cad79b9);
}

/// A named cell as a reference, otherwise leave its definition visible.
/// Contextual projections use this for expression and callable references.
fn shallow_cell<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    shallow_cell_with(input, |label| label)
}

pub(crate) fn shallow_cell_with<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    decorate: impl FnOnce(Layout<W>) -> Layout<W>,
) -> Option<Layout<W>> {
    let cell = input.value?.as_cell()?;
    let name = input.env.name(cell)?;
    let site = input.site();
    Some(crate::display::stop(activatable(
        decorate(faced(name, Face::Name)),
        &site,
    )))
}

/// A parameter is a cell. An empty slot offers only a new one, named as
/// typed.
fn parameter<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    match input.value {
        Some(_) => declaration_cell(input),
        None => Some(picker(Picker::new(
            CompletionKind::Value,
            ranked(std::rc::Rc::new(|request| {
                let spelling = request.query.trim().to_owned();
                Some(
                    (!spelling.is_empty())
                        .then(|| {
                            Completion::new(
                                crate::display::CompletionText::Literal(spelling.clone()),
                                move |session| {
                                    if let Some(picking) = session.pending() {
                                        picking.commit_new_cell(
                                            name::record(&spelling, []),
                                            Some(crate::selection::at(
                                                &[],
                                                crate::selection::payload::edge(),
                                            )),
                                        );
                                    }
                                },
                            )
                            .with_detail("new parameter")
                        })
                        .into_iter()
                        .collect(),
                )
            })),
        ))),
    }
}

fn declaration_cell<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let cell = input.value?.as_cell()?;
    let definition = input.env.resolve(cell)?;
    Some(group(selectable_bracket(
        Delim::Paren,
        descend_local(
            [Step::Follow(definition.resolution)],
            crate::display::partial(declaration_name),
            &input.default_projection,
        ),
    )))
}

fn declaration_name<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Layout<W>> {
    input.pending.is_none().then_some(())?;
    name::read(input.value?)?;
    Some(descend_local(
        [Step::Key(name::vocabulary::NAME)],
        name::projection(),
        &input.default_projection,
    ))
}

fn lambda_name<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Layout<W>> {
    Some(match input.value {
        Some(value) => crate::display::line_edit(name::editor(value)?),
        // Naming an anonymous lambda is clickable, not an arrow-navigation
        // stop; selected, it's an empty name to type into.
        None if input.selection.is_some() => name::hole(),
        None => {
            let site = input.site();
            activatable(faced("λ", Face::Name), &site)
        }
    })
}

pub fn shallow_path<W: Host>(
    steps: impl AsRef<[Step]> + 'static,
    default: &Partial<W>,
) -> Layout<W> {
    descend_local(
        steps,
        crate::display::runtime_partial(shallow_cell),
        default,
    )
}

/// A named direct expression reference is shallow. A compound expression's
/// projection explicitly chooses the roles of its own children.
pub(crate) fn expression_path<W: Host>(
    steps: impl AsRef<[Step]> + 'static,
    default: &Partial<W>,
) -> Layout<W> {
    descend_local(steps, crate::display::runtime_partial(expression), default)
}

/// A record naming a Grap program: the program its `grap` field names is
/// code, so an empty one offers Grap's suggestions. Its other fields, and
/// code further in, keep their ordinary projections.
fn program_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    input.value?.field(vocabulary::GRAP)?;
    crate::display::structure::record_layout(input, |key| {
        (key == vocabulary::GRAP).then(|| crate::display::runtime_partial(program))
    })
}

/// The program's cell keeps its parentheses; what fills it is an expression.
fn program<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let cell = input.value?.as_cell()?;
    let source = input
        .env
        .resolve(cell)
        .map_or(gid::Resolution::Document, |definition| {
            definition.resolution
        });
    Some(group(selectable_bracket(
        Delim::Paren,
        expression_path([Step::Follow(source)], &input.default_projection),
    )))
}

/// Declaration cells keep their parentheses, with a named definition
/// shown as an unquoted editor at the real name field.
pub(crate) fn declaration_path<W: Host>(
    steps: impl AsRef<[Step]> + 'static,
    default: &Partial<W>,
) -> Layout<W> {
    descend_local(
        steps,
        crate::display::runtime_partial(declaration_cell),
        default,
    )
}

/// Parameter order is source metadata, not an evaluation. Follow
/// transparent cell references to a stored lambda or closure; a
/// computed callable has no order available to the projection.
fn function_parameters<'a>(
    function: &::grap::RuntimeValue,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
) -> Option<Vec<CellId>> {
    parameters_past(function, resolve, &[])
}

/// `function`'s parameters past the cells already `followed`: reaching one
/// of them again is a cycle, with none.
fn parameters_past<'a>(
    function: &::grap::RuntimeValue,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
    followed: &[CellId],
) -> Option<Vec<CellId>> {
    match function.as_cell() {
        None => function.callable_parameters(),
        Some(cell) => {
            (!followed.contains(&cell)).then_some(())?;
            let definition = resolve(cell)?;
            if definition.native {
                definition
                    .value
                    .as_record()?
                    .get(&PARAMS)?
                    .as_list()?
                    .values()
                    .map(Value::as_cell)
                    .collect()
            } else {
                parameters_past(
                    &definition.value.into(),
                    resolve,
                    &[followed, &[cell]].concat(),
                )
            }
        }
    }
}

/// Construct a call and focus its first declared parameter, when known.
fn call_completion<'a>(
    function: Value,
    display: impl Into<crate::display::CompletionText>,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
) -> Completion {
    let first = function_parameters(&(&function).into(), resolve)
        .and_then(|parameters| parameters.first().copied());
    let continuation = match first {
        Some(parameter) => crate::selection::pending_at(&[Step::Key(parameter)]),
        None => crate::selection::at(&[], crate::selection::payload::edge()),
    };
    crate::libraries::completion::insert(display, ::grap::call(function, []), Some(continuation))
}

/// Calls read as calls. Their function position is a shallow
/// reference when it is a named cell; arguments retain Grap's contextual
/// projection.
fn call_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    call_with_function(input, None)
}

pub(crate) fn call_with_function<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    function_projection: Option<Partial<W>>,
) -> Option<Layout<W>> {
    // A call's arguments are its parameters, so it takes no new field from
    // Enter; one being added some other way shows in the plain record view,
    // as for the other compact forms.
    (input.pending != Some(Pending::Field)).then_some(())?;
    let fields = input.value?;
    let function = fields.field(FUNCTION)?;
    let parameters = function_parameters(&function, &|cell| input.env.resolve(cell));
    let mut parameter_positions = std::collections::BTreeMap::new();
    for (position, parameter) in parameters.iter().flatten().enumerate() {
        parameter_positions.entry(*parameter).or_insert(position);
    }
    let argument = |field| {
        let site = input.site().below(&[Step::Key(field)]);
        RecordField {
            label: activatable(field_label(input.env, field), &site),
            value: expression_path([Step::Key(field)], &input.default_projection),
        }
    };
    let trailing = match &input.pending {
        Some(Pending::Child(Step::Key(field)))
            if !fields.contains_field(*field) && !parameter_positions.contains_key(field) =>
        {
            vec![argument(*field)]
        }
        _ => Vec::new(),
    };
    let function = descend_local(
        [Step::Key(FUNCTION)],
        function_projection.unwrap_or_else(|| crate::display::runtime_partial(shallow_cell)),
        &input.default_projection,
    );
    let arguments = record_fragment_with(
        fields
            .record_keys()?
            .into_iter()
            .filter(|field| *field != FUNCTION)
            .map(|field| (field, ()))
            .chain(
                parameter_positions
                    .keys()
                    .filter(|field| !fields.contains_field(**field))
                    .map(|field| (*field, ())),
            ),
        |left, right| match (
            parameter_positions.get(left),
            parameter_positions.get(right),
        ) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => field_order(input.env, left, right),
        },
        |field, _| argument(field),
        trailing,
    );
    Some(group_hug(function, arguments, 0.0))
}

/// A stored lambda exposes its parameter declarations deeply and
/// projects its body as an expression.
fn lambda_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let fields = input.value?;
    matches!(
        &input.pending,
        None | Some(Pending::Child(Step::Key(name::vocabulary::NAME | BODY)))
    )
    .then_some(())?;
    let params = fields.field(PARAMS)?;
    params
        .list_values()?
        .all(|param| param.as_cell().is_some())
        .then_some(())?;
    let params = descend_local(
        [Step::Key(PARAMS)],
        crate::display::structure::list(Some(crate::display::runtime_partial(parameter))),
        &input.default_projection,
    );
    let body_site = input.site().below(&[Step::Key(BODY)]);
    let lambda = descend_local(
        [Step::Key(name::vocabulary::NAME)],
        crate::display::partial(lambda_name),
        &input.default_projection,
    );
    let arrow = activatable(crate::display::arrow(), &body_site);
    let head = row(3.0, [lambda, params, arrow]);
    Some(group_hug(
        head,
        expression_path([Step::Key(BODY)], &input.default_projection),
        6.0,
    ))
}

/// A named expression, not a lexical binding. The name slot is always
/// present in the projection; only the value field matters to evaluation.
fn value_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let fields = input.value?;
    fields.field(VALUE)?;
    matches!(
        &input.pending,
        None | Some(Pending::Child(Step::Key(name::vocabulary::NAME | VALUE)))
    )
    .then_some(())?;
    let name = descend_local(
        [Step::Key(name::vocabulary::NAME)],
        crate::display::partial(|input| name::editor(input.value?).map(crate::display::line_edit)),
        &input.default_projection,
    );
    let site = input.site().below(&[Step::Key(VALUE)]);
    Some(group_hug(
        row(3.0, [name, activatable(dim("="), &site)]),
        expression_path([Step::Key(VALUE)], &input.default_projection),
        6.0,
    ))
}

/// Foreignness is an evaluator implementation detail. In source, an
/// FFI callable projects exactly like the cell it names.
fn ffi_display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let ffi = input.value?.field(FFI)?;
    ffi.as_cell()?;
    Some(group(shallow_path(
        [Step::Key(FFI)],
        &input.default_projection,
    )))
}

fn evaluate_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let expression = input.value?.field(EVALUATE)?;
    let result =
        input
            .env
            .evaluate_runtime(&expression, ::grap::DEFAULT_FUEL, &[Step::Key(EVALUATE)]);
    let expression = shared(expression_path(
        [Step::Key(EVALUATE)],
        &input.default_projection,
    ));
    let shaft_site = input.site();
    let shaft = shared(activatable(crate::display::arrow(), &shaft_site));
    let result = shared(crate::libraries::presentation::result_layout(&result));
    Some(group(alternatives([
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(
            0,
            2.0,
            [row(6.0, [expression, shaft]), indent(20.0, result)],
        ),
    ])))
}

fn evaluate_foreign(
    context: &mut Context,
    call: &Expression,
    calling_environment: &Environment,
) -> Result<::grap::RuntimeValue, Halt> {
    let Some(expression) = context.field(call, ::grap::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(::grap::vocabulary::EXPRESSION));
    };
    let Some(environment) = context.field(call, ::grap::vocabulary::ENVIRONMENT) else {
        return Ok(context.missing_runtime_argument(::grap::vocabulary::ENVIRONMENT));
    };
    let environment = context.eval_to_value(environment, calling_environment)?;
    match context.environment(&environment) {
        Some(environment) => context.eval(expression, &environment),
        None => Ok(::grap::absent::with_detail(
            ::grap::absent::INVALID_ENVIRONMENT,
            ::grap::absent::VALUE,
            environment,
        )
        .into()),
    }
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        ::grap::vocabulary::EVALUATE,
        ForeignFunction::new(evaluate_foreign)
            .parameters([
                ::grap::vocabulary::EXPRESSION,
                ::grap::vocabulary::ENVIRONMENT,
            ])
            .tracked(),
    )
}

pub fn library<W: Host>() -> Library<W> {
    let (number_names, number_functions, numbers) = numbers::contribution();
    name::named_library(
        ID,
        "grap",
        [
            (::grap::vocabulary::FUNCTION, "function"),
            (::grap::vocabulary::PARAMS, "params"),
            (::grap::vocabulary::BODY, "body"),
            (::grap::vocabulary::VALUE, "value"),
            (::grap::vocabulary::CLOSURE, "closure"),
            (::grap::vocabulary::ENVIRONMENT, "environment"),
            (::grap::vocabulary::FFI, "ffi"),
            (::grap::vocabulary::EVALUATE, "evaluate"),
            (::grap::vocabulary::EXPRESSION, "expression"),
            (::grap::source::vocabulary::BODY_ORIGIN, "body origin"),
            (::grap::source::vocabulary::INPUT, "input source"),
            (::grap::source::vocabulary::STORED, "document source"),
            (::grap::source::vocabulary::CELL, "source cell"),
            (::grap::source::vocabulary::PATH, "source path"),
            (::grap::source::vocabulary::SOURCE, "definition source"),
            (vocabulary::GRAP, "grap"),
            (::grap::absent::FUEL_EXHAUSTED, "fuel exhausted"),
            (memo::CYCLE, "computation cycle"),
            (memo::INPUTS_CHANGED, "inputs changed during computation"),
            (
                memo::DIFFERENT_RUNTIME,
                "computation belongs to another runtime",
            ),
            (memo::CANCELLED, "computation cancelled"),
            (
                ::grap::absent::EFFECTFUL_DECLINE,
                "declined after an effect",
            ),
            (::grap::absent::MISSING_CELL, "missing cell"),
            (::grap::absent::CELL_CYCLE, "cell cycle"),
            (::grap::absent::MALFORMED_LAMBDA, "malformed lambda"),
            (::grap::absent::INVALID_PARAMETER, "invalid parameter"),
            (::grap::absent::NOT_CALLABLE, "not callable"),
            (::grap::absent::MISSING_ARGUMENT, "missing argument"),
            (::grap::absent::INVALID_ENVIRONMENT, "invalid environment"),
        ]
        .into_iter()
        .chain(
            number_names
                .iter()
                .map(|(cell, name)| (*cell, name.as_str())),
        ),
        functions().merge(number_functions),
    )
    .with_projection(
        // Calls, lambdas, and value wrappers follow evaluator precedence,
        // but ordinary partial failures still try the next projection.
        // Arithmetic reads as itself before calls in general, and a number
        // takes its operators as typing.
        crate::display::compose_partials([
            numbers,
            crate::display::runtime_partial(evaluate_display),
            crate::display::fold::folding(crate::display::runtime_partial(call_display)),
            crate::display::runtime_partial(lambda_display),
            crate::display::runtime_partial(value_display),
            crate::display::runtime_partial(ffi_display),
            crate::display::runtime_partial(program_display),
        ]),
    )
    .with_completions(completions)
}

fn completions(request: &crate::display::CompletionRequest<'_>) -> Option<Vec<Completion>> {
    use crate::display::CompletionScope;
    (request.kind == CompletionKind::Value).then_some(())?;
    match request.scope {
        // An empty document can start as a program in a pane.
        CompletionScope::Start => Some(vec![new_program()]),
        CompletionScope::Everything => Some(
            std::iter::once(
                crate::libraries::completion::insert(
                    "new lambda",
                    Value::record([(PARAMS, Value::list([]))]),
                    Some(crate::selection::at(
                        &[Step::Key(BODY)],
                        crate::selection::payload::edge(),
                    )),
                )
                .with_aliases(["lambda", "λ"])
                .with_detail(ID),
            )
            .chain(
                (!request.query.trim().is_empty())
                    .then(|| call_completions(request))
                    .into_iter()
                    .flatten(),
            )
            .collect(),
        ),
        CompletionScope::Literal => None,
    }
}

fn new_program() -> Completion {
    crate::libraries::completion::starter(vocabulary::GRAP, |cell| {
        Value::record([(
            crate::libraries::presentation::vocabulary::RENDER,
            cell.into(),
        )])
    })
    .with_detail(ID)
}

/// Where a call offer's function comes from: the document, or a library by name.
fn call_detail(
    request: &crate::display::CompletionRequest<'_>,
    resolution: gid::Resolution,
) -> String {
    match resolution {
        gid::Resolution::Document => "call".to_owned(),
        gid::Resolution::Library(library) => format!(
            "call · {}",
            crate::identity::name_or_short_id(
                (request.resolve)(library).and_then(|definition| name::read(definition.value)),
                library,
            )
        ),
    }
}

fn call_completions(request: &crate::display::CompletionRequest<'_>) -> Vec<Completion> {
    if matches!(request.path.last(), Some(Step::Key(field)) if *field == FUNCTION || *field == FFI)
    {
        Vec::new()
    } else {
        let mut offers: Vec<_> = (request.cells)()
            .into_iter()
            .filter_map(|cell| {
                let definition = (request.resolve)(cell)?;
                let name = name::read(definition.value)?;
                (definition.native
                    || function_parameters(&Value::from(cell).into(), request.resolve).is_some())
                .then(|| {
                    let offer = call_completion(cell.into(), cell, request.resolve)
                        .with_detail(call_detail(request, definition.resolution));
                    (
                        matches!(definition.resolution, gid::Resolution::Library(_)),
                        name,
                        offer,
                    )
                })
            })
            .collect();
        offers.sort_by_cached_key(|(external, name, _)| (*external, name.to_lowercase(), *name));
        offers.into_iter().map(|(_, _, offer)| offer).collect()
    }
}

#[cfg(test)]
mod tests;
