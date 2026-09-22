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
fn conflated_work_finishes_and_publishes_stale_before_starting_only_the_latest() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1usize);
    let prepare = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let (started, receive) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let node = tasks.memo_conflated(prepare, {
        let calls = calls.clone();
        move |value, cancel| {
            calls.lock().unwrap().push(value);
            if value == 1 {
                started.send(cancel.clone()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
            }
            cancel.check()?;
            Ok(value * 10)
        }
    });
    let parent = observe(&runtime, node);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    let worker = std::thread::spawn(queue.next());
    let cancel = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    for value in 2..=5 {
        input.set(value);
        assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
        assert!(
            cancel.check().is_ok(),
            "input updates must not cancel admitted work"
        );
        assert_eq!(queue.len(), 0);
    }
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(
        *runtime.read(&parent).unwrap(),
        (true, None),
        "publication waits for poll"
    );
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(10)));
    assert_eq!(
        queue.len(),
        1,
        "completion starts the latest request without further input"
    );
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(50)));
    assert_eq!(*calls.lock().unwrap(), vec![1, 5]);
    assert_eq!(queue.len(), 0);
}

#[test]
fn conflated_obsolete_failure_does_not_block_the_latest_request() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(1usize);
    let prepare = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let node = tasks.memo_conflated(prepare, |value, _| {
        if value % 2 == 1 {
            Err(Error::Cancelled)
        } else {
            Ok(value)
        }
    });
    let parent = observe(&runtime, node);
    runtime.read(&parent).unwrap();
    input.set(2);
    runtime.read(&parent).unwrap();
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    queue.next()();
    tasks.poll();
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(2)));
    input.set(3);
    runtime.read(&parent).unwrap();
    queue.next()();
    tasks.poll();
    assert_eq!(
        runtime.read(&parent),
        Err(Error::Cancelled),
        "current failures still propagate"
    );
}

#[test]
fn conflated_work_is_still_cancelled_when_its_owner_closes() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let (started, receive) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node = tasks.memo_conflated(runtime.memo(|_| Ok(1usize)), move |value, cancel| {
        started.send(cancel.clone()).unwrap();
        resumed.lock().unwrap().recv().unwrap();
        cancel.check()?;
        Ok(value)
    });
    let parent = observe(&runtime, node);
    runtime.read(&parent).unwrap();
    let worker = std::thread::spawn(queue.next());
    let cancel = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    drop(tasks);
    assert_eq!(cancel.check(), Err(Error::Cancelled));
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(runtime.read(&parent), Err(Error::Cancelled));
    assert_eq!(queue.len(), 0);
}

#[test]
fn start_condition_defers_latest_request_but_preserves_current_results() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(Some(1usize));
    let allowed = runtime.input(false);
    let prepare = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let permitted = runtime.memo({
        let allowed = allowed.clone();
        move |read| Ok(*allowed.read(read))
    });
    let node =
        tasks.memo_reporting_with_start_condition(prepare, Some(permitted), |input, _, _, _| {
            Ok(input * 10)
        });
    let parent = observe(&runtime, node);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    input.set(Some(2));
    runtime.read(&parent).unwrap();
    assert_eq!(queue.len(), 0, "waiting does not occupy an executor slot");
    allowed.set(true);
    runtime.read(&parent).unwrap();
    assert_eq!(queue.len(), 1);
    queue.next()();
    assert!(tasks.poll());
    let ready = runtime.read(&parent).unwrap();
    assert_eq!(*ready, (false, Some(20)));
    allowed.set(false);
    assert!(Rc::ptr_eq(&ready, &runtime.read(&parent).unwrap()));
    allowed.set(true);
    assert!(Rc::ptr_eq(&ready, &runtime.read(&parent).unwrap()));
    assert_eq!(
        queue.len(),
        0,
        "permission alone never repeats completed work"
    );

    input.set(Some(3));
    runtime.read(&parent).unwrap();
    allowed.set(false);
    input.set(Some(4));
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(20)));
    queue.next()(); // Retire the admitted slot; its obsolete work was removed.
    assert!(!tasks.poll());
    assert_eq!(queue.len(), 0);
    input.set(None);
    runtime.read(&parent).unwrap();
    allowed.set(true);
    runtime.read(&parent).unwrap();
    assert_eq!(queue.len(), 0, "permission cannot bypass unready inputs");
    input.set(Some(5));
    runtime.read(&parent).unwrap();
    queue.next()();
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(50)));
}

