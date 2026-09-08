//! A live projection for a record with an `evaluate` field. The evaluator
//! does not observe this field; a host that never loads this
//! projection never sees it.

use crate::libraries::name::short_id;
use crate::libraries::{Library, absent, name};
use gid::{CellId, Cells, Step, Value};

pub const ID: CellId = CellId::from_u128(0xf7735b90f6826b25c350a8fd83af8c47);
use crate::display::{
    Completion, CompletionKind, CompletionProvider, Delim, Face, Layout, Pending, ProjectionInput,
    RecordField, ResolvedCell, activatable, alternatives, at_local, col, completion, descend_local,
    dim, faced, hug, record_with, row, selectable_bracket, shared, slot, transient,
};
use ::grap::vocabulary::{BODY, EVALUATE, FFI, FUNCTION, PARAMS};
use ::grap::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};

pub mod vocabulary {
    use gid::CellId;

    /// Source whose contents belong to the Grap domain. This is an
    /// ordinary field and requests no evaluation.
    pub const GRAP: CellId = CellId::from_u128(0x315ca8459cfc64a210d518da1cad79b9);
}

fn spelling(env: &dyn crate::display::Env, cell: CellId) -> (String, Face) {
    match env.name(cell) {
        Some(name) => (name.to_owned(), Face::Name),
        None => (short_id(cell), Face::Id),
    }
}

/// A cell as a reference, not as an invitation to inspect its value.
/// Contextual projections use this for expression and callable references.
pub(crate) fn shallow_cell(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let cell = input.value?.as_cell()?;
    let (spelling, face) = spelling(input.env, cell);
    let target = input.targets.current();
    Some(activatable(
        faced(spelling, face),
        target.hover,
        target.select,
    ))
}

fn declaration_cell(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let cell = input.value?.as_cell()?;
    let definition = input.env.resolve(cell)?;
    Some(selectable_bracket(
        Delim::Paren,
        descend_local(
            Step::Follow(definition.source),
            crate::display::partial(declaration_name),
            &input.default_projection,
        ),
    ))
}

fn declaration_name(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    input.pending.is_none().then_some(())?;
    (input.value?.as_record()?.len() == 1).then_some(())?;
    name::read(input.value?)?;
    Some(descend_local(
        Step::Key(name::vocabulary::NAME),
        crate::display::partial(|input| {
            Some(crate::libraries::line_edit::layout(
                crate::libraries::text::read(input.value?)?,
                crate::libraries::line_edit::native(crate::libraries::text::edit),
                "",
                "",
            ))
        }),
        &input.default_projection,
    ))
}

fn lambda_name(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    Some(match input.value {
        Some(value) => crate::libraries::line_edit::layout(
            crate::libraries::text::read(value)?,
            crate::libraries::line_edit::native(crate::libraries::text::edit),
            "",
            "",
        ),
        None => {
            input.selection.is_none().then_some(())?;
            let target = input.targets.current();
            activatable(faced("λ", Face::Name), target.hover, target.select)
        }
    })
}

pub fn shallow_at(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    default: &crate::display::Partial<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    at_local(steps, value, crate::display::partial(shallow_cell), default)
}

/// A direct expression reference is shallow. A compound expression's
/// projection explicitly chooses the roles of its own children.
pub(crate) fn expression_at(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    default: &crate::display::Partial<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    shallow_at(steps, value, default)
}

/// Declaration cells keep their parentheses, with a name-only definition
/// shown as an unquoted editor at the real name field.
pub(crate) fn declaration_at(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    default: &crate::display::Partial<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    at_local(
        steps,
        value,
        crate::display::partial(declaration_cell),
        default,
    )
}

pub(crate) fn shallow_descend(
    step: Step,
    default: &crate::display::Partial<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    descend_local(step, crate::display::partial(shallow_cell), default)
}

pub(crate) fn expression_descend(
    step: Step,
    default: &crate::display::Partial<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    shallow_descend(step, default)
}

fn field_spelling(env: &dyn crate::display::Env, field: CellId) -> (String, Face) {
    match env.name(field) {
        Some(name) => (name.to_owned(), Face::Label),
        None => (short_id(field), Face::Id),
    }
}

fn parameters(value: &Value) -> Option<Vec<CellId>> {
    let fields = value.as_record()?;
    let fields = match fields.get(&::grap::vocabulary::CLOSURE) {
        Some(closure) => closure.as_record()?,
        None => fields,
    };
    fields.get(&BODY)?;
    fields
        .get(&PARAMS)?
        .as_list()?
        .values()
        .map(Value::as_cell)
        .collect()
}

/// Parameter order is source metadata, not an evaluation. Follow
/// transparent cell references to a stored lambda or closure; a
/// computed callable has no order available to the projection.
pub fn function_parameters<'a>(
    function: &Value,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
) -> Option<Vec<CellId>> {
    let mut function = function;
    let mut followed = std::collections::BTreeSet::new();
    while let Some(cell) = function.as_cell() {
        if !followed.insert(cell) {
            return None;
        }
        let definition = resolve(cell)?;
        if definition.native {
            return None;
        }
        function = definition.value;
    }
    parameters(function)
}

