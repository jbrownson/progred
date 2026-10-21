//! Grap's evaluation control, supplied by Rust functions. `match`
//! selects one case through structural matching, `let` and `where`
//! extend an environment through sequential bindings, `do` evaluates
//! expressions until the first absent, `all` retains every result, and
//! `quote` constructs data while evaluating explicit unquotes.

use crate::display::Projected;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use gid::{CellId, Step, Value};

use crate::display::hug;
use crate::display::structure::record_form;
use crate::display::{activatable, alternatives, col, descend, dim, row, shared};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue, Stage,
};
use std::ops::ControlFlow;
use std::rc::Rc;

#[cfg(test)]
mod runtime_tests;

pub mod vocabulary {
    use gid::CellId;

    pub const MATCH: CellId = CellId::from_u128(0xb3f6a62e4926889bcfcd338025f4a6f9);
    pub const VALUE: CellId = CellId::from_u128(0x00dafdc01c7e014edd857e174c6c8b6f);
    pub const CASES: CellId = CellId::from_u128(0xa9aadcb44755963498c743fa114f0f00);
    pub const PATTERN: CellId = CellId::from_u128(0xb9dc97198709bc6f7bae4c7afc7d424f);
    pub const BIND: CellId = CellId::from_u128(0x5e46d12705690e8a377eb0f16ad9dba6);
    pub const QUOTE: CellId = CellId::from_u128(0x7f81d4812ceb33d4222e9e5cb9c82497);
    pub const UNQUOTE: CellId = CellId::from_u128(0xda48703c290e3b35d7353c38110bc953);
    pub const LET: CellId = CellId::from_u128(0xf4f93c70910c4d8a8eeb8e049cc64c14);
    pub const WHERE: CellId = CellId::from_u128(0x2a3432e9c5ce4c62b8261a6b248f13c2);
    pub const BINDINGS: CellId = CellId::from_u128(0xf6fb0062e8a14f808e416a9edece2a9d);
    pub const DO: CellId = CellId::from_u128(0xb1fc4cb45c58b1a662c431feef5bd140);
    pub const ALL: CellId = CellId::from_u128(0xbc2f76148affd64e009a1a37a32aa2bf);
    pub const EXPRESSIONS: CellId = CellId::from_u128(0x5fab151c006ae1487c28837f2003f43c);

    pub const INVALID_CASES: CellId = CellId::from_u128(0x1b94a59ed759da212fa72d7094796c6e);
    pub const INVALID_CASE: CellId = CellId::from_u128(0xaa627ebeb1091e8359f7a8eea45a6ccd);
    pub const INVALID_BINDER: CellId = CellId::from_u128(0x59ad0fb67728f245dce57b0cee360969);
    pub const INVALID_BINDINGS: CellId = CellId::from_u128(0x480ae287377249459a3438cb4b05229f);
    pub const INVALID_BINDING: CellId = CellId::from_u128(0x4ecae0db406e426aba1c02f2f04570ad);
    pub const INVALID_EXPRESSIONS: CellId = CellId::from_u128(0xa41a40f12691414971dbd9f788c291d6);
    pub const PATTERN_MISMATCH: CellId = CellId::from_u128(0xc44d3f1a71dea754a02bc14561a143b7);
    pub const MISSING_FINAL_EXPRESSION: CellId =
        CellId::from_u128(0x9e3fa2dd570e0b38a8b0053585f4dde2);
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::MATCH,
            ForeignFunction::staged(match_prepare)
                .parameters([vocabulary::VALUE, vocabulary::CASES])
                .tracked(),
        )
        .register(
            vocabulary::LET,
            ForeignFunction::staged(bindings_prepare)
                .parameters([vocabulary::BINDINGS, ::grap::vocabulary::EXPRESSION])
                .tracked(),
        )
        .register(
            vocabulary::WHERE,
            ForeignFunction::staged(bindings_prepare)
                .parameters([vocabulary::BINDINGS, ::grap::vocabulary::EXPRESSION])
                .tracked(),
        )
        .register(
            vocabulary::DO,
            ForeignFunction::new(do_foreign)
                .parameters([vocabulary::EXPRESSIONS])
                .tracked(),
        )
        .register(
            vocabulary::ALL,
            ForeignFunction::new(all_foreign)
                .parameters([vocabulary::EXPRESSIONS])
                .tracked(),
        )
        .register(
            vocabulary::QUOTE,
            ForeignFunction::new(quote_foreign)
                .parameters([::grap::vocabulary::EXPRESSION])
                .tracked(),
        )
}

