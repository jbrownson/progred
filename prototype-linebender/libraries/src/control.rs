//! Grap evaluation control supplied by Rust functions. `case`
//! selects one expression through structural matching; `quote`
//! constructs data while evaluating explicit unquotes.

use crate::{Library, absent, name};
use gid::{CellId, Cells, Step, Value};
#[cfg(test)]
use grap_runtime as grap;
use grap_runtime::{Context, Environment, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{
    Delim, Layout, ProjectionInput, bracket, col, dim, hug, row,
};
use std::collections::BTreeMap;

pub mod vocabulary {
    use gid::CellId;

    pub const CASE: CellId = CellId::from_u128(0xb3f6a62e4926889bcfcd338025f4a6f9);
    pub const VALUE: CellId = CellId::from_u128(0x00dafdc01c7e014edd857e174c6c8b6f);
    pub const ALTERNATIVES: CellId = CellId::from_u128(0xa9aadcb44755963498c743fa114f0f00);
    pub const DEFAULT: CellId = CellId::from_u128(0x3ad1a352453c8b21da757b48913b2c9f);
    pub const PATTERN: CellId = CellId::from_u128(0xb9dc97198709bc6f7bae4c7afc7d424f);
    pub const BIND: CellId = CellId::from_u128(0x5e46d12705690e8a377eb0f16ad9dba6);
    pub const QUOTE: CellId = CellId::from_u128(0x7f81d4812ceb33d4222e9e5cb9c82497);
    pub const UNQUOTE: CellId = CellId::from_u128(0xda48703c290e3b35d7353c38110bc953);

    pub const INVALID_ALTERNATIVES: CellId = CellId::from_u128(0x1b94a59ed759da212fa72d7094796c6e);
    pub const INVALID_ALTERNATIVE: CellId = CellId::from_u128(0xaa627ebeb1091e8359f7a8eea45a6ccd);
    pub const INVALID_BINDER: CellId = CellId::from_u128(0x59ad0fb67728f245dce57b0cee360969);
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(vocabulary::CASE, ForeignFunction::new(case_foreign))
        .register(vocabulary::QUOTE, ForeignFunction::new(quote_foreign))
}

fn quote_foreign(
    context: &mut Context,
    call: &Value,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(expression) = context.field(call, grap_runtime::vocabulary::EXPRESSION) else {
        return Ok(context.missing_argument(grap_runtime::vocabulary::EXPRESSION));
    };
    replace_unquotes(expression, context, environment)
}

fn replace_unquotes(
    value: &Value,
    context: &mut Context,
    environment: &Environment,
) -> Result<Value, Halt> {
    match value {
        Value::Record(fields) => match fields.get(&vocabulary::UNQUOTE) {
            Some(expression) => context.eval(expression, environment),
            None => Ok(Value::Record(
                fields
                    .iter()
                    .map(|(field, value)| {
                        Ok((*field, replace_unquotes(value, context, environment)?))
                    })
                    .collect::<Result<_, Halt>>()?,
            )),
        },
        Value::List(values) => Ok(Value::List(
            values
                .iter()
                .map(|(position, value)| {
                    Ok((
                        position.clone(),
                        replace_unquotes(value, context, environment)?,
                    ))
                })
                .collect::<Result<_, Halt>>()?,
        )),
        Value::Cell(_) | Value::Blob(_) => Ok(value.clone()),
    }
}

fn case_foreign(
    context: &mut Context,
    call: &Value,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(value) = context.field(call, vocabulary::VALUE) else {
        return Ok(context.missing_argument(vocabulary::VALUE));
    };
    let Some(alternatives) = context.field(call, vocabulary::ALTERNATIVES) else {
        return Ok(context.missing_argument(vocabulary::ALTERNATIVES));
    };
    let Some(default) = context.field(call, vocabulary::DEFAULT) else {
        return Ok(context.missing_argument(vocabulary::DEFAULT));
    };
    let value = context.eval(value, environment)?;
    let alternatives = context.eval(alternatives, environment)?;
    match select(&value, &alternatives) {
        Selection::Expression {
            expression,
            bindings,
        } => context.eval(expression, &environment.extended(bindings)),
        Selection::Default => context.eval(default, environment),
        Selection::Invalid(cell) => Ok(Value::from(cell)),
    }
}

enum Selection<'a> {
    Expression {
        expression: &'a Value,
        bindings: BTreeMap<CellId, Value>,
    },
    Default,
    Invalid(CellId),
}

