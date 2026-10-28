use super::*;
use crate::libraries::{f64, u64};
use ::grap;
use gid::Value;

fn draw(key: u64, index: f64) -> Value {
    crate::test_support::test_evaluate(
        &grap::call(
            Value::from(vocabulary::BETWEEN),
            [
                (vocabulary::KEY, u64::value(key)),
                (vocabulary::INDEX, f64::value(index)),
                (vocabulary::MIN, f64::value(-2.0)),
                (vocabulary::MAX, f64::value(3.0)),
            ],
        ),
        |_| None,
        &functions(),
        30,
    )
    .result
    .to_value()
}

#[test]
fn a_draw_depends_only_on_its_key_and_index() {
    assert_eq!(draw(42, 0.0), draw(42, 0.0));
    assert_ne!(draw(42, 0.0), draw(42, 1.0));
    assert_ne!(draw(42, 0.0), draw(43, 0.0));
    let number = f64::read(&draw(42, 0.0)).unwrap();
    assert!((-2.0..3.0).contains(&number));
}

#[test]
fn derived_keys_differ_from_their_parent_and_each_other() {
    let derived = |index: f64| {
        crate::test_support::test_evaluate(
            &grap::call(
                Value::from(vocabulary::DERIVE),
                [
                    (vocabulary::KEY, u64::value(42)),
                    (vocabulary::INDEX, f64::value(index)),
                ],
            ),
            |_| None,
            &functions(),
            30,
        )
        .result
        .to_value()
    };
    assert_eq!(derived(0.0), u64::value(derive(42, 0.0)));
    assert_ne!(derive(42, 0.0), derive(42, 1.0));
    assert_ne!(derive(42, 0.0), 42);
    // A key and a draw from the same key and index are unrelated.
    assert_ne!(
        between(42, 0.0, 0.0, 1.0),
        between(derive(42, 0.0), 0.0, 0.0, 1.0)
    );
}

#[test]
fn draws_are_spread_across_the_range() {
    let draws: Vec<_> = (0..1000)
        .map(|index| between(7, index as f64, 0.0, 1.0))
        .collect();
    assert!(draws.iter().all(|draw| (0.0..1.0).contains(draw)));
    for quarter in 0..4 {
        let low = quarter as f64 / 4.0;
        let count = draws
            .iter()
            .filter(|draw| (low..low + 0.25).contains(*draw))
            .count();
        assert!((200..300).contains(&count), "{count} draws in {low}..");
    }
}