#[test]
fn changed_inputs_cancel_running_work_while_replacement_is_deferred() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(Some(1usize));
    let allowed = runtime.input(true);
    let prepare = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let permitted = runtime.memo({
        let allowed = allowed.clone();
        move |read| Ok(*allowed.read(read))
    });
    let (started, receive) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node = tasks.memo_reporting_with_start_condition(
        prepare,
        Some(permitted),
        move |input, cancel, publish, _| {
            if input == 1 {
                started.send(cancel.clone()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                assert_eq!(publish(99), Err(Error::Cancelled));
            }
            Ok(input * 10)
        },
    );
    let parent = observe(&runtime, node);
    runtime.read(&parent).unwrap();
    let worker = std::thread::spawn(queue.next());
    let cancel = receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    allowed.set(false);
    runtime.read(&parent).unwrap();
    assert!(
        cancel.check().is_ok(),
        "current admitted work remains valid"
    );
    input.set(Some(2));
    runtime.read(&parent).unwrap();
    assert_eq!(cancel.check(), Err(Error::Cancelled));
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert!(!tasks.poll(), "obsolete publications are rejected");
    assert_eq!(queue.len(), 0);
    allowed.set(true);
    runtime.read(&parent).unwrap();
    queue.next()();
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(20)));
}

#[test]
fn releasing_start_condition_collects_inline_completion() {
    let runtime = Runtime::default();
    let tasks = Tasks::new(&runtime, Executor::inline(), || {});
    let allowed = runtime.input(false);
    let permitted = runtime.memo({
        let allowed = allowed.clone();
        move |read| Ok(*allowed.read(read))
    });
    let node = tasks.memo_reporting_with_start_condition(
        runtime.memo(|_| Ok(Some(7usize))),
        Some(permitted),
        |input, _, _, _| Ok(input),
    );
    let parent = observe(&runtime, node);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    allowed.set(true);
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(7)));
}

#[test]
fn waiting_preparation_submits_nothing_and_clears_queued_work() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(None::<usize>);
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let calls = Arc::new(AtomicUsize::new(0));
    let node = tasks.memo_reporting_when_ready(prepared, {
        let calls = calls.clone();
        move |value, _, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(value * 10)
        }
    });
    let parent = observe(&runtime, node);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    assert_eq!(queue.len(), 0);
    input.set(Some(1));
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    assert_eq!(queue.len(), 1);
    input.set(None);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, None));
    queue.next()(); // The executor's queued slot now contains no work.
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!tasks.poll());
    input.set(Some(2));
    runtime.read(&parent).unwrap();
    queue.next()();
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), (false, Some(20)));
    input.set(None);
    assert_eq!(*runtime.read(&parent).unwrap(), (true, Some(20)));
    assert_eq!(queue.len(), 0);
    // Even resuming identical inputs must launch again after cancellation.
    input.set(Some(2));
    runtime.read(&parent).unwrap();
    assert_eq!(queue.len(), 1);
}

