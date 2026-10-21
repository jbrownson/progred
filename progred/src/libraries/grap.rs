//! Grap vocabulary and source projections, including named value wrappers.
//! The live `evaluate` projection is separate from evaluator syntax: a host
//! that never loads it never gives that field evaluation behavior. Grap's
//! evaluation control is in [`control`], and its arithmetic over the number
//! representations in [`numbers`].

use crate::computations::memo;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::libraries::{Library, name};
use gid::{CellId, Step, Value};

pub const ID: CellId = CellId::from_u128(0xf7735b90f6826b25c350a8fd83af8c47);
use crate::display::hug;
use crate::display::structure::record_form;
use crate::display::{
    Completion, CompletionKind, Delim, Face, Name, Pending, Picker, ResolvedCell, alternatives,
    col, descend, dim, faced, indent, picker, ranked, row, selectable, selectable_bracket, shared,
};
use ::grap::source::vocabulary::BODY_ORIGIN;
use ::grap::vocabulary::{BODY, ENVIRONMENT, EVALUATE, FFI, FUNCTION, LAMBDA, PARAMS, VALUE};
use ::grap::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use puri::text::Script;

pub mod control;
pub mod notebook;
pub mod numbers;
mod suggestions;

pub(crate) fn expression<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    match input.value {
        Some(_) => reference(input),
        None => (input.default_projection)(input).or_else(|| {
            Some(picker(Picker::new(
                CompletionKind::Value,
                ranked(std::rc::Rc::new(suggestions::offers)),
            )))
        }),
    }
}

/// A reference reads as its cell's name. A parameter with no name of its
/// own reads, grey, as what its list calls it, found by walking out from
/// the reference, as a slot's offers find what's in scope.
fn reference<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    name::shallow_cell(input).or_else(|| {
        let cell = input.value?.as_cell()?;
        let computed = scope_around(input)?.remove(&cell)?.computed?;
        Some(name::shallow_reference(
            input,
            crate::display::named(&computed, Face::Dim),
        ))
    })
}

/// The bindings in scope where `input` occurs: walking out through where
/// it's stored, or through the computed value it lies in, as a closure in a
/// result does.
fn scope_around<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<std::collections::BTreeMap<CellId, suggestions::Binding>> {
    let path = input.env.occurrence().to_vec();
    let value_of = |cell| input.env.resolve(cell).map(|definition| definition.value);
    let (scope, _) = match crate::occurrence::location(&path) {
        Some(location) => suggestions::scope(
            &location.path,
            &|steps| input.env.read_from(location.start, steps),
            &value_of,
        ),
        None => suggestions::scope(
            &crate::occurrence::within_computed(&path)?,
            &|steps| input.env.read_computed(steps),
            &value_of,
        ),
    };
    Some(scope)
}

/// What a parameter list calls its parameters with no name of their own: in
/// order, the first of a, b, c… that none of its named parameters uses.
/// Computed wherever it's shown, never stored.
pub(crate) fn unnamed_parameters<'a>(
    params: &[CellId],
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
) -> std::collections::BTreeMap<CellId, Name<'static>> {
    computed_names(
        params,
        value_of,
        (0..).map(|index| Name::Formatted([(letters(index), Script::Normal)].into())),
    )
}

/// What a closure's environment calls the bindings it holds with no name of
/// their own: in its order, the first of e₀, e₁, e₂… that none of its named
/// bindings uses. The parameter list that named them isn't in the closure.
pub(crate) fn unnamed_bindings<'a>(
    bindings: &[CellId],
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
) -> std::collections::BTreeMap<CellId, Name<'static>> {
    computed_names(
        bindings,
        value_of,
        (0..).map(|index| {
            Name::Formatted(
                [
                    ("e".to_owned(), Script::Normal),
                    (index.to_string(), Script::Subscript),
                ]
                .into(),
            )
        }),
    )
}

