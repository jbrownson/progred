use super::*;
use gid::new_cell_id;
use grap::{Context, Environment, Evaluator, Expression, ForeignFunction, ForeignFunctions, call};
use incremental::{Input, Memo, Runtime};
use std::cell::Cell;
use std::rc::Rc;

use crate::test_support::TestDefinition;

type Table = Vec<(CellId, TestDefinition)>;

fn definitions(runtime: &Runtime, table: Table) -> (Input<Table>, Definitions<Table>) {
    let input = runtime.input(table);
    let source = Source::new(input.clone(), |table: &Table, key| {
        let definition = table
            .iter()
            .find(|(cell, _)| cell == key)
            .map(|(_, definition)| definition);
        Resolved {
            value: definition.map(|definition| definition.value.clone()),
            implementation: definition.and_then(|definition| definition.implementation.clone()),
        }
    });
    (input, source)
}

fn counted(cell: CellId, runs: &Rc<Cell<usize>>, result: Value) -> TestDefinition {
    let runs = runs.clone();
    TestDefinition::foreign(
        cell,
        ForeignFunction::from_value(move |_, _, _| {
            runs.set(runs.get() + 1);
            Ok(result.clone())
        })
        .cacheable(),
    )
}

fn evaluate(
    runtime: &Runtime,
    definitions: Definitions<Table>,
    expression: Input<Value>,
    fuel: Input<usize>,
) -> Memo<Evaluation> {
    runtime.memo_by(
        move |read| {
            let expression = expression.read(read);
            let fuel = *fuel.read(read);
            Ok(run(&definitions, read, |host| {
                grap::Evaluator::new(host, fuel).evaluate(&expression)
            }))
        },
        crate::computations::same_evaluation,
    )
}

#[test]
fn negative_resolution_and_native_replacement_are_dependencies() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let unrelated = new_cell_id();
    let runs = Rc::new(Cell::new(0));
    let (table, source) = definitions(&runtime, vec![]);
    let expression = runtime.input(call(cell.into(), []));
    let fuel = runtime.input(100);
    let query = evaluate(&runtime, source, expression, fuel);
    assert_eq!(
        ::grap::absent::reason(runtime.read(&query).unwrap().result.as_value()),
        Some(absent::MISSING_CELL)
    );
    let first = counted(cell, &runs, Value::from(vec![1]));
    table.set_by(vec![(cell, first.clone())], |_, _| false);
    assert_eq!(
        runtime.read(&query).unwrap().result.to_value(),
        Value::from(vec![1])
    );
    table.set_by(
        vec![
            (cell, first),
            (unrelated, TestDefinition::from(Value::record([]))),
        ],
        |_, _| false,
    );
    runtime.read(&query).unwrap();
    assert_eq!(runs.get(), 1);
    table.set_by(
        vec![(cell, counted(cell, &runs, Value::from(vec![2])))],
        |_, _| false,
    );
    assert_eq!(
        runtime.read(&query).unwrap().result.to_value(),
        Value::from(vec![2])
    );
    assert_eq!(runs.get(), 2);
}

#[test]
fn library_order_document_override_and_removal_are_observed() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let a = (cell, TestDefinition::from(Value::from(vec![1])));
    let b = (cell, TestDefinition::from(Value::from(vec![2])));
    let doc = (cell, TestDefinition::from(Value::from(vec![3])));
    let (table, source) = definitions(&runtime, vec![a.clone(), b.clone()]);
    let query = evaluate(
        &runtime,
        source,
        runtime.input(cell.into()),
        runtime.input(100),
    );
    for (entries, expected) in [
        (vec![a.clone(), b.clone()], 1),
        (vec![b.clone(), a], 2),
        (vec![doc, b.clone()], 3),
        (vec![b], 2),
    ] {
        table.set_by(entries, |_, _| false);
        assert_eq!(
            runtime.read(&query).unwrap().result.to_value(),
            Value::from(vec![expected])
        );
    }
    table.set_by(vec![], |_, _| false);
    assert_eq!(
        ::grap::absent::reason(runtime.read(&query).unwrap().result.as_value()),
        Some(absent::MISSING_CELL)
    );
}

