use super::*;

#[test]
fn fidget_figures_preserve_the_standalone_previews_beside_their_code() {
    use crate::libraries::presentation;

    let (doc, names) = crate::gid_text::parse(crate::command::Example::Fidget.source()).unwrap();
    assert!(crate::workspace::declarations(doc.root.as_ref()).is_empty());
    let libraries = core_libraries();
    let sources = src(&doc, &libraries);
    let key = |name: &str| Step::Key(names[name]);
    let figures = sources
        .resolve_path(&[key("figures")])
        .and_then(Value::as_list)
        .unwrap()
        .iter()
        .map(|(position, figure)| (position.clone(), figure.clone()))
        .collect::<Vec<_>>();
    assert_eq!(figures.len(), crate::test_examples::FIDGET.len());
    let mut world = crate::test_editor(doc.clone());
    let mut codes = Vec::new();

    for ((position, figure), (name, fixture)) in figures.iter().zip(crate::test_examples::FIDGET) {
        let figure = figure.as_record().unwrap();
        let (part, preview) = if figure.contains_key(&names["figure"]) {
            ("figure", "rendered")
        } else {
            ("mesh", "meshed")
        };
        let call = Value::record([
            (grap::vocabulary::FUNCTION, Value::from(names[preview])),
            (
                presentation::vocabulary::VALUE,
                Value::record([(
                    grap::vocabulary::FUNCTION,
                    figure.get(&names[part]).unwrap().clone(),
                )]),
            ),
        ]);
        let result = grap::evaluate_value(&call, &sources, grap::DEFAULT_FUEL);
        assert!(result.completed, "{name}");

        let (standalone, _) = crate::gid_text::parse(fixture).unwrap();
        let old_sources = src(&standalone, &libraries);
        let pane = crate::workspace::declarations(standalone.root.as_ref()).remove(0);
        let declaration = old_sources.resolve_path(&pane.path).unwrap();
        let expected =
            presentation::viewport_output(declaration, &old_sources, 256.0, 256.0).unwrap();
        assert_eq!(
            result.result,
            unnamed(&expected),
            "{name}: unchanged geometry and color"
        );
        codes.push((
            [key("figures"), Step::Element(position.clone()), key(part)],
            name,
        ));
    }

    // A gallery: each figure's code beside its preview, tops aligned, and
    // every preview at the start of its row.
    let frame = settle(editing_frame(&mut world, false));
    let landmark = |path: &[Step]| {
        frame
            .descends
            .iter()
            .find(|landmark| landmark.path.as_ref() == path)
            .map(|landmark| landmark.rect)
    };
    for (code, name) in codes {
        let picture = [
            code[0].clone(),
            code[1].clone(),
            Step::Key(presentation::vocabulary::RESULT),
        ];
        let (code, picture) = (
            landmark(&code).expect(name),
            landmark(&picture).expect(name),
        );
        assert!(
            (code.y0 - picture.y0).abs() < 0.5,
            "{name}: {code:?} beside {picture:?}"
        );
    }
    // Only the figures are pictured: the views' own patterns carry the keys
    // they own, and must decline rather than picture an absent.
    let pictured = frame
        .descends
        .iter()
        .filter(|landmark| {
            landmark.path.last() == Some(&Step::Key(presentation::vocabulary::RESULT))
        })
        .count();
    assert_eq!(pictured, figures.len());
    let previews = frame
        .list
        .0
        .iter()
        .filter_map(|command| match command {
            DrawCmd::Image { transform, .. } | DrawCmd::Mesh { transform, .. } => {
                Some(transform.translation())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(previews.len(), figures.len(), "{previews:?}");
    assert!(
        previews.iter().all(|at| (at.x - previews[0].x).abs() < 0.5),
        "{previews:?}"
    );
}

/// A standalone shape's name rides along in its field tree; it isn't
/// geometry, and a figure's function returns the tree alone.
fn unnamed(value: &Value) -> Value {
    match value {
        Value::Record(fields) => Value::record(
            fields
                .iter()
                .filter(|(label, _)| *label != crate::libraries::name::vocabulary::NAME)
                .map(|(label, value)| (*label, unnamed(value))),
        ),
        _ => value.clone(),
    }
}

#[test]
#[ignore = "writes the combined shapes document without launching the app"]
fn fidget_shapes_svg_capture() {
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Fidget.source()).unwrap();
    super::svg::render(&doc, None, 1000.0, "fidget_shapes.svg");
}
