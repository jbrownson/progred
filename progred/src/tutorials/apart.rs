//! tutorial.html's second half, which takes the forest apart, and the
//! peel on the home page.

use super::*;

fn peel_forest(page: &str) {
    let mut peel = Embed::open(page, "peel");
    let leaves = peniko::Brush::from(peniko::Color::from_rgb8(0x39, 0x9b, 0x75));

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
    assert_eq!(source.start, peel.location(&["first", "follow"]).start);

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

/// Clicking an anonymous lambda's λ names it with what's typed, even a word
/// that also names a function.
fn name_a_lambda(page: &str) {
    let mut peel = Embed::open(page, "peel");
    let name = peel.path(&["first", "follow", "controls", "name"]);
    peel.click(&name);
    peel.type_text("slider");
    peel.press(NamedKey::Enter);
    assert_eq!(peel.value(&name), crate::libraries::text::value("slider"));
}

#[test]
fn home_page() {
    peel_forest("index.html");
    name_a_lambda("index.html");
}

#[test]
fn peel() {
    peel_forest("tutorial.html");
    name_a_lambda("tutorial.html");
}

#[test]
fn cells() {
    let mut cells = page("cells");
    let original = cells.positions(&[]);
    let shared = cells.id("shared");
    let references = |cells: &Embed, cell: CellId| {
        cells
            .value(&[])
            .as_list()
            .unwrap()
            .values()
            .filter(|value| value.as_cell() == Some(cell))
            .count()
    };

    cells.task("shared-edit");
    cells.double_click(&cells.element(&[], 0, &[FOLLOW]));
    cells.type_text("8");
    assert_eq!(
        crate::libraries::f64::read(&cells.cell("shared")),
        Some(8.0)
    );
    assert_eq!(references(&cells, shared), 2);

    cells.task("create");
    cells.click_between(&[], 0);
    cells.type_text("(");
    let made = cells
        .positions(&[])
        .into_iter()
        .find(|position| !original.contains(position))
        .expect("a new item");
    let fresh = cells
        .value(&[Step::Element(made.clone())])
        .as_cell()
        .unwrap();
    assert_ne!(fresh, shared);
    // Typing goes straight inside the new cell.
    cells.type_text("11");
    cells.press(NamedKey::Enter);
    assert_eq!(
        cells.number(&[Step::Element(made.clone()), FOLLOW]),
        Some(11.0)
    );

    cells.task("link");
    cells.click_between(&[], 1);
    // A parenthesis: inside the cell's box, before its value.
    let parenthesized = cells.rect(&[Step::Element(made.clone())]);
    let inside = cells.rect(&[Step::Element(made.clone()), FOLLOW]);
    cells.pick_at(Point::new(
        (parenthesized.x0 + inside.x0) / 2.0,
        parenthesized.center().y,
    ));
    assert_eq!(references(&cells, fresh), 2);

    cells.task("linked-edit");
    let partner = cells
        .positions(&[])
        .into_iter()
        .find(|position| {
            *position != made
                && cells.value(&[Step::Element(position.clone())]).as_cell() == Some(fresh)
        })
        .unwrap();
    cells.double_click(&[Step::Element(partner), FOLLOW]);
    cells.type_text("12");
    assert_eq!(cells.number(&[Step::Element(made), FOLLOW]), Some(12.0));
    assert_eq!(
        crate::libraries::f64::read(&cells.cell("shared")),
        Some(8.0)
    );
}

#[test]
fn names() {
    let mut names = page("names");
    let base = names.drawn_under(&[names.key("third")]);
    let calls = names.cell("calls");

    // In the recipe, click x and rename it.
    names.task("rename");
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
    bytes.task("hex");
    bytes.double_click(&raw);
    bytes.type_text("4d617273");
    assert_eq!(bytes.cell("raw"), Value::from(b"Mars".to_vec()));
    assert_eq!(
        crate::libraries::text::read(&reading(&bytes, "third")),
        Some("Mars")
    );

    let record = |bytes: &Embed, slot, key| {
        let mut path = crate::test_examples::result_path(&[bytes.key(slot)]);
        path.push(crate::occurrence::Step::Key(key));
        bytes.drawn(&path)
    };
    let color = crate::libraries::color::vocabulary::RGBA;
    let float = crate::libraries::f32::vocabulary::F32;
    assert!(!record(&bytes, "fourth", color) && !record(&bytes, "fifth", float));

    // Give it only three bytes, and the text still reads, but the color and
    // the f32 decline and show the record instead.
    bytes.task("three");
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
    let view = numbers.rect(&crate::test_examples::result_path(&[numbers.key("fifth")]));
    let before = numbers.images();
    numbers.drag(
        view.center(),
        &[view.center() + kurbo::Vec2::new(40.0, 0.0)],
    );
    assert_ne!(numbers.images(), before, "it turns");

    // And change a radius.
    let before = numbers.images();
    let radius = numbers.path(&[
        "fifth",
        "expression",
        "value",
        "expression",
        "shape_left",
        "unquote",
        "radius",
    ]);
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
    let argument = |embed: &Embed, slot| embed.path(&[slot, "expression", "x"]);
    let result = |embed: &Embed, slot| embed.result(&[embed.key(slot)]);
    assert!(::grap::absent::is_absent(&result(&unfinished, "third")));

    // Fill the empty x, and its absent becomes a number.
    unfinished.task("fill");
    unfinished.click(&argument(&unfinished, "third"));
    unfinished.type_text("4");
    unfinished.press(NamedKey::Enter);
    assert_eq!(
        crate::libraries::f64::read(&result(&unfinished, "third")),
        Some(8.0)
    );

    // The call given "three" returns an absent too: the x * 2 inside scale
    // couldn't use its arguments, because "three" is not an f64.
    let fourth = result(&unfinished, "fourth");
    assert_eq!(
        ::grap::absent::reason(&fourth),
        Some(::grap::absent::INVALID_ARGUMENTS)
    );
    assert_eq!(
        ::grap::absent::root_causes(&fourth)
            .into_iter()
            .map(::grap::absent::reason)
            .collect::<Vec<_>>(),
        [Some(crate::libraries::grap::numbers::f64::NOT_F64)]
    );
    assert!(unfinished.drawn(&crate::test_examples::result_path(&[
        unfinished.key("fourth")
    ])));
}

/// Change the planet's moons and the color its view paints it.
fn draw_planets(page: &str, id: &str) {
    let mut views = Embed::open(page, id);
    let gray = peniko::Brush::from(peniko::Color::from_rgb8(0x8a, 0x8a, 0x8a));
    let moons = |views: &Embed| {
        views
            .circles()
            .into_iter()
            .filter(|(_, _, brush)| *brush == gray)
            .count()
    };
    assert_eq!(moons(&views), 2);

    // Change moons to 5.
    views.task("moons");
    views.double_click(&views.path(&["first", "follow", "moons"]));
    views.type_text("5");
    assert_eq!(moons(&views), 5);

    // Then change the planet's color inside planet drawing.
    views.task("color");
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
    draw_planets("tutorial.html", "views");
}

#[test]
fn yours() {
    let yours = page("yours");
    assert!(!yours.circles().is_empty(), "the forest is drawn");
    assert!(yours.drawn(&[yours.key("first")]), "with all of its code");
}
