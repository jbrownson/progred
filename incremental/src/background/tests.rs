use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Default)]
struct Queue(Arc<Mutex<VecDeque<Job>>>);

impl Queue {
    fn executor(&self) -> Executor {
        let queue = self.clone();
        Executor::new(move |job| queue.0.lock().unwrap().push_back(job))
    }

    fn next(&self) -> Job {
        self.0.lock().unwrap().pop_front().unwrap()
    }

    fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

fn observe(runtime: &Runtime, node: AsyncMemo<usize, usize>) -> Memo<(bool, Option<usize>)> {
    runtime.memo(move |read| {
        Ok(match &*node.read(read)? {
            Availability::Pending { previous } => (true, previous.as_deref().copied()),
            Availability::Ready(value) | Availability::Refining(value) => (false, Some(**value)),
        })
    })
}

#[test]
fn progressive_values_are_current_and_publish_only_between_revisions() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let prepared = runtime.memo(|_| Ok(10usize));
    let (stage, staged) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node = tasks.memo_progressive(prepared, move |value, _, publish| {
        for step in 0..2 {
            publish(value + step)?;
            stage.send(step).unwrap();
            resumed.lock().unwrap().recv().unwrap();
        }
        Ok(value + 2)
    });
    let parent = runtime.memo(move |read| {
        Ok(match &*node.read(read)? {
            Availability::Pending { previous } => (0, previous.as_deref().copied()),
            Availability::Refining(value) => (1, Some(**value)),
            Availability::Ready(value) => (2, Some(**value)),
        })
    });
    let first = runtime.read(&parent).unwrap();
    assert_eq!(*first, (0, None));
    let worker = std::thread::spawn(queue.next());
    assert_eq!(staged.recv().unwrap(), 0);
    assert!(Rc::ptr_eq(&first, &runtime.read(&parent).unwrap()));
    assert!(tasks.poll());
    let coarse = runtime.read(&parent).unwrap();
    assert_eq!(*coarse, (1, Some(10)));
    resume.send(()).unwrap();
    assert_eq!(staged.recv().unwrap(), 1);
    assert!(Rc::ptr_eq(&coarse, &runtime.read(&parent).unwrap()));
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (1, Some(11)));
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (2, Some(12)));
    assert_eq!(queue.len(), 0);
}

#[test]
fn replacing_a_progressive_request_retains_its_preview_but_rejects_late_reports() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1usize);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let (stage, staged) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node = tasks.memo_progressive(prepared, move |value, _, publish| {
        publish(value * 10)?;
        if value == 1 {
            stage.send(()).unwrap();
            resumed.lock().unwrap().recv().unwrap();
            assert_eq!(publish(999), Err(Error::Cancelled));
        }
        Ok(value * 100)
    });
    let parent = observe(&runtime, node);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    let worker = std::thread::spawn(queue.next());
    staged.recv().unwrap();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(10)));
    input.set(2);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(10)));
    input.set(3);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(10)));
    resume.send(()).unwrap();
    worker.join().unwrap();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(10)));
    assert_eq!(queue.len(), 1);
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(300)));
}

#[test]
fn unpublished_progress_is_discarded_if_inputs_changed_before_poll() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1usize);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let (stage, staged) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node = tasks.memo_progressive(prepared, move |value, _, publish| {
        publish(value)?;
        stage.send(()).unwrap();
        resumed.lock().unwrap().recv().unwrap();
        Ok(value)
    });
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    let worker = std::thread::spawn(queue.next());
    staged.recv().unwrap();
    input.set(2);
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&observed).unwrap(), (true, None));
    resume.send(()).unwrap();
    worker.join().unwrap();
}

#[test]
fn inline_progress_finishes_immediately_and_does_not_swallow_a_final_failure() {
    let runtime = Runtime::default();
    let tasks = Tasks::new(&runtime, Executor::inline(), || {});
    let node = tasks.memo_progressive(runtime.memo(|_| Ok(0usize)), |_, _, publish| {
        publish(1)?;
        publish(2)?;
        Ok(3)
    });
    assert_eq!(
        *runtime.read(&observe(&runtime, node)).unwrap(),
        (false, Some(3))
    );
    let node = tasks.memo_progressive(runtime.memo(|_| Ok(0usize)), |_, _, publish| {
        publish(1)?;
        Err(Error::Cancelled)
    });
    assert!(matches!(
        runtime.read(&observe(&runtime, node)),
        Err(Error::Cancelled)
    ));
}

#[test]
fn completion_invalidates_parents_and_retains_previous_while_updating() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let wakes = Arc::new(AtomicUsize::new(0));
    let tasks = Tasks::new(&runtime, queue.executor(), {
        let wakes = wakes.clone();
        move || {
            wakes.fetch_add(1, Ordering::Relaxed);
        }
    });
    let input = runtime.input(1);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let node = tasks.memo(prepared, |value, _| Ok(value * 2));
    let observed = observe(&runtime, node);
    let parent = runtime.memo(move |read| Ok(*observed.read(read)?));
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    queue.next()();
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(2)));
    input.set(2);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(2)));
    input.set(3);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(2)));
    assert_eq!(queue.len(), 1);
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(6)));
    runtime.input(0).set(1);
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(6)));
    assert_eq!(queue.len(), 0);
}

#[test]
fn completion_from_before_an_input_edit_is_never_published() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let node = tasks.memo(prepared, |value, _| Ok(value));
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    queue.next()();
    input.set(2);
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&observed).unwrap(), (true, None));
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&observed).unwrap(), (false, Some(2)));
}

