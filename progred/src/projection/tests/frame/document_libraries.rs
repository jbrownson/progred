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
fn the_lab_libraries_draw_fractions_angles_and_tints() {
    let (doc, names) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lab/library.gid"
    ))
    .unwrap();
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
    for position in elements(&world, "numbers") {
        for part in ["over", "under"] {
            let at = [
                key("numbers"),
                Step::Element(position.clone()),
                Step::Follow(gid::Resolution::Document),
                key(part),
            ];
            assert!(drawn(&mut world, &at), "{part} of a fraction");
        }
    }
    let total = world
        .sources()
        .resolve_path(&[key("total")])
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
    assert!(drawn(&mut world, &[key("heading"), key("degrees")]));
    for position in elements(&world, "colors") {
        assert!(drawn(
            &mut world,
            &[key("colors"), Step::Element(position), key("tint")]
        ));
    }
    let painted = settle(editing_frame(&mut world, false)).list.0;
    let fills = |wanted: fn(&Shape) -> bool| {
        painted
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill { shape, brush, .. } if wanted(shape) => Some(brush.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        fills(|shape| matches!(shape, Shape::Circle(_))).len(),
        2,
        "the heading's dial and its dot"
    );
    let rects = fills(|shape| matches!(shape, Shape::Rect(_)));
    assert!(
        rects.contains(&Brush::from(puri::Color::from_rgb8(0x99, 0x99, 0x99))),
        "the dial marks zero"
    );
    for color in [0x3b82a0u32, 0xc1440e, 0x548b64] {
        let [r, g, b] = [16, 8, 0].map(|shift| (color >> shift) as u8);
        assert!(
            rects.contains(&Brush::from(puri::Color::from_rgb8(r, g, b))),
            "a swatch of {color:06x}"
        );
    }
}
