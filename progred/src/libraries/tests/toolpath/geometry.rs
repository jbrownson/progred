use super::*;
use crate::{computations::Computations, gid_text::Binders, libraries::fidget, sources::Sources};
use fidget_engine::{shape::EzShape, vm::VmShape};
use gid::{Document, Step};
use std::rc::Rc;

pub(super) fn set_parameter(doc: &mut Document, names: &Binders, name: &str, number: f64) {
    let definition = doc.cells.value(names["cube"]).unwrap();
    let bindings = definition
        .as_record()
        .unwrap()
        .get(&::grap::vocabulary::BODY)
        .unwrap()
        .as_record()
        .unwrap()
        .get(&control::vocabulary::BINDINGS)
        .unwrap()
        .as_list()
        .unwrap();
    let position = bindings
        .iter()
        .find(|(_, binding)| {
            binding
                .as_record()
                .unwrap()
                .get(&control::vocabulary::BIND)
                .and_then(Value::as_cell)
                == Some(names[name])
        })
        .unwrap()
        .0
        .clone();
    let edited = crate::spine::set(
        Some(definition),
        &[
            Step::Key(::grap::vocabulary::BODY),
            Step::Key(control::vocabulary::BINDINGS),
            Step::Element(position),
            Step::Key(control::vocabulary::VALUE),
        ],
        f64::value(number),
    )
    .unwrap();
    doc.cells.set_value(names["cube"], edited);
}

fn path(sources: &Sources<'_>, names: &Binders) -> Recording {
    let mut recording = Recording::default();
    let result = run(&mut recording, |scope| {
        ::grap::Evaluator::new(sources, 500_000)
            .functions(scope)
            .apply_expression(
                &::grap::RuntimeValue::from(gid::Value::from(names["ball_path"])),
                [],
            )
    });
    assert!(
        result.result.absent_reason().is_none(),
        "{:?}",
        result.result
    );
    recording
}

#[test]
fn cube_edits_change_paths_and_cached_results_match_fresh_evaluation() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let computations = Computations::from_sources(Sources {
        doc: &original,
        libraries: &libraries,
    });
    let memo = computations.runtime.memo({
        let definitions = computations.definitions.clone();
        let program = Value::from(names["ball_path"]);
        move |read| {
            let mut recording = Recording::default();
            let result =
                crate::computations::memo::with_recorded_effects(&definitions, read, |host| {
                    run(&mut recording, |scope| {
                        ::grap::Evaluator::new(host, 500_000)
                            .functions(scope)
                            .apply_expression(&(&program).into(), [])
                    })
                });
            assert!(
                result.result.absent_reason().is_none(),
                "{:?}",
                result.result
            );
            Ok(recording)
        }
    });
    let initial = computations.runtime.read(&memo).unwrap();
    assert!(Rc::ptr_eq(
        &initial,
        &computations.runtime.read(&memo).unwrap()
    ));
    for (name, value) in [("size", 0.9), ("chamfer", 0.14), ("control_depth", 0.3)] {
        let mut doc = original.clone();
        set_parameter(&mut doc, &names, name, value);
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        let cached = computations.runtime.read(&memo).unwrap();
        let fresh = path(
            &Sources {
                doc: &doc,
                libraries: &libraries,
            },
            &names,
        );
        assert_eq!(cached.commands(), fresh.commands(), "{name}");
        assert_ne!(
            cached.commands(),
            initial.commands(),
            "{name} must change the toolpath"
        );
        assert!(Rc::ptr_eq(
            &cached,
            &computations.runtime.read(&memo).unwrap()
        ));
    }
}