fn do_foreign(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expressions) = context.field(call, vocabulary::EXPRESSIONS) else {
        return Ok(context.missing_runtime_argument(call, vocabulary::EXPRESSIONS));
    };
    match context.elements(&expressions) {
        Some(elements) => in_turn(elements, context, environment),
        None => Ok(::grap::absent::value(vocabulary::INVALID_EXPRESSIONS).into()),
    }
}

/// Evaluates `expressions` in turn. The first halt or absent is the result;
/// without one, the last value is.
fn in_turn(
    expressions: &[Expression],
    context: &mut Context,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    match expressions.iter().try_fold(None, |_, expression| {
        match context.eval(expression.clone(), environment) {
            Ok(value) if value.absent_reason().is_none() => ControlFlow::Continue(Some(value)),
            stopped => ControlFlow::Break(stopped),
        }
    }) {
        ControlFlow::Continue(last) => Ok(last
            .unwrap_or_else(|| ::grap::absent::value(vocabulary::MISSING_FINAL_EXPRESSION).into())),
        ControlFlow::Break(stopped) => stopped,
    }
}

fn all_foreign(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expressions) = context.field(call, vocabulary::EXPRESSIONS) else {
        return Ok(context.missing_runtime_argument(call, vocabulary::EXPRESSIONS));
    };
    match context.elements(&expressions) {
        Some(elements) => try_map_sized(elements, |expression| {
            context.eval(expression.clone(), environment)
        })
        .map(RuntimeValue::list),
        None => Ok(::grap::absent::value(vocabulary::INVALID_EXPRESSIONS).into()),
    }
}

fn quote_foreign(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expression) = context.field(call, ::grap::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(call, ::grap::vocabulary::EXPRESSION));
    };
    replace_unquotes(expression, context, environment)
}

fn replace_unquotes(
    expression: Expression,
    context: &mut Context,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    if let Some(fields) = context.fields(&expression) {
        match fields
            .iter()
            .find(|(field, _)| *field == vocabulary::UNQUOTE)
        {
            Some((_, unquote)) => context.eval(unquote.clone(), environment),
            None => try_map_sized(fields, |(field, value)| {
                Ok((
                    *field,
                    replace_unquotes(value.clone(), context, environment)?,
                ))
            })
            .map(RuntimeValue::record),
        }
    } else if let Some(elements) = context.elements(&expression) {
        try_map_sized(elements, |value| {
            replace_unquotes(value.clone(), context, environment)
        })
        .map(RuntimeValue::list)
    } else {
        let value = context.value(&expression).clone();
        replace_unquotes_value(&value, context, environment)
    }
}

/// Collecting through `Result` loses the length hint, and quoted templates
/// sit on hot paths, so the output is sized up front.
fn try_map_sized<T, U>(
    items: &[T],
    mut map: impl FnMut(&T) -> Result<U, Halt>,
) -> Result<Vec<U>, Halt> {
    let mut mapped = Vec::with_capacity(items.len());
    for item in items {
        mapped.push(map(item)?);
    }
    Ok(mapped)
}

fn replace_unquotes_value(
    value: &Value,
    context: &mut Context,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    match value {
        Value::Record(fields) => match fields.get(&vocabulary::UNQUOTE) {
            Some(expression) => context.eval_value_runtime(expression, environment),
            None => Ok(RuntimeValue::record(
                fields
                    .iter()
                    .map(|(field, value)| {
                        Ok((*field, replace_unquotes_value(value, context, environment)?))
                    })
                    .collect::<Result<Vec<(CellId, RuntimeValue)>, Halt>>()?,
            )),
        },
        Value::List(values) => Ok(RuntimeValue::list(
            values
                .values()
                .map(|value| replace_unquotes_value(value, context, environment))
                .collect::<Result<Vec<RuntimeValue>, Halt>>()?,
        )),
        Value::Cell(_) | Value::Blob(_) => Ok(value.clone().into()),
    }
}

