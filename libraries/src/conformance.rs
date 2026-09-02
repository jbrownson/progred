//! An executable slice of Grap's cross-evaluator contract. Each
//! program pins its result, diagnostics, and exact remaining fuel
//! under the burn model documented on `Context::burn`; a conforming
//! evaluator reproduces every tuple. Expectations are hand-derived
//! from the naive evaluation shape, never copied from a run.

use crate::{control, f64, list};
use gid::{CellId, Value, new_cell_id};
use grap_runtime as grap;
use grap_runtime::{Diagnostic, ForeignFunctions};
use std::collections::BTreeSet;

fn functions() -> ForeignFunctions {
    ForeignFunctions::merge_all([
        control::functions(),
        f64::functions(),
        list::library::<(), ()>().functions(),
    ])
}

fn blob(text: &str) -> Value {
    Value::from(text.as_bytes().to_vec())
}

fn evaluate(expression: &Value, fuel: usize) -> grap::Evaluation {
    crate::test_evaluate(expression, |_| None, &functions(), fuel)
}

fn evaluate_resolving(
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
    fuel: usize,
) -> grap::Evaluation {
    crate::test_evaluate(expression, resolve, &functions(), fuel)
}

/// call(1) + lambda(1) + argument(1) + body cell, bound(1) = 4.
#[test]
fn a_lambda_call_burns_its_call_function_argument_and_body() {
    let x = new_cell_id();
    let expression = grap::call(grap::lambda([x], Value::from(x)), [(x, blob("bound"))]);
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, blob("bound"));
    assert!(evaluation.diagnostics.is_empty());
    assert_eq!(evaluation.remaining_fuel, 6);
}

/// Shadowing costs nothing extra: the same four burns even when the
/// parameter is a registered foreign function's cell.
#[test]
fn shadowing_a_foreign_function_burns_like_any_binding() {
    let expression = grap::call(
        grap::lambda([f64::vocabulary::SUM], Value::from(f64::vocabulary::SUM)),
        [(f64::vocabulary::SUM, blob("bound"))],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, blob("bound"));
    assert_eq!(evaluation.remaining_fuel, 6);
}

/// Each link in a cell chain is one evaluation: A(1) + B(1) + blob(1).
#[test]
fn a_cell_chain_burns_one_per_link() {
    let first = new_cell_id();
    let second = new_cell_id();
    let evaluation = evaluate_resolving(
        &Value::from(first),
        |cell| match cell {
            cell if cell == first => Some(Value::from(second)),
            cell if cell == second => Some(blob("end")),
            _ => None,
        },
        10,
    );
    assert_eq!(evaluation.result, blob("end"));
    assert_eq!(evaluation.remaining_fuel, 7);
    assert_eq!(evaluation.dependencies, BTreeSet::from([first, second]));
}

/// A missing cell burns its one evaluation and reports exactly once.
#[test]
fn a_missing_cell_burns_once_and_diagnoses() {
    let missing = new_cell_id();
    let evaluation = evaluate(&Value::from(missing), 10);
    assert_eq!(
        evaluation.result,
        grap::absent::value(grap::absent::MISSING_CELL),
    );
    assert_eq!(evaluation.diagnostics, [Diagnostic::MissingCell(missing)]);
    assert_eq!(evaluation.remaining_fuel, 9);
}

/// do never evaluates its expression list, only the elements:
/// call(1) + function(1) + first(1) + second(1) = 4.
#[test]
fn a_do_sequence_burns_only_its_elements() {
    let expression = grap::call(
        Value::from(control::vocabulary::DO),
        [(
            control::vocabulary::EXPRESSIONS,
            Value::list([blob("first"), blob("second")]),
        )],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, blob("second"));
    assert_eq!(evaluation.remaining_fuel, 6);
}