pub fn parameter_labels(function: Value) -> CompletionProvider {
    std::rc::Rc::new(move |request| {
        matches!(request.kind, CompletionKind::Field).then_some(())?;
        Some(
            crate::libraries::completion::labels(function_parameters(&function, request.resolve)?)
                .into_iter()
                .map(|offer| offer.with_detail(PARAMS))
                .collect(),
        )
    })
}

/// Construct a call and focus its first declared parameter, when known.
pub fn call_completion<'a>(
    function: Value,
    display: impl Into<crate::display::CompletionText>,
    resolve: &dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
) -> Completion {
    let first =
        function_parameters(&function, resolve).and_then(|parameters| parameters.first().copied());
    let offer = Completion::new(display, ::grap::call(function, []));
    match first {
        Some(parameter) => offer.on_commit(crate::libraries::selection::pending_at(&[Step::Key(
            parameter,
        )])),
        None => crate::libraries::completion::select(offer),
    }
}

fn standard_field_order(
    env: &dyn crate::display::Env,
    left: &CellId,
    right: &CellId,
) -> std::cmp::Ordering {
    match (env.name(*left), env.name(*right)) {
        (Some(left_name), Some(right_name)) => left_name.cmp(right_name).then(left.cmp(right)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

/// Calls read as calls. Their function position is a shallow
/// reference when it is a cell; arguments retain Grap's contextual
/// projection.
pub fn call_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?;
    let function = fields.get(&FUNCTION)?;
    let parameters = function_parameters(function, &|cell| input.env.resolve(cell));
    let mut parameter_positions = std::collections::BTreeMap::new();
    for (position, parameter) in parameters.iter().flatten().enumerate() {
        parameter_positions.entry(*parameter).or_insert(position);
    }
    let trailing = match &input.pending {
        Some(Pending::Field) => {
            vec![RecordField {
                label: completion(
                    CompletionKind::Field,
                    Some(parameter_labels(function.clone())),
                ),
                value: slot(),
            }]
        }
        Some(Pending::Child(Step::Key(field))) if !fields.contains_key(field) => {
            let (spelling, face) = field_spelling(input.env, *field);
            let target = input.targets.at([Step::Key(*field)]);
            vec![RecordField {
                label: activatable(faced(spelling, face), target.hover, target.select),
                value: descend_local(
                    Step::Key(*field),
                    crate::display::partial(shallow_cell),
                    &input.default_projection,
                ),
            }]
        }
        _ => Vec::new(),
    };
    let function = expression_at([Step::Key(FUNCTION)], function, &input.default_projection);
    let arguments = record_with(
        fields
            .iter()
            .filter(|(field, _)| *field != FUNCTION)
            .map(|(field, value)| (*field, value)),
        |left, right| match (
            parameter_positions.get(left),
            parameter_positions.get(right),
        ) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => standard_field_order(input.env, left, right),
        },
        |field, value| {
            let (spelling, face) = field_spelling(input.env, field);
            let target = input.targets.at([Step::Key(field)]);
            RecordField {
                label: activatable(faced(spelling, face), target.hover, target.select),
                value: expression_at([Step::Key(field)], value, &input.default_projection),
            }
        },
        trailing,
    );
    Some(hug(function, arguments, 0.0, 20.0))
}

/// A stored lambda exposes its parameter declarations deeply and
/// projects its body as an expression.
pub fn lambda_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?;
    fields
        .keys()
        .all(|key| matches!(*key, PARAMS | BODY) || *key == name::vocabulary::NAME)
        .then_some(())?;
    match &input.pending {
        None | Some(Pending::Child(Step::Key(name::vocabulary::NAME | BODY))) => (),
        _ => return None,
    }
    let params = fields.get(&PARAMS)?;
    params
        .as_list()?
        .values()
        .all(|param| param.as_cell().is_some())
        .then_some(())?;
    let params = at_local(
        [Step::Key(PARAMS)],
        params,
        crate::display::structure::list(Some(crate::display::partial(declaration_cell))),
        &input.default_projection,
    );
    let body_target = input.targets.at([Step::Key(BODY)]);
    let lambda = descend_local(
        Step::Key(name::vocabulary::NAME),
        crate::display::partial(lambda_name),
        &input.default_projection,
    );
    let arrow = activatable(dim("→"), body_target.hover, body_target.select);
    let head = row(3.0, [lambda, params, arrow]);
    Some(hug(
        head,
        expression_descend(Step::Key(BODY), &input.default_projection),
        6.0,
        20.0,
    ))
}

/// Foreignness is an evaluator implementation detail. In source, an
/// FFI callable projects exactly like the cell it names.
pub fn ffi_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let ffi = input.value?.as_record()?.get(&FFI)?;
    ffi.as_cell()?;
    Some(shallow_at([Step::Key(FFI)], ffi, &input.default_projection))
}

