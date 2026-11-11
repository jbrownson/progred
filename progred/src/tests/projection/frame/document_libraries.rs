use super::*;
use crate::sample::{PLANET_VIEW, planets};
use gid::Step;

fn drawn<S>(world: &mut crate::Editor, path: &[S]) -> bool
where
    crate::occurrence::Step: PartialEq<S>,
{
    editing_frame(world).landmarks.landmark(path).is_some()
}

#[test]
fn a_declared_library_draws_its_values_and_descends_into_them() {
    let (mut world, names) = planets(PLANET_VIEW);
    let [mars, note, planet, moons] = ["mars", "note", "planet", "moons"].map(|name| names[name]);
    assert!(
        drawn(&mut world, &[Step::Key(mars), Step::Key(moons)]),
        "the view descends into the moons"
    );
    assert!(
        !drawn(&mut world, &[Step::Key(mars), Step::Key(planet)]),
        "and draws the rest itself"
    );
    assert!(
        drawn(&mut world, &[Step::Key(note), Step::Key(planet)]),
        "a value the view declines draws as before"
    );
}

#[test]
fn the_library_follows_edits_to_its_own_view() {
    let (mut world, names) = planets(PLANET_VIEW);
    let [mars, planet, moons, view] =
        ["mars", "planet", "moons", "planet_view"].map(|name| names[name]);
    assert!(!drawn(&mut world, &[Step::Key(mars), Step::Key(planet)]));
    let (redrawn, _) = planets(&PLANET_VIEW.replace("{key: moons}", "{key: planet}"));
    let edited = redrawn.model.doc.cells.value(view).cloned().unwrap();
    std::rc::Rc::make_mut(&mut world.model.doc)
        .cells
        .set_value(view, edited);
    assert!(drawn(&mut world, &[Step::Key(mars), Step::Key(planet)]));
    assert!(!drawn(&mut world, &[Step::Key(mars), Step::Key(moons)]));
}

#[test]
fn a_view_that_draws_its_own_value_ends() {
    // Descending to the same value, the library steps aside there.
    let (mut world, names) = planets("{isa: call, function: descend_path, steps: []}");
    assert!(drawn(
        &mut world,
        &[Step::Key(names["mars"]), Step::Key(names["moons"])]
    ));
    // Returning it draws it as a result, with the loaded libraries.
    let (mut world, names) = planets("value");
    assert!(drawn(
        &mut world,
        &crate::occurrence::below(
            &crate::test_examples::result_path(&[Step::Key(names["mars"])]),
            &[Step::Key(names["moons"])],
        )
    ));
}

