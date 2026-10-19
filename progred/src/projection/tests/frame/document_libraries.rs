use super::*;
use crate::sample::{PLANET_VIEW, planets};

fn drawn(world: &mut crate::Editor, path: &[Step]) -> bool {
    editing_frame(world, false)
        .landmarks
        .iter()
        .any(|landmark| landmark.path.as_ref() == path)
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
    let (mut world, names) = planets("{function: descend_path, steps: []}");
    assert!(drawn(
        &mut world,
        &[Step::Key(names["mars"]), Step::Key(names["moons"])]
    ));
    // Returning it draws it as a result, with the loaded libraries.
    let (mut world, names) = planets("value");
    assert!(drawn(
        &mut world,
        &[
            Step::Key(names["mars"]),
            Step::Key(crate::libraries::presentation::vocabulary::RESULT),
            Step::Key(names["moons"]),
        ]
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
                Step::Follow(gid::Resolution::Document),
                key(part),
            ];
            assert!(drawn(&mut world, &at), "{part} of a fraction");
        }
    }
    // Stacked: each part centered over the other, with a rule as wide as
    // the wider part between them.
    let frame = settle(editing_frame(&mut world, false));
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
                Step::Follow(gid::Resolution::Document),
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
    let sum = ::grap::evaluate_value(
        total
            .as_record()
            .unwrap()
            .get(&::grap::vocabulary::EVALUATE)
            .unwrap(),
        &world.sources(),
        ::grap::DEFAULT_FUEL,
    )
    .result;
    assert_eq!(
        [key("over"), key("under")].map(|part| {
            let Step::Key(part) = part else {
                unreachable!()
            };
            sum.as_record()
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
    let frame = settle(editing_frame(&mut world, false));
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
            .find(|landmark| {
                landmark.path.as_ref() == [key("angles"), Step::Element(position.clone())]
            })
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
                Step::Follow(gid::Resolution::Document),
                Step::Key(::grap::vocabulary::BODY),
            ];
            assert!(
                frame
                    .landmarks
                    .iter()
                    .any(|landmark| landmark.path.as_ref() == body),
                "{body:?}"
            );
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
        Step::Follow(gid::Resolution::Document),
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
