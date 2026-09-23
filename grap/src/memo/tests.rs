use super::*;
use gid::new_cell_id;
use std::cell::Cell;

type Table = Vec<(CellId, Resolution, Definition)>;

fn definitions(runtime: &Runtime, table: Table) -> (Input<Table>, Definitions<Table>) {
    let input = runtime.input(table);
    let source = Source::new(input.clone(), |table: &Table, key| {
        Resolved(
            table
                .iter()
                .find(|(cell, _, _)| cell == key)
                .map(|(_, origin, value)| (origin.clone(), value.clone())),
        )
    });
    (input, source)
}

fn counted(runs: &Rc<Cell<usize>>, result: Value) -> Definition {
    let runs = runs.clone();
    Definition::foreign(
        Value::record([]),
        ForeignFunction::from_value(move |_, _, _| {
            runs.set(runs.get() + 1);
            Ok(result.clone())
        })
        .tracked(),
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
        absent::reason(&runtime.read(&query).unwrap().result),
        Some(absent::MISSING_CELL)
    );
    let first = counted(&runs, Value::from(vec![1]));
    table.set_by(vec![(cell, Resolution::Document, first.clone())], |_, _| {
        false
    });
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![1]));
    table.set_by(
        vec![
            (cell, Resolution::Document, first),
            (
                unrelated,
                Resolution::Document,
                Definition::Value(Value::record([])),
            ),
        ],
        |_, _| false,
    );
    runtime.read(&query).unwrap();
    assert_eq!(runs.get(), 1);
    table.set_by(
        vec![(
            cell,
            Resolution::Document,
            counted(&runs, Value::from(vec![2])),
        )],
        |_, _| false,
    );
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![2]));
    assert_eq!(runs.get(), 2);
}

#[test]
fn library_order_document_override_and_removal_are_observed() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let a = (
        cell,
        Resolution::Library(new_cell_id()),
        Definition::Value(Value::from(vec![1])),
    );
    let b = (
        cell,
        Resolution::Library(new_cell_id()),
        Definition::Value(Value::from(vec![2])),
    );
    let doc = (
        cell,
        Resolution::Document,
        Definition::Value(Value::from(vec![3])),
    );
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
            runtime.read(&query).unwrap().result,
            Value::from(vec![expected])
        );
    }
    table.set_by(vec![], |_, _| false);
    assert_eq!(
        absent::reason(&runtime.read(&query).unwrap().result),
        Some(absent::MISSING_CELL)
    );
}

#[test]
fn undeclared_foreign_reads_disable_reuse() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let hidden = Rc::new(Cell::new(1u8));
    let definition = Definition::foreign(
        Value::record([]),
        ForeignFunction::from_value({
            let hidden = hidden.clone();
            move |_, _, _| Ok(Value::from(vec![hidden.get()]))
        }),
    );
    let (_, source) = definitions(&runtime, vec![(cell, Resolution::Document, definition)]);
    let query = evaluate(
        &runtime,
        source,
        runtime.input(call(cell.into(), [])),
        runtime.input(100),
    );
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![1]));
    hidden.set(2);
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![2]));
}

#[test]
fn foreign_input_observations_and_fuel_are_dependencies() {
    let runtime = Runtime::default();
    let cell = new_cell_id();
    let value = runtime.input(1u8);
    let runs = Rc::new(Cell::new(0));
    let definition = Definition::foreign(
        Value::record([]),
        ForeignFunction::from_value({
            let value = value.clone();
            let runs = runs.clone();
            move |context, _, _| {
                runs.set(runs.get() + 1);
                Ok(Value::from(vec![*context.read(&value)]))
            }
        })
        .tracked(),
    );
    let (_, source) = definitions(&runtime, vec![(cell, Resolution::Document, definition)]);
    let fuel = runtime.input(100);
    let query = evaluate(
        &runtime,
        source,
        runtime.input(call(cell.into(), [])),
        fuel.clone(),
    );
    runtime.read(&query).unwrap();
    runtime.read(&query).unwrap();
    assert_eq!(runs.get(), 1);
    value.set(2);
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![2]));
    assert_eq!(runs.get(), 2);
    fuel.set(0);
    assert!(!runtime.read(&query).unwrap().completed);
    fuel.set(100);
    assert_eq!(runtime.read(&query).unwrap().result, Value::from(vec![2]));
}

#[test]
fn effects_are_repeated_unless_the_boundary_owns_the_recording() {
    for recorded in [false, true] {
        let runtime = Runtime::default();
        let cell = new_cell_id();
        let (_, source) = definitions(&runtime, vec![]);
        let runs = Rc::new(Cell::new(0));
        let query = runtime.memo({
            let runs = runs.clone();
            move |read| {
                runs.set(runs.get() + 1);
                let output = RefCell::new(Vec::new());
                let emit = |_, context: &mut Context<'_>, _: &Expression, _: &Environment| {
                    context.effect(|| output.borrow_mut().push(7));
                    Ok(Value::record([]))
                };
                let cells = [cell];
                let overlay = ForeignOverlay::from_value(&cells, &emit).tracked();
                let eval = |host: &dyn Host| {
                    evaluate_value_scoped(&call(cell.into(), []), host, &overlay, 100)
                };
                let evaluation = if recorded {
                    with_recorded_effects(&source, read, eval)
                } else {
                    run(&source, read, eval)
                };
                Ok((evaluation, output.into_inner()))
            }
        });
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
        let (_, source) = definitions(
            &runtime,
            vec![(cell, Resolution::Document, counted(&runs, result.clone()))],
        );
        let query = evaluate(
            &runtime,
            source,
            runtime.input(call(cell.into(), [])),
            runtime.input(100),
        );
        let first = runtime.read(&query).unwrap();
        assert!(first.completed);
        assert_eq!(first.result, result);
        assert!(Rc::ptr_eq(&first, &runtime.read(&query).unwrap()));
        assert_eq!(runs.get(), 1);
    }
}

#[test]
fn halts_are_not_reused_even_when_the_returned_value_is_equal() {
    let runtime = Runtime::default();
    let (_, source) = definitions(&runtime, vec![]);
    let runs = Rc::new(Cell::new(0));
    let query = runtime.memo({
        let runs = runs.clone();
        move |read| {
            runs.set(runs.get() + 1);
            Ok(run(&source, read, |host| {
                super::super::evaluate_value(&Value::record([]), host, 0)
            }))
        }
    });
    assert!(!runtime.read(&query).unwrap().completed);
    runtime.read(&query).unwrap();
    assert_eq!(runs.get(), 2);
}