#[test]
fn the_example_libraries_draw_fractions_angles_and_their_functions() {
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../../../examples/libraries.gid")).unwrap();
    let mut world = crate::test_editor(doc);
    let key = |name: &str| Step::Key(names[name]);
    let elements = |world: &crate::Editor, list: &str| {
        world
            .sources()
            .resolve_path(&[key(list)])
            .and_then(Value::as_list)
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    };
    for position in elements(&world, "fractions") {
        for part in ["over", "under"] {
            let at = [
                key("fractions"),
                Step::Element(position.clone()),
                Step::Follow,
                key(part),
            ];
            assert!(drawn(&mut world, &at), "{part} of a fraction");
        }
    }
    // Stacked: each part centered over the other, with a rule as wide as
    // the wider part between them.
    let frame = settle(editing_frame(&mut world), &world.sources());
    let ink = crate::display::widget::style::editor(
        crate::display::widget::style::Theme::Light.palette(),
        1.0,
    )
    .ink
    .brush;
    let rules = frame
        .list
        .0
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Fill {
                shape: Shape::Rect(rect),
                brush,
                transform,
            } if *brush == ink => Some(transform.transform_rect_bbox(*rect)),
            _ => None,
        })
        .collect::<Vec<_>>();
    for position in elements(&world, "fractions") {
        let part = |name: &str| {
            let path = [
                key("fractions"),
                Step::Element(position.clone()),
                Step::Follow,
                key(name),
            ];
            frame.landmarks.landmark(&path).unwrap().rect
        };
        let (over, under) = (part("over"), part("under"));
        assert!(
            (over.center().x - under.center().x).abs() < 0.5,
            "{over:?} {under:?}"
        );
        assert!(
            rules
                .iter()
                .any(|rule| (rule.x0 - over.x0.min(under.x0)).abs() < 0.5
                    && (rule.x1 - over.x1.max(under.x1)).abs() < 0.5
                    && rule.y0 >= over.y1
                    && rule.y1 <= under.y0),
            "a rule between {over:?} and {under:?}: {rules:?}"
        );
    }
    let total = world
        .sources()
        .resolve_path(&[key("fraction_sum")])
        .cloned()
        .unwrap();
    let sum = ::grap::Evaluator::new(&world.sources(), crate::sources::DEFAULT_FUEL)
        .evaluate(
            total
                .as_record()
                .unwrap()
                .get(&::grap::vocabulary::EXPRESSION)
                .unwrap(),
        )
        .result;
    assert_eq!(
        [key("over"), key("under")].map(|part| {
            let Step::Key(part) = part else {
                unreachable!()
            };
            sum.as_value()
                .as_record()
                .and_then(|fields| fields.get(&part))
                .and_then(crate::libraries::f64::read)
        }),
        [Some(6.0), Some(8.0)]
    );
    for position in elements(&world, "angles") {
        assert!(drawn(
            &mut world,
            &[key("angles"), Step::Element(position), key("degrees")]
        ));
    }
    // Each dial: a gray zero line, and a dark line turned by its angle.
    let frame = settle(editing_frame(&mut world), &world.sources());
    let lines = |within: Rect, [r, g, b]: [u8; 3]| {
        let brush = Brush::from(peniko::Color::from_rgb8(r, g, b));
        frame
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::Rect(_),
                    brush: paint,
                    transform,
                } if *paint == brush && within.contains(transform.translation().to_point()) => {
                    let [a, b, ..] = transform.as_coeffs();
                    // Screen y points down, so a counterclockwise turn is negative.
                    Some((-b.atan2(a)).to_degrees().round())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for (position, degrees) in elements(&world, "angles").into_iter().zip([45.0, 120.0]) {
        let angle = frame
            .landmarks
            .iter()
            .find(|landmark| landmark.path == [key("angles"), Step::Element(position.clone())][..])
            .unwrap()
            .rect;
        assert_eq!(lines(angle, [0x99; 3]), [0.0], "{degrees}: the zero line");
        assert_eq!(lines(angle, [0x44; 3]), [degrees], "{degrees}: its line");
    }
    // Functions only code calls are listed in their library, so they're drawn.
    let libraries = world
        .sources()
        .resolve_path(&[key("libraries")])
        .and_then(Value::as_list)
        .unwrap()
        .iter()
        .map(|(position, library)| (position.clone(), library.clone()))
        .collect::<Vec<_>>();
    for (library, record) in libraries {
        let functions = record
            .as_record()
            .and_then(|fields| fields.get(&names["functions"]))
            .and_then(Value::as_list)
            .unwrap();
        assert!(!functions.is_empty());
        for (function, _) in functions.iter() {
            let body = [
                key("libraries"),
                Step::Element(library.clone()),
                key("functions"),
                Step::Element(function.clone()),
                Step::Follow,
                Step::Key(::grap::vocabulary::BODY),
            ];
            assert!(frame.landmarks.landmark(&body).is_some(), "{body:?}");
        }
    }
}

#[test]
fn the_example_views_select_what_they_draw() {
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../../../examples/libraries.gid")).unwrap();
    let mut driver =
        crate::driver::Driver::new(crate::test_editor(doc), crate::driver::Driver::VIEWPORT);
    let key = |name: &str| Step::Key(names[name]);
    let fraction = vec![
        key("fractions"),
        Step::Element(driver.positions(&[key("fractions")])[0].clone()),
        Step::Follow,
    ];
    let over = [fraction.clone(), vec![key("over")]].concat();
    let under = [fraction.clone(), vec![key("under")]].concat();
    let angle = vec![
        key("angles"),
        Step::Element(driver.positions(&[key("angles")])[0].clone()),
    ];
    let (over_rect, under_rect, angle_rect) =
        (driver.rect(&over), driver.rect(&under), driver.rect(&angle));

    // The rule lies between the parts.
    driver.click_at(Point::new(
        over_rect.center().x,
        (over_rect.y1 + under_rect.y0) / 2.0,
    ));
    assert_eq!(
        driver.selection(),
        fraction,
        "the rule selects its fraction"
    );
    driver.click_at(over_rect.center());
    assert_eq!(driver.selection(), over, "a part still selects itself");
    // The word after the number. (The dial's ink names the code that drew
    // it, so only its blank corners select the angle.)
    driver.click_at(Point::new(angle_rect.x1 - 4.0, angle_rect.center().y));
    assert_eq!(driver.selection(), angle, "the label selects its angle");
}

/// The planets document with its view a template: "planet with", then the
/// moons.
fn planets_template() -> (crate::Editor, crate::gid_text::Binders) {
    use crate::libraries::layout::vocabulary::{self as l, kind};
    let (mut world, names) = planets("value");
    // What a node leaves out, a row's gap or a text's paint, has a default.
    let node = ::grap::isa::record(
        kind::ROW,
        [(
            l::CHILDREN,
            Value::list([
                ::grap::isa::record(
                    kind::TEXT,
                    [(l::CONTENT, crate::libraries::text::value("planet with"))],
                ),
                crate::libraries::layout::template::part(names["moons"]),
            ]),
        )],
    );
    std::rc::Rc::make_mut(&mut world.model.doc)
        .cells
        .set_value(names["planet_view"], node);
    (world, names)
}

