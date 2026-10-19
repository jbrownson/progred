use super::*;

#[test]
fn every_convention_updates_a_missing_location_from_text() {
    use crate::libraries::{f32, f64, u64};
    assert_eq!(edit("2.5", None, f64::value), Some(f64::value(2.5)));
    assert_eq!(edit("2.5", None, f32::value), Some(f32::value(2.5)));
    assert_eq!(edit("2", None, u64::value), Some(u64::value(2)));
}

#[test]
fn numeric_completions_offer_only_what_reads_as_a_number() {
    for (complete, zero, representation) in [
        (
            (|query| crate::libraries::f32::convention().completions(query))
                as fn(&str) -> Vec<crate::display::Completion>,
            crate::libraries::f32::value(0.0),
            crate::libraries::f32::vocabulary::F32,
        ),
        (
            |query| crate::libraries::f64::convention().completions(query),
            crate::libraries::f64::value(0.0),
            crate::libraries::f64::vocabulary::F64,
        ),
        (
            |query| crate::libraries::u64::convention().completions(query),
            crate::libraries::u64::value(0),
            crate::libraries::u64::vocabulary::U64,
        ),
    ] {
        for query in ["0", " 0 "] {
            let offers = complete(query);
            let [offer] = offers.as_slice() else {
                panic!("expected one {representation} zero offer for {query:?}");
            };
            assert_eq!(offer.display, "0".into());
            assert_eq!(offer.detail, Some(representation.into()));
            assert_eq!(offer.preview.as_ref(), Some(&zero));
        }
        // An empty query reads as no number, nor does one that isn't.
        for query in ["", " \t\n", "not a number", "-", ".", "1e"] {
            assert!(complete(query).is_empty(), "{representation}: {query:?}");
        }
    }
}

#[test]
fn scrubbing_uses_the_active_decimal_precision() {
    let mut scrub = NumberScrub::new(100.0);

    assert_eq!(
        scrub
            .update(ScrubEvent {
                movement_x: 4.0,
                distance_y: 0.0,
            })
            .value,
        101.0,
    );
    assert_eq!(
        NumberScrub::new(0.5)
            .update(ScrubEvent {
                movement_x: 4.0,
                distance_y: 0.0,
            })
            .value,
        0.51,
    );

    let one_decimal_place = PIXELS_PER_DECADE;
    let mut scrub = NumberScrub::new(100.0);
    assert_eq!(
        scrub
            .update(ScrubEvent {
                movement_x: 4.0,
                distance_y: one_decimal_place,
            })
            .value,
        100.1,
    );
    assert_eq!(
        scrub
            .update(ScrubEvent {
                movement_x: 16.0,
                distance_y: -one_decimal_place,
            })
            .value,
        120.0,
    );
}

#[test]
fn rightward_motion_never_lowers_the_displayed_value() {
    let mut scrub = NumberScrub::new(100.0);
    let first = scrub
        .update(ScrubEvent {
            movement_x: 16.0,
            distance_y: 0.0,
        })
        .value;
    let scale_changed = scrub
        .update(ScrubEvent {
            movement_x: 0.0,
            distance_y: -PIXELS_PER_DECADE,
        })
        .value;
    let moved_right = scrub
        .update(ScrubEvent {
            movement_x: 0.1,
            distance_y: -PIXELS_PER_DECADE,
        })
        .value;

    assert_eq!(first, 104.0);
    assert_eq!(scale_changed, first);
    assert!(moved_right >= scale_changed);
}

#[test]
fn float_spelling_retains_the_active_decimal_precision() {
    assert_eq!(100.0.spelling(0.1), "100.0");
    assert_eq!(100.1.spelling(0.1), "100.1");
    assert_eq!(100.0.spelling(1.0), "100");
    assert_eq!(110.0.spelling(10.0), "110");
}

#[test]
fn a_gesture_fixes_its_scale_from_the_starting_value() {
    assert_eq!(NumberScrub::new(0.1234838495).base, 0.01);
    assert_eq!(NumberScrub::new(60.0).base, 1.0);
    assert_eq!(NumberScrub::new(123.0).base, 1.0);
    assert_eq!(NumberScrub::new(1234.0).base, 1.0);
    assert_eq!(NumberScrub::new(0_u64).base, 1.0);
    assert_eq!(NumberScrub::new(123_u64).base, 1.0);
}

#[test]
fn integer_precision_stops_at_one() {
    let scrubbed = NumberScrub::new(42_u64).update(ScrubEvent {
        movement_x: 4.0,
        distance_y: 100.0,
    });

    assert_eq!(scrubbed.precision, 1.0);
    assert_eq!(scrubbed.value, 43);
}

#[test]
fn precision_uses_one_two_five_steps() {
    assert_eq!(nice_precision(0.01), 0.01);
    assert_eq!(nice_precision(0.02), 0.02);
    assert_eq!(nice_precision(0.05), 0.05);
    assert_eq!(nice_precision(0.1), 0.1);
    assert_eq!(nice_precision(2.0), 2.0);
    assert_eq!(nice_precision(5.0), 5.0);
}

#[test]
fn vertical_decades_spread_out_as_they_get_coarser() {
    let close = |left: f64, right: f64| (left - right).abs() < 1e-12;

    assert!(close(vertical_decades(-24.0), 1.0));
    assert!(close(vertical_decades(-60.0), 2.0));
    assert!(close(vertical_decades(-114.0), 3.0));
    assert!(close(vertical_decades(60.0), -2.0));
}

#[test]
fn coarse_precision_grows_horizontal_sensitivity_sublinearly() {
    let horizontal_scale = |gain: f64| gain / gain.max(1.0).cbrt();

    assert_eq!(horizontal_scale(0.01), 0.01);
    assert_eq!(horizontal_scale(1.0), 1.0);
    assert!((horizontal_scale(1_000.0) - 100.0).abs() < 1e-12);
}

mod frames;
mod typing;
