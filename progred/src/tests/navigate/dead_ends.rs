//! Wherever the selection is, some arrow takes it somewhere else, so no
//! projection that leaves out a stop strands it.

use crate::driver::Driver;
use crate::occurrence::Step;
use crate::selection::Stage;
use ui_events::keyboard::{Key, Modifiers, NamedKey};

#[test]
fn every_drawn_occurrence_in_the_sample_can_be_left_by_an_arrow() {
    sweep("sample", include_str!("../../../../examples/sample.gid"));
}

#[test]
#[ignore = "sweeps the larger examples, about forty seconds"]
fn every_drawn_occurrence_in_the_examples_can_be_left_by_an_arrow() {
    for (name, source) in [
        (
            "grap-demo",
            include_str!("../../../../examples/grap-demo.gid"),
        ),
        (
            "navigation",
            include_str!("../../../../examples/navigation.gid"),
        ),
        (
            "libraries",
            include_str!("../../../../examples/libraries.gid"),
        ),
    ] {
        sweep(name, source);
    }
}

#[test]
fn tab_and_shift_tab_pass_each_place_in_the_sample_once() {
    tab_sweep("sample", include_str!("../../../../examples/sample.gid"));
}

#[test]
#[ignore = "tabs through every example, about a minute"]
fn tab_and_shift_tab_pass_each_place_in_the_examples_once() {
    for example in crate::command::Example::ALL {
        tab_sweep(example.file_stem(), example.source());
    }
    tab_sweep(
        "navigation",
        include_str!("../../../../examples/navigation.gid"),
    );
}

/// Tabs through every place `source` has to fill in or add to, from nothing
/// selected until Tab has nowhere left, and back with Shift+Tab: each is
/// passed once, and back in the opposite order.
fn tab_sweep(name: &str, source: &str) {
    let (doc, _) = crate::gid_text::parse(source).unwrap();
    let mut driver = Driver::new(crate::test_editor(doc), kurbo::Size::new(1400.0, 1000.0));
    let mut pass = |back: bool| {
        let modifiers = if back {
            Modifiers::SHIFT
        } else {
            Modifiers::empty()
        };
        let mut passed = Vec::new();
        loop {
            driver.keystroke(Key::Named(NamedKey::Tab), modifiers);
            if driver.editor().model.selection.is_none() {
                return passed;
            }
            let at = (driver.selection(), driver.stage());
            assert!(!passed.contains(&at), "{name}: Tab came back to {at:?}");
            passed.push(at);
        }
    };
    let forward = pass(false);
    let mut back = pass(true);
    back.reverse();
    assert_eq!(forward, back, "{name}: Shift+Tab retraces Tab");
}

/// Selects each occurrence `source` draws and checks some arrow leaves it.
fn sweep(name: &str, source: &str) {
    let (doc, _) = crate::gid_text::parse(source).unwrap();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let mut paths: Vec<_> = driver
        .drawn_under::<Step>(&[])
        .into_iter()
        .map(|(path, _)| path.to_vec())
        .collect();
    paths.dedup();
    for path in paths {
        driver.select(&path);
        // A picker's arrows are its own: they choose among its offers.
        if driver.stage() == Some(Stage::Pending) {
            continue;
        }
        let start = driver.selection();
        let leaves = [
            NamedKey::ArrowLeft,
            NamedKey::ArrowRight,
            NamedKey::ArrowUp,
            NamedKey::ArrowDown,
        ]
        .into_iter()
        .any(|key| {
            driver.select(&start);
            // A line's caret moves first, as far as its text goes.
            (0..24).any(|_| {
                driver.keystroke(Key::Named(key.clone()), Modifiers::empty());
                driver.selection() != start
            })
        });
        assert!(leaves, "{name}: no arrow leaves {start:?}");
    }
}
