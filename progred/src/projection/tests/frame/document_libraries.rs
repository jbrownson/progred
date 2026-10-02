use super::*;

/// A document with a planet library: its view says "planet with" and descends
/// into the moons, and declines anything without them.
fn planets(view_body: &str) -> (crate::Editor, crate::gid_text::Binders) {
    let source = format!(
        r#"{{
          "binders": {{
            "name": 02e562654d6d0828d3a7559e6f75fffe,
            "f64": ed11fde03b7c2c1ba2fccc3cdba5d561,
            "function": 751fca4373debdd0b7e6eb73e08d684b,
            "params": 195b378d0d31d90ab0d7366c15346b70,
            "body": 986143866eda2e2fbf9ab8484357a0c9,
            "expression": ccc55b0eb63b9f564ea74436094d4014,
            "where": 2a3432e9c5ce4c62b8261a6b248f13c2,
            "bindings": f6fb0062e8a14f808e416a9edece2a9d,
            "pattern": b9dc97198709bc6f7bae4c7afc7d424f,
            "bind": 5e46d12705690e8a377eb0f16ad9dba6,
            "subject": 00dafdc01c7e014edd857e174c6c8b6f,
            "do": b1fc4cb45c58b1a662c431feef5bd140,
            "expressions": 5fab151c006ae1487c28837f2003f43c,
            "row": 1af52c96e380b7d40c9e1f6a2d5b83e7,
            "children": e10b73d5482f96ca7d3852c0f16b49ea,
            "text": 08e64d1f3a92c5b7b7f0d38a165e29c4,
            "content": 3e6b91d4a25f70c8815d29f6c4a30e7b,
            "descend": 35c7a8e2f10d49b6d2f8016c4b9ea375,
            "descend_path": fa9520a04e0c79e7ff78adc6e3f6e754,
            "step": 67a2d5e0b93c48f14e28b671d0a5c39f,
            "steps": 1298c6f4a7053edb09b64d2e8371fa5c,
            "key": f12dea12c741fe36312750a264f3a235,
            "value": 84d3ba81fd2a52ea37478f4a868106f4,
            "projection": 873503e2e37a1722a0dd21399be9ee7f,
            "libraries": bc197ad66fea59a9bd97c4191c7845b2,
            "keys": bb30d215605f5599e31f6274641ca89f,
            "planet": 6655e4ba9e8ae76706056e0b8edf94f0,
            "moons": 6122302fdbe3db3624731717321b94a1,
            "planet_view": 0b570cad527f02e4ed4b06110a62bf67,
          }},
          "cells": {{
            planet: {{name: "planet"}},
            moons: {{name: "moons"}},
            n: {{name: "n"}},
            planet_view: {{name: "planet view", params: [value], body: {view_body}}},
            planets: {{name: "planets", keys: [planet], projection: planet_view}},
          }},
          "root": {{
            libraries: [planets],
            mars: {{planet: "Mars", moons: {{f64: 0x0000000000000040}}}},
            note: {{planet: "Pluto"}},
          }},
        }}"#
    );
    let (doc, names) = crate::gid_text::parse(&source).unwrap();
    (crate::test_editor(doc), names)
}

const PLANET_VIEW: &str = r#"{
  function: where,
  bindings: [{pattern: {moons: {bind: n}}, subject: value}],
  expression: {function: row, children: {function: do, expressions: [
    {function: text, content: "planet with"},
    {function: descend, step: {key: moons}},
  ]}},
}"#;

fn drawn(world: &mut crate::Editor, path: &[Step]) -> bool {
    editing_frame(world, false)
        .descends
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
fn only_the_root_declares_libraries() {
    let (world, names) = planets(PLANET_VIEW);
    let sources = world.sources();
    assert_eq!(crate::workspace::libraries(&sources).len(), 1);
    let mut nested = (*world.model.doc).clone();
    nested.root = Some(Value::record([(
        names["mars"],
        nested.root.clone().unwrap(),
    )]));
    let nested_sources = crate::sources::Sources {
        doc: &nested,
        libraries: &world.stack.libraries,
    };
    assert!(crate::workspace::libraries(&nested_sources).is_empty());
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
    let ink = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0)
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
            frame
                .descends
                .iter()
                .find(|landmark| landmark.path.as_ref() == path)
                .unwrap()
                .rect
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
        let brush = Brush::from(puri::Color::from_rgb8(r, g, b));
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
            .descends
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
                    .descends
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
    let mut world = crate::test_editor(doc);
    let key = |name: &str| Step::Key(names[name]);
    let first = |world: &crate::Editor, list: &str| {
        world
            .sources()
            .resolve_path(&[key(list)])
            .and_then(Value::as_list)
            .unwrap()
            .keys()
            .next()
            .cloned()
            .unwrap()
    };
    let fraction = vec![
        key("fractions"),
        Step::Element(first(&world, "fractions")),
        Step::Follow(gid::Resolution::Document),
    ];
    let over = [fraction.clone(), vec![key("over")]].concat();
    let under = [fraction.clone(), vec![key("under")]].concat();
    let angle = vec![key("angles"), Step::Element(first(&world, "angles"))];
    let frame = settle(editing_frame(&mut world, false));
    let rect = |path: &[Step]| {
        frame
            .descends
            .iter()
            .find(|landmark| landmark.path.as_ref() == path)
            .unwrap()
            .rect
    };
    let (over_rect, under_rect, angle_rect) = (rect(&over), rect(&under), rect(&angle));
    let selected = |world: &crate::Editor| world.model.selection.as_ref().unwrap().path().to_vec();

    // The rule lies between the parts.
    let rule = Point::new(over_rect.center().x, (over_rect.y1 + under_rect.y0) / 2.0);
    assert!(click_at(&mut world, rule).is_some());
    assert_eq!(selected(&world), fraction, "the rule selects its fraction");
    assert!(click_at(&mut world, over_rect.center()).is_some());
    assert_eq!(selected(&world), over, "a part still selects itself");
    // The word after the number. (The dial's ink names the code that drew
    // it, so only its blank corners select the angle.)
    let label = Point::new(angle_rect.x1 - 4.0, angle_rect.center().y);
    assert!(click_at(&mut world, label).is_some());
    assert_eq!(selected(&world), angle, "the label selects its angle");
}