/// A case parsed once at prepare: its position is kept so a malformed
/// case still declines at the moment selection reaches it.
enum CompiledCase {
    Case {
        pattern: Value,
        expression: Expression,
    },
    Malformed,
}

enum CompiledCases {
    Cases(Vec<CompiledCase>),
    /// Cases behind a reference are still evaluated per visit.
    Deferred(Expression),
}

fn match_prepare(context: &Context, call: &Expression) -> Stage {
    let subject = context.argument(call, vocabulary::VALUE);
    let compiled =
        context
            .argument(call, vocabulary::CASES)
            .map(|cases| match context.elements(&cases) {
                None => CompiledCases::Deferred(cases),
                Some(elements) => CompiledCases::Cases(
                    elements
                        .iter()
                        .map(|case| {
                            match (
                                context.field(case, vocabulary::PATTERN),
                                context.field(case, ::grap::vocabulary::EXPRESSION),
                            ) {
                                (Some(pattern), Some(expression)) => CompiledCase::Case {
                                    pattern: context.value(&pattern).clone(),
                                    expression,
                                },
                                _ => CompiledCase::Malformed,
                            }
                        })
                        .collect(),
                ),
            });
    Rc::new(move |context, environment| {
        let subject = match &subject {
            Ok(subject) => subject.clone(),
            Err(missing) => return Ok(missing.clone()),
        };
        let compiled = match &compiled {
            Ok(compiled) => compiled,
            Err(missing) => return Ok(missing.clone()),
        };
        let value = context.eval(subject, environment)?;
        match compiled {
            CompiledCases::Cases(cases) => {
                // Selecting over prepared cases skips evaluating the
                // list expression; burn its fuel so the stage stays
                // burn-invisible.
                context.burn()?;
                // The first case that matches, or isn't a case, decides.
                cases
                    .iter()
                    .find_map(|case| match case {
                        CompiledCase::Case {
                            pattern,
                            expression,
                        } => match destructure(pattern, &value) {
                            Ok(Some(bindings)) => {
                                Some(context.eval(
                                    expression.clone(),
                                    &environment.extended_runtime(bindings),
                                ))
                            }
                            Ok(None) => None,
                            Err(InvalidBinder) => {
                                Some(Ok(::grap::absent::value(vocabulary::INVALID_BINDER).into()))
                            }
                        },
                        CompiledCase::Malformed => {
                            Some(Ok(::grap::absent::value(vocabulary::INVALID_CASE).into()))
                        }
                    })
                    .unwrap_or_else(|| {
                        Ok(::grap::absent::from_causes(cases.iter().filter_map(
                            |case| match case {
                                CompiledCase::Case { pattern, .. } => {
                                    Some(pattern_mismatch(pattern))
                                }
                                CompiledCase::Malformed => None,
                            },
                        ))
                        .into())
                    })
            }
            CompiledCases::Deferred(cases) => {
                let cases_value = context.eval(cases.clone(), environment)?;
                match select(&value, &cases_value) {
                    Selection::Expression {
                        expression,
                        bindings,
                    } => context
                        .eval_runtime_code(&expression, &environment.extended_runtime(bindings)),
                    Selection::NoMatch(absents) => Ok(::grap::absent::from_causes(absents).into()),
                    Selection::Invalid(cell) => Ok(::grap::absent::value(cell).into()),
                }
            }
        }
    })
}

/// A binding clause parsed once at prepare; a malformed clause keeps
/// its position and reason so earlier bindings still evaluate first.
enum CompiledBinding {
    Bind { binder: CellId, value: Expression },
    Pattern { pattern: Value, value: Expression },
    Malformed(CellId),
}

enum CompiledBindings {
    Bindings(Vec<CompiledBinding>),
    /// Bindings behind a reference are still evaluated per visit.
    Deferred(Expression),
}

