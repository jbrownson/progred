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
use ::grap::vocabulary::{
    BODY, ENVIRONMENT, FUNCTION, IMPLEMENTATION, LAMBDA, PARAMS, VALUE, kind,
};
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
    let path = input
        .env
        .occurrence()
        .map_or_else(Vec::new, crate::occurrence::Trail::to_vec);
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

    /// The kinds of the Grap library's own records, as their `isa` names
    /// them.
    pub mod kind {
        use gid::CellId;

        /// An expression shown with its result: `{isa: evaluation,
        /// expression}`.
        pub const EVALUATION: CellId = CellId::from_u128(0x549dd85073474d6296d773c8df9c0526);
    }
}

/// `expression` shown with its result.
#[cfg(test)]
pub fn evaluation(expression: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::EVALUATION,
        [(::grap::vocabulary::EXPRESSION, expression)],
    )
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
        crate::selection::pending_at(&[Step::Follow, Step::Key(name::vocabulary::NAME)])
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

/// A declaration's cell in parentheses, one thing with its definition, its
/// name a stop inside editable there; an empty name shows `computed` grey in
/// its editor until a name is typed.
pub(crate) fn declaration_cell<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    computed: Option<String>,
) -> Option<Layout<W>> {
    input.value?.as_cell()?;
    // A cell with no definition yet gets one in the document, where typing
    // its name writes it.
    Some(selectable_bracket(
        Delim::Paren,
        crate::display::part(
            [Step::Follow],
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
/// follow. The definition is part of its cell's stop, the name a stop
/// inside. A name that isn't text declines.
fn declaration_name<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    computed: Option<&str>,
) -> Option<Layout<W>> {
    match input.value {
        None => Some(definition_hole()),
        Some(_) => record_form(input, [name::vocabulary::NAME], |[spelled]| {
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
        }),
    }
}

/// Where a declaration has no definition: a hole, unlike a definition with
/// no name, which shows its computed name grey. Its one offer is the name
/// typed, which defines the declaration; with nothing typed, the name is
/// picked next.
fn definition_hole<W: Host>() -> Layout<W> {
    picker(Picker::new(
        CompletionKind::Value,
        ranked(std::rc::Rc::new(|request| {
            let spelling = request.query.trim().to_owned();
            let named = !spelling.is_empty();
            let display = if named {
                spelling.clone()
            } else {
                "no name".to_owned()
            };
            Some(vec![
                Completion::new(display, move |session| {
                    if let Some(picking) = session.pending() {
                        let (definition, then) = if named {
                            (
                                name::record(&spelling, []),
                                crate::selection::at(
                                    &[Step::Key(name::vocabulary::NAME)],
                                    crate::selection::payload::edge(),
                                ),
                            )
                        } else {
                            (
                                Value::record([]),
                                crate::selection::pending_at(&[Step::Key(name::vocabulary::NAME)]),
                            )
                        };
                        picking.commit(definition, Some(then));
                    }
                })
                .with_detail("name"),
            ])
        })),
    ))
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
    input.value?.as_cell()?;
    Some(selectable_bracket(
        Delim::Paren,
        expression_path(input, [Step::Follow]),
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
/// A call, drawn as its function provides, else as a call.
fn call_projection<W: Host>() -> Partial<W> {
    let call = crate::display::fold::folding(crate::display::runtime_partial(call_display));
    crate::display::runtime_partial(move |input| {
        input
            .value?
            .field(FUNCTION)
            .and_then(|function| function.as_cell())
            .and_then(|function| input.projection_for(function))
            .and_then(|projection| projection(input))
            .or_else(|| call(input))
    })
}

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
    // It's a call while its function is still to be picked.
    (fields.kind() == Some(kind::CALL)).then_some(())?;
    let parameters = fields
        .field(FUNCTION)
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
            .after(
                input,
                |key| key == FUNCTION || key == ::grap::isa::ISA,
                function,
            ),
    )
}

/// A stored lambda exposes its parameter declarations deeply and
/// projects its body as an expression; its name and body may be missing,
/// drawn as holes.
fn lambda_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    crate::display::structure::kind_form(
        input,
        kind::LAMBDA,
        [name::vocabulary::NAME, PARAMS, BODY],
        |[_, params, _]| {
            let params = parameters(input, &params?)?;
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

/// A function's `params`, each a parameter cell, as a lambda's head lists
/// them.
fn parameters<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    params: &::grap::RuntimeValue,
) -> Option<Layout<W>> {
    let cells = params
        .list_values()?
        .map(|param| param.as_cell())
        .collect::<Option<Vec<_>>>()?;
    let computed = unnamed_parameters(&cells, &|cell| {
        input.env.resolve(cell).map(|definition| definition.value)
    });
    Some(descend(
        [Step::Key(PARAMS)],
        Some(crate::display::first(
            input,
            crate::display::structure::list(Some(crate::display::runtime_partial(move |input| {
                parameter(input, &computed)
            }))),
        )),
        None,
    ))
}

/// A foreign function reads as a lambda's head does, with no body to show:
/// its implementation's cell, by its name, which its own `name` gives where
/// it's that cell's definition, then its parameters.
fn foreign_function_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    crate::display::structure::kind_form(
        input,
        kind::FOREIGN_FUNCTION,
        [IMPLEMENTATION, name::vocabulary::NAME, PARAMS],
        |[implementation, _, params]| {
            implementation?.as_cell()?;
            let implementation = shallow_path(input, [Step::Key(IMPLEMENTATION)]);
            let params = match params {
                Some(params) => Some(parameters(input, &params)?),
                None => None,
            };
            Some(row(3.0, std::iter::once(implementation).chain(params)))
        },
    )
}

/// A closure reads as the record it is: its lambda, drawn as a lambda is,
/// then the environment it closes over, then where its body came from, as a
/// record shut until it's opened.
fn closure_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    (input.value?.kind() == Some(kind::CLOSURE)).then_some(())?;
    crate::display::structure::Fields::default()
        .leading([::grap::isa::ISA, LAMBDA, ENVIRONMENT, BODY_ORIGIN])
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
    crate::display::structure::kind_form(
        input,
        kind::VALUE_WRAPPER,
        [name::vocabulary::NAME, VALUE],
        |_| {
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
        },
    )
}

/// An expression and its result, beside it when they fit, else below it.
fn evaluation_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    use ::grap::vocabulary::EXPRESSION;
    crate::display::structure::kind_form(
        input,
        vocabulary::kind::EVALUATION,
        [EXPRESSION],
        |[expression]| {
            let result = input.env.evaluate_runtime(
                &expression?,
                ::grap::DEFAULT_FUEL,
                &[Step::Key(EXPRESSION)],
            );
            let expression = shared(expression_path(input, [Step::Key(EXPRESSION)]));
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
        },
    )
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
    let (number_names, number_functions, operators, numbers) = numbers::contribution();
    name::library_named(
        ID,
        "grap",
        [
            (::grap::vocabulary::FUNCTION, "function"),
            (::grap::vocabulary::PARAMS, "params"),
            (::grap::vocabulary::BODY, "body"),
            (::grap::vocabulary::VALUE, "value"),
            (::grap::vocabulary::LAMBDA, "lambda"),
            (kind::CALL, "call"),
            (kind::LAMBDA, "lambda"),
            (kind::CLOSURE, "closure"),
            (kind::FOREIGN_FUNCTION, "foreign function"),
            (IMPLEMENTATION, "implementation"),
            (kind::VALUE_WRAPPER, "value wrapper"),
            (::grap::vocabulary::ENVIRONMENT, "environment"),
            (::grap::vocabulary::EVALUATE, "evaluate"),
            (vocabulary::kind::EVALUATION, "evaluation"),
            (::grap::vocabulary::EXPRESSION, "expression"),
            (::grap::source::vocabulary::BODY_ORIGIN, "body origin"),
            (::grap::source::vocabulary::INPUT, "input source"),
            (::grap::source::vocabulary::STORED, "document source"),
            (::grap::source::vocabulary::CELL, "source cell"),
            (::grap::source::vocabulary::PATH, "source path"),
            (vocabulary::GRAP, "grap"),
            (notebook::vocabulary::kind::NOTEBOOK, "notebook"),
            (notebook::vocabulary::ENTRIES, "entries"),
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
            (::grap::absent::MISSING_FIELD, "missing field"),
            (::grap::absent::INVALID_ENVIRONMENT, "invalid environment"),
        ]
        .into_iter()
        .chain(control_names)
        .map(|(cell, name)| (cell, crate::libraries::text::value(name)))
        .chain(number_names),
        functions().merge(control_functions).merge(number_functions),
    )
    // Grap's kinds draw as they provide, a call as its function does, else
    // as a call: control's forms and the operators provide theirs.
    .with_projections_for(
        [
            (kind::CALL, call_projection()),
            (
                kind::LAMBDA,
                crate::display::runtime_partial(lambda_display),
            ),
            (
                kind::CLOSURE,
                crate::display::runtime_partial(closure_display),
            ),
            (
                kind::FOREIGN_FUNCTION,
                crate::display::runtime_partial(foreign_function_display),
            ),
            (
                kind::VALUE_WRAPPER,
                crate::display::runtime_partial(value_display),
            ),
            (
                vocabulary::kind::EVALUATION,
                crate::display::runtime_partial(evaluation_display),
            ),
            (
                notebook::vocabulary::kind::NOTEBOOK,
                crate::display::runtime_partial(notebook::display),
            ),
        ]
        .into_iter()
        .chain(control)
        .chain(operators),
    )
    .with_projection(
        // A number takes its operators as typing.
        crate::display::compose_partials([
            numbers,
            crate::display::runtime_partial(program_display),
        ]),
    )
    .with_completions(completions)
}

/// An empty document can start as a program in a pane, or as a notebook.
/// Grap's other offers are its slots' own.
fn completions(request: &crate::display::CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Value
        && request.scope == crate::display::CompletionScope::Start)
        .then(|| vec![new_program(), notebook::starter()])
}

/// What Grap builds in a slot that names no function: a lambda, its first
/// parameter picked next, and a call, its function picked next: a binding,
/// a function, or anything else that evaluates to one.
fn constructs() -> [Completion; 2] {
    [
        crate::libraries::completion::insert(
            "new lambda",
            ::grap::isa::record(kind::LAMBDA, [(PARAMS, Value::list([]))]),
            Some(crate::selection::pending_at(&[
                Step::Key(PARAMS),
                Step::Element(gid::position::between(None, None).expect("an empty list has room")),
            ])),
        )
        .with_aliases(["lambda", "λ"])
        .with_detail(ID),
        crate::libraries::completion::insert(
            "new call",
            ::grap::isa::record(kind::CALL, []),
            Some(crate::selection::pending_at(&[Step::Key(FUNCTION)])),
        )
        .with_detail(ID),
    ]
}

fn new_program() -> Completion {
    crate::libraries::completion::starter(vocabulary::GRAP, |cell| {
        crate::libraries::presentation::render(cell.into())
    })
    .with_detail(ID)
}

/// What an offer is, `call` or `function`, and where its function comes
/// from: the document, or a library by name.
fn function_detail(
    request: &crate::display::CompletionRequest<'_>,
    what: &str,
    library: Option<CellId>,
) -> String {
    match library {
        None => what.to_owned(),
        Some(library) => format!(
            "{what} · {}",
            crate::identity::name_or_short_id(
                (request.resolve)(library).and_then(|definition| name::read(definition.value)),
                library,
            )
        ),
    }
}

/// The functions in reach by name, the document's before the libraries'. A
/// call's function slot offers each as itself; any other slot, a call of
/// it. Quote, offered as its mark, and the cells `bound` says are bindings,
/// offered by name already, are left out.
fn function_offers(
    request: &crate::display::CompletionRequest<'_>,
    bound: &dyn Fn(CellId) -> bool,
) -> Vec<Completion> {
    let as_function = match request.path.last() {
        Some(Step::Key(field)) => *field == FUNCTION,
        _ => false,
    };
    let mut offers: Vec<_> = (request.cells)()
        .into_iter()
        .filter(|&cell| cell != control::vocabulary::QUOTE && !bound(cell))
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
                let offer = if as_function {
                    function_completion(cell, request.resolve).with_detail(function_detail(
                        request,
                        "function",
                        definition.library,
                    ))
                } else {
                    call_completion(cell.into(), cell, request.resolve)
                        .with_detail(function_detail(request, "call", definition.library))
                };
                (definition.library.is_some(), name, offer)
            })
        })
        .collect();
    offers.sort_by_cached_key(|(external, name, _)| (*external, name.to_lowercase(), name.clone()));
    offers.into_iter().map(|(_, _, offer)| offer).collect()
}

/// `function` picked for the call whose function slot this is: the call's
/// first declared parameter is picked next, as a call picked whole opens it.
fn function_completion<'a>(
    function: CellId,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
) -> Completion {
    let first = function_parameters(&Value::from(function).into(), resolve)
        .and_then(|parameters| parameters.first().copied());
    let then = match first {
        Some(parameter) => crate::selection::pending_at(&[Step::Key(parameter)]),
        None => crate::selection::at(&[], crate::selection::payload::edge()),
    };
    // The call is where its function slot is.
    let continuation: crate::site::Continuation = std::rc::Rc::new(move |read, slot, staged| {
        slot.split_last()
            .is_some_and(|(_, call)| then(read, call, staged))
    });
    crate::libraries::completion::insert(function, function.into(), Some(continuation))
}

/// What a Grap slot offers, for tests that read its list without a picker.
#[cfg(test)]
pub(crate) fn slot_offers() -> crate::display::CompletionProvider {
    std::rc::Rc::new(suggestions::offers)
}

#[cfg(test)]
#[path = "tests/grap/mod.rs"]
mod tests;
