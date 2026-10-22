use super::*;

#[test]
fn memo_applications_track_reads_preserve_data_arguments_and_do_not_skip_effects() {
    use grap::{Definition, ForeignFunction};
    use std::cell::Cell;

    for mode in ["tracked", "effectful", "untracked"] {
        let [function, argument, dependency, library, unrelated] =
            std::array::from_fn(|_| gid::new_cell_id());
        let runs = Rc::new(Cell::new(0));
        let implementation = ForeignFunction::from_value({
            let runs = runs.clone();
            move |context, call, environment| {
                runs.set(runs.get() + 1);
                let value =
                    context.eval_to_value(context.field(call, argument).unwrap(), environment)?;
                let observed =
                    context.eval_to_value(context.field(call, dependency).unwrap(), environment)?;
                if mode == "effectful" {
                    context.effect(|| ());
                }
                Ok(Value::list([value, observed]))
            }
        });
        let implementation = if mode == "untracked" {
            implementation
        } else {
            implementation.tracked()
        };
        let mut definitions = crate::library::Definitions::default();
        definitions.insert(
            function,
            Definition::foreign(function, Value::record([]), implementation),
        );
        let mut libraries = Libraries::default();
        libraries.insert(library, definitions);
        let computations = Computations::default();
        let mut doc = Document::default();
        let callable = grap::lambda(
            [argument],
            grap::call(
                function.into(),
                [(argument, argument.into()), (dependency, dependency.into())],
            ),
        );
        let data = grap::call(unrelated.into(), []);
        let run = |doc: &Document, data: &Value| {
            computations.begin(Rc::new(doc.clone()), libraries.clone());
            computations
                .apply_runtime(Trail::ROOT, &callable, &[(argument, data.clone())], 100)
                .into_value()
        };
        let first = run(&doc, &data);
        let values: Vec<_> = first.as_list().unwrap().values().collect();
        assert_eq!(
            values[0], &data,
            "a code-shaped data argument must stay inert"
        );
        assert_eq!(
            grap::absent::reason(values[1]),
            Some(grap::absent::MISSING_CELL)
        );
        assert_eq!(first, run(&doc, &data));
        doc.cells.set_value(unrelated, Value::record([]));
        assert_eq!(first, run(&doc, &data));
        assert_eq!(runs.get(), if mode == "tracked" { 1 } else { 3 });
        let before = runs.get();
        doc.cells.set_value(dependency, Value::from(vec![1]));
        assert_ne!(first, run(&doc, &data));
        assert_eq!(
            runs.get(),
            before + 1,
            "a previously missing dependency was read"
        );
        assert_ne!(first, run(&doc, &Value::record([])));
        assert_eq!(runs.get(), before + 2, "changed data arguments invalidate");
    }
}

#[test]
fn evaluation_roots_reuse_observed_results_and_invalidate_missing_definitions() {
    use grap::{Definition, ForeignFunction};
    use std::cell::Cell;
    let (function, input, unrelated, library) = (
        gid::new_cell_id(),
        gid::new_cell_id(),
        gid::new_cell_id(),
        gid::new_cell_id(),
    );
    let runs = Rc::new(Cell::new(0));
    let mut definitions = crate::library::Definitions::default();
    definitions.insert(
        function,
        Definition::foreign(
            function,
            Value::record([]),
            ForeignFunction::from_value({
                let runs = runs.clone();
                move |context, call, environment| {
                    runs.set(runs.get() + 1);
                    let argument = context.field(call, input).unwrap();
                    context.eval_to_value(argument, environment)
                }
            })
            .tracked(),
        ),
    );
    let mut libraries = Libraries::default();
    libraries.insert(library, definitions);
    let mut doc = Document::default();
    let computations = Computations::default();
    let expression = grap::call(function.into(), [(input, input.into())]);
    let run = |doc: &Document| {
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        computations
            .evaluate_runtime(Trail::ROOT, &(&expression).into(), 100, None)
            .into_value()
    };
    let missing = run(&doc);
    assert_eq!(
        grap::absent::reason(&missing),
        Some(grap::absent::MISSING_CELL)
    );
    assert_eq!(run(&doc), missing);
    doc.cells.set_value(unrelated, Value::record([]));
    assert_eq!(run(&doc), missing);
    assert_eq!(
        runs.get(),
        1,
        "cache ordinary absents and ignore unrelated changes"
    );
    let value = Value::from(vec![42]);
    doc.cells.set_value(input, value.clone());
    assert_eq!(run(&doc), value);
    assert_eq!(run(&doc), value);
    assert_eq!(runs.get(), 2);
    let other_expression = grap::call(function.into(), [(input, Value::record([]))]);
    assert_eq!(
        computations
            .evaluate_runtime(Trail::ROOT, &(&other_expression).into(), 100, None)
            .into_value(),
        Value::record([])
    );
    assert_eq!(runs.get(), 3);
    assert_eq!(
        grap::absent::reason(
            &computations
                .evaluate_runtime(Trail::ROOT, &(&other_expression).into(), 0, None)
                .into_value()
        ),
        Some(grap::absent::FUEL_EXHAUSTED)
    );
    assert_eq!(
        computations
            .evaluate_runtime(Trail::ROOT, &(&other_expression).into(), 100, None)
            .into_value(),
        Value::record([])
    );
    assert_eq!(runs.get(), 4);
}

