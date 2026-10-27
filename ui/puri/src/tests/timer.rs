use super::*;

#[test]
fn completion_is_one_shot_and_dropping_the_handle_cancels_it() {
    let now = Instant::now();
    let (timer, completion) = Timer::new(now);
    assert!(completion.fire());
    assert!(timer.deadline().is_none());
    assert!(!completion.fire());
    let (timer, cancelled) = Timer::new(now);
    drop(timer);
    assert!(cancelled.deadline().is_none());
    assert!(!cancelled.fire());
}