#[test]
fn undeclared_foreign_reads_disable_reuse() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let hidden = Rc::new(Cell::new(1u8));
    let definition = TestDefinition::foreign(
        cell,
        ForeignFunction::from_value({
            let hidden = hidden.clone();
            move |_, _, _| Ok(Value::from(vec![hidden.get()]))
        }),
    );
    let (_, source) = definitions(&runtime, vec![(cell, definition)]);
    let query = evaluate(
        &runtime,
        source,
        runtime.input(call(cell.into(), [])),
        runtime.input(100),
    );
    assert_eq!(
        runtime.read(&query).unwrap().result.to_value(),
        Value::from(vec![1])
    );
    hidden.set(2);
    assert_eq!(
        runtime.read(&query).unwrap().result.to_value(),
        Value::from(vec![2])
    );
}

#[test]
fn effects_are_repeated_unless_the_boundary_owns_the_recording() {
    for recorded in [false, true] {
        let runtime = Runtime::default();
        let cell = new_cell_id();
        let (_, source) = definitions(&runtime, vec![]);
        let runs = Rc::new(Cell::new(0));
        let query = runtime.memo_by(
            {
                let runs = runs.clone();
                move |read| {
                    runs.set(runs.get() + 1);
                    let output = RefCell::new(Vec::new());
                    let emit = |_, context: &mut Context<'_>, _: &Expression, _: &Environment| {
                        context.effect(|| output.borrow_mut().push(7));
                        Ok(Value::record([]))
                    };
                    let cells = [cell];
                    let overlay = ForeignFunctions::shared(&cells, &emit).cacheable();
                    let eval = |host: &dyn Host| {
                        Evaluator::new(host, 100)
                            .functions(overlay)
                            .evaluate(&call(cell.into(), []))
                    };
                    let evaluation = if recorded {
                        with_recorded_effects(&source, read, eval)
                    } else {
                        run(&source, read, eval)
                    };
                    Ok((evaluation, output.into_inner()))
                }
            },
            |a: &(Evaluation, Vec<i32>), b| {
                crate::computations::same_evaluation(&a.0, &b.0) && a.1 == b.1
            },
        );
        assert_eq!(runtime.read(&query).unwrap().1, vec![7]);
        assert_eq!(runtime.read(&query).unwrap().1, vec![7]);
        assert_eq!(runs.get(), if recorded { 1 } else { 2 });
    }
}

#[test]
fn normally_returned_absents_are_cacheable_including_halt_reasons() {
    for reason in [absent::MISSING_CELL, CANCELLED, absent::FUEL_EXHAUSTED] {
        let runtime = Runtime::default();
        let cell = new_cell_id();
        let runs = Rc::new(Cell::new(0));
        let result = absent::value(reason);
        let (_, source) = definitions(&runtime, vec![(cell, counted(cell, &runs, result.clone()))]);
        let query = evaluate(
            &runtime,
            source,
            runtime.input(call(cell.into(), [])),
            runtime.input(100),
        );
        let first = runtime.read(&query).unwrap();
        assert_eq!(first.halt, None);
        assert_eq!(first.result.to_value(), result);
        assert!(Rc::ptr_eq(&first, &runtime.read(&query).unwrap()));
        assert_eq!(runs.get(), 1);
    }
}

#[test]
fn halts_are_not_reused_even_when_the_returned_value_is_equal() {
    let runtime = Runtime::default();
    let (_, source) = definitions(&runtime, vec![]);
    let runs = Rc::new(Cell::new(0));
    let query = runtime.memo_by(
        {
            let runs = runs.clone();
            move |read| {
                runs.set(runs.get() + 1);
                Ok(run(&source, read, |host| {
                    grap::Evaluator::new(host, 0).evaluate(&Value::record([]))
                }))
            }
        },
        crate::computations::same_evaluation,
    );
    assert!(runtime.read(&query).unwrap().halt.is_some());
    runtime.read(&query).unwrap();
    assert_eq!(runs.get(), 2);
}
