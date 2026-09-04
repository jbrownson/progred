//! Grap evaluation control supplied by Rust functions. `match`
//! selects one case through structural matching, `let` and `where`
//! extend an environment through sequential bindings, `do` evaluates
//! expressions in order, and `quote` constructs data while evaluating
//! explicit unquotes.

use crate::{Library, absent, name};
use gid::{CellId, Cells, Step, Value};

pub const ID: CellId = CellId::from_u128(0xec17915df2d42377574dc90f22500fe2);
#[cfg(test)]
use grap_runtime as grap;
use grap_runtime::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue, Stage,
};
use progred_display::{
    Layout, ProjectionInput, activatable, alternatives, at_with_projection, centered_row, col, dim,
    hug, row, shared,
};
use std::rc::Rc;

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
    pub const EXPRESSIONS: CellId = CellId::from_u128(0x5fab151c006ae1487c28837f2003f43c);

    pub const INVALID_CASES: CellId = CellId::from_u128(0x1b94a59ed759da212fa72d7094796c6e);
    pub const INVALID_CASE: CellId = CellId::from_u128(0xaa627ebeb1091e8359f7a8eea45a6ccd);
    pub const INVALID_BINDER: CellId = CellId::from_u128(0x59ad0fb67728f245dce57b0cee360969);
    pub const INVALID_BINDINGS: CellId = CellId::from_u128(0x480ae287377249459a3438cb4b05229f);
    pub const INVALID_BINDING: CellId = CellId::from_u128(0x4ecae0db406e426aba1c02f2f04570ad);
    pub const INVALID_EXPRESSIONS: CellId = CellId::from_u128(0xa41a40f12691414971dbd9f788c291d6);
    pub const PATTERN_MISMATCH: CellId = CellId::from_u128(0xc44d3f1a71dea754a02bc14561a143b7);
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(vocabulary::MATCH, ForeignFunction::staged(match_prepare))
        .register(vocabulary::LET, ForeignFunction::staged(bindings_prepare))
        .register(vocabulary::WHERE, ForeignFunction::staged(bindings_prepare))
        .register(vocabulary::DO, ForeignFunction::runtime(do_foreign))
        .register(vocabulary::QUOTE, ForeignFunction::runtime(quote_foreign))
}

fn do_foreign(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expressions) = context.field(call, vocabulary::EXPRESSIONS) else {
        return Ok(context.missing_runtime_argument(vocabulary::EXPRESSIONS));
    };
    let Some(count) = context.elements(expressions).map(<[_]>::len) else {
        return Ok(absent::with_reason(vocabulary::INVALID_EXPRESSIONS).into());
    };
    let mut result = None;
    for index in 0..count {
        let expression = context.elements(expressions).unwrap()[index];
        result = Some(context.eval_runtime(expression, environment)?);
    }
    Ok(result.unwrap_or_else(|| absent::value().into()))
}

fn quote_foreign(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expression) = context.field(call, grap_runtime::vocabulary::EXPRESSION) else {
        return Ok(context.missing_runtime_argument(grap_runtime::vocabulary::EXPRESSION));
    };
    replace_unquotes(expression, context, environment)
}

