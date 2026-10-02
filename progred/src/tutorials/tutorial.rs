//! tutorial.html

use super::*;

fn page(id: &str) -> Embed {
    Embed::open("tutorial.html", id)
}

#[test]
fn growing_forest() {
    let mut forest = page("growing-forest");
    let leaves = puri::Brush::from(puri::Color::from_rgb8(0x39, 0x9b, 0x75));
    let trees = |forest: &Embed| {
        forest
            .circles()
            .iter()
            .filter(|(_, _, brush)| *brush == leaves)
            .count()
    };

    // Drag the slider to grow it.
    let before = forest.circles();
    let slider = forest.slider(forest.rect(&[forest.key("second")]));
    forest.drag(
        slider.center(),
        &[Point::new(slider.x1 - 30.0, slider.center().y)],
    );
    assert_ne!(forest.circles(), before, "the forest grows");

    // Or change a number and watch the trees follow.
    let planted = trees(&forest);
    let calls = forest.path(&["first", "follow", "forest_drawing", "body", "expressions"]);
    let count = forest.element(&calls, 3, &[forest.key("count")]);
    forest.double_click(&count);
    forest.type_text("9");
    assert_eq!(forest.number(&count), Some(9.0));
    assert!(trees(&forest) > planted, "more trees");
}

#[test]
fn values() {
    let mut values = page("values");
    let [planet, moons, colors] = ["planet", "moons", "colors"].map(|name| [values.key(name)]);

    values.task("rename");
    values.click(&planet);
    values.type_text("Venus");
    let renamed = values.text(&planet).unwrap();
    assert!(renamed != "Mars" && renamed.contains("Venus"), "{renamed}");

    values.task("moons");
    values.double_click(&moons);
    values.type_text("5");
    assert_eq!(values.number(&moons), Some(5.0));

    values.task("add");
    values.click_between(&colors, 0);
    values.type_text("\"blue\"");
    values.press(NamedKey::Enter);
    assert_eq!(values.texts(&colors), ["red", "blue", "orange"]);

    values.task("remove");
    values.double_click(&values.element(&colors, 0, &[]));
    for presses in 1.. {
        values.press(NamedKey::Backspace);
        if values.texts(&colors).len() < 3 {
            break;
        }
        assert!(presses < 3, "still there: {:?}", values.texts(&colors));
    }
    assert_eq!(values.texts(&colors), ["blue", "orange"]);

    values.task("restore");
    values.command("z");
    assert_eq!(values.texts(&colors), ["red", "blue", "orange"]);
}

#[test]
fn projections() {
    let mut views = page("projections");
    let right = |views: &Embed, slot| views.path(&[slot, "follow", "evaluate", "right"]);
    let total = |views: &Embed| {
        crate::libraries::f64::read(&views.result(&views.path(&["first", "follow"])))
    };

    views.task("full");
    views.double_click(&right(&views, "first"));
    views.type_text("4");
    assert_eq!(views.number(&right(&views, "first")), Some(4.0));
    assert_eq!(views.selection()[0], views.key("first"));
    assert_eq!(total(&views), Some(7.0));

    views.task("plain");
    views.double_click(&right(&views, "second"));
    views.type_text("6");
    assert_eq!(views.selection()[0], views.key("second"));
    assert_eq!(total(&views), Some(9.0), "the result on top updates");

    views.task("raw");
    views.click(&right(&views, "third"));
    let selected = views.selection();
    assert_eq!(selected[0], views.key("third"));
    assert!(selected.contains(&views.key("right")), "{selected:?}");
}