fn bindings_prepare(context: &Context, call: &Expression) -> Stage {
    let compiled = context
        .argument(call, vocabulary::BINDINGS)
        .map(|bindings| match context.elements(&bindings) {
            None => CompiledBindings::Deferred(bindings),
            Some(elements) => CompiledBindings::Bindings(
                elements
                    .iter()
                    .map(|binding| {
                        match (
                            context.field(binding, vocabulary::VALUE),
                            context.field(binding, vocabulary::BIND),
                            context.field(binding, vocabulary::PATTERN),
                        ) {
                            (Some(value), Some(binder), None) => {
                                match context.value(&binder).as_cell() {
                                    Some(binder) => CompiledBinding::Bind { binder, value },
                                    None => CompiledBinding::Malformed(vocabulary::INVALID_BINDER),
                                }
                            }
                            (Some(value), None, Some(pattern)) => CompiledBinding::Pattern {
                                pattern: context.value(&pattern).clone(),
                                value,
                            },
                            _ => CompiledBinding::Malformed(vocabulary::INVALID_BINDING),
                        }
                    })
                    .collect(),
            ),
        });
    let expression = context.argument(call, ::grap::vocabulary::EXPRESSION);
    Rc::new(move |context, environment| {
        let compiled = match &compiled {
            Ok(compiled) => compiled,
            Err(missing) => return Ok(missing.clone()),
        };
        let expression = match &expression {
            Ok(expression) => expression.clone(),
            Err(missing) => return Ok(missing.clone()),
        };
        let mut bound = environment.clone();
        let outcome = match compiled {
            CompiledBindings::Bindings(bindings) => {
                // The prepared walk skips evaluating the bindings list
                // expression; burn its fuel so the stage stays
                // burn-invisible.
                context.burn()?;
                bound.reserve(bindings.len());
                bindings.iter().try_for_each(|binding| match binding {
                    CompiledBinding::Malformed(reason) => ended(*reason),
                    CompiledBinding::Bind { binder, value } => {
                        let value = context.eval(value.clone(), &bound);
                        bind(&mut bound, *binder, value)
                    }
                    CompiledBinding::Pattern { pattern, value } => {
                        let value = context.eval(value.clone(), &bound);
                        bind_pattern(&mut bound, pattern, value)
                    }
                })
            }
            CompiledBindings::Deferred(bindings) => {
                match context.eval(bindings.clone(), environment)?.list_values() {
                    None => ended(vocabulary::INVALID_BINDINGS),
                    Some(mut bindings) => bindings.try_for_each(|binding| {
                        match (
                            binding.field(vocabulary::VALUE),
                            binding.field(vocabulary::BIND),
                            binding.field(vocabulary::PATTERN),
                        ) {
                            (Some(value), Some(binder), None) => match binder.as_cell() {
                                Some(binder) => {
                                    let value = context.eval_runtime_code(&value, &bound);
                                    bind(&mut bound, binder, value)
                                }
                                None => ended(vocabulary::INVALID_BINDER),
                            },
                            (Some(value), None, Some(pattern)) => {
                                let value = context.eval_runtime_code(&value, &bound);
                                bind_pattern(&mut bound, &pattern.to_value(), value)
                            }
                            _ => ended(vocabulary::INVALID_BINDING),
                        }
                    }),
                }
            }
        };
        match outcome {
            ControlFlow::Continue(()) => context.eval(expression, &bound),
            ControlFlow::Break(stopped) => stopped,
        }
    })
}

/// Ends the bindings in the absent `reason`.
fn ended(reason: CellId) -> ControlFlow<Result<RuntimeValue, Halt>> {
    ControlFlow::Break(Ok(::grap::absent::value(reason).into()))
}

/// Binds `value` to `binder` in `environment`, or ends the bindings with its
/// halt.
fn bind(
    environment: &mut Environment,
    binder: CellId,
    value: Result<RuntimeValue, Halt>,
) -> ControlFlow<Result<RuntimeValue, Halt>> {
    match value {
        Ok(value) => {
            environment.push_runtime([(binder, value)]);
            ControlFlow::Continue(())
        }
        Err(halt) => ControlFlow::Break(Err(halt)),
    }
}