/// `cells` with no name of their own, each called the next of `names` that
/// none of the named ones uses.
fn computed_names<'a>(
    cells: &[CellId],
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
    names: impl Iterator<Item = Name<'static>>,
) -> std::collections::BTreeMap<CellId, Name<'static>> {
    let named: Vec<_> = cells
        .iter()
        .filter_map(|cell| name::of_cell(value_of, *cell))
        .map(|name| name.spelling().into_owned())
        .collect();
    let mut free = names.filter(|name| !named.contains(&name.spelling().into_owned()));
    let mut computed = std::collections::BTreeMap::new();
    for cell in cells {
        if name::of_cell(value_of, *cell).is_none() && !computed.contains_key(cell) {
            computed.insert(*cell, free.next().expect("names never run out"));
        }
    }
    computed
}

/// The `index`th of a…z, aa…az, ba…, as a spreadsheet's columns go.
fn letters(index: usize) -> String {
    let mut digits = Vec::new();
    let mut rest = index + 1;
    while rest > 0 {
        rest -= 1;
        digits.push(b'a' + (rest % 26) as u8);
        rest /= 26;
    }
    digits
        .iter()
        .rev()
        .map(|digit| char::from(*digit))
        .collect()
}

pub mod vocabulary {
    use gid::CellId;

    /// Source whose contents belong to the Grap domain. This is an
    /// ordinary field and requests no evaluation.
    pub const GRAP: CellId = CellId::from_u128(0x315ca8459cfc64a210d518da1cad79b9);
}

/// A parameter is a cell, with no name of its own called what its list
/// calls it, among `computed`. An empty slot offers only a new one, named as
/// typed, or with nothing typed, named next.
fn parameter<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    computed: &std::collections::BTreeMap<CellId, Name<'static>>,
) -> Option<Layout<W>> {
    match input.value {
        Some(value) => declaration_cell(
            input,
            value
                .as_cell()
                .and_then(|cell| computed.get(&cell))
                .map(|name| name.spelling().into_owned()),
        ),
        None => Some(picker(Picker::new(
            CompletionKind::Value,
            ranked(std::rc::Rc::new(|request| {
                Some(vec![new_parameter(request.query.trim().to_owned())])
            })),
        ))),
    }
}

/// A new parameter named `spelling`, then selected; with no spelling, it
/// has no name, and its name is picked next.
fn new_parameter(spelling: String) -> Completion {
    let named = !spelling.is_empty();
    let then = if named {
        crate::selection::at(&[], crate::selection::payload::edge())
    } else {
        crate::selection::pending_at(&[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ])
    };
    let display = if named {
        spelling.clone()
    } else {
        "new parameter".to_owned()
    };
    let offer = Completion::new(display, move |session| {
        if let Some(picking) = session.pending() {
            let definition = if spelling.is_empty() {
                Value::record([])
            } else {
                name::record(&spelling, [])
            };
            picking.commit_new_cell(definition, Some(then.clone()));
        }
    });
    if named {
        offer.with_detail("new parameter")
    } else {
        offer
    }
}

/// A declaration's cell in parentheses, its name editable there; an empty
/// name shows `computed` grey in its editor until a name is typed.
pub(crate) fn declaration_cell<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    computed: Option<String>,
) -> Option<Layout<W>> {
    let cell = input.value?.as_cell()?;
    let definition = input.env.resolve(cell)?;
    Some(selectable_bracket(
        Delim::Paren,
        descend(
            [Step::Follow(definition.resolution)],
            Some(crate::display::first(
                input,
                crate::display::runtime_partial(move |input| {
                    declaration_name(input, computed.as_deref())
                }),
            )),
            None,
        ),
    ))
}