fn select<'a>(value: &Value, alternatives: &'a Value) -> Selection<'a> {
    let Some(alternatives) = alternatives.as_list() else {
        return Selection::Invalid(vocabulary::INVALID_ALTERNATIVES);
    };
    for alternative in alternatives.values() {
        let Some(fields) = alternative.as_record() else {
            return Selection::Invalid(vocabulary::INVALID_ALTERNATIVE);
        };
        let (Some(pattern), Some(expression)) = (
            fields.get(&vocabulary::PATTERN),
            fields.get(&grap_runtime::vocabulary::EXPRESSION),
        ) else {
            return Selection::Invalid(vocabulary::INVALID_ALTERNATIVE);
        };
        match destructure(pattern, value) {
            Ok(Some(bindings)) => {
                return Selection::Expression {
                    expression,
                    bindings,
                };
            }
            Ok(None) => {}
            Err(InvalidBinder) => {
                return Selection::Invalid(vocabulary::INVALID_BINDER);
            }
        }
    }
    Selection::Default
}

struct InvalidBinder;

fn destructure(
    pattern: &Value,
    value: &Value,
) -> Result<Option<BTreeMap<CellId, Value>>, InvalidBinder> {
    let mut bindings = BTreeMap::new();
    matches_pattern(pattern, value, &mut bindings).map(|matched| matched.then_some(bindings))
}