/// Binds `value` to `pattern`'s binders in `environment`, or ends the
/// bindings: with a halt, or the absent of a pattern that doesn't match.
fn bind_pattern(
    environment: &mut Environment,
    pattern: &Value,
    value: Result<RuntimeValue, Halt>,
) -> ControlFlow<Result<RuntimeValue, Halt>> {
    match value.map(|value| destructure(pattern, &value)) {
        Ok(Ok(Some(bindings))) => {
            environment.push_runtime(bindings);
            ControlFlow::Continue(())
        }
        Ok(Ok(None)) => ControlFlow::Break(Ok(pattern_mismatch(pattern).into())),
        Ok(Err(InvalidBinder)) => ended(vocabulary::INVALID_BINDER),
        Err(halt) => ControlFlow::Break(Err(halt)),
    }
}

enum Selection {
    Expression {
        expression: RuntimeValue,
        bindings: Vec<(CellId, RuntimeValue)>,
    },
    NoMatch(Vec<Value>),
    Invalid(CellId),
}

fn select(value: &RuntimeValue, cases: &RuntimeValue) -> Selection {
    match cases.list_values() {
        None => Selection::Invalid(vocabulary::INVALID_CASES),
        // The first case that matches, or isn't a case, decides.
        Some(mut elements) => elements
            .find_map(|case| {
                match (
                    case.field(vocabulary::PATTERN),
                    case.field(::grap::vocabulary::EXPRESSION),
                ) {
                    (Some(pattern), Some(expression)) => {
                        match destructure(&pattern.to_value(), value) {
                            Ok(Some(bindings)) => Some(Selection::Expression {
                                expression,
                                bindings,
                            }),
                            Ok(None) => None,
                            Err(InvalidBinder) => {
                                Some(Selection::Invalid(vocabulary::INVALID_BINDER))
                            }
                        }
                    }
                    _ => Some(Selection::Invalid(vocabulary::INVALID_CASE)),
                }
            })
            // Built only when nothing matched: most calls fail some cases
            // first.
            .unwrap_or_else(|| {
                Selection::NoMatch(
                    cases
                        .list_values()
                        .into_iter()
                        .flatten()
                        .filter_map(|case| case.field(vocabulary::PATTERN))
                        .map(|pattern| pattern_mismatch(&pattern.to_value()))
                        .collect(),
                )
            }),
    }
}

fn pattern_mismatch(pattern: &Value) -> Value {
    ::grap::absent::with_detail(
        vocabulary::PATTERN_MISMATCH,
        vocabulary::PATTERN,
        pattern.clone(),
    )
}

struct InvalidBinder;

fn destructure(
    pattern: &Value,
    value: &RuntimeValue,
) -> Result<Option<Vec<(CellId, RuntimeValue)>>, InvalidBinder> {
    let mut bindings = Vec::new();
    matches_pattern(pattern, value, &mut bindings).map(|matched| matched.then_some(bindings))
}

fn matches_pattern(
    pattern: &Value,
    value: &RuntimeValue,
    bindings: &mut Vec<(CellId, RuntimeValue)>,
) -> Result<bool, InvalidBinder> {
    match pattern {
        Value::Record(pattern_fields) => {
            if let Some(binder) = pattern_fields.get(&vocabulary::BIND) {
                match binder.as_cell() {
                    Some(binder) => match bindings.iter().find(|(bound, _)| *bound == binder) {
                        Some((_, bound)) => Ok(bound.to_value() == value.to_value()),
                        None => {
                            bindings.push((binder, value.clone()));
                            Ok(true)
                        }
                    },
                    None => Err(InvalidBinder),
                }
            } else if value.record_len().is_some() {
                pattern_fields
                    .iter()
                    .try_fold(true, |matched, (field, pattern)| {
                        if matched {
                            value
                                .field(*field)
                                .map(|value| matches_pattern(pattern, &value, bindings))
                                .unwrap_or(Ok(false))
                        } else {
                            Ok(false)
                        }
                    })
            } else {
                Ok(false)
            }
        }
        Value::List(pattern_values) => {
            if value.list_len() == Some(pattern_values.len()) {
                pattern_values
                    .values()
                    .enumerate()
                    .try_fold(true, |matched, (index, pattern)| {
                        if matched {
                            match value.list_get(index) {
                                Some(value) => matches_pattern(pattern, &value, bindings),
                                None => Ok(false),
                            }
                        } else {
                            Ok(false)
                        }
                    })
            } else {
                Ok(false)
            }
        }
        // Only plain data can equal an atom, so these compare it directly
        // rather than converting the value back into GID.
        Value::Cell(cell) => Ok(value.as_cell() == Some(*cell)),
        Value::Blob(_) => Ok(value.as_blob() == pattern.as_blob()),
    }
}

