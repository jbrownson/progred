//! story.html

use super::*;

fn page(id: &str) -> Embed {
    Embed::open("story.html", id)
}

/// The painted circles' centers and radii, in their drawing's coordinates.
fn dots(embed: &Embed) -> Vec<(Point, f64)> {
    embed
        .circles()
        .into_iter()
        .map(|(_, circle, _)| (circle.center, circle.radius))
        .collect()
}

#[test]
fn growing() {
    let mut forest = page("growing");
    let before = forest.circles();
    let drawing = forest.rect(&[forest.key("second")]);
    let slider = forest.slider(drawing);
    forest.drag(
        slider.center(),
        &[Point::new(slider.x1 - 30.0, slider.center().y)],
    );
    assert_ne!(forest.circles(), before, "the forest grows");
}

#[test]
fn dot() {
    let mut dot = page("dot");
    let x = dot.path(&["first", "follow", "shape", "circle", "x"]);

    dot.task("pick");
    let (center, _, _) = dot.circles()[0].clone();
    dot.pick_at(center);
    assert_eq!(dot.source_selection(), Some(dot.path(&["first", "follow"])));

    dot.task("scrub");
    dot.hold(ModifiersState::CONTROL);
    let start = dot.text_at(&x);
    dot.press_at(start);
    let mut farthest = 60.0;
    for step in 1..=6 {
        dot.move_to(start + kurbo::Vec2::new(step as f64 * 5.0, 0.0));
        let held = dot.number(&x).unwrap();
        assert_eq!(dots(&dot)[0].0.x, held, "the dot slides as you drag");
        farthest = held;
    }
    assert!(farthest > 60.0, "dragging right raises it: {farthest}");
    dot.move_to(start + kurbo::Vec2::new(10.0, 0.0));
    assert!(dot.number(&x).unwrap() < farthest, "and left lowers it");
    assert_eq!(dots(&dot)[0].0.x, dot.number(&x).unwrap());
    dot.release();
    dot.hold(ModifiersState::empty());
    assert_ne!(dot.number(&x), Some(60.0));
}

#[test]
fn copies() {
    let mut copies = page("copies");
    let program = copies.path(&["first", "follow", "expressions"]);
    assert_eq!(
        dots(&copies).iter().map(|dot| dot.1).collect::<Vec<_>>(),
        [24.0, 24.0]
    );

    copies.task("bigger");
    for index in 0..2 {
        let radius = copies.element(
            &program,
            index,
            &copies.path(&["shape", "circle", "radius"]),
        );
        copies.double_click(&radius);
        copies.type_text("36");
    }
    assert_eq!(
        dots(&copies).iter().map(|dot| dot.1).collect::<Vec<_>>(),
        [36.0, 36.0]
    );
}

#[test]
fn recipe() {
    let mut recipe = page("recipe");

    recipe.task("bigger");
    recipe.double_click(&recipe.path(&[
        "first",
        "follow",
        "body",
        "shape",
        "expression",
        "circle",
        "radius",
    ]));
    recipe.type_text("36");
    assert_eq!(
        dots(&recipe).iter().map(|dot| dot.1).collect::<Vec<_>>(),
        [36.0, 36.0],
        "one edit, both dots"
    );

    recipe.task("pick");
    let (center, _, _) = recipe.circles()[1].clone();
    recipe.pick_at(center);
    assert_eq!(
        recipe.source_selection(),
        Some(recipe.path(&["first", "follow", "body"])),
        "inside dot, on the fill that painted it"
    );
}

#[test]
fn trunks() {
    let mut trunks = page("trunks");
    let root = trunks.editor().model.doc.root.clone();

    trunks.task("rename");
    trunks.double_click(&trunks.path(&["first", "follow", "name"]));
    trunks.type_text("tree");
    assert_eq!(
        crate::libraries::name::read(&trunks.cell("recipe")),
        Some("tree")
    );
    assert_eq!(trunks.editor().model.doc.root, root);
}

