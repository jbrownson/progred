//! The lessons the tutorials embed, checked through the editor's frames rather
//! than the pages' steps.

use crate::driver::Driver;
use crate::frame::Hovered;
use crate::libraries::f64;
use crate::test_examples::website_growing_forest_editor;
use crate::test_support::{completion_entries_with, library_offers};
use gid::{CellId, Step, Value};
use kurbo::Point;
use peniko::Color;
use puri::draw::DrawCmd;
use puri::hover::Claim;
use std::rc::Rc;
#[test]
fn website_forest_hover_finds_the_available_call() {
    use crate::display::widget::source::SourceTrace;
    use crate::hover::Hover;
    let (editor, names) = website_growing_forest_editor();
    let mut runner = crate::EditorRunner::new(editor);
    runner.refresh_frame(1.0, kurbo::Size::new(720.0, 684.0));
    let sources = runner.editor.sources();
    let expected = runner
        .frame
        .dispatch
        .landmarks
        .iter()
        .find_map(|landmark| {
            let location = landmark.scope.location(&landmark.path)?;
            let value = sources.read(&location)?;
            (value
                .as_record()?
                .get(&grap::vocabulary::FUNCTION)?
                .as_cell()?
                == names["forest_view"])
                .then(|| SourceTrace::from_location(&sources, &location))
        })
        .expect("the forest call is projected");
    let hovered = (20..300).step_by(10).find_map(|y| {
        (20..690).step_by(10).find_map(|x| {
            let (_, claim) = runner.frame.dispatch.hover_geometry.probe(
                Some(Point::new(x as f64, y as f64)), None, 0.0,
            )?;
            let Claim::Direct(Hovered::Tree(Hover::Calls(calls))) = claim else { return None; };
            let tree = calls.sources().any(|source| matches!(source, SourceTrace::InCell { cell, .. } if cell == names["growing_tree"]));
            tree.then_some(Hover::Calls(calls))
        })
    }).expect("a tree shape has a captured call chain");
    assert_eq!(
        crate::display::widget::source_link::hover_source(
            &sources,
            &runner.frame.dispatch.landmarks,
            &hovered
        ),
        Some(expected),
        "{hovered:?}",
    );
}

#[test]
fn website_growing_forest_controls_and_edits_change_the_drawing() {
    use crate::libraries::{color, controls, presentation};
    let (editor, names) = website_growing_forest_editor();
    assert!(crate::workspace::declarations(editor.model.doc.root.as_ref()).is_empty());
    let mut driver = Driver::new(editor, kurbo::Size::new(720.0, 684.0));
    // The sun and the trees' leaves, whatever else is drawn.
    let painted = [(57, 155, 117), (220, 90, 40), (248, 201, 112)]
        .map(|(r, g, b)| peniko::Brush::from(Color::from_rgb8(r, g, b)));
    let render = |driver: &mut Driver| {
        driver.present();
        driver
            .circles()
            .into_iter()
            .filter(|(_, _, brush)| painted.contains(brush))
            .map(|(_, circle, brush)| (circle, brush))
            .collect::<Vec<_>>()
    };
    let initial = render(&mut driver);
    assert_eq!(
        initial.len(),
        22,
        "sun followed by three leaf-colored lobes per tree"
    );
    assert_eq!(
        initial[0].1,
        peniko::Brush::from(Color::from_rgb8(248, 201, 112))
    );
    let mut frames = Vec::new();
    for growth in [0.0, 0.35, 1.0, 0.35] {
        driver
            .runner
            .editor
            .model
            .workspace
            .document
            .annotations
            .set_field(
                &[
                    Step::Key(names["second"]),
                    Step::Key(presentation::vocabulary::RESULT),
                ],
                controls::vocabulary::STATE,
                Some(f64::value(growth)),
            );
        frames.push(render(&mut driver));
    }
    assert_ne!(frames[0][1..], frames[1][1..]);
    assert_ne!(frames[1][1..], frames[2][1..]);
    assert!(frames[0][0].0.center.x < frames[1][0].0.center.x);
    assert!(frames[1][0].0.center.x < frames[2][0].0.center.x);
    assert!(frames[1][0].0.center.y < frames[0][0].0.center.y);
    assert!((frames[0][0].0.center.y - frames[2][0].0.center.y).abs() < 1e-9);
    assert_eq!(
        frames[1], frames[3],
        "scrubbing is deterministic and reversible"
    );
    let mut previous = frames.pop().unwrap();
    for (parameter, value) in [
        ("count", f64::value(4.25)),
        ("count", f64::value(4.75)),
        ("count", f64::value(4.0)),
        ("rate", f64::value(1.5)),
        ("leaves", color::tests::value(Color::from_rgb8(220, 90, 40))),
        ("count", f64::value(0.0)),
    ] {
        let mut doc = (*driver.editor().model.doc).clone();
        fn edit_call(source: &Value, function: CellId, field: CellId, value: &Value) -> Value {
            match source {
                Value::Record(record)
                    if record.get(&::grap::vocabulary::FUNCTION)
                        == Some(&Value::from(function)) =>
                {
                    let mut record = record.clone();
                    record.insert(field, value.clone());
                    Value::Record(record)
                }
                Value::Record(record) => Value::record(
                    record
                        .iter()
                        .map(|(key, child)| (*key, edit_call(child, function, field, value))),
                ),
                Value::List(list) => Value::list(
                    list.values()
                        .map(|child| edit_call(child, function, field, value)),
                ),
                _ => source.clone(),
            }
        }
        let sample = edit_call(
            doc.cells.value(names["sample"]).unwrap(),
            names["forest_view"],
            names[parameter],
            &value,
        );
        doc.cells.set_value(names["sample"], sample);
        driver.runner.editor.model.doc = Rc::new(doc);
        let changed = render(&mut driver);
        if parameter == "count" {
            assert_eq!(
                changed.len(),
                1 + 3 * f64::read(&value).unwrap().ceil() as usize,
                "sun plus trees for each integer index below the requested count"
            );
        }
        assert_ne!(
            previous[1..],
            changed[1..],
            "{parameter} must affect the trees"
        );
        assert_eq!(
            previous[0], changed[0],
            "forest arguments must not change the sun"
        );
        previous = changed;
    }
}