/// A declaration's definition as its name, there or not; its other fields
/// follow. A name that isn't text declines.
fn declaration_name<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    computed: Option<&str>,
) -> Option<Layout<W>> {
    record_form(input, [name::vocabulary::NAME], |[spelled]| {
        if let Some(spelled) = spelled {
            crate::libraries::text::read(spelled.as_value())?;
        }
        Some(descend(
            [Step::Key(name::vocabulary::NAME)],
            Some(crate::display::first(
                input,
                name::projection(computed.map(str::to_owned)),
            )),
            None,
        ))
    })
}

fn lambda_name<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Layout<W>> {
    Some(match input.value {
        Some(value) => crate::display::line_edit(name::editor(value)?),
        // The missing name's place; selected, it's an empty name to type
        // into.
        None if input.selection.is_some() => crate::display::line_edit(name::line(None)?),
        None => {
            let site = input.site();
            selectable(faced("λ", Face::Name), &site)
        }
    })
}

pub fn shallow_path<W: Host, V>(
    input: &ProjectionInput<'_, W, V>,
    steps: impl AsRef<[Step]> + 'static,
) -> Layout<W> {
    descend(
        steps,
        Some(crate::display::first(
            input,
            crate::display::runtime_partial(name::shallow_cell),
        )),
        None,
    )
}

/// A named direct expression reference is shallow. A compound expression's
/// projection explicitly chooses the roles of its own children.
pub(crate) fn expression_path<W: Host, V>(
    input: &ProjectionInput<'_, W, V>,
    steps: impl AsRef<[Step]> + 'static,
) -> Layout<W> {
    descend(
        steps,
        Some(crate::display::first(
            input,
            crate::display::runtime_partial(expression),
        )),
        None,
    )
}

/// A record naming a Grap program: the program its `grap` field names is
/// code, so an empty one offers Grap's suggestions. Its other fields, and
/// code further in, keep their ordinary projections.
fn program_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    input.value?.field(vocabulary::GRAP)?;
    crate::display::structure::Fields::default()
        .child(|key| (key == vocabulary::GRAP).then(|| crate::display::runtime_partial(program)))
        .record(input)
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
    Some(selectable_bracket(
        Delim::Paren,
        expression_path(input, [Step::Follow(source)]),
    ))
}

/// Declaration cells keep their parentheses, with a named definition
/// shown as an unquoted editor at the real name field.
pub(crate) fn declaration_path<W: Host, V>(
    input: &ProjectionInput<'_, W, V>,
    steps: impl AsRef<[Step]> + 'static,
) -> Layout<W> {
    descend(
        steps,
        Some(crate::display::first(
            input,
            crate::display::runtime_partial(|input| declaration_cell(input, None)),
        )),
        None,
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
    // Enter; one being added some other way shows in the plain record view.
    (input.pending != Some(Pending::Field)).then_some(())?;
    let fields = input.value?;
    // One whose function is still being picked is a call too.
    let function = match fields.field(FUNCTION) {
        Some(function) => Some(function),
        None => (input.pending == Some(Pending::Child(Step::Key(FUNCTION)))).then_some(None)?,
    };
    let parameters = function
        .and_then(|function| function_parameters(&function, &|cell| input.env.resolve(cell)))
        .unwrap_or_default();
    let function = descend(
        [Step::Key(FUNCTION)],
        Some(crate::display::first(
            input,
            function_projection.unwrap_or_else(|| crate::display::runtime_partial(expression)),
        )),
        None,
    );
    // Its arguments are its function's parameters, in their order, each a
    // slot until it's given, then any others.
    Some(
        crate::display::structure::Fields::default()
            .leading(parameters.clone())
            .slots(parameters)
            .child(|_| Some(crate::display::runtime_partial(expression)))
            .shown_empty()
            .after(input, |key| key == FUNCTION, function),
    )
}

/// A stored lambda exposes its parameter declarations deeply and
/// projects its body as an expression; its name and body may be missing,
/// drawn as holes.
fn lambda_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    record_form(
        input,
        [name::vocabulary::NAME, PARAMS, BODY],
        |[_, params, _]| {
            let cells = params?
                .list_values()?
                .map(|param| param.as_cell())
                .collect::<Option<Vec<_>>>()?;
            let computed = unnamed_parameters(&cells, &|cell| {
                input.env.resolve(cell).map(|definition| definition.value)
            });
            let params = descend(
                [Step::Key(PARAMS)],
                Some(crate::display::first(
                    input,
                    crate::display::structure::list(Some(crate::display::runtime_partial(
                        move |input| parameter(input, &computed),
                    ))),
                )),
                None,
            );
            let body_site = input.site().below(&[Step::Key(BODY)]);
            let lambda = descend(
                [Step::Key(name::vocabulary::NAME)],
                Some(crate::display::first(
                    input,
                    crate::display::partial(lambda_name),
                )),
                None,
            );
            let arrow = selectable(crate::display::arrow(), &body_site);
            let head = row(3.0, [lambda, params, arrow]);
            Some(hug(head, expression_path(input, [Step::Key(BODY)]), 6.0))
        },
    )
}