#[test]
fn ladder() {
    let mut ladder = page("ladder");
    let sixty = |ladder: &Embed, slot| {
        ladder.element(
            &ladder.path(&[slot, "follow", "expressions"]),
            0,
            &[ladder.key("x_param")],
        )
    };
    let under = |paths: &[Rc<[Step]>], slot: Step| paths.iter().any(|path| path[0] == slot);

    ladder.task("top");
    ladder.click(&sixty(&ladder, "first"));
    let selected = ladder.selection();
    assert_eq!(selected[0], ladder.key("first"));
    assert!(selected.contains(&ladder.key("x_param")));
    let lit = ladder.lit();
    for slot in ["first", "second", "fourth"] {
        assert!(under(&lit, ladder.key(slot)), "{slot} lights up: {lit:?}");
    }

    ladder.task("bottom");
    ladder.click(&sixty(&ladder, "fourth"));
    let selected = ladder.selection();
    assert_eq!(selected[0], ladder.key("fourth"));
    assert!(selected.contains(&ladder.key("x_param")));
}

#[test]
fn heights() {
    let mut heights = page("heights");
    let leaves = heights.element(
        &heights.path(&["first", "follow", "body", "expressions"]),
        1,
        &heights.path(&["shape", "expression", "circle", "y", "unquote", "right"]),
    );

    heights.task("fit");
    heights.hold(ModifiersState::CONTROL);
    let start = heights.text_at(&leaves);
    heights.press_at(start);
    let mut moved = 0.0;
    while (heights.number(&leaves).unwrap() - 90.0).abs() > 3.0 {
        moved += 2.0;
        assert!(
            moved < 300.0,
            "never reached 90: {:?}",
            heights.number(&leaves)
        );
        heights.move_to(start + kurbo::Vec2::new(moved, 0.0));
    }
    heights.release();
    heights.hold(ModifiersState::empty());
    let [_, middle, _] = dots(&heights)[..] else {
        panic!("three trees")
    };
    assert!(
        (middle.0.y - 70.0).abs() <= 3.0,
        "the leaves sit on the trunk: {middle:?}"
    );
}

#[test]
fn growth() {
    let mut growth = page("growth");
    let calls = growth.path(&["second", "follow", "expressions"]);
    let before = dots(&growth);

    growth.task("shrink");
    growth.double_click(&growth.element(
        &calls,
        0,
        &growth.path(&["height_param", "right", "follow"]),
    ));
    growth.type_text("0.5");
    assert_eq!(
        crate::libraries::f64::read(&growth.cell("growth")),
        Some(0.5)
    );
    let after = dots(&growth);
    assert_eq!(after.len(), 3);
    for (shrunk, tree) in after.iter().zip(&before) {
        assert!(
            shrunk.0.y > tree.0.y,
            "every tree shrinks: {before:?} → {after:?}"
        );
    }

    growth.task("plant");
    growth.click_between(&calls, 1);
    growth.type_text("tree");
    growth.press(NamedKey::Enter);
    growth.type_text("150");
    growth.press(NamedKey::Enter);
    let planted = (0..4)
        .find(|index| {
            growth.number(&growth.element(&calls, *index, &[growth.key("x_param")])) == Some(150.0)
        })
        .expect("a tree at 150");
    let height = growth.element(&calls, planted, &[growth.key("height_param")]);
    growth.click(&height);
    growth.type_text("50");
    growth.press(NamedKey::Enter);
    assert_eq!(growth.number(&height), Some(50.0));
    assert_eq!(dots(&growth).len(), 4);
}

#[test]
fn counted() {
    let mut counted = page("counted");
    assert_eq!(dots(&counted).len(), 7);

    counted.task("count");
    counted.double_click(&counted.path(&["second", "follow", "tree_count"]));
    counted.type_text("12");
    assert_eq!(dots(&counted).len(), 12);

    counted.task("pick");
    let (center, _, _) = counted.circles()[5].clone();
    counted.pick_at(center);
    let source = counted.source_selection().expect("the code that drew it");
    assert_eq!(source[..2], counted.path(&["first", "follow"])[..]);
    assert!(source.len() > 2, "{source:?}");
}