pub fn evaluate_display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let expression = input.value?.as_record()?.get(&EVALUATE)?;
    let (result, fuel) = input.env.evaluate(expression);
    let expression = shared(expression_at(
        [Step::Key(EVALUATE)],
        expression,
        &input.default_projection,
    ));
    let shaft_target = input.targets.current();
    let shaft = shared(activatable(
        dim("→"),
        shaft_target.hover,
        shaft_target.select,
    ));
    let result = shared(transient(&result, fuel));
    Some(alternatives([
        row(6.0, [expression.clone(), shaft.clone(), result.clone()]),
        col(0, 2.0, [expression, row(6.0, [shaft, result])]),
    ]))
}

fn evaluate_foreign(
    context: &mut Context,
    call: Expression,
    calling_environment: &Environment,
) -> Result<::grap::RuntimeValue, Halt> {
    let Some(expression) = context.field(call, ::grap::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(::grap::vocabulary::EXPRESSION));
    };
    let Some(environment) = context.field(call, ::grap::vocabulary::ENVIRONMENT) else {
        return Ok(context.missing_runtime_argument(::grap::vocabulary::ENVIRONMENT));
    };
    let environment = context.eval(environment, calling_environment)?;
    match context.environment(&environment) {
        Some(environment) => context.eval_runtime(expression, &environment),
        None => Ok(::grap::absent::with_detail(
            ::grap::absent::INVALID_ENVIRONMENT,
            ::grap::absent::VALUE,
            environment,
        )
        .into()),
    }
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        ::grap::vocabulary::EVALUATE,
        ForeignFunction::runtime(evaluate_foreign),
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, value) in [
        (::grap::vocabulary::FUNCTION, "function"),
        (::grap::vocabulary::PARAMS, "params"),
        (::grap::vocabulary::BODY, "body"),
        (::grap::vocabulary::CLOSURE, "closure"),
        (::grap::vocabulary::ENVIRONMENT, "environment"),
        (::grap::vocabulary::FFI, "ffi"),
        (::grap::vocabulary::EVALUATE, "evaluate"),
        (::grap::vocabulary::EXPRESSION, "expression"),
        (vocabulary::GRAP, "grap"),
    ] {
        cells.set_value(cell, name::record(value, []));
    }
    for (cell, value) in [
        (::grap::absent::FUEL_EXHAUSTED, "fuel exhausted"),
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
    ] {
        cells.set_value(cell, absent::named_reason(value));
    }
    Library::named(
        ID,
        "grap",
        crate::libraries::Definitions::from_parts(cells, functions()),
        // Projection order mirrors evaluator precedence: the explicit
        // Grap-result wrapper, calls, lambdas, then FFI values.
        crate::display::compose_partials([
            crate::display::partial(evaluate_display),
            crate::display::partial(call_display),
            crate::display::partial(lambda_display),
            crate::display::partial(ffi_display),
        ]),
    )
    .with_completions(completions)
}