/// A closure reads as the record it is: its lambda, drawn as a lambda is,
/// then the environment it closes over, then where its body came from, as a
/// record shut until it's opened.
fn closure_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let fields = input.value?;
    fields.field(LAMBDA)?;
    fields.field(ENVIRONMENT)?;
    crate::display::structure::Fields::default()
        .leading([LAMBDA, ENVIRONMENT, BODY_ORIGIN])
        .child(|key| match key {
            ENVIRONMENT => Some(crate::display::runtime_partial(environment)),
            BODY_ORIGIN => Some(crate::display::runtime_partial(|input| {
                let open = (input.default_projection)(input)
                    .or_else(|| crate::display::structure::Fields::default().record(input))?;
                Some(crate::display::fold::foldable(
                    Delim::Brace,
                    None,
                    |_| true,
                    open,
                ))
            })),
            _ => None,
        })
        .record(input)
}

/// A closure's environment, its bindings with no name of their own read as
/// it calls them, as its lambda's references to them do.
fn environment<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let bindings = input.value?.record_keys()?;
    let computed = unnamed_bindings(&bindings, &|cell| {
        input.env.resolve(cell).map(|definition| definition.value)
    });
    let open = crate::display::structure::Fields::default()
        .label(move |key| {
            computed
                .get(&key)
                .map(|name| crate::display::named(name, Face::Dim))
        })
        .record(input)?;
    Some(match bindings.is_empty() {
        true => open,
        false => crate::display::fold::foldable(Delim::Brace, None, |_| false, open),
    })
}

/// A named expression, not a lexical binding. The name slot is always
/// present in the projection; only the value field matters to evaluation.
fn value_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    record_form(input, [name::vocabulary::NAME, VALUE], |[_, value]| {
        value?;
        let name = descend(
            [Step::Key(name::vocabulary::NAME)],
            Some(crate::display::first(
                input,
                crate::display::partial(|input| {
                    name::editor(input.value?).map(crate::display::line_edit)
                }),
            )),
            None,
        );
        let site = input.site().below(&[Step::Key(VALUE)]);
        Some(hug(
            row(3.0, [name, selectable(dim("="), &site)]),
            expression_path(input, [Step::Key(VALUE)]),
            6.0,
        ))
    })
}

/// Foreignness is an evaluator implementation detail. In source, an
/// FFI callable projects exactly like the cell it names.
fn ffi_display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    record_form(input, [FFI], |[ffi]| {
        ffi?.as_cell()?;
        Some(shallow_path(input, [Step::Key(FFI)]))
    })
}

fn evaluate_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    record_form(input, [EVALUATE], |[expression]| {
        let result =
            input
                .env
                .evaluate_runtime(&expression?, ::grap::DEFAULT_FUEL, &[Step::Key(EVALUATE)]);
        let expression = shared(expression_path(input, [Step::Key(EVALUATE)]));
        let shaft_site = input.site();
        let shaft = shared(selectable(crate::display::arrow(), &shaft_site));
        let result = shared(crate::libraries::presentation::result_layout(&result));
        Some(alternatives([
            row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
            col(
                0,
                2.0,
                [row(6.0, [expression, shaft]), indent(20.0, result)],
            ),
        ]))
    })
}