fn replace_unquotes(
    expression: Expression,
    context: &mut Context,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    if let Some(field_count) = context.fields(expression).map(<[_]>::len) {
        let unquote = context.fields(expression).and_then(|fields| {
            fields
                .iter()
                .find(|(field, _)| *field == vocabulary::UNQUOTE)
                .map(|(_, unquote)| *unquote)
        });
        if let Some(unquote) = unquote {
            return context.eval_runtime(unquote, environment);
        }
        let mut replaced = Vec::with_capacity(field_count);
        for index in 0..field_count {
            let (field, value) = context.fields(expression).unwrap()[index];
            replaced.push((field, replace_unquotes(value, context, environment)?));
        }
        return Ok(RuntimeValue::record(replaced));
    }
    if let Some(element_count) = context.elements(expression).map(<[_]>::len) {
        let mut replaced = Vec::with_capacity(element_count);
        for index in 0..element_count {
            let value = context.elements(expression).unwrap()[index];
            replaced.push(replace_unquotes(value, context, environment)?);
        }
        return Ok(RuntimeValue::list(replaced));
    }
    let value = context.value(expression).clone();
    replace_unquotes_value(&value, context, environment)
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

fn match_prepare(context: &Context, call: Expression) -> Stage {
    let subject = context.field(call, vocabulary::VALUE);
    let compiled =
        context
            .field(call, vocabulary::CASES)
            .map(|cases| match context.elements(cases) {
                None => CompiledCases::Deferred(cases),
                Some(elements) => CompiledCases::Cases(
                    elements
                        .iter()
                        .map(|case| {
                            match (
                                context.field(*case, vocabulary::PATTERN),
                                context.field(*case, grap_runtime::vocabulary::EXPRESSION),
                            ) {
                                (Some(pattern), Some(expression)) => CompiledCase::Case {
                                    pattern: context.value(pattern).clone(),
                                    expression,
                                },
                                _ => CompiledCase::Malformed,
                            }
                        })
                        .collect(),
                ),
            });
    Rc::new(move |context, environment| {
        let Some(subject) = subject else {
            return Ok(context.missing_runtime_argument(vocabulary::VALUE));
        };
        let Some(compiled) = &compiled else {
            return Ok(context.missing_runtime_argument(vocabulary::CASES));
        };
        let value = context.eval_runtime(subject, environment)?;
        match compiled {
            CompiledCases::Cases(cases) => {
                // Selecting over prepared cases skips evaluating the
                // list expression; burn its fuel so the stage stays
                // burn-invisible.
                context.burn()?;
                let mut absents = Vec::new();
                for case in cases {
                    let CompiledCase::Case {
                        pattern,
                        expression,
                    } = case
                    else {
                        return Ok(absent::with_reason(vocabulary::INVALID_CASE).into());
                    };
                    match destructure_runtime(pattern, &value) {
                        Ok(Some(bindings)) => {
                            return context.eval_runtime(
                                *expression,
                                &environment.extended_runtime(bindings),
                            );
                        }
                        Ok(None) => absents.push(pattern_mismatch(pattern)),
                        Err(InvalidBinder) => {
                            return Ok(absent::with_reason(vocabulary::INVALID_BINDER).into());
                        }
                    }
                }
                Ok(absent::from_causes(absents).into())
            }
            CompiledCases::Deferred(cases) => {
                let cases_value = context.eval(*cases, environment)?;
                match select(&value.to_value(), &cases_value) {
                    Selection::Expression {
                        expression,
                        bindings,
                    } => context.eval_value_runtime(expression, &environment.extended(bindings)),
                    Selection::NoMatch(absents) => Ok(absent::from_causes(absents).into()),
                    Selection::Invalid(cell) => Ok(absent::with_reason(cell).into()),
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

fn bindings_prepare(context: &Context, call: Expression) -> Stage {
    let compiled = context.field(call, vocabulary::BINDINGS).map(|bindings| {
        match context.elements(bindings) {
            None => CompiledBindings::Deferred(bindings),
            Some(elements) => CompiledBindings::Bindings(
                elements
                    .iter()
                    .map(|binding| {
                        let Some(value) = context.field(*binding, vocabulary::VALUE) else {
                            return CompiledBinding::Malformed(vocabulary::INVALID_BINDING);
                        };
                        match (
                            context.field(*binding, vocabulary::BIND),
                            context.field(*binding, vocabulary::PATTERN),
                        ) {
                            (Some(binder), None) => match context.value(binder).as_cell() {
                                Some(binder) => CompiledBinding::Bind { binder, value },
                                None => CompiledBinding::Malformed(vocabulary::INVALID_BINDER),
                            },
                            (None, Some(pattern)) => CompiledBinding::Pattern {
                                pattern: context.value(pattern).clone(),
                                value,
                            },
                            _ => CompiledBinding::Malformed(vocabulary::INVALID_BINDING),
                        }
                    })
                    .collect(),
            ),
        }
    });
    let expression = context.field(call, grap_runtime::vocabulary::EXPRESSION);
    Rc::new(move |context, environment| {
        let Some(compiled) = &compiled else {
            return Ok(context.missing_runtime_argument(vocabulary::BINDINGS));
        };
        let Some(expression) = expression else {
            return Ok(context.missing_runtime_argument(grap_runtime::vocabulary::EXPRESSION));
        };
        match compiled {
            CompiledBindings::Bindings(bindings) => {
                // The prepared walk skips evaluating the bindings list
                // expression; burn its fuel so the stage stays
                // burn-invisible.
                context.burn()?;
                let mut environment = environment.clone();
                for binding in bindings {
                    match binding {
                        CompiledBinding::Malformed(reason) => {
                            return Ok(absent::with_reason(*reason).into());
                        }
                        CompiledBinding::Bind { binder, value } => {
                            let value = context.eval_runtime(*value, &environment)?;
                            environment.push_runtime([(*binder, value)]);
                        }
                        CompiledBinding::Pattern { pattern, value } => {
                            let value = context.eval_runtime(*value, &environment)?;
                            match destructure_runtime(pattern, &value) {
                                Ok(Some(bindings)) => environment.push_runtime(bindings),
                                Ok(None) => return Ok(absent::value().into()),
                                Err(InvalidBinder) => {
                                    return Ok(
                                        absent::with_reason(vocabulary::INVALID_BINDER).into()
                                    );
                                }
                            }
                        }
                    }
                }
                context.eval_runtime(expression, &environment)
            }
            CompiledBindings::Deferred(bindings) => {
                let bindings_value = context.eval(*bindings, environment)?;
                let Some(bindings) = bindings_value.as_list() else {
                    return Ok(absent::with_reason(vocabulary::INVALID_BINDINGS).into());
                };
                let mut environment = environment.clone();
                for binding in bindings.values() {
                    let Some(fields) = binding.as_record() else {
                        return Ok(absent::with_reason(vocabulary::INVALID_BINDING).into());
                    };
                    let Some(value) = fields.get(&vocabulary::VALUE) else {
                        return Ok(absent::with_reason(vocabulary::INVALID_BINDING).into());
                    };
                    let (binder, pattern) = match (
                        fields.get(&vocabulary::BIND),
                        fields.get(&vocabulary::PATTERN),
                    ) {
                        (Some(binder), None) => match binder.as_cell() {
                            Some(binder) => (Some(binder), None),
                            None => {
                                return Ok(absent::with_reason(vocabulary::INVALID_BINDER).into());
                            }
                        },
                        (None, Some(pattern)) => (None, Some(pattern)),
                        _ => {
                            return Ok(absent::with_reason(vocabulary::INVALID_BINDING).into());
                        }
                    };
                    let value = context.eval_value(value, &environment)?;
                    if let Some(binder) = binder {
                        environment = environment.extended([(binder, value)]);
                    } else if let Some(pattern) = pattern {
                        match destructure(pattern, &value) {
                            Ok(Some(bindings)) => environment = environment.extended(bindings),
                            Ok(None) => return Ok(absent::value().into()),
                            Err(InvalidBinder) => {
                                return Ok(absent::with_reason(vocabulary::INVALID_BINDER).into());
                            }
                        }
                    }
                }
                context.eval_runtime(expression, &environment)
            }
        }
    })
}

enum Selection<'a> {
    Expression {
        expression: &'a Value,
        bindings: Vec<(CellId, Value)>,
    },
    NoMatch(Vec<Value>),
    Invalid(CellId),
}

fn select<'a>(value: &Value, cases: &'a Value) -> Selection<'a> {
    let Some(cases) = cases.as_list() else {
        return Selection::Invalid(vocabulary::INVALID_CASES);
    };
    let mut absents = Vec::new();
    for case in cases.values() {
        let Some(fields) = case.as_record() else {
            return Selection::Invalid(vocabulary::INVALID_CASE);
        };
        let (Some(pattern), Some(expression)) = (
            fields.get(&vocabulary::PATTERN),
            fields.get(&grap_runtime::vocabulary::EXPRESSION),
        ) else {
            return Selection::Invalid(vocabulary::INVALID_CASE);
        };
        match destructure(pattern, value) {
            Ok(Some(bindings)) => {
                return Selection::Expression {
                    expression,
                    bindings,
                };
            }
            Ok(None) => absents.push(pattern_mismatch(pattern)),
            Err(InvalidBinder) => {
                return Selection::Invalid(vocabulary::INVALID_BINDER);
            }
        }
    }
    Selection::NoMatch(absents)
}

fn pattern_mismatch(pattern: &Value) -> Value {
    Value::record([
        (
            absent::vocabulary::ABSENT,
            Value::from(vocabulary::PATTERN_MISMATCH),
        ),
        (vocabulary::PATTERN, pattern.clone()),
    ])
}

struct InvalidBinder;

fn destructure(
    pattern: &Value,
    value: &Value,
) -> Result<Option<Vec<(CellId, Value)>>, InvalidBinder> {
    let mut bindings = Vec::new();
    matches_pattern(pattern, value, &mut bindings).map(|matched| matched.then_some(bindings))
}

fn destructure_runtime(
    pattern: &Value,
    value: &RuntimeValue,
) -> Result<Option<Vec<(CellId, RuntimeValue)>>, InvalidBinder> {
    let mut bindings = Vec::new();
    matches_runtime_pattern(pattern, value, &mut bindings)
        .map(|matched| matched.then_some(bindings))
}

fn matches_runtime_pattern(
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
                                .map(|value| matches_runtime_pattern(pattern, &value, bindings))
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
            let Some(value_len) = value.list_len() else {
                return Ok(false);
            };
            if pattern_values.len() != value_len {
                return Ok(false);
            }
            pattern_values
                .values()
                .enumerate()
                .try_fold(true, |matched, (index, pattern)| {
                    if matched {
                        match value.list_get(index) {
                            Some(value) => matches_runtime_pattern(pattern, &value, bindings),
                            None => Ok(false),
                        }
                    } else {
                        Ok(false)
                    }
                })
        }
        Value::Cell(_) | Value::Blob(_) => Ok(value.to_value() == *pattern),
    }
}

fn matches_pattern(
    pattern: &Value,
    value: &Value,
    bindings: &mut Vec<(CellId, Value)>,
) -> Result<bool, InvalidBinder> {
    match pattern {
        Value::Record(pattern_fields) => {
            if let Some(binder) = pattern_fields.get(&vocabulary::BIND) {
                match binder.as_cell() {
                    Some(binder) => match bindings.iter().find(|(bound, _)| *bound == binder) {
                        Some((_, bound)) => Ok(bound == value),
                        None => {
                            bindings.push((binder, value.clone()));
                            Ok(true)
                        }
                    },
                    None => Err(InvalidBinder),
                }
            } else if let Some(value_fields) = value.as_record() {
                pattern_fields
                    .iter()
                    .try_fold(true, |matched, (field, pattern)| {
                        if matched {
                            match value_fields.get(field) {
                                Some(value) => matches_pattern(pattern, value, bindings),
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
        Value::List(pattern_values) => match value.as_list() {
            Some(values) if pattern_values.len() == values.len() => pattern_values
                .values()
                .zip(values.values())
                .try_fold(true, |matched, (pattern, value)| {
                    if matched {
                        matches_pattern(pattern, value, bindings)
                    } else {
                        Ok(false)
                    }
                }),
            _ => Ok(false),
        },
        Value::Cell(_) | Value::Blob(_) => Ok(pattern == value),
    }
}

/// Match is a control form in projection even though evaluation sees
/// an ordinary call to the Rust implementation.
pub fn match_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&grap_runtime::vocabulary::FUNCTION)?;
    (function.as_cell()? == vocabulary::MATCH).then_some(())?;
    let subject = fields.get(&vocabulary::VALUE)?;
    let cases = fields.get(&vocabulary::CASES)?;
    cases.as_list()?;
    let head = row(
        4.0,
        [
            crate::grap::shallow_at([Step::Key(grap_runtime::vocabulary::FUNCTION)], function),
            crate::grap::expression_at([Step::Key(vocabulary::VALUE)], subject),
        ],
    );
    Some(hug(
        head,
        at_with_projection(
            [Step::Key(vocabulary::CASES)],
            cases,
            [progred_display::partial(case_display::<World, Hover>)],
        ),
        4.0,
        20.0,
    ))
}

fn case_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let (pattern, expression) = case_parts(input.value)?;
    let expression_target = input
        .targets
        .at([Step::Key(grap_runtime::vocabulary::EXPRESSION)]);
    let arrow = activatable(dim("→"), expression_target.hover, expression_target.select);
    Some(hug(
        centered_row(
            6.0,
            [
                crate::grap::deep_at([Step::Key(vocabulary::PATTERN)], pattern),
                arrow,
            ],
        ),
        crate::grap::expression_at(
            [Step::Key(grap_runtime::vocabulary::EXPRESSION)],
            expression,
        ),
        6.0,
        20.0,
    ))
}

fn case_parts(case: &Value) -> Option<(&Value, &Value)> {
    let fields = case.as_record()?;
    Some((
        fields.get(&vocabulary::PATTERN)?,
        fields.get(&grap_runtime::vocabulary::EXPRESSION)?,
    ))
}

#[derive(Clone, Copy)]
enum BindingForm {
    Let,
    Where,
}

/// `let` and `where` are the same sequential binding call with two
/// arrangements. Keeping the function field visible makes switching
/// between the prefix and postfix forms an ordinary graph edit.
pub fn bindings_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&grap_runtime::vocabulary::FUNCTION)?;
    let form = match function.as_cell()? {
        vocabulary::LET => BindingForm::Let,
        vocabulary::WHERE => BindingForm::Where,
        _ => return None,
    };
    let bindings = fields.get(&vocabulary::BINDINGS)?;
    bindings.as_list()?;
    let expression = fields.get(&grap_runtime::vocabulary::EXPRESSION)?;
    let bindings = shared(at_with_projection(
        [Step::Key(vocabulary::BINDINGS)],
        bindings,
        [progred_display::partial(binding_display::<World, Hover>)],
    ));
    let function = shared(crate::grap::shallow_at(
        [Step::Key(grap_runtime::vocabulary::FUNCTION)],
        function,
    ));
    let expression = shared(crate::grap::expression_at(
        [Step::Key(grap_runtime::vocabulary::EXPRESSION)],
        expression,
    ));
    match form {
        BindingForm::Let => {
            let expression_target = input
                .targets
                .at([Step::Key(grap_runtime::vocabulary::EXPRESSION)]);
            let in_marker = shared(activatable(
                dim("in"),
                expression_target.hover,
                expression_target.select,
            ));
            Some(alternatives([
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
                        hug(function, bindings, 4.0, 20.0),
                        hug(in_marker, expression, 4.0, 20.0),
                    ],
                ),
            ]))
        }
        BindingForm::Where => Some(alternatives([
            row(
                4.0,
                [expression.clone(), function.clone(), bindings.clone()],
            ),
            col(0, 2.0, [expression, hug(function, bindings, 4.0, 20.0)]),
        ])),
    }
}

fn binding_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let left = match (
        fields.get(&vocabulary::BIND),
        fields.get(&vocabulary::PATTERN),
    ) {
        (Some(binder), None) => {
            binder.as_cell()?;
            crate::grap::deep_at([Step::Key(vocabulary::BIND)], binder)
        }
        (None, Some(pattern)) => crate::grap::deep_at([Step::Key(vocabulary::PATTERN)], pattern),
        _ => return None,
    };
    let value = fields.get(&vocabulary::VALUE)?;
    let value_target = input.targets.at([Step::Key(vocabulary::VALUE)]);
    let equals = activatable(dim("="), value_target.hover, value_target.select);
    Some(hug(
        centered_row(6.0, [left, equals]),
        crate::grap::expression_at([Step::Key(vocabulary::VALUE)], value),
        6.0,
        20.0,
    ))
}