#[test]
fn cube_contact_points_and_normals_agree_with_its_implicit_solid() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for (size, chamfer, depth) in [
        (1.0, 0.1, 0.5),
        (0.9, 0.1, 0.5),
        (1.0, 0.14, 0.5),
        (1.0, 0.1, 0.3),
    ] {
        let mut doc = original.clone();
        for (name, value) in [
            ("size", size),
            ("chamfer", chamfer),
            ("control_depth", depth),
        ] {
            set_parameter(&mut doc, &names, name, value);
        }
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let geometry = cube_geometry(&sources, &names);
        let geometry = geometry.as_record().unwrap();
        let solid = ::grap::Evaluator::new(&sources, 10_000).apply_expression(
            &::grap::RuntimeValue::from(geometry.get(&names["field"]).unwrap()),
            [],
        );
        assert!(solid.result.absent_reason().is_none());
        let preview = ::grap::Evaluator::new(&sources, 10_000).apply_expression(
            &::grap::RuntimeValue::from(&Value::from(fidget::vocabulary::PREVIEW_MESH)),
            [(
                crate::libraries::presentation::vocabulary::VALUE,
                solid.result.into_value().into(),
            )],
        );
        let model = fidget::mesh::read(preview.result.as_value()).unwrap().0;
        let shape = VmShape::from(model.objects[0].tree.clone());
        let tape = shape.ez_float_slice_tape();
        let mut evaluator = VmShape::new_float_slice_eval();
        let mut sample = |p: [f64; 3]| {
            evaluator
                .eval(&tape, &[p[0] as f32], &[p[1] as f32], &[p[2] as f32])
                .unwrap()[0] as f64
        };
        for u in [0.1, 0.25, 0.5, 0.7, 0.9] {
            for v in [0.1, 0.3, 0.5, 0.75, 0.9] {
                let contact = ::grap::Evaluator::new(&sources, 10_000).apply_expression(
                    &::grap::RuntimeValue::from(geometry.get(&names["face"]).unwrap()),
                    [X, Y, Z]
                        .into_iter()
                        .zip([u, v, 0.0].map(f64::value))
                        .into_iter()
                        .map(|(k, v)| (k, v.into())),
                );
                let contact = read_point(contact.result.as_value()).unwrap();
                assert!(
                    sample(contact).abs() < 2e-6,
                    "{size}, {chamfer}, {depth}: {contact:?}"
                );
                let normal = ::grap::Evaluator::new(&sources, 10_000).apply_expression(
                    &::grap::RuntimeValue::from(geometry.get(&names["normal"]).unwrap()),
                    [X, Y, Z]
                        .into_iter()
                        .zip(contact.map(f64::value))
                        .into_iter()
                        .map(|(k, v)| (k, v.into())),
                );
                let normal = read_point(normal.result.as_value()).unwrap();
                let gradient: [f64; 3] = std::array::from_fn(|axis| {
                    let mut a = contact;
                    let mut b = contact;
                    a[axis] += 0.0001;
                    b[axis] -= 0.0001;
                    (sample(a) - sample(b)) / 0.0002
                });
                for axis in 0..3 {
                    assert!(
                        (gradient[axis] - normal[axis]).abs() < 0.002,
                        "{gradient:?} vs {normal:?}"
                    );
                }
                let center = ::grap::Evaluator::new(&sources, 10_000).apply_expression(
                    &::grap::RuntimeValue::from(&Value::from(names["ball_center"])),
                    [
                        (X, f64::value(contact[0]).into()),
                        (Y, f64::value(contact[1]).into()),
                        (Z, f64::value(contact[2]).into()),
                        (
                            names["normal"],
                            geometry.get(&names["normal"]).unwrap().clone().into(),
                        ),
                    ],
                );
                let center = read_point(center.result.as_value()).unwrap();
                let length = normal[0].hypot(normal[1]).hypot(normal[2]);
                let radius =
                    f64::read(doc.cells.value(names["tool_diameter"]).unwrap()).unwrap() / 2.0;
                for axis in 0..3 {
                    assert!(
                        (center[axis] - contact[axis] - radius * normal[axis] / length).abs()
                            < 1e-12
                    );
                }
            }
        }
    }
}