fn evaluate_foreign(
    context: &mut Context,
    call: &Expression,
    calling_environment: &Environment,
) -> Result<::grap::RuntimeValue, Halt> {
    let Some(expression) = context.field(call, ::grap::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(call, ::grap::vocabulary::EXPRESSION));
    };
    let Some(environment) = context.field(call, ::grap::vocabulary::ENVIRONMENT) else {
        return Ok(context.missing_runtime_argument(call, ::grap::vocabulary::ENVIRONMENT));
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
    let (control_names, control_functions, control) = control::contribution();
    let (number_names, number_functions, numbers) = numbers::contribution();
    name::library_named(
        ID,
        "grap",
        [
            (::grap::vocabulary::FUNCTION, "function"),
            (::grap::vocabulary::PARAMS, "params"),
            (::grap::vocabulary::BODY, "body"),
            (::grap::vocabulary::VALUE, "value"),
            (::grap::vocabulary::LAMBDA, "lambda"),
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
            (notebook::vocabulary::NOTEBOOK, "notebook"),
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
        .chain(control_names)
        .map(|(cell, name)| (cell, crate::libraries::text::value(name)))
        .chain(number_names),
        functions().merge(control_functions).merge(number_functions),
    )
    .with_projection(
        // Calls, lambdas, and value wrappers follow evaluator precedence,
        // but ordinary partial failures still try the next projection.
        // Control and arithmetic read as themselves before calls in
        // general, and a number takes its operators as typing.
        crate::display::compose_partials([
            control,
            numbers,
            crate::display::runtime_partial(evaluate_display),
            crate::display::fold::folding(crate::display::runtime_partial(call_display)),
            crate::display::runtime_partial(lambda_display),
            crate::display::runtime_partial(closure_display),
            crate::display::runtime_partial(value_display),
            crate::display::runtime_partial(ffi_display),
            crate::display::runtime_partial(program_display),
            crate::display::runtime_partial(notebook::display),
        ]),
    )
    .with_completions(completions)
}

fn completions(request: &crate::display::CompletionRequest<'_>) -> Option<Vec<Completion>> {
    use crate::display::CompletionScope;
    (request.kind == CompletionKind::Value).then_some(())?;
    match request.scope {
        // An empty document can start as a program in a pane, or as a
        // notebook.
        CompletionScope::Start => Some(vec![new_program(), notebook::starter()]),
        CompletionScope::Everything => Some(
            std::iter::once(
                // Its first parameter is picked next.
                crate::libraries::completion::insert(
                    "new lambda",
                    Value::record([(PARAMS, Value::list([]))]),
                    Some(crate::selection::pending_at(&[
                        Step::Key(PARAMS),
                        Step::Element(
                            gid::position::between(None, None).expect("an empty list has room"),
                        ),
                    ])),
                )
                .with_aliases(["lambda", "λ"])
                .with_detail(ID),
            )
            .chain(std::iter::once(
                // Its function is picked next: a binding, a lambda, or
                // anything else that evaluates to one.
                crate::libraries::completion::insert(
                    "new call",
                    Value::record([]),
                    Some(crate::selection::pending_at(&[Step::Key(FUNCTION)])),
                )
                .with_detail(ID),
            ))
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
                let name = name::of_cell(
                    &|cell| (request.resolve)(cell).map(|definition| definition.value),
                    cell,
                )?
                .spelling();
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
        offers.sort_by_cached_key(|(external, name, _)| {
            (*external, name.to_lowercase(), name.clone())
        });
        offers.into_iter().map(|(_, _, offer)| offer).collect()
    }
}

#[cfg(test)]
mod tests;
