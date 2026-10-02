//! The peel, on the home page and lab/peel.html, and the rest of
//! lab/peel.html.

use super::*;

fn page(id: &str) -> Embed {
    Embed::open("lab/peel.html", id)
}

fn peel_forest(page: &str) {
    let mut peel = Embed::open(page, "peel");
    let leaves = puri::Brush::from(puri::Color::from_rgb8(0x39, 0x9b, 0x75));

    // Drag its slider to grow it.
    let before = peel.circles();
    let slider = peel.slider(peel.rect(&[peel.key("second")]));
    peel.drag(
        slider.center(),
        &[Point::new(slider.x1 - 30.0, slider.center().y)],
    );
    assert_ne!(peel.circles(), before, "the forest grows");

    // Hold Ctrl and click a tree to find the code that drew it.
    let (tree, _, _) = peel
        .circles()
        .into_iter()
        .find(|(_, _, brush)| *brush == leaves)
        .expect("a tree");
    peel.pick_at(tree);
    let source = peel.source_selection().expect("the code that drew it");
    assert_eq!(source[..2], [peel.key("first"), FOLLOW]);

    // Click the 7 after tree count, then peel all the way down: it stays
    // selected while its drawing changes to eight bytes.
    let calls = peel.path(&["first", "follow", "forest_drawing", "body", "expressions"]);
    let count = peel.element(&calls, 3, &[peel.key("count")]);
    peel.click(&count);
    assert_eq!(peel.selection(), count);
    for depth in 1..=Embed::layers() {
        peel.peel(depth);
        assert_eq!(peel.selection(), count, "selected after peeling {depth}");
        assert!(peel.drawn(&count), "drawn after peeling {depth}");
    }
    let mut bytes = count;
    bytes.push(Step::Key(crate::libraries::f64::vocabulary::F64));
    assert!(peel.drawn(&bytes));
}

#[test]
fn home_page() {
    peel_forest("index.html");
}

#[test]
fn forest() {
    peel_forest("lab/peel.html");
}

#[test]
fn cells() {
    let mut cells = page("cells");
    let calls = cells.path(&["second", "follow", "expressions"]);
    let one = |cells: &Embed, index| {
        cells.element(&calls, index, &cells.path(&["height_param", "right"]))
    };

    // Click one (1); the other two light up, because it's one value.
    let mut first = one(&cells, 0);
    first.push(FOLLOW);
    cells.click(&first);
    let lit = cells.lit();
    for index in 1..3 {
        let other = one(&cells, index);
        assert!(
            lit.iter().any(|path| path.starts_with(&other)),
            "{other:?} lights up: {lit:?}"
        );
    }

    // Change it to 0.5.
    cells.double_click(&first);
    cells.type_text("0.5");
    assert_eq!(
        crate::libraries::f64::read(&cells.cell("growth")),
        Some(0.5)
    );
}

#[test]
fn names() {
    let mut names = page("names");
    let base = names.drawn_under(&[names.key("third")]);
    let calls = names.cell("calls");

    // In the recipe, click x and rename it.
    let parameters = names.path(&["first", "follow", "params"]);
    names.double_click(&names.element(&parameters, 0, &[]));
    names.type_text("position");
    assert_eq!(
        crate::libraries::name::read(&names.cell("x_param")),
        Some("position")
    );
    // Every call's label follows, because the calls point at x, not its name.
    assert_eq!(names.cell("calls"), calls);
    // The base view doesn't change at all.
    assert_eq!(names.drawn_under(&[names.key("third")]), base);
}

#[test]
fn bytes() {
    let mut bytes = page("bytes");
    let reading = |bytes: &Embed, slot| bytes.rendered(&[bytes.key(slot)]);
    let raw = [bytes.key("first"), FOLLOW];

    // Click the bytes at the top and type new hex digits: every reading
    // follows.
    bytes.double_click(&raw);
    bytes.type_text("4d617273");
    assert_eq!(bytes.cell("raw"), Value::from(b"Mars".to_vec()));
    assert_eq!(
        crate::libraries::text::read(&reading(&bytes, "third")),
        Some("Mars")
    );

    let record = |bytes: &Embed, slot, key| {
        bytes.drawn(&[
            bytes.key(slot),
            Step::Key(crate::libraries::presentation::vocabulary::RESULT),
            Step::Key(key),
        ])
    };
    let color = crate::libraries::color::vocabulary::RGBA;
    let float = crate::libraries::f32::vocabulary::F32;
    assert!(!record(&bytes, "fourth", color) && !record(&bytes, "fifth", float));

    // Give it only three bytes, and the text still reads, but the color and
    // the f32 decline and show the record instead.
    bytes.press(NamedKey::Backspace);
    bytes.press(NamedKey::Backspace);
    assert_eq!(bytes.cell("raw"), Value::from(b"Mar".to_vec()));
    assert_eq!(
        crate::libraries::text::read(&reading(&bytes, "third")),
        Some("Mar")
    );
    assert!(record(&bytes, "fourth", color) && record(&bytes, "fifth", float));
}