/// Match is a control form in projection even though evaluation sees
/// an ordinary call to the Rust implementation.
fn match_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(
        input,
        [
            ::grap::vocabulary::FUNCTION,
            vocabulary::VALUE,
            vocabulary::CASES,
        ],
        |[function, value, cases]| {
            (function?.as_cell()? == vocabulary::MATCH).then_some(())?;
            value?;
            cases?.list_len()?;
            let head = row(
                4.0,
                [
                    crate::libraries::grap::shallow_path(
                        [Step::Key(::grap::vocabulary::FUNCTION)],
                        &input.default_projection,
                    ),
                    crate::libraries::grap::expression_path(
                        [Step::Key(vocabulary::VALUE)],
                        &input.default_projection,
                    ),
                ],
            );
            Some(hug(
                head,
                descend(
                    [Step::Key(vocabulary::CASES)],
                    Projected::here(
                        crate::display::structure::list(Some(crate::display::runtime_partial(
                            case_display,
                        ))),
                        &input.default_projection,
                    ),
                ),
                4.0,
            ))
        },
    )
}

/// A pattern at `steps`, its binders declared there and in every pattern
/// nested below.
fn pattern_path<W: Host>(steps: impl AsRef<[Step]> + 'static, default: &Partial<W>) -> Layout<W> {
    crate::display::descend(
        steps,
        Projected::throughout(crate::display::runtime_partial(pattern_binder), default),
    )
}

/// A binder reads as the record it is, its cell declared; matching reads
/// only that field, but the others still show.
fn pattern_binder<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    input.value?.field(vocabulary::BIND)?.as_cell()?;
    crate::display::structure::Fields::default()
        .child(|key| {
            (key == vocabulary::BIND).then(|| {
                crate::display::runtime_partial(|input| {
                    crate::libraries::grap::declaration_cell(input, None)
                })
            })
        })
        .record(input)
}

fn case_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(
        input,
        [vocabulary::PATTERN, ::grap::vocabulary::EXPRESSION],
        |[pattern, expression]| {
            pattern?;
            expression?;
            let expression_site = input
                .site()
                .below(&[Step::Key(::grap::vocabulary::EXPRESSION)]);
            let arrow = activatable(crate::display::arrow(), &expression_site);
            Some(hug(
                crate::display::centered_row(
                    6.0,
                    [
                        pattern_path([Step::Key(vocabulary::PATTERN)], &input.default_projection),
                        arrow,
                    ],
                ),
                crate::libraries::grap::expression_path(
                    [Step::Key(::grap::vocabulary::EXPRESSION)],
                    &input.default_projection,
                ),
                6.0,
            ))
        },
    )
}

#[derive(Clone, Copy)]
enum BindingForm {
    Let,
    Where,
}

/// `let` and `where` are the same sequential binding call with two
/// arrangements. Keeping the function field visible makes switching
/// between the prefix and postfix forms an ordinary graph edit.
fn bindings_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(
        input,
        [
            ::grap::vocabulary::FUNCTION,
            vocabulary::BINDINGS,
            ::grap::vocabulary::EXPRESSION,
        ],
        |[function, bindings, expression]| {
            let form = match function?.as_cell()? {
                vocabulary::LET => Some(BindingForm::Let),
                vocabulary::WHERE => Some(BindingForm::Where),
                _ => None,
            }?;
            bindings?.list_len()?;
            expression?;
            Some(bindings_form(input, form))
        },
    )
}

