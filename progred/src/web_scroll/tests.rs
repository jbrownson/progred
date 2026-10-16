use super::*;
use puri::{Placement, Rect, Vec2};

fn probe(offset: f64, maximum: f64) -> Probe {
    Probe {
        placement: Placement::new(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(0.0, 0.0, 80.0, 100.0),
        ),
        offset: Vec2::new(0.0, offset),
        maximum: Vec2::new(0.0, maximum),
        scale: 2.0,
    }
}

#[test]
fn browser_scroll_capture_checks_direction_clip_and_units_without_mutating() {
    for mode in [0, 1] {
        for _ in 0..4 {
            assert!(captures(
                &[probe(0.0, 10.0)],
                2.0,
                50.0,
                50.0,
                0.0,
                500.0,
                mode
            ));
            assert!(!captures(
                &[probe(0.0, 10.0)],
                2.0,
                50.0,
                50.0,
                0.0,
                -1.0,
                mode
            ));
            assert!(!captures(
                &[probe(10.0, 10.0)],
                2.0,
                50.0,
                50.0,
                0.0,
                1.0,
                mode
            ));
            assert!(captures(
                &[probe(10.0, 10.0)],
                2.0,
                50.0,
                50.0,
                0.0,
                -1.0,
                mode
            ));
        }
    }
    assert!(!captures(&[probe(0.0, 10.0)], 2.0, 90.0, 50.0, 0.0, 1.0, 0));
    assert!(!captures(&[probe(0.0, 0.0)], 2.0, 50.0, 50.0, 0.0, 1.0, 0));
    assert!(!captures(&[probe(0.0, 10.0)], 2.0, 50.0, 50.0, 1.0, 0.0, 0));
    assert!(!captures(&[probe(0.0, 10.0)], 2.0, 50.0, 50.0, 0.0, 1.0, 2));
    assert!(!captures(&[], 2.0, 50.0, 50.0, 0.0, 1.0, 0));
}