fn completions(request: &crate::display::CompletionRequest<'_>) -> Option<Vec<Completion>> {
    use crate::display::CompletionScope;
    match (request.scope, request.kind, request.path) {
        (CompletionScope::Suggested, CompletionKind::Value, []) => Some(vec![
            Completion::generated(vocabulary::GRAP, || {
                let cell = gid::new_cell_id();
                Value::record([
                    (vocabulary::GRAP, cell.into()),
                    (
                        crate::libraries::workspace::vocabulary::PANES,
                        Value::record([(
                            crate::libraries::workspace::vocabulary::LEFT,
                            Value::list([Value::record([(
                                crate::libraries::presentation::vocabulary::RENDER,
                                cell.into(),
                            )])]),
                        )]),
                    ),
                ])
            })
            .with_detail(ID)
            .on_commit(crate::libraries::selection::pending_at(&[
                gid::Step::Key(vocabulary::GRAP),
                gid::Step::Follow(gid::Resolution::Document),
            ])),
        ]),
        (CompletionScope::Suggested, CompletionKind::Field, []) => Some(vec![
            crate::libraries::completion::label(vocabulary::GRAP).with_detail(ID),
        ]),
        (CompletionScope::Everything, CompletionKind::Value, _) => Some(vec![
            Completion::new("new lambda", Value::record([(PARAMS, Value::list([]))]))
                .with_aliases(["lambda", "λ"])
                .with_detail(ID)
                .on_commit(crate::libraries::selection::at(
                    &[Step::Key(BODY)],
                    crate::libraries::selection::edge(),
                )),
        ]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recordable, Recorded};

    use crate::display::Env;
    use crate::display::test_support::{ProjectionCall, inspect};
    use gid::new_cell_id;

    struct TestEnv {
        result: Value,
    }

    impl Env for TestEnv {
        fn apply_scoped(
            &self,
            _: &gid::Value,
            _: &[(gid::CellId, gid::Value)],
            _scope: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (self.result.clone(), 7)
        }
    }

    fn wrapper(expression: Value, extra: impl IntoIterator<Item = (gid::CellId, Value)>) -> Value {
        Value::record(std::iter::once((EVALUATE, expression)).chain(extra))
    }

    fn env() -> TestEnv {
        TestEnv {
            result: Value::from(vec![1]),
        }
    }

    fn unit_target(
        _: Vec<Step>,
    ) -> crate::display::ProjectionTarget<crate::Editor, crate::frame::Hovered> {
        crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        }
    }

    fn relative_target(
        steps: Vec<Step>,
    ) -> crate::display::ProjectionTarget<crate::Editor, crate::frame::Hovered> {
        crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: crate::libraries::test_widgets::hover(steps),
        }
    }

    fn input<'a>(
        env: &'a dyn Env,
        value: &'a Value,
    ) -> ProjectionInput<'a, crate::Editor, crate::frame::Hovered> {
        ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env,
            value: Some(value),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&unit_target),
        }
    }

    fn relative_input<'a>(
        env: &'a dyn Env,
        value: &'a Value,
    ) -> ProjectionInput<'a, crate::Editor, crate::frame::Hovered> {
        ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env,
            value: Some(value),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&relative_target),
        }
    }

    fn projected(
        env: &dyn Env,
        value: &Value,
    ) -> Option<Recorded<crate::Editor, crate::frame::Hovered>> {
        evaluate_display(&input(env, value)).map(|layout| layout.record())
    }

    #[test]
    fn declaration_cells_follow_their_source_and_edit_the_name_field() {
        struct DefinitionEnv(Value, gid::Resolution);
        impl Env for DefinitionEnv {
            fn apply_scoped(
                &self,
                _: &Value,
                _: &[(CellId, Value)],
                _scope: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation {
                panic!("declarations do not evaluate");
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                panic!("declarations do not evaluate");
            }

            fn resolve(&self, _: CellId) -> Option<ResolvedCell<'_>> {
                Some(ResolvedCell {
                    value: &self.0,
                    source: self.1,
                    native: false,
                })
            }
        }

        for source in [
            gid::Resolution::Document,
            gid::Resolution::Library(new_cell_id()),
        ] {
            for spelling in ["size", ""] {
                let env = DefinitionEnv(name::record(spelling, []), source);
                let cell = Value::from(new_cell_id());
                let declaration = declaration_cell(&input(&env, &cell)).unwrap().record();
                let (_, child, _) = crate::display::test_support::delimited(&declaration);
                let ProjectionCall::Descend {
                    step,
                    projection: Some(projection),
                    ..
                } = &inspect(&(*child))
                else {
                    panic!("a declaration follows the cell definition");
                };
                assert_eq!(*step, Step::Follow(source));
                let ProjectionCall::Descend {
                    step,
                    projection: Some(projection),
                    ..
                } = &inspect(&(projection(&input(&env, &env.0)).unwrap()))
                else {
                    panic!("the compact definition descends to its actual name");
                };
                assert_eq!(*step, Step::Key(name::vocabulary::NAME));
                let value = env
                    .0
                    .as_record()
                    .unwrap()
                    .get(&name::vocabulary::NAME)
                    .unwrap();
                let Some(line) =
                    crate::libraries::test_widgets::line(&projection(&input(&env, value)).unwrap())
                else {
                    panic!("a declaration name uses the stock editor");
                };
                assert_eq!(line.text, spelling);
                assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
                assert_eq!(
                    (line.update)(&env, "next", Some(value)),
                    Some(crate::libraries::text::value("next"))
                );
                assert!(projection(&input(&env, &Value::record([]))).is_none());
            }
        }
    }

    #[test]
    fn compact_declarations_decline_extra_fields_invalid_names_and_active_insertions() {
        let env = env();
        for value in [
            Value::record([]),
            name::record("size", [(new_cell_id(), Value::record([]))]),
            Value::record([(name::vocabulary::NAME, Value::from(vec![1]))]),
        ] {
            assert!(declaration_name(&input(&env, &value)).is_none());
        }
        let value = name::record("size", []);
        let mut input = input(&env, &value);
        input.pending = Some(Pending::Field);
        assert!(declaration_name(&input).is_none());
        input.pending = Some(Pending::Child(Step::Key(new_cell_id())));
        assert!(declaration_name(&input).is_none());
    }

    #[test]
    fn parameter_completions_resolve_lazily_and_follow_current_definitions() {
        use crate::display::{CompletionRequest, CompletionScope};
        let function = new_cell_id();
        let first = new_cell_id();
        let second = new_cell_id();
        let provider = parameter_labels(function.into());
        let reads = std::cell::Cell::new(0);
        for expected in [vec![first, second], vec![second, first], vec![]] {
            let definition = ::grap::lambda(expected.iter().copied(), Value::record([]));
            let resolve = |cell| {
                reads.set(reads.get() + 1);
                (cell == function).then_some(ResolvedCell {
                    source: gid::Resolution::Document,
                    value: &definition,
                    native: false,
                })
            };
            let request = CompletionRequest {
                query: "",
                kind: CompletionKind::Field,
                scope: CompletionScope::Suggested,
                path: &[],
                value_at: &|_| None,
                resolve: &resolve,
            };
            let before = reads.get();
            assert!(
                provider(&CompletionRequest {
                    kind: CompletionKind::Value,
                    ..request
                })
                .is_none()
            );
            assert_eq!(reads.get(), before);
            let offers = provider(&request).unwrap();
            assert_eq!(reads.get(), before + 1);
            assert_eq!(
                offers
                    .iter()
                    .map(|offer| offer.value.instantiate())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .copied()
                    .map(Value::from)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                offers
                    .iter()
                    .map(|offer| offer.display.clone())
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .copied()
                    .map(crate::display::CompletionText::Name)
                    .collect::<Vec<_>>()
            );
            let call = call_completion(function.into(), function, &resolve);
            assert_eq!(call.value.instantiate(), ::grap::call(function.into(), []));
            assert_eq!(
                call.on_commit,
                expected
                    .first()
                    .map(|first| crate::libraries::selection::pending_at(&[Step::Key(*first)]))
                    .or_else(|| Some(crate::libraries::selection::at(
                        &[],
                        crate::libraries::selection::edge()
                    )))
            );
        }
    }

    #[test]
    fn parameter_metadata_accepts_lambdas_closures_and_aliases_but_not_unknown_callables() {
        let function = new_cell_id();
        let alias = new_cell_id();
        let parameter = new_cell_id();
        let lambda = ::grap::lambda([parameter], Value::record([]));
        let closure = Value::record([(
            ::grap::vocabulary::CLOSURE,
            Value::record([
                (PARAMS, Value::list([parameter.into()])),
                (BODY, Value::record([])),
                (::grap::vocabulary::ENVIRONMENT, Value::record([])),
            ]),
        )]);
        for value in [&lambda, &closure] {
            assert_eq!(
                function_parameters(value, &|_| panic!("inline function needs no lookup")),
                Some(vec![parameter])
            );
        }
        let reference = Value::from(function);
        let resolve = |cell| {
            Some(ResolvedCell {
                source: gid::Resolution::Library(ID),
                value: if cell == alias { &reference } else { &lambda },
                native: false,
            })
        };
        assert_eq!(
            function_parameters(&alias.into(), &resolve),
            Some(vec![parameter])
        );
        assert_eq!(function_parameters(&reference, &|_| None), None);
        assert_eq!(
            function_parameters(&reference, &|_| Some(ResolvedCell {
                source: gid::Resolution::Document,
                value: &reference,
                native: false,
            })),
            None
        );
        assert_eq!(
            function_parameters(&reference, &|_| Some(ResolvedCell {
                source: gid::Resolution::Library(ID),
                value: &lambda,
                native: true,
            })),
            None
        );
        for invalid in [
            Value::record([]),
            ::grap::call(reference.clone(), []),
            Value::record([
                (PARAMS, Value::list([Value::record([])])),
                (BODY, Value::record([])),
            ]),
        ] {
            assert_eq!(
                function_parameters(&invalid, &|_| panic!("metadata must not evaluate a call")),
                None
            );
        }
    }

    #[test]
    fn parameter_offers_defer_name_lookup_to_the_picker() {
        struct CountingEnv(std::cell::Cell<usize>);
        impl Env for CountingEnv {
            fn apply_scoped(
                &self,
                _: &gid::Value,
                _: &[(gid::CellId, gid::Value)],
                _scope: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation {
                panic!("unexpected projection application")
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                panic!("completion does not evaluate the function")
            }

            fn name(&self, _: CellId) -> Option<&str> {
                self.0.set(self.0.get() + 1);
                Some("parameter")
            }
        }

        let parameter = new_cell_id();
        let value = Value::record([(
            FUNCTION,
            Value::record([
                (PARAMS, Value::list([Value::from(parameter)])),
                (BODY, Value::record([])),
            ]),
        )]);
        let env = CountingEnv(std::cell::Cell::new(0));
        assert!(call_display(&input(&env, &value)).is_some());
        assert_eq!(env.0.get(), 0);
        assert!(
            call_display(&ProjectionInput {
                default_projection: crate::display::partial(|_| None),
                pending: Some(Pending::Field),
                ..input(&env, &value)
            })
            .is_some()
        );
        assert_eq!(env.0.get(), 0);
    }

    fn unshared(
        mut layout: &Recorded<crate::Editor, crate::frame::Hovered>,
    ) -> &Recorded<crate::Editor, crate::frame::Hovered> {
        while let Recorded::Shared { child, .. } = layout {
            layout = child.as_ref();
        }
        layout
    }

    fn arms(
        layout: &Recorded<crate::Editor, crate::frame::Hovered>,
    ) -> (
        &Recorded<crate::Editor, crate::frame::Hovered>,
        &Recorded<crate::Editor, crate::frame::Hovered>,
    ) {
        let Recorded::Alternatives(options) = layout else {
            panic!("expected alternatives");
        };
        let Some(Recorded::Row { children, .. }) = options.first() else {
            panic!("expected a row first");
        };
        assert_eq!(children.len(), 3);
        (unshared(&children[0]), unshared(&children[2]))
    }

    fn argument_order(
        layout: &impl Recordable<crate::Editor, crate::frame::Hovered>,
    ) -> Vec<CellId> {
        let Recorded::Alternatives(call_options) = layout.record() else {
            panic!("call has responsive forms");
        };
        let Recorded::Row { children, .. } = &call_options[0] else {
            panic!("flat call first");
        };
        let (_, child, _) = crate::display::test_support::delimited(unshared(&children[1]));
        let Recorded::Alternatives(argument_options) = child else {
            panic!("arguments have responsive forms");
        };
        let Recorded::Row { children, .. } = &argument_options[0] else {
            panic!("flat arguments first");
        };
        children
            .iter()
            .step_by(2)
            .map(|argument| {
                let Recorded::Row { children, .. } = argument else {
                    panic!("argument has a label and value");
                };
                let ProjectionCall::At { steps, .. } = &inspect(&(unshared(&children[2]))) else {
                    panic!("argument value retains its path");
                };
                let [Step::Key(field)] = steps.as_slice() else {
                    panic!("argument path is its field");
                };
                *field
            })
            .collect()
    }

    #[test]
    fn a_record_with_the_field_is_expression_then_result() {
        let expression = Value::from(vec![0]);
        let layout = projected(&env(), &wrapper(expression.clone(), [])).unwrap();
        let (shown, result) = arms(&layout);
        assert!(matches!(&inspect(&(shown)),
            ProjectionCall::At { steps, value, .. }
                if *steps == [Step::Key(EVALUATE)] && *value == expression
        ));
        assert!(matches!(&inspect(&(result)),
            ProjectionCall::Transient { value, fuel: 7 } if *value == Value::from(vec![1])
        ));
    }

    #[test]
    fn other_fields_do_not_block_recognition() {
        assert!(matches!(
            projected(
                &env(),
                &wrapper(
                    Value::from(vec![0]),
                    [(new_cell_id(), Value::from(vec![2]))]
                ),
            ),
            Some(Recorded::Alternatives(_))
        ));
    }

    #[test]
    fn an_evaluate_shaped_result_is_another_projection() {
        let inner = Value::from(vec![2]);
        let result = wrapper(inner.clone(), []);
        let layout = projected(
            &TestEnv {
                result: result.clone(),
            },
            &wrapper(Value::from(vec![0]), []),
        )
        .unwrap();
        let (_, shown) = arms(&layout);
        assert!(matches!(&inspect(&(shown)),
            ProjectionCall::Transient { value, fuel: 7 } if *value == result
        ));
        let layout = projected(&env(), &result).unwrap();
        let (nested, _) = arms(&layout);
        assert!(matches!(&inspect(&(nested)),
            ProjectionCall::At { steps, value, .. }
                if *steps == [Step::Key(EVALUATE)] && *value == inner
        ));
    }

    #[test]
    fn a_call_projects_its_function_cell_shallowly() {
        let function = new_cell_id();
        let argument = new_cell_id();
        let layout = call_display(&input(
            &env(),
            &::grap::call(Value::from(function), [(argument, Value::from(vec![1]))]),
        ))
        .unwrap();
        let Recorded::Alternatives(options) = layout.record() else {
            panic!("call has responsive forms");
        };
        let Recorded::Row { children, .. } = &options[0] else {
            panic!("flat call first");
        };
        assert!(matches!(&inspect(&(unshared(&children[0]))),
            ProjectionCall::At {
                steps,
                value,
                projection: Some(_),
                ..
            } if *steps == [Step::Key(FUNCTION)]
                && *value == Value::from(function)
        ));
    }

    #[test]
    fn a_call_argument_label_targets_its_value() {
        let function = new_cell_id();
        let argument = new_cell_id();
        let call = ::grap::call(Value::from(function), [(argument, Value::from(vec![1]))]);
        let layout = call_display(&relative_input(&env(), &call)).unwrap();
        let Recorded::Alternatives(call_options) = layout.record() else {
            panic!("call has responsive forms");
        };
        let Recorded::Row { children, .. } = &call_options[0] else {
            panic!("flat call first");
        };
        let (left, child, right) = crate::display::test_support::delimited(unshared(&children[1]));
        crate::libraries::test_widgets::assert_delimiter(
            left,
            crate::display::Delim::Brace,
            crate::display::Side::Open,
        );
        crate::libraries::test_widgets::assert_delimiter(
            right,
            crate::display::Delim::Brace,
            crate::display::Side::Close,
        );
        let Recorded::Alternatives(argument_options) = child else {
            panic!("arguments have responsive forms");
        };
        let Recorded::Row { children, .. } = &argument_options[0] else {
            panic!("flat arguments first");
        };
        let Recorded::Row { children, .. } = &children[0] else {
            panic!("argument has a label and value");
        };
        let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("field head contains its label and colon");
        };
        let Recorded::Before { child, .. } = &head[0] else {
            panic!("the label targets its argument");
        };
        assert_eq!(
            crate::libraries::test_widgets::claim(&head[0]),
            Some(puri::hover::Claim::Direct(
                crate::libraries::test_widgets::hover(vec![Step::Key(argument)])
            ))
        );
        assert!(matches!(child.as_ref(), Recorded::Before { .. }));
    }

    #[test]
    fn a_call_uses_its_stored_lambdas_parameter_order_then_stable_extras() {
        const FUNCTION_CELL: CellId = CellId::from_u128(10);
        const FIRST_PARAMETER: CellId = CellId::from_u128(30);
        const SECOND_PARAMETER: CellId = CellId::from_u128(20);
        const FIRST_EXTRA: CellId = CellId::from_u128(40);
        const SECOND_EXTRA: CellId = CellId::from_u128(50);

        struct DefinitionEnv {
            definition: Value,
            native: bool,
        }

        impl Env for DefinitionEnv {
            fn apply_scoped(
                &self,
                _: &gid::Value,
                _: &[(gid::CellId, gid::Value)],
                _scope: Option<&::grap::ForeignOverlay<'_>>,
            ) -> ::grap::Evaluation {
                panic!("unexpected projection application")
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                (Value::record([]), 0)
            }

            fn resolve(&self, cell: CellId) -> Option<ResolvedCell<'_>> {
                (cell == FUNCTION_CELL).then_some(ResolvedCell {
                    source: gid::Resolution::Document,
                    value: &self.definition,
                    native: self.native,
                })
            }
        }

        let env = DefinitionEnv {
            native: false,
            definition: ::grap::lambda(
                [FIRST_PARAMETER, SECOND_PARAMETER],
                Value::from(FIRST_PARAMETER),
            ),
        };
        let call = ::grap::call(
            Value::from(FUNCTION_CELL),
            [
                (SECOND_EXTRA, Value::from(vec![5])),
                (SECOND_PARAMETER, Value::from(vec![2])),
                (FIRST_EXTRA, Value::from(vec![4])),
                (FIRST_PARAMETER, Value::from(vec![3])),
            ],
        );
        let layout = call_display(&input(&env, &call)).unwrap();

        assert_eq!(
            argument_order(&layout),
            [FIRST_PARAMETER, SECOND_PARAMETER, FIRST_EXTRA, SECOND_EXTRA,]
        );
        let native = DefinitionEnv {
            native: true,
            ..env
        };
        assert_eq!(
            argument_order(&call_display(&input(&native, &call)).unwrap()),
            [SECOND_PARAMETER, FIRST_PARAMETER, FIRST_EXTRA, SECOND_EXTRA],
        );
    }

    #[test]
    fn a_call_uses_an_inline_lambdas_parameter_order() {
        const FIRST_PARAMETER: CellId = CellId::from_u128(2);
        const SECOND_PARAMETER: CellId = CellId::from_u128(1);
        let call = ::grap::call(
            ::grap::lambda(
                [FIRST_PARAMETER, SECOND_PARAMETER],
                Value::from(FIRST_PARAMETER),
            ),
            [
                (SECOND_PARAMETER, Value::from(vec![1])),
                (FIRST_PARAMETER, Value::from(vec![2])),
            ],
        );
        let layout = call_display(&input(&env(), &call)).unwrap();

        assert_eq!(argument_order(&layout), [FIRST_PARAMETER, SECOND_PARAMETER]);
    }

    #[test]
    fn a_lambda_targets_its_syntax_and_projects_its_body_as_grap() {
        let parameter = new_cell_id();
        let definition = ::grap::lambda([parameter], Value::from(parameter));
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Recorded::Alternatives(options) = layout.record() else {
            panic!("lambda has responsive forms");
        };
        let Recorded::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        assert!(matches!(&inspect(&(&head[0])),
            ProjectionCall::Descend {
                step: Step::Key(key),
                projection: Some(_),
                ..
            } if *key == name::vocabulary::NAME
        ));
        assert!(matches!(&inspect(&(&head[1])),
            ProjectionCall::At {
                steps,
                projection: Some(_),
                ..
            } if *steps == [Step::Key(PARAMS)]
        ));
        let Recorded::Before { child, .. } = &head[2] else {
            panic!("lambda arrow targets its body");
        };
        assert_eq!(
            crate::libraries::test_widgets::claim(&head[2]),
            Some(puri::hover::Claim::Direct(
                crate::libraries::test_widgets::hover(vec![Step::Key(BODY)])
            ))
        );
        assert!(matches!(child.as_ref(), Recorded::Before { .. }));
        assert!(matches!(
            &inspect(&(unshared(&children[1]))),
            ProjectionCall::Descend {
                step: Step::Key(BODY),
                projection: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn an_unfinished_lambda_keeps_its_missing_body_visible() {
        let env = env();
        let definition = Value::record([(PARAMS, Value::list([]))]);
        let mut input = relative_input(&env, &definition);
        assert!(lambda_display(&input).is_some());
        input.pending = Some(Pending::Child(Step::Key(BODY)));
        assert!(lambda_display(&input).is_some());
        input.pending = Some(Pending::Child(Step::Key(new_cell_id())));
        assert!(lambda_display(&input).is_none());

        for malformed in [
            Value::record([]),
            Value::record([(PARAMS, Value::record([]))]),
            Value::record([(
                PARAMS,
                Value::list([crate::libraries::text::value("not a cell")]),
            )]),
            Value::record([
                (PARAMS, Value::list([])),
                (new_cell_id(), Value::record([])),
            ]),
        ] {
            assert!(lambda_display(&relative_input(&env, &malformed)).is_none());
        }
    }

    #[test]
    fn a_named_lambda_projects_its_editable_name_in_place_of_the_marker() {
        let definition = name::record(
            "tree",
            [(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))],
        );
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Recorded::Alternatives(options) = layout.record() else {
            panic!("lambda has responsive forms");
        };
        let Recorded::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        let ProjectionCall::Descend {
            step: Step::Key(key),
            projection: Some(projection),
            ..
        } = &inspect(&(&head[0]))
        else {
            panic!("lambda name is projected contextually");
        };
        assert_eq!(*key, name::vocabulary::NAME);
        let value = definition
            .as_record()
            .unwrap()
            .get(&name::vocabulary::NAME)
            .unwrap();
        let Some(line) = crate::libraries::test_widgets::line(
            &projection(&relative_input(&env(), value)).unwrap(),
        ) else {
            panic!("lambda name uses the stock line editor");
        };
        assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
    }

    #[test]
    fn the_lambda_name_partial_handles_only_unselected_missing_names_or_text() {
        let env = env();
        let value = Value::record([]);
        let mut input = input(&env, &value);
        input.value = None;
        assert!(lambda_name(&input).is_some());

        let selected = crate::libraries::selection::pending();
        input.selection = Some(&selected);
        assert!(lambda_name(&input).is_none());
        input.selection = None;
        input.value = Some(&value);
        assert!(lambda_name(&input).is_none());

        let absent = crate::libraries::absent::value();
        input.value = Some(&absent);
        assert!(lambda_name(&input).is_none());
    }

    #[test]
    fn an_explicit_empty_lambda_name_projects_as_an_empty_string() {
        let definition = name::record(
            "",
            [(PARAMS, Value::list([])), (BODY, Value::from(vec![1]))],
        );
        let layout = lambda_display(&relative_input(&env(), &definition)).unwrap();
        let Recorded::Alternatives(options) = layout.record() else {
            panic!("lambda has responsive forms");
        };
        let Recorded::Row { children, .. } = &options[0] else {
            panic!("flat lambda first");
        };
        let Recorded::Row { children: head, .. } = unshared(&children[0]) else {
            panic!("lambda has a syntax head");
        };
        let ProjectionCall::Descend {
            projection: Some(projection),
            ..
        } = &inspect(&(&head[0]))
        else {
            panic!("lambda name is projected contextually");
        };
        let value = definition
            .as_record()
            .unwrap()
            .get(&name::vocabulary::NAME)
            .unwrap();
        let Some(line) = crate::libraries::test_widgets::line(
            &projection(&relative_input(&env(), value)).unwrap(),
        ) else {
            panic!("lambda name uses the stock line editor");
        };
        assert_eq!(line.text, "");
        assert_eq!(line.placeholder, None);
        assert_eq!((line.prefix.as_str(), line.suffix.as_str()), ("", ""));
    }

    #[test]
    fn the_library_describes_grap_and_owns_evaluate() {
        let library = library();
        assert_eq!(
            library
                .value(::grap::vocabulary::FUNCTION)
                .and_then(name::read),
            Some("function")
        );
        assert_eq!(
            library
                .value(::grap::absent::MISSING_CELL)
                .and_then(name::read),
            Some("missing cell")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(::grap::absent::MISSING_CELL)),
            Some(::grap::absent::MISSING_CELL)
        );

        let input = gid::new_cell_id();
        let expression = ::grap::call(
            ::grap::lambda([input], Value::from(input)),
            [(input, Value::from(b"evaluated".to_vec()))],
        );
        let evaluation = crate::libraries::test_evaluate(
            &::grap::call(
                Value::from(::grap::vocabulary::EVALUATE),
                [
                    (::grap::vocabulary::EXPRESSION, expression),
                    (::grap::vocabulary::ENVIRONMENT, Value::record([])),
                ],
            ),
            |_| None,
            &library.functions(),
            40,
        );
        assert_eq!(evaluation.result, Value::from(b"evaluated".to_vec()));
    }
}