#[test]
fn model() {
    let mut model = page("model");
    let at = |model: &Embed, slot, field| model.path(&[slot, "follow", field]);

    model.task("rename");
    model.click(&at(&model, "first", "planet"));
    model.type_text("Venus");
    assert_ne!(model.text(&at(&model, "first", "planet")).unwrap(), "Mars");
    // Its bytes change below.
    let mut bytes = at(&model, "second", "planet");
    bytes.push(Step::Key(crate::libraries::text::vocabulary::UTF8));
    assert!(model.drawn(&bytes));

    model.task("add");
    model.click_between(&at(&model, "first", "colors"), 0);
    model.type_text("\"blue\"");
    model.press(NamedKey::Enter);
    assert_eq!(
        model.texts(&at(&model, "first", "colors")),
        ["red", "blue", "orange"]
    );
    let below = at(&model, "second", "colors");
    for index in 0..3 {
        assert!(model.drawn(&model.element(&below, index, &[])));
    }

    model.task("key");
    // The planet's key, …f94f0, is drawn just left of its bytes.
    let bytes = model.rect(&at(&model, "second", "planet"));
    model.click_at(Point::new(bytes.x0 - 24.0, bytes.center().y));
    assert!(
        model
            .selection()
            .starts_with(&at(&model, "second", "planet"))
    );
    let lit = model.lit();
    assert!(
        lit.iter()
            .any(|path| path.starts_with(&at(&model, "first", "planet"))),
        "the top view selects the same value: {lit:?}"
    );
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
    cells.click(&[Step::Element(made.clone()), FOLLOW]);
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
fn grap() {
    let mut grap = page("grap");
    let right = |grap: &Embed, slot| grap.path(&[slot, "evaluate", "right"]);
    let input = |grap: &Embed| grap.path(&["first", "follow"]);
    let results = |grap: &Embed| {
        ["second", "third"]
            .map(|slot| crate::libraries::f64::read(&grap.result(&[grap.key(slot)])).unwrap())
    };
    assert_eq!(results(&grap), [5.0, 6.0]);

    grap.task("argument");
    grap.double_click(&right(&grap, "second"));
    grap.type_text("4");
    assert_eq!(results(&grap), [7.0, 6.0], "only the sum changes");

    grap.task("shared-edit");
    grap.double_click(&input(&grap));
    grap.type_text("5");
    assert_eq!(results(&grap), [9.0, 10.0]);

    grap.task("equal");
    grap.double_click(&input(&grap));
    grap.type_text("4");
    assert_eq!(results(&grap), [8.0, 8.0]);

    grap.task("hundred");
    grap.double_click(&right(&grap, "third"));
    grap.type_text("25");
    assert_eq!(results(&grap)[1], 100.0);
}

#[test]
fn functions() {
    let mut functions = page("functions");
    let results = |functions: &Embed| {
        ["second", "third"].map(|slot| {
            crate::libraries::f64::read(&functions.result(&[functions.key(slot)])).unwrap()
        })
    };
    assert_eq!(results(&functions), [6.0, 10.0]);

    functions.task("argument");
    functions.double_click(&functions.path(&["second", "evaluate", "x"]));
    functions.type_text("4");
    assert_eq!(results(&functions), [8.0, 10.0]);

    functions.task("body");
    functions.double_click(&functions.path(&["first", "follow", "body", "right"]));
    functions.type_text("3");
    assert_eq!(results(&functions), [12.0, 15.0], "both results follow");

    functions.task("rename");
    let root = functions.editor().model.doc.root.clone();
    let parameters = functions.path(&["first", "follow", "params"]);
    functions.double_click(&functions.element(&parameters, 0, &[]));
    functions.type_text("amount");
    assert_eq!(
        crate::libraries::name::read(&functions.cell("x")),
        Some("amount")
    );
    assert_eq!(
        functions.editor().model.doc.root,
        root,
        "calls keep pointing at the parameter"
    );
    let used = functions.path(&["first", "follow", "body", "left"]);
    assert_eq!(
        functions.drawn_under(&used).len(),
        1,
        "its use in the body stays a reference"
    );
    assert_eq!(
        results(&functions),
        [12.0, 15.0],
        "the results don't change"
    );

    functions.task("nest");
    let argument = functions.path(&["third", "evaluate", "x"]);
    functions.double_click(&argument);
    functions.press(NamedKey::Backspace);
    functions.press(NamedKey::Backspace);
    functions.click(&argument);
    functions.type_text("scale");
    functions.press(NamedKey::Enter);
    functions.type_text("4");
    functions.press(NamedKey::Enter);
    let inner = functions.value(&argument);
    assert_eq!(
        inner
            .as_record()
            .and_then(|fields| fields.get(&::grap::vocabulary::FUNCTION))
            .and_then(Value::as_cell),
        Some(functions.id("scale")),
        "{inner:?}"
    );
    assert_eq!(results(&functions)[1], 36.0);
}

#[test]
fn drawing() {
    let mut drawing = page("drawing");
    let dots = |drawing: &Embed| {
        drawing
            .circles()
            .into_iter()
            .map(|(_, circle, _)| (circle.center.x, circle.radius))
            .collect::<Vec<_>>()
    };
    let calls = drawing.path(&["second", "follow", "expressions"]);
    assert_eq!(dots(&drawing), [(60.0, 24.0), (160.0, 24.0)]);

    drawing.task("argument");
    drawing.double_click(&drawing.element(&calls, 0, &[drawing.key("x")]));
    drawing.type_text("80");
    assert_eq!(
        dots(&drawing),
        [(80.0, 24.0), (160.0, 24.0)],
        "only that dot moves"
    );

    drawing.task("body");
    drawing.double_click(&drawing.path(&[
        "first",
        "follow",
        "body",
        "shape",
        "expression",
        "circle",
        "radius",
    ]));
    drawing.type_text("36");
    assert_eq!(
        dots(&drawing),
        [(80.0, 36.0), (160.0, 36.0)],
        "both dots change"
    );

    drawing.task("add");
    drawing.click_between(&calls, 0);
    drawing.type_text("dot");
    drawing.press(NamedKey::Enter);
    drawing.type_text("200");
    drawing.press(NamedKey::Enter);
    let mut placed = dots(&drawing);
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(placed, [(80.0, 36.0), (160.0, 36.0), (200.0, 36.0)]);

    drawing.task("source");
    let (dot, _, _) = drawing.circles()[0].clone();
    drawing.pick_at(dot);
    assert_eq!(
        drawing.source_selection(),
        Some(drawing.path(&["first", "follow", "body"]))
    );
}

#[test]
fn forest() {
    let mut forest = page("forest");
    let green = puri::Brush::from(puri::Color::from_rgb8(0x54, 0x8b, 0x64));
    let leaves = |forest: &Embed| {
        forest
            .circles()
            .into_iter()
            .map(|(_, circle, brush)| (circle.center.y, brush))
            .collect::<Vec<_>>()
    };
    let calls = forest.path(&["first", "follow", "expressions"]);
    assert_eq!(
        leaves(&forest),
        [80.0, 50.0, 68.0].map(|y| (y, green.clone()))
    );

    forest.task("height");
    forest.double_click(&forest.element(&calls, 0, &[forest.key("height")]));
    forest.type_text("100");
    assert_eq!(
        leaves(&forest),
        [40.0, 50.0, 68.0].map(|y| (y, green.clone())),
        "only that tree grows"
    );

    forest.task("color");
    let body = forest.path(&["second", "follow", "body", "expressions"]);
    forest.pick_color(&forest.element(&body, 1, &[forest.key("paint")]));
    let painted = leaves(&forest);
    assert!(
        painted
            .iter()
            .all(|(_, brush)| *brush == painted[0].1 && *brush != green),
        "every tree changes: {painted:?}"
    );

    forest.task("plant");
    forest.click_between(&calls, 0);
    forest.type_text("tree");
    forest.press(NamedKey::Enter);
    forest.type_text("180");
    forest.press(NamedKey::Enter);
    let planted = (0..4)
        .find(|index| {
            forest.number(&forest.element(&calls, *index, &[forest.key("x")])) == Some(180.0)
        })
        .expect("a tree at 180");
    let height = forest.element(&calls, planted, &[forest.key("height")]);
    forest.click(&height);
    forest.type_text("50");
    forest.press(NamedKey::Enter);
    assert_eq!(forest.number(&height), Some(50.0));
    assert_eq!(forest.circles().len(), 4);
}