#[test]
fn a_view_that_is_layout_data_is_a_template_for_its_records() {
    let (mut world, names) = planets_template();
    let [mars, planet, moons] = ["mars", "planet", "moons"].map(|name| names[name]);
    // Its `descend` leads into the record it draws, and nothing else of the
    // record shows.
    assert!(drawn(&mut world, &[Step::Key(mars), Step::Key(moons)]));
    assert!(!drawn(&mut world, &[Step::Key(mars), Step::Key(planet)]));
    // It has no way to decline, so it draws every record carrying the key,
    // one without moons included.
    assert!(!drawn(
        &mut world,
        &[Step::Key(names["note"]), Step::Key(planet)]
    ));
}

#[test]
fn a_template_is_edited_where_its_library_declares_it() {
    use crate::driver::Driver;
    use crate::libraries::layout::vocabulary as l;
    use ui_events::keyboard::{Key, Modifiers, NamedKey};
    let (world, names) = planets_template();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    let library = driver.element(&[Step::Key(names["libraries"])], 0, &[Step::Follow]);
    let children = [
        library.as_slice(),
        &[
            Step::Key(names["projection"]),
            Step::Follow,
            Step::Key(l::CHILDREN),
        ],
    ]
    .concat();
    // Its nodes are drawn there as nodes to edit, not as the layout they
    // describe: the part stands where the template has it, and nothing is
    // drawn inside it for the template to descend into.
    let part = driver.element(&children, 1, &[]);
    assert!(driver.drawn(&part));
    assert!(!driver.drawn(&[part.as_slice(), &[Step::Key(names["moons"])]].concat()));
    // Enter after a node opens a place beside it, which offers the parts of
    // the records the library draws by name.
    driver.select(&part);
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    driver.type_text("plan");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        driver.value(&driver.element(&children, 2, &[])),
        crate::libraries::layout::template::part(names["planet"])
    );
    // The records follow at once: the planet's name is drawn in each now.
    assert!(driver.drawn(&[Step::Key(names["mars"]), Step::Key(names["planet"])]));
    // What's typed that names nothing is the text it would draw.
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    driver.type_text("big");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        driver.value(&driver.element(&children, 3, &[Step::Key(l::CONTENT)])),
        crate::libraries::text::value("big")
    );
    // A node that holds another, chosen over a node, goes around it.
    let moons = crate::libraries::layout::template::part(names["moons"]);
    driver.select(&part);
    driver.type_text("cent");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        driver.value(&part),
        ::grap::isa::record(l::kind::CENTER, [(l::CHILD, moons)])
    );
    // A node's kind is changed where it's named, around what the node holds.
    let template = &children[..children.len() - 1];
    let held = driver.value(&children);
    driver.select(&[template, &[Step::Key(::grap::isa::ISA)]].concat());
    driver.type_text("col");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        driver.value(&[template, &[Step::Key(::grap::isa::ISA)]].concat()),
        Value::from(l::kind::COL)
    );
    assert_eq!(driver.value(&children), held);
}

#[test]
fn a_template_draws_a_part_with_a_projection_it_names() {
    use crate::driver::Driver;
    use crate::libraries::controls::vocabulary::AS_SLIDER;
    use crate::libraries::layout::template::part_with;
    use crate::workspace::vocabulary::{KEYS, LIBRARIES, PROJECTION};
    let [level, tank] = std::array::from_fn(|_| gid::new_cell_id());
    let mut driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(Value::record([
                (
                    tank,
                    Value::record([(level, crate::libraries::f64::value(0.25))]),
                ),
                (
                    LIBRARIES,
                    Value::list([Value::record([
                        (KEYS, Value::list([level.into()])),
                        (PROJECTION, part_with(level, AS_SLIDER)),
                    ])]),
                ),
            ])),
            cells: gid::Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    // The level is a slider, a quarter of the way along; dragging it most of
    // the way writes the number it's dragged to.
    let part = [Step::Key(tank), Step::Key(level)];
    let slider = driver.rect(&part);
    let at = |along: f64| kurbo::Point::new(slider.x0 + along * slider.width(), slider.center().y);
    driver.drag(at(0.25), &[at(0.5), at(0.8)]);
    let dragged = crate::libraries::f64::read(&driver.value(&part)).unwrap();
    assert!(dragged > 0.6 && dragged <= 1.0, "{dragged}");
    // The whole drag is one step to undo.
    assert!(driver.undo());
    assert_eq!(driver.value(&part), crate::libraries::f64::value(0.25));
}
