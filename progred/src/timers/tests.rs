use super::*;
use puri::timer::{Duration, Timers as _};

#[test]
fn early_wakeup_retains_the_earliest_live_deadline() {
    let now = Instant::now();
    let mut timers = Timers::default();
    let early = timers.schedule(now + Duration::from_millis(100));
    let late = timers.schedule(now + Duration::from_millis(200));
    assert!(!timers.fire_due(now + Duration::from_millis(99)));
    assert_eq!(timers.deadline(), early.deadline());
    assert!(timers.fire_due(now + Duration::from_millis(100)));
    assert!(early.deadline().is_none());
    assert_eq!(timers.deadline(), late.deadline());
    assert!(timers.fire_due(now + Duration::from_millis(300)));
    assert!(late.deadline().is_none());
    assert!(timers.deadline().is_none());
    assert!(!timers.fire_due(now + Duration::from_millis(300)));
}

#[test]
fn replacing_requests_does_not_accumulate_or_deliver_cancelled_timers() {
    let now = Instant::now();
    let mut timers = Timers::default();
    let mut debounce = puri_widgets::debounce::Debounce::default();
    for n in 0..1000 {
        debounce.trigger(
            &mut timers,
            now + Duration::from_millis(n),
            Duration::from_millis(150),
        );
        assert_eq!(timers.0.len(), 1);
    }
    assert!(!timers.fire_due(now + Duration::from_millis(150)));
    drop(debounce);
    assert!(timers.deadline().is_none());
    assert!(!timers.fire_due(now + Duration::from_secs(2)));
    assert!(timers.0.is_empty());
}
