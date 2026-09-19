use super::*;

#[test]
fn cancellation_shares_one_flag_across_clones_and_late_observers() {
    let cancel = Cancellation::default();
    let clone = cancel.clone();
    let flag = cancel.shared_flag().clone();
    assert!(Arc::ptr_eq(&flag, clone.shared_flag()));
    assert_eq!(clone.check(), Ok(()));
    cancel.cancel();
    cancel.cancel();
    assert!(flag.load(Ordering::Relaxed));
    assert!(cancel.shared_flag().load(Ordering::Relaxed));
    assert_eq!(clone.check(), Err(Error::Cancelled));
    assert_eq!(Cancellation::default().check(), Ok(()));
}

#[test]
fn cancellation_observes_external_cancellation_on_another_thread() {
    let cancel = Cancellation::default();
    let flag = cancel.shared_flag().clone();
    std::thread::spawn(move || flag.store(true, Ordering::Relaxed))
        .join()
        .unwrap();
    assert_eq!(cancel.check(), Err(Error::Cancelled));
}

#[test]
fn validation_stops_before_obsolete_branch_dependencies() {
    let runtime = Runtime::default();
    let condition = runtime.input(true);
    let child = runtime.memo({
        let condition = condition.clone();
        move |read| {
            assert!(
                *condition.read(read),
                "the obsolete branch must not execute"
            );
            Ok(1)
        }
    });
    let parent = runtime.memo({
        let condition = condition.clone();
        move |read| {
            Ok(if *condition.read(read) {
                *child.read(read)?
            } else {
                2
            })
        }
    });
    assert_eq!(*runtime.read(&parent).unwrap(), 1);
    condition.set(false);
    assert_eq!(*runtime.read(&parent).unwrap(), 2);
}

#[test]
fn recovered_child_failure_retries_after_repair() {
    for initially_failing in [false, true] {
        let runtime = Runtime::default();
        let failing = runtime.input(initially_failing);
        let child = runtime.memo({
            let failing = failing.clone();
            move |read| {
                if *failing.read(read) {
                    Err(Error::Cycle)
                } else {
                    Ok(7)
                }
            }
        });
        let parent = runtime.memo(move |read| Ok(child.read(read).map(|v| *v).unwrap_or(-1)));
        if !initially_failing {
            assert_eq!(*runtime.read(&parent).unwrap(), 7);
            failing.set(true);
        }
        assert_eq!(*runtime.read(&parent).unwrap(), -1);
        failing.set(false);
        assert_eq!(*runtime.read(&parent).unwrap(), 7);
    }
}

#[test]
fn roots_use_caller_keys_and_release_unused_graphs() {
    let roots = Roots::default();
    let a = roots.get("left", || 1);
    let b = roots.get("right", || 2);
    assert!(Rc::ptr_eq(&a, &roots.get("left", || 3)));
    let weak = Rc::downgrade(&b);
    drop(b);
    roots.begin();
    roots.get("left", || 4);
    roots.begin();
    assert!(weak.upgrade().is_none());
    assert!(Rc::ptr_eq(&a, &roots.get("left", || 5)));
    assert_eq!(
        *roots.get("left", || String::from("another type")),
        "another type"
    );
}

#[test]
fn nested_dependencies_and_equal_results_stop_propagation() {
    let runtime = Runtime::default();
    let input = runtime.input(2i32);
    let child_runs = Rc::new(Cell::new(0));
    let parent_runs = Rc::new(Cell::new(0));
    let child = runtime.memo({
        let input = input.clone();
        let runs = child_runs.clone();
        move |read| {
            runs.set(runs.get() + 1);
            Ok(input.read(read).pow(2))
        }
    });
    let parent = runtime.memo({
        let runs = parent_runs.clone();
        move |read| {
            runs.set(runs.get() + 1);
            Ok(*child.read(read)? + 1)
        }
    });
    assert_eq!(*runtime.read(&parent).unwrap(), 5);
    assert_eq!(*runtime.read(&parent).unwrap(), 5);
    input.set(-2);
    assert_eq!(*runtime.read(&parent).unwrap(), 5);
    assert_eq!((child_runs.get(), parent_runs.get()), (2, 1));
    input.set(3);
    assert_eq!(*runtime.read(&parent).unwrap(), 10);
    assert_eq!((child_runs.get(), parent_runs.get()), (3, 2));
}