fn matches_pattern(
    pattern: &Value,
    value: &Value,
    bindings: &mut BTreeMap<CellId, Value>,
) -> Result<bool, InvalidBinder> {
    match pattern {
        Value::Record(pattern_fields) => {
            if let Some(binder) = pattern_fields.get(&vocabulary::BIND) {
                match binder.as_cell() {
                    Some(binder) => match bindings.get(&binder) {
                        Some(bound) => Ok(bound == value),
                        None => {
                            bindings.insert(binder, value.clone());
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

/// Case is a control form in projection even though evaluation sees
/// an ordinary call to the Rust implementation.
pub fn case_display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&grap_runtime::vocabulary::FUNCTION)?;
    (function.as_cell()? == vocabulary::CASE).then_some(())?;
    let subject = fields.get(&vocabulary::VALUE)?;
    let alternatives = fields.get(&vocabulary::ALTERNATIVES)?.as_list()?;
    let default = fields.get(&vocabulary::DEFAULT)?;

    let mut arms = alternatives
        .iter()
        .map(|(position, alternative)| {
            let alternative = alternative.as_record()?;
            let pattern = alternative.get(&vocabulary::PATTERN)?;
            let expression = alternative.get(&grap_runtime::vocabulary::EXPRESSION)?;
            let pattern = crate::grap::at(
                [
                    Step::Key(vocabulary::ALTERNATIVES),
                    Step::Element(position.clone()),
                    Step::Key(vocabulary::PATTERN),
                ],
                pattern,
            );
            let expression = crate::grap::at(
                [
                    Step::Key(vocabulary::ALTERNATIVES),
                    Step::Element(position.clone()),
                    Step::Key(grap_runtime::vocabulary::EXPRESSION),
                ],
                expression,
            );
            Some(hug(
                row(6.0, [pattern, dim("→")]),
                expression,
                6.0,
                20.0,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    arms.push(hug(
        row(6.0, [dim("else"), dim("→")]),
        crate::grap::at([Step::Key(vocabulary::DEFAULT)], default),
        6.0,
        20.0,
    ));

    let head = row(
        4.0,
        [
            crate::grap::shallow_at(
                [Step::Key(grap_runtime::vocabulary::FUNCTION)],
                function,
            ),
            crate::grap::at([Step::Key(vocabulary::VALUE)], subject),
        ],
    );
    Some(hug(
        head,
        bracket(Delim::Brace, col(0, 4.0, arms)),
        4.0,
        20.0,
    ))
}

/// A binding pattern must remain visibly distinct from a literal cell
/// pattern, while its binder is still the real selectable cell value.
pub fn bind_display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let binder = input.value.as_record()?.get(&vocabulary::BIND)?;
    binder.as_cell()?;
    Some(row(
        4.0,
        [
            dim("bind"),
            crate::grap::shallow_at([Step::Key(vocabulary::BIND)], binder),
        ],
    ))
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::CASE, "case"),
        (vocabulary::VALUE, "value"),
        (vocabulary::ALTERNATIVES, "alternatives"),
        (vocabulary::DEFAULT, "default"),
        (vocabulary::PATTERN, "pattern"),
        (vocabulary::BIND, "bind"),
        (vocabulary::QUOTE, "quote"),
        (vocabulary::UNQUOTE, "unquote"),
    ] {
        cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::INVALID_ALTERNATIVES, "invalid alternatives"),
        (vocabulary::INVALID_ALTERNATIVE, "invalid alternative"),
        (vocabulary::INVALID_BINDER, "invalid binder"),
    ] {
        cells.set_value(cell, absent::named(name));
    }
    Library {
        cells,
        functions: functions(),
        projections: vec![case_display::<World, Hover>, bind_display::<World, Hover>],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;
    use progred_display::Env;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct NoEval;

    impl Env for NoEval {
        fn evaluate(&self, expression: &Value) -> (Value, usize) {
            (expression.clone(), 0)
        }
    }

    fn projection_input(value: &Value) -> ProjectionInput<'_, (), ()> {
        ProjectionInput {
            env: &NoEval,
            value,
            selection: None,
            state: None,
            select: std::rc::Rc::new(|_: &mut ()| false),
            hover: (),
        }
    }

    fn blob(text: &str) -> Value {
        Value::from(text.as_bytes().to_vec())
    }

    fn binding(cell: CellId) -> Value {
        Value::record([(vocabulary::BIND, Value::from(cell))])
    }

    fn alternative(pattern: Value, expression: Value) -> Value {
        Value::record([
            (vocabulary::PATTERN, pattern),
            (grap::vocabulary::EXPRESSION, expression),
        ])
    }

    fn case_call(
        value: Value,
        alternatives: impl IntoIterator<Item = Value>,
        default: Value,
    ) -> Value {
        grap::call(
            Value::from(vocabulary::CASE),
            [
                (vocabulary::VALUE, value),
                (vocabulary::ALTERNATIVES, Value::list(alternatives)),
                (vocabulary::DEFAULT, default),
            ],
        )
    }

    fn quote_call(expression: Value) -> Value {
        grap::call(
            Value::from(vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, expression)],
        )
    }

    fn evaluate(expression: &Value) -> grap::Evaluation {
        grap::evaluate(expression, |_| None, &functions(), 100)
    }

    #[test]
    fn quote_returns_recognized_expressions_as_data() {
        let function = new_cell_id();
        let expression = grap::call(Value::from(function), []);
        let evaluation = evaluate(&quote_call(expression.clone()));
        assert_eq!(evaluation.result, expression);
        assert!(evaluation.diagnostics.is_empty());
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
        assert!(evaluation.diagnostics.is_empty());
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn quote_does_not_revisit_an_unquoted_result() {
        let missing = new_cell_id();
        let inner = Value::record([(vocabulary::UNQUOTE, Value::from(missing))]);
        let expression = quote_call(Value::record([(vocabulary::UNQUOTE, inner.clone())]));
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, inner);
        assert!(evaluation.diagnostics.is_empty());
        assert!(evaluation.dependencies.is_empty());
    }

    #[test]
    fn case_evaluates_the_first_matching_expression_with_open_record_bindings() {
        let first = new_cell_id();
        let last = new_cell_id();
        let metadata = new_cell_id();
        let name = new_cell_id();
        let never = new_cell_id();
        let expression = case_call(
            Value::record([
                (first, blob("Ada")),
                (last, blob("Lovelace")),
                (metadata, blob("extra")),
            ]),
            [
                alternative(Value::record([(first, blob("Grace"))]), Value::from(never)),
                alternative(Value::record([(first, binding(name))]), Value::from(name)),
            ],
            Value::from(never),
        );
        let evaluation = evaluate(&expression);
        assert_eq!(evaluation.result, blob("Ada"));
        assert!(evaluation.diagnostics.is_empty());
    }

    #[test]
    fn case_evaluates_its_subject_once_across_all_alternatives() {
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

        let expression = case_call(
            grap::call(Value::from(subject), []),
            [
                alternative(blob("first"), blob("first result")),
                alternative(blob("second"), blob("second result")),
            ],
            blob("default"),
        );
        assert_eq!(
            grap::evaluate(&expression, |_| None, &foreign, 100).result,
            blob("default")
        );
        assert_eq!(SUBJECT_EVALUATIONS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn case_patterns_match_exact_lists_and_repeated_binders_must_agree() {
        let element = new_cell_id();
        let other = new_cell_id();
        let expression = case_call(
            Value::list([blob("left"), blob("right")]),
            [
                alternative(
                    Value::list([binding(element), binding(element)]),
                    blob("repeated"),
                ),
                alternative(
                    Value::list([binding(element), binding(other)]),
                    Value::from(other),
                ),
            ],
            blob("default"),
        );
        assert_eq!(evaluate(&expression).result, blob("right"));

        let too_short = case_call(
            Value::list([blob("only")]),
            [alternative(
                Value::list([binding(element), binding(other)]),
                blob("matched"),
            )],
            blob("default"),
        );
        assert_eq!(evaluate(&too_short).result, blob("default"));
    }

    #[test]
    fn first_pattern_wins_even_when_its_expression_returns_an_absent() {
        let missing = new_cell_id();
        let expression = case_call(
            Value::record([]),
            [
                alternative(Value::record([]), Value::from(missing)),
                alternative(Value::record([]), blob("second")),
            ],
            blob("default"),
        );
        assert_eq!(
            evaluate(&expression).result,
            Value::from(grap::absent::MISSING_CELL)
        );
    }

    #[test]
    fn case_projects_ordered_pattern_expression_arms_and_a_default() {
        let binder = new_cell_id();
        let expression = case_call(
            blob("subject"),
            [
                alternative(blob("first"), blob("one")),
                alternative(binding(binder), Value::from(binder)),
            ],
            blob("default"),
        );
        let layout = case_display(projection_input(&expression)).unwrap();
        let Layout::Alternatives(options) = layout else {
            panic!("case has responsive forms");
        };
        let Layout::Row { children, .. } = &options[0] else {
            panic!("flat case first");
        };
        let mut arms = &children[1];
        while let Layout::Shared { child, .. } = arms {
            arms = child.as_ref();
        }
        let Layout::Surround { child, .. } = arms else {
            panic!("case arms are braced");
        };
        let Layout::Col { children, .. } = child.as_ref() else {
            panic!("case arms are ordered vertically");
        };
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn malformed_case_data_falls_through_to_the_generic_call_projection() {
        let malformed = case_call(blob("subject"), [blob("not an arm")], blob("default"));
        assert!(case_display(projection_input(&malformed)).is_none());
    }

    #[test]
    fn malformed_case_data_returns_library_absents() {
        let invalid_alternatives = grap::call(
            Value::from(vocabulary::CASE),
            [
                (vocabulary::VALUE, blob("subject")),
                (vocabulary::ALTERNATIVES, blob("not a list")),
                (vocabulary::DEFAULT, blob("default")),
            ],
        );
        assert_eq!(
            evaluate(&invalid_alternatives).result,
            Value::from(vocabulary::INVALID_ALTERNATIVES)
        );

        let invalid_alternative = case_call(
            blob("subject"),
            [blob("not an alternative")],
            blob("default"),
        );
        assert_eq!(
            evaluate(&invalid_alternative).result,
            Value::from(vocabulary::INVALID_ALTERNATIVE)
        );

        let invalid_binder = case_call(
            blob("subject"),
            [alternative(
                Value::record([(vocabulary::BIND, blob("not a cell"))]),
                blob("matched"),
            )],
            blob("default"),
        );
        assert_eq!(
            evaluate(&invalid_binder).result,
            Value::from(vocabulary::INVALID_BINDER)
        );
    }

    #[test]
    fn library_describes_quote_and_classifies_absences() {
        let library = library::<(), ()>();
        assert_eq!(library.projections.len(), 2);
        assert_eq!(
            library.cells.value(vocabulary::QUOTE).and_then(name::read),
            Some("quote")
        );
        assert_eq!(
            library
                .cells
                .value(vocabulary::UNQUOTE)
                .and_then(name::read),
            Some("unquote")
        );
        for cell in [
            vocabulary::INVALID_ALTERNATIVES,
            vocabulary::INVALID_ALTERNATIVE,
            vocabulary::INVALID_BINDER,
        ] {
            assert!(absent::is_absent(library.cells.value(cell).unwrap()));
        }
    }
}