fn website_growing_forest_insert(
    editor: &mut crate::Editor,
    names: &crate::gid_text::Binders,
    expression: &Value,
) {
    use crate::libraries::control::vocabulary::{ALL, EXPRESSIONS};
    fn insert(source: &Value, clouds: &Value) -> Value {
        match source {
            Value::Record(fields)
                if fields.get(&::grap::vocabulary::FUNCTION) == Some(&Value::from(ALL)) =>
            {
                let mut fields = fields.clone();
                let mut expressions: Vec<_> = fields
                    .get(&EXPRESSIONS)
                    .unwrap()
                    .as_list()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                expressions.insert(1, clouds.clone());
                fields.insert(EXPRESSIONS, Value::list(expressions));
                Value::Record(fields)
            }
            Value::Record(fields) => Value::record(
                fields
                    .iter()
                    .map(|(key, value)| (*key, insert(value, clouds))),
            ),
            Value::List(list) => Value::list(list.values().map(|value| insert(value, clouds))),
            _ => source.clone(),
        }
    }
    let doc = Rc::make_mut(&mut editor.model.doc);
    let sample = insert(doc.cells.value(names["sample"]).unwrap(), expression);
    doc.cells.set_value(names["sample"], sample);
}

fn website_growing_forest_add_clouds(
    editor: &mut crate::Editor,
    names: &crate::gid_text::Binders,
    speed: f64,
) {
    let clouds = ::grap::call(
        Value::from(names["clouds"]),
        [
            (names["sun_time"], Value::from(names["controls_parameters"])),
            (names["cloud_speed"], f64::value(speed)),
            (
                names["color"],
                crate::libraries::color::tests::value(Color::WHITE),
            ),
        ],
    );
    website_growing_forest_insert(editor, names, &clouds);
}

#[test]
fn website_growing_forest_all_keeps_drawing_after_incomplete_clouds() {
    let (editor, names) = website_growing_forest_editor();
    let mut driver = Driver::new(editor, kurbo::Size::new(720.0, 960.0));
    let render = |driver: &mut Driver| {
        driver.present();
        driver
            .circles()
            .into_iter()
            .map(|(_, circle, brush)| (circle, brush))
            .collect::<Vec<_>>()
    };
    let before = render(&mut driver);
    assert!(before.len() >= 22, "sun and forest are visible");
    website_growing_forest_insert(
        &mut driver.runner.editor,
        &names,
        &::grap::call(names["clouds"].into(), []),
    );
    assert_eq!(render(&mut driver), before);
}

#[test]
fn website_growing_forest_clouds_are_discoverable_optional_and_slider_driven() {
    use crate::libraries::{controls, presentation};
    let (editor, names) = website_growing_forest_editor();
    let offers = completion_entries_with(
        &editor.sources(),
        &crate::display::CompletionKind::Value,
        "clouds",
        None,
        Some(&library_offers(
            editor.stack.completions.clone(),
            crate::display::CompletionScope::Everything,
        )),
    );
    assert!(
        offers
            .iter()
            .any(|offer| offer.display == "clouds" && offer.detail.as_deref() == Some("call"))
    );
    let mut driver = Driver::new(editor, kurbo::Size::new(720.0, 960.0));
    // The clouds' white lobes, `time` into the sun's day.
    let render = |driver: &mut Driver, time: f64| {
        driver
            .runner
            .editor
            .model
            .workspace
            .document
            .annotations
            .set_field(
                &[
                    Step::Key(names["second"]),
                    Step::Key(presentation::vocabulary::RESULT),
                ],
                controls::vocabulary::STATE,
                Some(f64::value(time)),
            );
        driver.present();
        driver
            .circles()
            .into_iter()
            .filter(|(_, _, brush)| *brush == peniko::Brush::from(Color::WHITE))
            .map(|(_, circle, _)| circle)
            .collect::<Vec<_>>()
    };
    assert!(
        render(&mut driver, 0.0).is_empty(),
        "clouds are not drawn by default"
    );
    let original = driver.editor().model.doc.clone();
    for speed in [0.0, 1.0, 2.0, -1.0] {
        driver.runner.editor.model.doc = original.clone();
        website_growing_forest_add_clouds(&mut driver.runner.editor, &names, speed);
        let start = render(&mut driver, 0.0);
        let end = render(&mut driver, 1.0);
        assert_eq!(start.len(), 9, "three clouds with three lobes each");
        assert_eq!(end.len(), start.len());
        for (start, end) in start.iter().zip(&end) {
            assert!((end.center.x - start.center.x - 60.0 * speed).abs() < 1e-9);
            assert_eq!(start.center.y, end.center.y);
            assert_eq!(start.radius, end.radius);
        }
        assert_eq!(start, render(&mut driver, 0.0), "scrubbing is reversible");
    }
}