fn quote_marker<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    (input.value.as_cell()? == vocabulary::QUOTE).then(|| {
        let target = input.targets.current();
        activatable(dim("\""), target.hover, target.select)
    })
}

/// Quote reads as a small structural marker followed by its template,
/// rather than as a generic call with a redundant `expression` label.
/// Decorated calls fall through so this compact form never hides data.
pub fn quote_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    (fields.len() == 2).then_some(())?;
    let function = fields.get(&grap_runtime::vocabulary::FUNCTION)?;
    (function.as_cell()? == vocabulary::QUOTE).then_some(())?;
    let expression = fields.get(&grap_runtime::vocabulary::EXPRESSION)?;
    let marker = at_with_projection(
        [Step::Key(grap_runtime::vocabulary::FUNCTION)],
        function,
        [progred_display::partial(quote_marker::<World, Hover>)],
    );
    Some(row(
        2.0,
        [
            marker,
            crate::grap::deep_at(
                [Step::Key(grap_runtime::vocabulary::EXPRESSION)],
                expression,
            ),
        ],
    ))
}

/// `do [a, b, c]` evaluates as a control form while retaining the
/// ordinary list projection for its ordered expressions.
pub fn do_display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    (fields.len() == 2).then_some(())?;
    let function = fields.get(&grap_runtime::vocabulary::FUNCTION)?;
    (function.as_cell()? == vocabulary::DO).then_some(())?;
    let expressions = fields.get(&vocabulary::EXPRESSIONS)?;
    expressions.as_list()?;
    Some(row(
        4.0,
        [
            crate::grap::shallow_at([Step::Key(grap_runtime::vocabulary::FUNCTION)], function),
            crate::grap::shallow_at([Step::Key(vocabulary::EXPRESSIONS)], expressions),
        ],
    ))
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, name) in [
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
        (vocabulary::EXPRESSIONS, "expressions"),
    ] {
        cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::INVALID_CASES, "invalid cases"),
        (vocabulary::INVALID_CASE, "invalid case"),
        (vocabulary::INVALID_BINDER, "invalid binder"),
        (vocabulary::INVALID_BINDINGS, "invalid bindings"),
        (vocabulary::INVALID_BINDING, "invalid binding"),
        (vocabulary::INVALID_EXPRESSIONS, "invalid expressions"),
        (vocabulary::PATTERN_MISMATCH, "pattern mismatch"),
    ] {
        cells.set_value(cell, absent::named_reason(name));
    }
    Library::named(
        "control",
        crate::Definitions::from_parts(cells, functions()),
        vec![
            progred_display::partial(match_display::<World, Hover>),
            progred_display::partial(bindings_display::<World, Hover>),
            progred_display::partial(do_display::<World, Hover>),
            progred_display::partial(quote_display::<World, Hover>),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use progred_display::Env;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct NoEval;

    impl Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, expression: &Value) -> (Value, usize) {
            (expression.clone(), 0)
        }
    }

    fn unit_target(_: Vec<Step>) -> progred_display::ProjectionTarget<(), ()> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: (),
        }
    }

    fn relative_target(steps: Vec<Step>) -> progred_display::ProjectionTarget<(), Vec<Step>> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: steps,
        }
    }

    fn projection_input(value: &Value) -> ProjectionInput<'_, (), ()> {
        ProjectionInput {
            env: &NoEval,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&unit_target),
        }
    }

    fn relative_projection_input(value: &Value) -> ProjectionInput<'_, (), Vec<Step>> {
        ProjectionInput {
            env: &NoEval,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&relative_target),
        }
    }

    fn blob(text: &str) -> Value {
        Value::from(text.as_bytes().to_vec())
    }

    fn binding(cell: CellId) -> Value {
        Value::record([(vocabulary::BIND, Value::from(cell))])
    }

    fn case_arm(pattern: Value, expression: Value) -> Value {
        Value::record([
            (vocabulary::PATTERN, pattern),
            (grap::vocabulary::EXPRESSION, expression),
        ])
    }

    fn binding_clause(pattern: Value, value: Value) -> Value {
        Value::record([(vocabulary::PATTERN, pattern), (vocabulary::VALUE, value)])
    }

    fn bind_clause(binder: CellId, value: Value) -> Value {
        Value::record([
            (vocabulary::BIND, Value::from(binder)),
            (vocabulary::VALUE, value),
        ])
    }

    fn bindings_call(
        function: CellId,
        bindings: impl IntoIterator<Item = Value>,
        expression: Value,
    ) -> Value {
        grap::call(
            Value::from(function),
            [
                (vocabulary::BINDINGS, Value::list(bindings)),
                (grap::vocabulary::EXPRESSION, expression),
            ],
        )
    }

    fn match_call(value: Value, cases: impl IntoIterator<Item = Value>) -> Value {
        grap::call(
            Value::from(vocabulary::MATCH),
            [
                (vocabulary::VALUE, value),
                (vocabulary::CASES, Value::list(cases)),
            ],
        )
    }

    fn quote_call(expression: Value) -> Value {
        grap::call(
            Value::from(vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, expression)],
        )
    }

    fn do_call(expressions: impl IntoIterator<Item = Value>) -> Value {
        grap::call(
            Value::from(vocabulary::DO),
            [(vocabulary::EXPRESSIONS, Value::list(expressions))],
        )
    }

    fn evaluate(expression: &Value) -> grap::Evaluation {
        crate::test_evaluate(expression, |_| None, &functions(), 100)
    }

    fn evaluate_with_fuel(expression: &Value, fuel: usize) -> grap::Evaluation {
        crate::test_evaluate(expression, |_| None, &functions(), fuel)
    }

    #[test]
    fn do_evaluates_in_order_and_returns_the_last_result() {
        let first = new_cell_id();
        let second = new_cell_id();
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let first_calls = calls.clone();
        let second_calls = calls.clone();
        let functions = functions()
            .register(
                first,
                ForeignFunction::new(move |_, _, _| {
                    first_calls.borrow_mut().push(first);
                    Ok(blob("first"))
                }),
            )
            .register(
                second,
                ForeignFunction::new(move |_, _, _| {
                    second_calls.borrow_mut().push(second);
                    Ok(blob("second"))
                }),
            );
        let expression = do_call([
            grap::call(Value::from(first), []),
            grap::call(Value::from(second), []),
        ]);
        let result = crate::test_evaluate(&expression, |_| None, &functions, 100).result;

        assert_eq!(&*calls.borrow(), &[first, second]);
        assert_eq!(result, blob("second"));
        assert!(absent::is_absent(&evaluate(&do_call([])).result));
    }

    #[test]
    fn do_projects_its_expression_list_without_hiding_extra_data() {
        let expression = do_call([blob("first"), blob("second")]);
        let Layout::Row { children, .. } =
            do_display(&relative_projection_input(&expression)).expect("coherent do projection")
        else {
            panic!("do is its marker followed by a list")
        };
        assert_eq!(children.len(), 2);

        let decorated = expression
            .as_record()
            .unwrap()
            .update(new_cell_id(), blob("extra"));
        assert!(do_display(&relative_projection_input(&Value::record(decorated))).is_none());
    }

    #[test]
    fn quote_returns_recognized_expressions_as_data() {
        let function = new_cell_id();
        let expression = grap::call(Value::from(function), []);
        let evaluation = evaluate(&quote_call(expression.clone()));
        assert_eq!(evaluation.result, expression);
    }

    #[test]
    fn quote_projects_a_selectable_marker_and_its_expression() {
        let expression = blob("body");
        let quoted = quote_call(expression.clone());
        let layout = quote_display(&projection_input(&quoted)).unwrap();
        let Layout::Row { children, .. } = &layout else {
            panic!("quote is an inline prefix");
        };
        let [marker, body] = children.as_slice() else {
            panic!("quote has a marker and expression");
        };
        let Layout::At {
            steps,
            projection: Some(projection),
            ..
        } = marker
        else {
            panic!("the marker retains the function-field location");
        };
        assert_eq!(steps, &[Step::Key(grap::vocabulary::FUNCTION)]);
        assert_eq!(projection.len(), 1);

        let Layout::At { steps, value, .. } = body else {
            panic!("the expression retains its field location");
        };
        assert_eq!(steps, &[Step::Key(grap::vocabulary::EXPRESSION)]);
        assert_eq!(value, &expression);

        let marker = quote_marker(&projection_input(&Value::from(vocabulary::QUOTE))).unwrap();
        let Layout::OnHover { child, .. } = marker else {
            panic!("the marker claims hover");
        };
        let Layout::OnActivate { child, .. } = *child else {
            panic!("the marker remains selectable");
        };
        let Layout::Leaf(puri::Leaf::Text { text, paint: face }) = *child else {
            panic!("the marker is text");
        };
        assert_eq!(text, "\"");
        assert!(matches!(
            face,
            progred_display::Paint::Face(progred_display::Face::Dim)
        ));
    }

    #[test]
    fn decorated_quotes_fall_through_instead_of_hiding_fields() {
        let extra = new_cell_id();
        let quote = grap::call(
            Value::from(vocabulary::QUOTE),
            [
                (grap::vocabulary::EXPRESSION, blob("body")),
                (extra, blob("visible")),
            ],
        );
        assert!(quote_display(&projection_input(&quote)).is_none());
    }

    #[test]
    fn quote_replaces_unquotes_in_its_calling_environment() {
        let parameter = new_cell_id();
        let field = new_cell_id();
        let template = Value::list([Value::record([(
            field,
            Value::record([(vocabulary::UNQUOTE, Value::from(parameter))]),
        )])]);
        let positions = template
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        let expression = grap::call(
            grap::lambda([parameter], quote_call(template)),
            [(parameter, blob("spliced"))],
        );
        let evaluation = evaluate(&expression);
        assert_eq!(
            evaluation.result,
            Value::list([Value::record([(field, blob("spliced"))])])
        );
        assert_eq!(
            evaluation
                .result
                .as_list()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            positions
        );
    }

    #[test]
    fn unquote_is_ordinary_data_outside_quote() {
        let missing = new_cell_id();
        let expression = Value::record([(vocabulary::UNQUOTE, Value::from(missing))]);
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, expression);

        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn quote_does_not_revisit_an_unquoted_result() {
        let missing = new_cell_id();
        let inner = Value::record([(vocabulary::UNQUOTE, Value::from(missing))]);
        let expression = quote_call(Value::record([(vocabulary::UNQUOTE, inner.clone())]));
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, inner);

        assert_eq!(evaluation.dependencies, [vocabulary::QUOTE].into());
    }

    #[test]
    fn match_evaluates_the_first_matching_case_with_open_record_bindings() {
        let first = new_cell_id();
        let last = new_cell_id();
        let metadata = new_cell_id();
        let name = new_cell_id();
        let never = new_cell_id();
        let expression = match_call(
            Value::record([
                (first, blob("Ada")),
                (last, blob("Lovelace")),
                (metadata, blob("extra")),
            ]),
            [
                case_arm(Value::record([(first, blob("Grace"))]), Value::from(never)),
                case_arm(Value::record([(first, binding(name))]), Value::from(name)),
            ],
        );
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, blob("Ada"));
    }

    #[test]
    fn match_evaluates_its_subject_once_across_all_cases() {
        static SUBJECT_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

        let subject = new_cell_id();
        let foreign = functions().register(
            subject,
            ForeignFunction::new(|_, _, _| {
                SUBJECT_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
                Ok(blob("subject"))
            }),
        );
        SUBJECT_EVALUATIONS.store(0, Ordering::SeqCst);

        let expression = match_call(
            grap::call(Value::from(subject), []),
            [
                case_arm(blob("first"), blob("first result")),
                case_arm(blob("second"), blob("second result")),
            ],
        );
        let evaluation = crate::test_evaluate(&expression, |_| None, &foreign, 100);
        assert_eq!(
            evaluation.result,
            absent::from_causes([
                pattern_mismatch(&blob("first")),
                pattern_mismatch(&blob("second")),
            ]),
        );

        assert_eq!(SUBJECT_EVALUATIONS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn lowered_match_cases_still_consume_their_evaluation_fuel() {
        let expression = match_call(blob("subject"), [case_arm(blob("subject"), blob("result"))]);

        let completed = evaluate_with_fuel(&expression, 6);
        assert_eq!(completed.result, blob("result"));
        assert_eq!(completed.remaining_fuel, 1);

        let exhausted = evaluate_with_fuel(&expression, 5);
        assert_eq!(
            exhausted.result,
            grap::absent::value(grap::absent::FUEL_EXHAUSTED),
        );
        assert_eq!(exhausted.remaining_fuel, 0);
    }

    #[test]
    fn match_patterns_match_exact_lists_and_repeated_binders_must_agree() {
        let element = new_cell_id();
        let other = new_cell_id();
        let expression = match_call(
            Value::list([blob("left"), blob("right")]),
            [
                case_arm(
                    Value::list([binding(element), binding(element)]),
                    blob("repeated"),
                ),
                case_arm(
                    Value::list([binding(element), binding(other)]),
                    Value::from(other),
                ),
            ],
        );
        assert_eq!(evaluate(&expression).result, blob("right"));

        let too_short = match_call(
            Value::list([blob("only")]),
            [case_arm(
                Value::list([binding(element), binding(other)]),
                blob("matched"),
            )],
        );
        assert_eq!(
            evaluate(&too_short).result,
            pattern_mismatch(&Value::list([binding(element), binding(other),])),
        );
    }

    #[test]
    fn first_pattern_wins_even_when_its_expression_returns_an_absent() {
        let missing = new_cell_id();
        let expression = match_call(
            Value::record([]),
            [
                case_arm(Value::record([]), Value::from(missing)),
                case_arm(Value::record([]), blob("second")),
            ],
        );
        assert_eq!(
            evaluate(&expression).result,
            grap::absent::with_detail(
                grap::absent::MISSING_CELL,
                grap::absent::CELL,
                missing.into()
            )
        );
    }

    #[test]
    fn let_and_where_evaluate_bindings_sequentially() {
        let first = new_cell_id();
        let second = new_cell_id();
        let bindings = || {
            [
                bind_clause(first, blob("bound")),
                bind_clause(second, Value::from(first)),
            ]
        };
        for function in [vocabulary::LET, vocabulary::WHERE] {
            let expression = bindings_call(function, bindings(), Value::from(second));
            let evaluation = evaluate(&expression);
            assert_eq!(evaluation.result, blob("bound"));
        }
    }

    #[test]
    fn lowered_bindings_still_consume_their_evaluation_fuel() {
        let binder = new_cell_id();
        let expression = bindings_call(
            vocabulary::LET,
            [bind_clause(binder, blob("bound"))],
            Value::from(binder),
        );

        let completed = evaluate_with_fuel(&expression, 6);
        assert_eq!(completed.result, blob("bound"));
        assert_eq!(completed.remaining_fuel, 1);

        let exhausted = evaluate_with_fuel(&expression, 5);
        assert_eq!(
            exhausted.result,
            grap::absent::value(grap::absent::FUEL_EXHAUSTED),
        );
        assert_eq!(exhausted.remaining_fuel, 0);
    }

    #[test]
    fn let_destructures_each_value_and_returns_an_absent_on_mismatch() {
        let field = new_cell_id();
        let binder = new_cell_id();
        let matched = bindings_call(
            vocabulary::LET,
            [binding_clause(
                Value::record([(field, binding(binder))]),
                Value::record([(field, blob("inside")), (new_cell_id(), blob("extra"))]),
            )],
            Value::from(binder),
        );
        assert_eq!(evaluate(&matched).result, blob("inside"));

        let unmatched = bindings_call(
            vocabulary::LET,
            [binding_clause(blob("expected"), blob("actual"))],
            blob("never"),
        );
        assert_eq!(evaluate(&unmatched).result, absent::value());
    }

    #[test]
    fn let_and_where_share_a_structure_but_project_in_opposite_orders() {
        let binder = new_cell_id();
        let clauses = || [bind_clause(binder, blob("value"))];
        let body = blob("body");
        let let_call = bindings_call(vocabulary::LET, clauses(), body.clone());
        let where_call = bindings_call(vocabulary::WHERE, clauses(), body.clone());

        let let_layout = bindings_display(&relative_projection_input(&let_call)).unwrap();
        let Layout::Alternatives(let_options) = let_layout else {
            panic!("let has responsive forms");
        };
        let Layout::Row {
            children: let_children,
            ..
        } = &let_options[0]
        else {
            panic!("flat let first");
        };
        assert_eq!(let_children.len(), 4);
        let Layout::Shared { child, .. } = &let_children[2] else {
            panic!("let shares its in marker");
        };
        let Layout::OnHover { hover, .. } = child.as_ref() else {
            panic!("in targets the body");
        };
        assert_eq!(
            hover.as_deref(),
            Some(&[Step::Key(grap::vocabulary::EXPRESSION)][..])
        );

        let Layout::Shared {
            child: bindings, ..
        } = &let_children[1]
        else {
            panic!("let shares its bindings");
        };
        let Layout::At {
            steps,
            projection: Some(projection),
            ..
        } = bindings.as_ref()
        else {
            panic!("let descends to its bindings list");
        };
        assert_eq!(steps, &[Step::Key(vocabulary::BINDINGS)]);
        assert_eq!(projection.len(), 1);

        let where_layout = bindings_display(&relative_projection_input(&where_call)).unwrap();
        let Layout::Alternatives(where_options) = where_layout else {
            panic!("where has responsive forms");
        };
        let Layout::Row { children, .. } = &where_options[0] else {
            panic!("flat where first");
        };
        let Layout::Shared { child, .. } = &children[0] else {
            panic!("where starts with its body");
        };
        assert!(matches!(
            child.as_ref(),
            Layout::At { steps, value, .. }
                if *steps == [Step::Key(grap::vocabulary::EXPRESSION)] && *value == body
        ));
    }

    #[test]
    fn direct_binding_is_deep_and_centered_beside_its_equals() {
        let binder = new_cell_id();
        let binding = bind_clause(binder, blob("value"));
        let layout = binding_display(&relative_projection_input(&binding)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("a binding has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("a flat binding first");
        };
        let Layout::Shared { child, .. } = &children[0] else {
            panic!("a binding shares its pattern and equals");
        };
        let Layout::Row {
            alignment: progred_display::RowAlignment::Center,
            children,
            ..
        } = child.as_ref()
        else {
            panic!("a binding head is vertically centered");
        };
        assert!(matches!(
            &children[0],
            Layout::At {
                steps,
                projection: Some(projection),
                ..
            } if *steps == [Step::Key(vocabulary::BIND)] && projection.len() == 1
        ));
    }

    #[test]
    fn match_augments_its_standard_cases_list_with_the_case_projection() {
        let binder = new_cell_id();
        let expression = match_call(
            blob("subject"),
            [
                case_arm(blob("first"), blob("one")),
                case_arm(binding(binder), Value::from(binder)),
            ],
        );
        let layout = match_display(&projection_input(&expression)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("match has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat match first");
        };
        let mut arms = &children[1];
        while let Layout::Shared { child, .. } = arms {
            arms = child.as_ref();
        }
        let Layout::At {
            steps,
            projection: Some(projection),
            value: cases,
            ..
        } = arms
        else {
            panic!("match descends to its cases list");
        };
        assert_eq!(steps, &[Step::Key(vocabulary::CASES)]);
        assert_eq!(projection.len(), 1);

        assert!(case_display(&relative_projection_input(cases)).is_none());
        let case = cases
            .as_list()
            .and_then(|cases| cases.values().next())
            .expect("the standard list contains its cases");
        let case = case_display(&relative_projection_input(case)).unwrap();
        let Layout::Alternatives(options) = case else {
            panic!("a case has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("a flat case is a row");
        };
        let Layout::Shared { child, .. } = &children[0] else {
            panic!("a case shares its head");
        };
        let Layout::Row { children, .. } = child.as_ref() else {
            panic!("a case head contains its pattern and arrow");
        };
        let Layout::OnHover { child, hover } = &children[1] else {
            panic!("the arrow claims the case hover");
        };
        assert_eq!(
            hover.as_deref(),
            Some(&[Step::Key(grap::vocabulary::EXPRESSION)][..])
        );
        assert!(matches!(child.as_ref(), Layout::OnActivate { .. }));
    }

    #[test]
    fn non_case_elements_use_the_standard_projection_inside_match() {
        let malformed = match_call(blob("subject"), [blob("not a case")]);
        assert!(match_display(&projection_input(&malformed)).is_some());
        let case = malformed
            .as_record()
            .and_then(|fields| fields.get(&vocabulary::CASES))
            .and_then(Value::as_list)
            .and_then(|cases| cases.values().next())
            .unwrap();
        assert!(case_display(&projection_input(case)).is_none());
    }

    #[test]
    fn malformed_match_data_returns_library_absents() {
        let invalid_cases = grap::call(
            Value::from(vocabulary::MATCH),
            [
                (vocabulary::VALUE, blob("subject")),
                (vocabulary::CASES, blob("not a list")),
            ],
        );
        assert_eq!(
            evaluate(&invalid_cases).result,
            absent::with_reason(vocabulary::INVALID_CASES)
        );

        let invalid_case = match_call(blob("subject"), [blob("not a case")]);
        assert_eq!(
            evaluate(&invalid_case).result,
            absent::with_reason(vocabulary::INVALID_CASE)
        );

        let invalid_binder = match_call(
            blob("subject"),
            [case_arm(
                Value::record([(vocabulary::BIND, blob("not a cell"))]),
                blob("matched"),
            )],
        );
        assert_eq!(
            evaluate(&invalid_binder).result,
            absent::with_reason(vocabulary::INVALID_BINDER)
        );
    }

    #[test]
    fn malformed_let_data_returns_library_absents() {
        let invalid_bindings = grap::call(
            Value::from(vocabulary::LET),
            [
                (vocabulary::BINDINGS, blob("not a list")),
                (grap::vocabulary::EXPRESSION, blob("body")),
            ],
        );
        assert_eq!(
            evaluate(&invalid_bindings).result,
            absent::with_reason(vocabulary::INVALID_BINDINGS)
        );

        let invalid_binding = bindings_call(vocabulary::LET, [blob("not a binding")], blob("body"));
        assert_eq!(
            evaluate(&invalid_binding).result,
            absent::with_reason(vocabulary::INVALID_BINDING)
        );

        let ambiguous = bindings_call(
            vocabulary::LET,
            [Value::record([
                (vocabulary::BIND, Value::from(new_cell_id())),
                (vocabulary::PATTERN, blob("pattern")),
                (vocabulary::VALUE, blob("value")),
            ])],
            blob("body"),
        );
        assert_eq!(
            evaluate(&ambiguous).result,
            absent::with_reason(vocabulary::INVALID_BINDING)
        );

        let invalid_binder = bindings_call(
            vocabulary::LET,
            [Value::record([
                (vocabulary::BIND, blob("not a cell")),
                (vocabulary::VALUE, blob("value")),
            ])],
            blob("body"),
        );
        assert_eq!(
            evaluate(&invalid_binder).result,
            absent::with_reason(vocabulary::INVALID_BINDER)
        );
    }

    #[test]
    fn library_describes_control_forms_and_absence_reasons() {
        let library = library::<(), ()>();
        assert_eq!(library.projections.len(), 4);
        assert_eq!(
            library.value(vocabulary::QUOTE).and_then(name::read),
            Some("quote")
        );
        assert_eq!(
            library.value(vocabulary::UNQUOTE).and_then(name::read),
            Some("unquote")
        );
        assert_eq!(
            library.value(vocabulary::DO).and_then(name::read),
            Some("do")
        );
        for cell in [
            vocabulary::INVALID_CASES,
            vocabulary::INVALID_CASE,
            vocabulary::INVALID_BINDER,
            vocabulary::INVALID_BINDINGS,
            vocabulary::INVALID_BINDING,
            vocabulary::INVALID_EXPRESSIONS,
        ] {
            assert!(name::read(library.value(cell).unwrap()).is_some());
            assert_eq!(absent::reason(&absent::with_reason(cell)), Some(cell));
        }
    }
}
