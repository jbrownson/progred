//! lab/see.html

use super::*;

fn page(id: &str) -> Embed {
    Embed::open("lab/see.html", id)
}

#[test]
fn text() {
    let mut text = page("see-text");
    let planet = [text.key("first"), text.key("planet")];

    // Click "Mars" and type a new name.
    text.click(&planet);
    text.type_text("Venus");
    let renamed = text.text(&planet).unwrap();
    assert!(renamed != "Mars" && renamed.contains("Venus"), "{renamed}");
}

#[test]
fn numbers() {
    let mut numbers = page("see-numbers");
    let moons = [numbers.key("first"), numbers.key("moons")];

    // Hold Ctrl and drag the 2 left and right: you're holding a number now,
    // not bytes.
    numbers.hold(ModifiersState::CONTROL);
    let start = numbers.text_at(&moons);
    numbers.press_at(start);
    numbers.move_to(start + kurbo::Vec2::new(40.0, 0.0));
    let right = numbers.number(&moons).unwrap();
    assert!(right > 2.0, "{right}");
    numbers.move_to(start - kurbo::Vec2::new(40.0, 0.0));
    let left = numbers.number(&moons).unwrap();
    assert!(left < 2.0, "{left}");
    numbers.release();
    numbers.hold(ModifiersState::empty());
}

#[test]
fn running() {
    let mut running = page("see-running");
    let result = |running: &Embed| {
        crate::libraries::f64::read(&running.result(&[running.key("second")])).unwrap()
    };
    assert_eq!(result(&running), 5.0);

    // Change the 2. The result follows.
    running.double_click(&[
        running.key("second"),
        running.key("evaluate"),
        running.key("right"),
    ]);
    running.type_text("7");
    assert_eq!(result(&running), 10.0);
}

#[test]
fn functions() {
    let mut functions = page("see-functions");
    let calls = functions.cell("calls");
    let parameters = [functions.key("fourth"), FOLLOW, functions.key("params")];

    // In the recipe, click height and rename it. Every call's label follows,
    // because each call points at the parameter, not its name.
    functions.double_click(
        &[
            parameters.as_slice(),
            &[Step::Element(functions.positions(&parameters)[1].clone())],
        ]
        .concat(),
    );
    functions.type_text("size");
    assert_eq!(
        crate::libraries::name::read(&functions.cell("height_param")),
        Some("size")
    );
    assert_eq!(functions.cell("calls"), calls);
}

#[test]
fn pictures() {
    let mut pictures = page("see-pictures");
    let leaves = puri::Brush::from(puri::Color::from_rgb8(0x54, 0x8b, 0x64));

    // Hold Ctrl and click a tree. You land in the code that drew it.
    let (tree, _, _) = pictures
        .circles()
        .into_iter()
        .find(|(_, _, brush)| *brush == leaves)
        .expect("a tree");
    pictures.pick_at(tree);
    let source = pictures.source_selection().expect("the code that drew it");
    assert_eq!(
        source[..3],
        [pictures.key("fourth"), FOLLOW, pictures.key("body")],
        "inside the recipe"
    );
}

#[test]
fn yourself() {
    super::peel::draw_planets("lab/see.html", "see-yourself");
}