#[test]
fn running_work_is_cancelled_and_only_the_latest_replacement_runs() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let (started, started_rx) = mpsc::channel();
    let (resume, resume_rx) = mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    let node = tasks.memo(prepared, move |value, cancel| {
        if value == 1 {
            started.send(cancel.clone()).unwrap();
            resume_rx.lock().unwrap().recv().unwrap();
        }
        Ok(value)
    });
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    let worker = std::thread::spawn(queue.next());
    let cancel = started_rx.recv().unwrap();
    input.set(2);
    runtime.read(&observed).unwrap();
    assert_eq!(cancel.check(), Err(Error::Cancelled));
    input.set(3);
    runtime.read(&observed).unwrap();
    assert_eq!(queue.len(), 0);
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert!(!tasks.poll());
    assert_eq!(queue.len(), 1);
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&observed).unwrap(), (false, Some(3)));
}

#[test]
fn dropping_the_owner_cancels_work_even_if_a_reader_survives() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let node = tasks.memo(runtime.memo(|_| Ok(1)), |value, _| Ok(value));
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    drop(tasks);
    assert_eq!(runtime.read(&observed), Err(Error::Cancelled));
    queue.next()();
}

#[test]
fn inline_execution_and_ordinary_failure_values_complete_normally() {
    let runtime = Runtime::default();
    let tasks = Tasks::new(&runtime, Executor::inline(), || {});
    let node = tasks.memo(runtime.memo(|_| Ok(())), |(), _| Ok(Err::<(), _>("absent")));
    let observed = runtime.memo(move |read| {
        Ok(match &*node.read(read)? {
            Availability::Ready(value) | Availability::Refining(value) => Some(**value),
            Availability::Pending { .. } => None,
        })
    });
    assert_eq!(*runtime.read(&observed).unwrap(), Some(Err("absent")));
}

#[test]
fn a_recovered_worker_error_does_not_cache_the_fallback() {
    let runtime = Runtime::default();
    let tasks = Tasks::new(&runtime, Executor::inline(), || {});
    let input = runtime.input(0);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let node = tasks.memo(prepared, |value, _| {
        if value == 0 {
            Err(Error::Cancelled)
        } else {
            Ok(value)
        }
    });
    let observed = runtime.memo(move |read| {
        Ok(node.read(read).ok().and_then(|value| match &*value {
            Availability::Ready(value) | Availability::Refining(value) => Some(**value),
            Availability::Pending { .. } => None,
        }))
    });
    assert_eq!(*runtime.read(&observed).unwrap(), None);
    input.set(3);
    assert_eq!(*runtime.read(&observed).unwrap(), Some(3));
}

#[test]
fn cancellation_callbacks_run_once_including_late_registration() {
    let cancel = Cancellation::default();
    let count = Arc::new(AtomicUsize::new(0));
    for before in [true, false] {
        if !before {
            cancel.cancel();
        }
        let count = count.clone();
        cancel.on_cancel(move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
    }
    cancel.cancel();
    assert_eq!(count.load(Ordering::Relaxed), 2);
}

#[test]
fn worker_panics_reach_the_owner_and_do_not_leave_permanent_pending_state() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let calls = AtomicUsize::new(0);
    let node = tasks.memo(runtime.memo(|_| Ok(1)), move |value, _| {
        assert_ne!(calls.fetch_add(1, Ordering::Relaxed), 0, "worker panic");
        Ok(value)
    });
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    queue.next()();
    tasks.poll();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { runtime.read(&observed) }))
            .is_err()
    );
    assert_eq!(*runtime.read(&observed).unwrap(), (true, None));
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&observed).unwrap(), (false, Some(1)));
}

#[test]
fn threaded_executor_wakes_the_owner_without_polling_for_work() {
    let runtime = Runtime::default();
    let (wake, woken) = mpsc::channel();
    let tasks = Tasks::new(
        &runtime,
        Executor::threaded(1.try_into().unwrap()).unwrap(),
        move || {
            wake.send(()).unwrap();
        },
    );
    let node = tasks.memo(runtime.memo(|_| Ok(4)), |value, _| Ok(value));
    let observed = observe(&runtime, node);
    runtime.read(&observed).unwrap();
    woken
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    tasks.poll();
    assert_eq!(*runtime.read(&observed).unwrap(), (false, Some(4)));
}

#[test]
fn unused_inline_nodes_do_not_accumulate_in_the_completion_registry() {
    let runtime = Runtime::default();
    let tasks = Tasks::new(&runtime, Executor::inline(), || {});
    for _ in 0..100 {
        let node = tasks.memo(runtime.memo(|_| Ok(1)), |value, _| Ok(value));
        let observed = observe(&runtime, node);
        runtime.read(&observed).unwrap();
        assert_eq!(tasks.nodes.borrow().len(), 1);
    }
}

#[test]
fn shared_readers_keep_one_snapshot_until_the_next_completion_revision() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let node = tasks.memo(runtime.memo(|_| Ok(3)), |value, _| Ok(value));
    let first = observe(&runtime, node.clone());
    let second = observe(&runtime, node);
    assert_eq!(*runtime.read(&first).unwrap(), (true, None));
    queue.next()();
    assert_eq!(*runtime.read(&second).unwrap(), (true, None));
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&first).unwrap(), (false, Some(3)));
    assert_eq!(*runtime.read(&second).unwrap(), (false, Some(3)));
}