#[test]
fn cached_child_is_still_a_dependency_and_old_branches_are_removed() {
    let runtime = Runtime::default();
    let left = runtime.input(1);
    let right = runtime.input(10);
    let choose_left = runtime.input(true);
    let left_memo = runtime.memo({
        let left = left.clone();
        move |r| Ok(*left.read(r))
    });
    assert_eq!(*runtime.read(&left_memo).unwrap(), 1);
    let runs = Rc::new(Cell::new(0));
    let parent = runtime.memo({
        let choose_left = choose_left.clone();
        let runs = runs.clone();
        move |r| {
            runs.set(runs.get() + 1);
            Ok(if *choose_left.read(r) {
                *left_memo.read(r)?
            } else {
                *right.read(r)
            })
        }
    });
    assert_eq!(*runtime.read(&parent).unwrap(), 1);
    left.set(2);
    assert_eq!(*runtime.read(&parent).unwrap(), 2);
    choose_left.set(false);
    assert_eq!(*runtime.read(&parent).unwrap(), 10);
    left.set(3);
    assert_eq!(*runtime.read(&parent).unwrap(), 10);
    assert_eq!(runs.get(), 3);
}

#[test]
fn selected_reads_track_missing_entries_and_ignore_unrelated_changes() {
    let runtime = Runtime::default();
    let input = runtime.input(vec![(1, 10)]);
    let source = Source::new(input.clone(), |values, key| {
        values.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    });
    let runs = Rc::new(Cell::new(0));
    let query = runtime.memo({
        let runs = runs.clone();
        move |r| {
            runs.set(runs.get() + 1);
            Ok(source.read(2, r))
        }
    });
    assert_eq!(*runtime.read(&query).unwrap(), None);
    input.set(vec![(1, 11)]);
    assert_eq!(*runtime.read(&query).unwrap(), None);
    assert_eq!(runs.get(), 1);
    input.set(vec![(1, 11), (2, 20)]);
    assert_eq!(*runtime.read(&query).unwrap(), Some(20));
    input.set(vec![(1, 11)]);
    assert_eq!(*runtime.read(&query).unwrap(), None);
    assert_eq!(runs.get(), 3);
}

#[test]
fn cancelled_or_interrupted_work_is_not_published() {
    let runtime = Runtime::default();
    let input = runtime.input(0);
    let token = Cancellation::default();
    let memo = runtime.memo({
        let input = input.clone();
        let token = token.clone();
        move |r| {
            let n = *input.read(r);
            if n == 1 {
                token.cancel();
            }
            Ok(n)
        }
    });
    assert_eq!(*runtime.read(&memo).unwrap(), 0);
    input.set(1);
    assert_eq!(runtime.read_with(&memo, &token), Err(Error::Cancelled));
    input.set(2);
    assert_eq!(*runtime.read(&memo).unwrap(), 2);
    let writes = runtime.memo(move |r| {
        let n = *input.read(r);
        input.set(n + 1);
        Ok(n)
    });
    assert_eq!(runtime.read(&writes), Err(Error::InputsChanged));
}

#[test]
fn cycles_are_reported_and_the_active_flag_recovers() {
    let runtime = Runtime::default();
    let recurse = runtime.input(true);
    let slot: Rc<RefCell<Option<Memo<i32>>>> = Rc::default();
    let memo = runtime.memo({
        let slot = Rc::downgrade(&slot);
        let recurse = recurse.clone();
        move |r| {
            if *recurse.read(r) {
                Ok(*slot.upgrade().unwrap().borrow().as_ref().unwrap().read(r)?)
            } else {
                Ok(7)
            }
        }
    });
    *slot.borrow_mut() = Some(memo.clone());
    assert_eq!(runtime.read(&memo), Err(Error::Cycle));
    recurse.set(false);
    assert_eq!(*runtime.read(&memo).unwrap(), 7);
}

#[test]
fn untracked_child_prevents_parent_reuse_even_when_results_are_equal() {
    let runtime = Runtime::default();
    let tracked = runtime.input(true);
    let runs = Rc::new(Cell::new(0));
    let child = runtime.memo({
        let tracked = tracked.clone();
        move |r| {
            if !*tracked.read(r) {
                r.untracked();
            }
            Ok(0)
        }
    });
    let parent = runtime.memo({
        let runs = runs.clone();
        move |r| {
            runs.set(runs.get() + 1);
            child.read(r)?;
            Ok(1)
        }
    });
    runtime.read(&parent).unwrap();
    tracked.set(false);
    runtime.read(&parent).unwrap();
    runtime.read(&parent).unwrap();
    assert_eq!(runs.get(), 3);
}

#[test]
fn runtime_boundaries_and_panics_cannot_poison_a_memo() {
    let runtime = Runtime::default();
    let foreign = Runtime::default().input(1);
    let memo = runtime.memo(move |r| Ok(*foreign.read(r)));
    assert_eq!(runtime.read(&memo), Err(Error::DifferentRuntime));
    let panic = Rc::new(Cell::new(true));
    let memo = runtime.memo({
        let panic = panic.clone();
        move |_| {
            assert!(!panic.get());
            Ok(1)
        }
    });
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.read(&memo))).is_err()
    );
    panic.set(false);
    assert_eq!(*runtime.read(&memo).unwrap(), 1);
}
