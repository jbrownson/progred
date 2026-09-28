use super::*;

#[test]
fn fidget_shapes_are_inline_and_preserve_the_standalone_previews() {
    use crate::libraries::{layout, presentation};

    let (doc, _) = crate::gid_text::parse(crate::command::Example::Fidget.source()).unwrap();
    assert!(crate::workspace::declarations(doc.root.as_ref()).is_empty());
    let entries = doc.root.as_ref().unwrap().as_list().unwrap();
    assert_eq!(entries.len(), crate::test_examples::FIDGET.len());
    let libraries = core_libraries();
    let sources = src(&doc, &libraries);

    for ((_, entry), (name, fixture)) in entries.iter().zip(crate::test_examples::FIDGET) {
        let items: Vec<_> = entry.as_list().unwrap().values().collect();
        assert_eq!(items.len(), 2, "{name}: definition and preview");
        assert!(doc.cells.value(items[0].as_cell().unwrap()).is_some());
        let expression = items[1]
            .as_record()
            .unwrap()
            .get(&grap::vocabulary::EVALUATE)
            .unwrap();
        let result = grap::evaluate_value(expression, &sources, grap::DEFAULT_FUEL);
        assert!(result.completed, "{name}");
        assert!(
            !crate::libraries::absent::is_absent(&result.result),
            "{name}"
        );

        let (standalone, _) = crate::gid_text::parse(fixture).unwrap();
        let old_sources = src(&standalone, &libraries);
        let pane = crate::workspace::declarations(standalone.root.as_ref()).remove(0);
        let declaration = old_sources.resolve_path(&pane.path).unwrap();
        let expected =
            presentation::viewport_output(declaration, &old_sources, 256.0, 256.0).unwrap();
        assert_eq!(
            result.result, expected,
            "{name}: unchanged geometry and color"
        );

        // Exercise the ordinary evaluate projection, not the pane interpreter.
        // Use a small raster here; the document itself uses the default size.
        let mut call = expression.as_record().unwrap().clone();
        for dimension in [layout::vocabulary::WIDTH, layout::vocabulary::HEIGHT] {
            call.insert(dimension, f64::value(64.0));
        }
        let inline = Document {
            root: Some(Value::record([(
                grap::vocabulary::EVALUATE,
                Value::Record(call),
            )])),
            cells: doc.cells.clone(),
        };
        let (bench, _) = place(&inline, None, 800.0);
        assert_eq!(
            bench
                .list
                .0
                .iter()
                .filter(|cmd| matches!(cmd, DrawCmd::Image { .. } | DrawCmd::Mesh { .. }))
                .count(),
            1,
            "{name}: inline preview must render"
        );
    }
}

#[test]
#[ignore = "writes the combined shapes document without launching the app"]
fn fidget_shapes_svg_capture() {
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Fidget.source()).unwrap();
    super::svg::render(&doc, None, 1000.0, "fidget_shapes.svg");
}