/// The shape lesson with a mesh-only preview: its tests inspect the mesh, and
/// running the lesson's implicit refinement inline would double their time.
fn website_shape_editor() -> (crate::Editor, crate::gid_text::Binders) {
    use crate::libraries::{
        absent, blob, color, control, controls, conversion, f32, f64, fidget, geometry, grap,
        layout, list, logic, name, presentation, text, toolpath, tree, u64,
    };
    let (doc, fields) = crate::gid_text::parse(
        &include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../website/public/lessons/shape.gid"
        ))
        .replace(
            &toolpath::vocabulary::PREVIEW_REFINED.to_string(),
            &toolpath::vocabulary::PREVIEW_MESH.to_string(),
        ),
    )
    .unwrap();
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            color::ID,
            control::ID,
            grap::ID,
            f64::ID,
            layout::ID,
            f32::ID,
            u64::ID,
            conversion::ID,
            fidget::ID,
            controls::ID,
            presentation::ID,
            geometry::ID,
            logic::ID,
            list::ID,
            tree::ID,
            toolpath::ID,
        ])
        .unwrap(),
    );
    crate::test_support::set_tutorial_slots(&mut editor, &[fields["second"], fields["first"]]);
    editor.drawn_menu = false;
    (editor, fields)
}

#[test]
fn website_shape_edits_and_playback_change_the_rendered_mesh() {
    use crate::libraries::{controls, presentation};
    fn meshes(commands: &[DrawCmd]) -> Vec<puri::mesh::Scene> {
        crate::display::test_support::painted(commands)
            .filter_map(|command| match command {
                DrawCmd::Mesh { scene, .. } => Some(scene.clone()),
                _ => None,
            })
            .collect()
    }
    let (editor, names) = website_shape_editor();
    assert!(crate::workspace::declarations(editor.model.doc.root.as_ref()).is_empty());
    let mut driver = Driver::new(editor, kurbo::Size::new(720.0, 520.0));
    let render = |driver: &mut Driver| {
        driver.present();
        let scenes = meshes(&driver.painted().0);
        assert_eq!(
            scenes.len(),
            1,
            "the CAM call must produce geometry, not an absent fallback"
        );
        assert!(scenes[0].geometry.indices.len() > 100);
        scenes.into_iter().next().unwrap()
    };
    let initial = render(&mut driver);
    let mut doc = (*driver.editor().model.doc).clone();
    let mut call = doc
        .cells
        .value(names["sample"])
        .unwrap()
        .as_record()
        .unwrap()
        .clone();
    call.insert(names["rows"], f64::value(5.0));
    doc.cells.set_value(names["sample"], Value::Record(call));
    driver.runner.editor.model.doc = Rc::new(doc);
    let edited = render(&mut driver);
    assert_ne!(
        initial.geometry.indices.len(),
        edited.geometry.indices.len()
    );

    driver
        .runner
        .editor
        .model
        .workspace
        .document
        .annotations
        .set_field(
            &[
                Step::Key(names["second"]),
                Step::Key(presentation::vocabulary::RESULT),
            ],
            controls::vocabulary::STATE,
            Some(Value::record([(
                names["preview_mode"],
                names["model"].into(),
            )])),
        );
    let model = render(&mut driver);
    assert_ne!(edited.geometry.indices.len(), model.geometry.indices.len());

    // These are arguments of the same call, not disconnected preview controls.
    let positions = |scene: &puri::mesh::Scene| {
        scene
            .geometry
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>()
    };
    let mut previous = positions(&model);
    for (parameter, value) in [("control_depth", 0.25), ("tilt", 20.0)] {
        let mut doc = (*driver.editor().model.doc).clone();
        let mut call = doc
            .cells
            .value(names["sample"])
            .unwrap()
            .as_record()
            .unwrap()
            .clone();
        call.insert(names[parameter], f64::value(value));
        doc.cells.set_value(names["sample"], Value::Record(call));
        driver.runner.editor.model.doc = Rc::new(doc);
        let current = positions(&render(&mut driver));
        assert!(previous != current, "{parameter} must change the geometry");
        previous = current;
    }
}