/// A match with no matching case still pays for its subject and its
/// cases list: call(1) + function(1) + subject(1) + cases(1) = 4.
#[test]
fn an_unmatched_subject_burns_everything_but_an_arm() {
    let expression = grap::call(
        Value::from(control::vocabulary::MATCH),
        [
            (control::vocabulary::VALUE, blob("subject")),
            (
                control::vocabulary::CASES,
                Value::list([Value::record([
                    (control::vocabulary::PATTERN, blob("other")),
                    (grap::vocabulary::EXPRESSION, blob("never")),
                ])]),
            ),
        ],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(
        evaluation.result,
        Value::record([
            (
                crate::absent::vocabulary::ABSENT,
                Value::from(control::vocabulary::PATTERN_MISMATCH),
            ),
            (control::vocabulary::PATTERN, blob("other")),
        ]),
    );
    assert!(evaluation.diagnostics.is_empty());
    assert_eq!(evaluation.remaining_fuel, 6);
}

/// Quote walks its template without burning; only the unquote body
/// evaluates: call(1) + function(1) + unquoted blob(1) = 3.
#[test]
fn a_quote_burns_only_its_unquotes() {
    let field = new_cell_id();
    let expression = grap::call(
        Value::from(control::vocabulary::QUOTE),
        [(
            grap::vocabulary::EXPRESSION,
            Value::record([(
                field,
                Value::record([(control::vocabulary::UNQUOTE, blob("spliced"))]),
            )]),
        )],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, Value::record([(field, blob("spliced"))]));
    assert_eq!(evaluation.remaining_fuel, 7);
}

/// An enriched number — extra fields beside the f64 — answers record
/// patterns exactly as its stored form would, however the
/// implementation carries it: call(1) + function(1) + subject(1) +
/// cases(1) + arm(1) = 5.
#[test]
fn an_enriched_number_matches_record_patterns_like_its_data() {
    let note = new_cell_id();
    let binder = new_cell_id();
    let subject = Value::record([
        (
            f64::vocabulary::F64,
            Value::from(1.0f64.to_le_bytes().to_vec()),
        ),
        (note, blob("annotated")),
    ]);
    let expression = grap::call(
        Value::from(control::vocabulary::MATCH),
        [
            (control::vocabulary::VALUE, subject),
            (
                control::vocabulary::CASES,
                Value::list([Value::record([
                    (
                        control::vocabulary::PATTERN,
                        Value::record([(
                            note,
                            Value::record([(control::vocabulary::BIND, Value::from(binder))]),
                        )]),
                    ),
                    (grap::vocabulary::EXPRESSION, Value::from(binder)),
                ])]),
            ),
        ],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, blob("annotated"));
    assert_eq!(evaluation.remaining_fuel, 5);
}

/// The prepared and deferred clause routes must agree: moving a
/// literal clause list behind a cell changes only the fuel of reaching
/// it — the cell evaluation plus the list evaluation, one burn more
/// than the literal's stand-in burn — never the result.
#[test]
fn clause_lists_behind_cells_agree_with_literal_clauses() {
    let binder = new_cell_id();
    let cases = Value::list([
        Value::record([
            (control::vocabulary::PATTERN, blob("other")),
            (grap::vocabulary::EXPRESSION, blob("never")),
        ]),
        Value::record([
            (
                control::vocabulary::PATTERN,
                Value::record([(control::vocabulary::BIND, Value::from(binder))]),
            ),
            (grap::vocabulary::EXPRESSION, Value::from(binder)),
        ]),
    ]);
    let bindings = Value::list([Value::record([
        (control::vocabulary::BIND, Value::from(binder)),
        (control::vocabulary::VALUE, blob("bound")),
    ])]);
    let matches = |clauses: Value| {
        grap::call(
            Value::from(control::vocabulary::MATCH),
            [
                (control::vocabulary::VALUE, blob("subject")),
                (control::vocabulary::CASES, clauses),
            ],
        )
    };
    let lets = |clauses: Value| {
        grap::call(
            Value::from(control::vocabulary::LET),
            [
                (control::vocabulary::BINDINGS, clauses),
                (grap::vocabulary::EXPRESSION, Value::from(binder)),
            ],
        )
    };
    let check = |program: &dyn Fn(Value) -> Value, clauses: Value| {
        let reference = new_cell_id();
        let literal = evaluate(&program(clauses.clone()), 20);
        let referenced = evaluate_resolving(
            &program(Value::from(reference)),
            |cell| (cell == reference).then(|| clauses.clone()),
            20,
        );
        assert_eq!(literal.result, referenced.result);
        assert!(literal.diagnostics.is_empty() && referenced.diagnostics.is_empty());
        assert_eq!(literal.remaining_fuel, referenced.remaining_fuel + 1);
    };
    check(&matches, cases);
    check(&lets, bindings);
}

/// Arithmetic burns like any strict call: call(1) + function(1) +
/// left(1) + right(1) = 4, whatever numeric fast path the
/// implementation takes.
#[test]
fn arithmetic_burns_its_call_function_and_operands() {
    let expression = grap::call(
        Value::from(f64::vocabulary::SUM),
        [
            (f64::vocabulary::LEFT, f64::value(1.0)),
            (f64::vocabulary::RIGHT, f64::value(2.0)),
        ],
    );
    let evaluation = evaluate(&expression, 10);
    assert_eq!(evaluation.result, f64::value(3.0));
    assert_eq!(evaluation.remaining_fuel, 6);
}

/// The prepared-call shortcut stays burn-invisible: an iterate step
/// that immediately declines costs call(1) + function(1) + initial(1)
/// + step lambda(1) + prepared call and function(2) + state bind(1) +
/// body(1) = 8.
#[test]
fn an_iterate_step_burns_like_an_ordinary_call() {
    let state = list::vocabulary::STATE;
    let expression = grap::call(
        Value::from(list::vocabulary::ITERATE),
        [
            (list::vocabulary::INITIAL, blob("start")),
            (
                list::vocabulary::STEP,
                grap::lambda([state], crate::absent::value()),
            ),
        ],
    );
    let evaluation = evaluate(&expression, 20);
    assert_eq!(evaluation.result, blob("start"));
    assert_eq!(evaluation.remaining_fuel, 12);
}