#[test]
fn waiting_preparation_cancels_running_work_and_rejects_late_reports() {
    let runtime = Runtime::default();
    let queue = Queue::default();
    let tasks = Tasks::new(&runtime, queue.executor(), || {});
    let input = runtime.input(Some(1usize));
    let prepared = runtime.memo({
        let input = input.clone();
        move |read| Ok(*input.read(read))
    });
    let (started, start) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let resumed = Mutex::new(resumed);
    let node =
        tasks.memo_reporting_when_ready(prepared, move |value, cancel, publish, progress| {
            if value == 1 {
                publish(11)?;
                progress(Progress {
                    completed: 1,
                    total: 2,
                });
                started.send(cancel.clone()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                assert_eq!(publish(99), Err(Error::Cancelled));
                progress(Progress {
                    completed: 2,
                    total: 2,
                });
            }
            Ok(value * 10)
        });
    let parent = runtime.memo(move |read| {
        let value = match &*node.read(read)? {
            Availability::Pending { previous } => (true, previous.as_deref().copied()),
            Availability::Ready(value) | Availability::Refining(value) => (false, Some(**value)),
        };
        Ok((value, node.progress(read)?))
    });
    runtime.read(&parent).unwrap();
    let worker = std::thread::spawn(queue.next());
    let cancel = start
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(tasks.poll());
    assert_eq!(runtime.read(&parent).unwrap().0, (false, Some(11)));
    input.set(None);
    assert_eq!(*runtime.read(&parent).unwrap(), ((true, Some(11)), None));
    assert_eq!(cancel.check(), Err(Error::Cancelled));
    resume.send(()).unwrap();
    worker.join().unwrap();
    assert!(
        !tasks.poll(),
        "cancelled completion cannot wake a dependent"
    );
    assert_eq!(queue.len(), 0);
    input.set(Some(2));
    runtime.read(&parent).unwrap();
    queue.next()();
    assert!(tasks.poll());
    assert_eq!(*runtime.read(&parent).unwrap(), ((false, Some(20)), None));
}

#[test]
fn work_progress_keeps_values_and_obeys_revision_and_generation_boundaries() {
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
    let node = tasks.memo_reporting(prepared, move |value, _, publish, progress| {
        if value == 1 {
            progress(Progress {
                completed: 2,
                total: 10,
            });
            stage.send(()).unwrap();
            resumed.lock().unwrap().recv().unwrap();
            publish(7)?;
            progress(Progress {
                completed: 5,
                total: 10,
            });
            stage.send(()).unwrap();
            resumed.lock().unwrap().recv().unwrap();
            // A new refinement resets the bar without replacing the current image.
            progress(Progress {
                completed: 0,
                total: 20,
            });
            stage.send(()).unwrap();
            resumed.lock().unwrap().recv().unwrap();
            // This old request has now been cancelled. Neither report may escape.
            progress(Progress {
                completed: 20,
                total: 20,
            });
            assert_eq!(publish(999), Err(Error::Cancelled));
        } else {
            progress(Progress {
                completed: 1,
                total: 1,
            });
        }
        Ok(value * 10)
    });
    let parent = runtime.memo(move |read| {
        let value = match &*node.read(read)? {
            Availability::Pending { previous } => (false, previous.as_deref().copied()),
            Availability::Refining(value) => (false, Some(**value)),
            Availability::Ready(value) => (true, Some(**value)),
        };
        Ok((value, node.progress(read)?))
    });
    let read = || runtime.read(&parent).unwrap();
    let wait = || {
        staged
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
    };
    let first = read();
    assert_eq!(*first, ((false, None), None));
    let worker = std::thread::spawn(queue.next());
    wait();
    assert!(
        Rc::ptr_eq(&first, &read()),
        "counts don't mutate an observed revision"
    );
    assert!(tasks.poll());
    assert_eq!(
        *read(),
        (
            (false, None),
            Some(Progress {
                completed: 2,
                total: 10
            })
        )
    );
    resume.send(()).unwrap();
    wait();
    tasks.poll();
    assert_eq!(
        *read(),
        (
            (false, Some(7)),
            Some(Progress {
                completed: 5,
                total: 10
            })
        ),
        "coalesced counts must not discard the preceding published value"
    );
    resume.send(()).unwrap();
    wait();
    tasks.poll();
    assert_eq!(
        *read(),
        (
            (false, Some(7)),
            Some(Progress {
                completed: 0,
                total: 20
            })
        )
    );
    input.set(2);
    assert_eq!(
        *read(),
        ((false, Some(7)), None),
        "new jobs don't inherit old progress"
    );
    resume.send(()).unwrap();
    worker.join().unwrap();
    tasks.poll();
    assert_eq!(*read(), ((false, Some(7)), None));
    queue.next()();
    tasks.poll();
    assert_eq!(
        *read(),
        ((true, Some(20)), None),
        "completion clears the bar"
    );
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
fn concurrent_slot_handoffs_neither_overlap_jobs_nor_lose_the_last_job() {
    let slot = Arc::new(Slot {
        executor: Executor::threaded(std::num::NonZeroUsize::new(2).unwrap()).unwrap(),
        scheduled: AtomicBool::new(false),
        pending: ConcurrentQueue::bounded(1),
    });
    let active = Arc::new(AtomicUsize::new(0));
    for _ in 0..20_000 {
        let active = active.clone();
        slot.submit(Box::new(move || {
            assert_eq!(active.fetch_add(1, Ordering::SeqCst), 0);
            std::thread::yield_now();
            assert_eq!(active.fetch_sub(1, Ordering::SeqCst), 1);
        }));
    }
    let (finished, received) = mpsc::channel();
    slot.submit(Box::new(move || {
        assert_eq!(active.load(Ordering::SeqCst), 0);
        finished.send(()).unwrap();
    }));
    received
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap();
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
