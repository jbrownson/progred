//! Every stop draws something of its own, so selecting it looks different
//! from selecting what's inside it: no stop's box is the box of the stop
//! around it.

use crate::driver::Driver;
use crate::occurrence::Step;

#[test]
fn every_stop_in_the_sample_draws_something_of_its_own() {
    check(
        "sample",
        &driver(include_str!("../../../../examples/sample.gid")),
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
    check("a new notebook", &driver);
}

#[test]
#[ignore = "draws the larger examples, a few seconds"]
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
        check(name, &driver(source));
    }
}

fn driver(source: &str) -> Driver {
    let (doc, _) = crate::gid_text::parse(source).unwrap();
    Driver::new(crate::test_editor(doc), Driver::VIEWPORT)
}

/// Each drawn stop against the nearest stop drawn around it.
fn check(name: &str, driver: &Driver) {
    let drawn = driver.drawn_under::<Step>(&[]);
    let paths: Vec<_> = drawn.iter().map(|(path, _)| path.to_vec()).collect();
    let same: Vec<_> = drawn
        .iter()
        .zip(&paths)
        .filter_map(|((_, rect), path)| {
            let (around, around_rect) = drawn
                .iter()
                .zip(&paths)
                .filter(|(_, other)| other.len() < path.len() && path.starts_with(other))
                .max_by_key(|(_, other)| other.len())
                .map(|((_, rect), other)| (other, rect))?;
            (rect == around_rect).then(|| format!("{path:?} in {around:?}"))
        })
        .collect();
    assert!(
        same.is_empty(),
        "{name}: stops drawn as the stop around them: {same:#?}"
    );
}