#[test]
fn numbers() {
    let mut numbers = page("f64");
    let result = |numbers: &Embed, slot| numbers.result(&[numbers.key(slot)]);

    // Compare 0.1 + 0.2 in f64 and f32.
    let wide = crate::libraries::f64::read(&result(&numbers, "first")).unwrap();
    assert_eq!(wide.to_string(), "0.30000000000000004");
    let narrow = crate::libraries::f32::read(&result(&numbers, "second")).unwrap();
    assert_eq!(narrow.to_string(), "0.3");

    // Drag the shape to turn it.
    let view = numbers.rect(&[
        numbers.key("fifth"),
        Step::Key(crate::libraries::presentation::vocabulary::RESULT),
    ]);
    let before = numbers.images();
    numbers.drag(
        view.center(),
        &[view.center() + kurbo::Vec2::new(40.0, 0.0)],
    );
    assert_ne!(numbers.images(), before, "it turns");

    // And change a radius.
    let before = numbers.images();
    let radius = numbers.path(&["fifth", "evaluate", "value", "left", "radius"]);
    numbers.double_click(&radius);
    numbers.type_text("20");
    assert_eq!(
        crate::libraries::f32::read(&numbers.value(&radius)),
        Some(20.0)
    );
    assert_ne!(numbers.images(), before);
}

#[test]
fn unfinished() {
    let mut unfinished = page("unfinished");
    let argument = |embed: &Embed, slot| embed.path(&[slot, "evaluate", "x"]);
    let result = |embed: &Embed, slot| embed.result(&[embed.key(slot)]);
    assert!(::grap::absent::is_absent(&result(&unfinished, "third")));

    // Fill the empty x, and its absent becomes a number.
    unfinished.click(&argument(&unfinished, "third"));
    unfinished.type_text("4");
    unfinished.press(NamedKey::Enter);
    assert_eq!(
        crate::libraries::f64::read(&result(&unfinished, "third")),
        Some(8.0)
    );

    // Change 3 to a piece of text and read the reason. Typing over the 3
    // only edits its spelling, so empty it to a box first.
    unfinished.double_click(&argument(&unfinished, "second"));
    unfinished.press(NamedKey::Backspace);
    unfinished.press(NamedKey::Backspace);
    unfinished.type_text("\"three\"");
    unfinished.press(NamedKey::Enter);
    assert_eq!(
        unfinished.text(&argument(&unfinished, "second")).as_deref(),
        Some("three")
    );
    assert!(::grap::absent::is_absent(&result(&unfinished, "second")));
}

/// Change the planet's moons and the color its view paints it.
pub(super) fn draw_planets(page: &str, id: &str) {
    let mut views = Embed::open(page, id);
    let gray = puri::Brush::from(puri::Color::from_rgb8(0x8a, 0x8a, 0x8a));
    let moons = |views: &Embed| {
        views
            .circles()
            .into_iter()
            .filter(|(_, _, brush)| *brush == gray)
            .count()
    };
    assert_eq!(moons(&views), 2);

    // Change moons to 5.
    views.double_click(&views.path(&["first", "follow", "moons"]));
    views.type_text("5");
    assert_eq!(moons(&views), 5);

    // Then change the planet's color inside planet drawing.
    let planet = |views: &Embed| {
        views
            .circles()
            .into_iter()
            .find(|(_, circle, _)| circle.radius == 36.0)
            .expect("the planet")
            .2
    };
    let before = planet(&views);
    let drawn = views.element(
        &views.path(&["third", "follow", "body", "expression", "expressions"]),
        0,
        &[views.key("paint")],
    );
    views.pick_color(&drawn);
    assert_ne!(planet(&views), before);
}

#[test]
fn views() {
    draw_planets("lab/peel.html", "views");
}