#[test]
fn runtime_evaluation_roots_keep_closures_and_invalidate_on_origin_changes() {
    use grap::{RuntimeValue, SourceOrigin};
    let computations = Computations::default();
    let doc = Document::default();
    let libraries = Libraries::default();
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let make = |field| {
        grap::evaluate_runtime_at(
            &grap::RuntimeValue::from(grap::lambda([], Value::record([]))),
            Some(SourceOrigin::Stored(vec![gid::Step::Key(field)])),
            &sources,
            100,
        )
        .result
    };
    let a = make(gid::new_cell_id());
    let b = make(gid::new_cell_id());
    assert_ne!(a.to_value(), b.to_value());
    assert!(!a.same_result(&b));
    for closure in [&a, &a, &b, &b, &a] {
        let expression = RuntimeValue::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
            ),
            (grap::vocabulary::VALUE, closure.clone()),
        ]);
        let result = computations.evaluate_runtime(Trail::ROOT, &expression, 100, None);
        assert!(result.same_result(closure));
    }
}

#[test]
fn occurrences_in_different_views_have_independent_root_lifetimes() {
    let computations = Computations::default();
    // The document view's root, and a pane's, whose path starts with its
    // declaration and a pane step.
    let document = Trail::root();
    let pane = Trail::from(crate::test_examples::pane_root(
        &crate::workspace::declarations(Some(
            &crate::workspace::append(
                &Value::record([]),
                crate::workspace::Side::Left,
                Value::record([]),
            )
            .unwrap()
            .0,
        ))[0]
            .path,
    ));
    let first = computations.at(&document, || computations.runtime.input(1));
    let second = computations.at(&pane, || computations.runtime.input(2));
    first.set(3);
    assert_eq!(*second.observed().0, 2);
    assert!(Rc::ptr_eq(
        &first,
        &computations.at(&document, || unreachable!())
    ));
    let weak = Rc::downgrade(&first);
    drop(first);
    drop(computations);
    assert!(weak.upgrade().is_none());
}

#[test]
fn replacement_cancels_old_jobs_and_preserves_the_executor_and_wake() {
    use incremental::background::{Availability, Job};
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };
    let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
    let wakes = Arc::new(AtomicUsize::new(0));
    let mut computations = Computations::new(
        Executor::new({
            let queue = queue.clone();
            move |job| queue.lock().unwrap().push_back(job)
        }),
        {
            let wakes = wakes.clone();
            move || {
                wakes.fetch_add(1, Ordering::Relaxed);
            }
        },
    );
    let root = |computations: &Computations| {
        let worker = computations
            .tasks
            .as_ref()
            .unwrap()
            .memo(computations.runtime.memo(|_| Ok(7)), |v, _| Ok(v));
        computations.runtime.memo(move |read| {
            Ok(match &*worker.read(read)? {
                Availability::Pending { .. } => None,
                Availability::Ready(value) | Availability::Refining(value) => Some(**value),
            })
        })
    };
    let old_runtime = computations.runtime.clone();
    let old = root(&computations);
    assert_eq!(*old_runtime.read(&old).unwrap(), None);
    computations.reset();
    assert_eq!(old_runtime.read(&old), Err(incremental::Error::Cancelled));
    let fresh = root(&computations);
    assert_eq!(*computations.runtime.read(&fresh).unwrap(), None);
    loop {
        let job = queue.lock().unwrap().pop_front();
        let Some(job) = job else { break };
        job();
    }
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert!(computations.poll());
    assert_eq!(*computations.runtime.read(&fresh).unwrap(), Some(7));
}