/// `let`, its bindings before its expression, or `where`, after.
fn bindings_form<W: Host>(
    input: &ProjectionInput<'_, W, RuntimeValue>,
    form: BindingForm,
) -> Layout<W> {
    let bindings = shared(descend(
        [Step::Key(vocabulary::BINDINGS)],
        Projected::here(
            crate::display::structure::list(Some(crate::display::runtime_partial(binding_display))),
            &input.default_projection,
        ),
    ));
    let function = shared(crate::libraries::grap::shallow_path(
        [Step::Key(::grap::vocabulary::FUNCTION)],
        &input.default_projection,
    ));
    let expression = shared(crate::libraries::grap::expression_path(
        [Step::Key(::grap::vocabulary::EXPRESSION)],
        &input.default_projection,
    ));
    match form {
        BindingForm::Let => {
            let expression_site = input
                .site()
                .below(&[Step::Key(::grap::vocabulary::EXPRESSION)]);
            let in_marker = shared(activatable(dim("in"), &expression_site));
            alternatives([
                row(
                    4.0,
                    [
                        function.clone(),
                        bindings.clone(),
                        in_marker.clone(),
                        expression.clone(),
                    ],
                ),
                col(
                    0,
                    2.0,
                    [
                        hug(function, bindings, 4.0),
                        hug(in_marker, expression, 4.0),
                    ],
                ),
            ])
        }
        BindingForm::Where => alternatives([
            row(
                4.0,
                [expression.clone(), function.clone(), bindings.clone()],
            ),
            col(0, 2.0, [expression, hug(function, bindings, 4.0)]),
        ]),
    }
}

/// A binding binds a cell or a pattern, whichever it has. One of them being
/// added has no place in that form, so a plainer view takes it.
fn binding_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(
        input,
        [vocabulary::BIND, vocabulary::PATTERN, vocabulary::VALUE],
        |[binder, pattern, value]| {
            (!matches!(
                input.pending,
                Some(crate::display::Pending::Child(Step::Key(
                    vocabulary::BIND | vocabulary::PATTERN
                )))
            ))
            .then_some(())?;
            let left = match (binder, pattern) {
                (Some(binder), None) => binder.as_cell().map(|_| {
                    crate::libraries::grap::declaration_path(
                        [Step::Key(vocabulary::BIND)],
                        &input.default_projection,
                    )
                }),
                (None, Some(_)) => Some(pattern_path(
                    [Step::Key(vocabulary::PATTERN)],
                    &input.default_projection,
                )),
                _ => None,
            }?;
            value?;
            let value_site = input.site().below(&[Step::Key(vocabulary::VALUE)]);
            let equals = activatable(dim("="), &value_site);
            Some(hug(
                crate::display::centered_row(6.0, [left, equals]),
                crate::libraries::grap::expression_path(
                    [Step::Key(vocabulary::VALUE)],
                    &input.default_projection,
                ),
                6.0,
            ))
        },
    )
}

fn quote_marker<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    (input.value?.as_cell()? == vocabulary::QUOTE).then(|| {
        let site = input.site();
        activatable(dim("\""), &site)
    })
}

/// Quote reads as a small structural marker followed by its template,
/// rather than as a generic call with a redundant `expression` label.
/// Unrelated fields do not change recognition; they follow the template.
fn quote_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(
        input,
        [::grap::vocabulary::FUNCTION, ::grap::vocabulary::EXPRESSION],
        |[function, expression]| {
            (function?.as_cell()? == vocabulary::QUOTE).then_some(())?;
            expression?;
            let marker = descend(
                [Step::Key(::grap::vocabulary::FUNCTION)],
                Projected::here(
                    crate::display::runtime_partial(quote_marker),
                    &input.default_projection,
                ),
            );
            Some(row(
                2.0,
                [
                    marker,
                    crate::display::descend(
                        [Step::Key(::grap::vocabulary::EXPRESSION)],
                        Projected::ordinary(),
                    ),
                ],
            ))
        },
    )
}

