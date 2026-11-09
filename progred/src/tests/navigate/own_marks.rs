//! Every stop draws something of its own, so selecting it looks different
//! from selecting what's inside it: selected, no stop's box is the box of
//! the stop around it or of a stop just inside it.

use crate::driver::Driver;
use crate::occurrence::Step;

#[test]
fn every_stop_in_the_sample_draws_something_of_its_own() {
    check(
        "sample",
        &mut driver(include_str!("../../../../examples/sample.gid")),
    );
}

#[test]
fn every_stop_in_a_new_notebook_draws_something_of_its_own() {
    let mut driver = Driver::new(
        crate::test_editor(gid::Document::default()),
        Driver::VIEWPORT,
    );
    driver.click::<Step>(&[]);
    driver.choose("notebook");
    check("a new notebook", &mut driver);
}

#[test]
#[ignore = "selects every stop in the larger examples, about half a minute"]
fn every_stop_in_the_examples_draws_something_of_its_own() {
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
        check(name, &mut driver(source));
    }
}

fn driver(source: &str) -> Driver {
    let (doc, _) = crate::gid_text::parse(source).unwrap();
    Driver::new(crate::test_editor(doc), Driver::VIEWPORT)
}

/// Each drawn stop, selected, against the nearest stop drawn around it and
/// the nearest drawn inside it.
fn check(name: &str, driver: &mut Driver) {
    let mut paths: Vec<_> = driver
        .drawn_under::<Step>(&[])
        .into_iter()
        .map(|(path, _)| path.to_vec())
        .collect();
    paths.dedup();
    let mut same = Vec::new();
    for path in paths {
        driver.select(&path);
        let drawn: Vec<_> = driver
            .drawn_under::<Step>(&[])
            .into_iter()
            .map(|(path, rect)| (path.to_vec(), rect))
            .collect();
        let Some((_, rect)) = drawn.iter().find(|(other, _)| *other == path) else {
            continue;
        };
        // The stop just around another: the longest of those its path starts
        // with.
        let around = |inner: &[Step]| {
            drawn
                .iter()
                .filter(|(other, _)| other.len() < inner.len() && inner.starts_with(other))
                .max_by_key(|(other, _)| other.len())
        };
        let outer = around(&path).into_iter();
        let inner = drawn
            .iter()
            .filter(|(other, _)| around(other).is_some_and(|(around, _)| *around == path));
        same.extend(
            outer
                .chain(inner)
                .filter(|(_, other)| other == rect)
                .map(|(other, _)| format!("{path:?} with {other:?}")),
        );
    }
    assert!(
        same.is_empty(),
        "{name}: selected stops drawn as a stop beside them: {same:#?}"
    );
}