/// Unquote marks an expression embedded in a quoted template. Its prefix
/// is a handle for the whole unquote; the body keeps its own stored location.
/// A just-inserted unquote is already one, so its empty hole offers code.
fn unquote_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    record_form(input, [vocabulary::UNQUOTE], |[unquote]| {
        match &input.pending {
            Some(crate::display::Pending::Child(Step::Key(vocabulary::UNQUOTE))) => {
                input.value?.record_len() == Some(0)
            }
            _ => unquote.is_some(),
        }
        .then_some(())?;
        let site = input.site();
        Some(row(
            2.0,
            [
                activatable(dim("`"), &site),
                crate::libraries::grap::expression_path(
                    [Step::Key(vocabulary::UNQUOTE)],
                    &input.default_projection,
                ),
            ],
        ))
    })
}

/// `do [a, b, c]` evaluates as a control form while retaining the
/// ordinary list projection for its ordered expressions.
fn do_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    expression_list_display(input, vocabulary::DO)
}

fn expression_list_display<W: Host>(
    input: &ProjectionInput<'_, W, RuntimeValue>,
    callable: CellId,
) -> Option<Layout<W>> {
    record_form(
        input,
        [::grap::vocabulary::FUNCTION, vocabulary::EXPRESSIONS],
        |[function, expressions]| {
            (function?.as_cell()? == callable).then_some(())?;
            expressions?.list_len()?;
            Some(row(
                4.0,
                [
                    crate::libraries::grap::shallow_path(
                        [Step::Key(::grap::vocabulary::FUNCTION)],
                        &input.default_projection,
                    ),
                    descend(
                        [Step::Key(vocabulary::EXPRESSIONS)],
                        Projected::here(
                            crate::display::structure::list(Some(crate::display::runtime_partial(
                                crate::libraries::grap::expression,
                            ))),
                            &input.default_projection,
                        ),
                    ),
                ],
            ))
        },
    )
}

/// What control adds to the Grap library: the names of its cells, its
/// functions, and its projection, which goes before Grap's calls'.
pub(super) fn contribution<W: Host>() -> (Vec<(CellId, &'static str)>, ForeignFunctions, Partial<W>)
{
    let names = vec![
        (vocabulary::MATCH, "match"),
        (vocabulary::VALUE, "value"),
        (vocabulary::CASES, "cases"),
        (vocabulary::PATTERN, "pattern"),
        (vocabulary::BIND, "bind"),
        (vocabulary::QUOTE, "quote"),
        (vocabulary::UNQUOTE, "unquote"),
        (vocabulary::LET, "let"),
        (vocabulary::WHERE, "where"),
        (vocabulary::BINDINGS, "bindings"),
        (vocabulary::DO, "do"),
        (vocabulary::ALL, "all"),
        (vocabulary::EXPRESSIONS, "expressions"),
        (vocabulary::INVALID_CASES, "invalid cases"),
        (vocabulary::INVALID_CASE, "invalid case"),
        (vocabulary::INVALID_BINDER, "invalid binder"),
        (vocabulary::INVALID_BINDINGS, "invalid bindings"),
        (vocabulary::INVALID_BINDING, "invalid binding"),
        (vocabulary::INVALID_EXPRESSIONS, "invalid expressions"),
        (vocabulary::PATTERN_MISMATCH, "pattern mismatch"),
        (
            vocabulary::MISSING_FINAL_EXPRESSION,
            "missing final expression",
        ),
    ];
    let projection = crate::display::fold::folding(crate::display::compose_partials([
        crate::display::runtime_partial(match_display),
        crate::display::runtime_partial(bindings_display),
        crate::display::runtime_partial(do_display),
        crate::display::runtime_partial(|input| expression_list_display(input, vocabulary::ALL)),
        crate::display::runtime_partial(quote_display),
        crate::display::runtime_partial(unquote_display),
    ]));
    (names, functions(), projection)
}

#[cfg(test)]
mod tests;
