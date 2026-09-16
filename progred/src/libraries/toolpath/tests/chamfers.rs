use super::super::{
    cutter::{Point, Section, SectionKind, Tool},
    stock::Stock,
};
use super::*;
use crate::{gid_text::Binders, sources::Sources};
use fidget_engine::{shape::EzShape, vm::VmShape};
use nalgebra::Vector3;

fn program(sources: &Sources<'_>, names: &Binders, name: &str) -> Recording {
    let mut path = Recording::default();
    let result = run(&mut path, |scope| {
        ::grap::apply_scoped(&names[name].into(), [], sources, scope, 100_000)
    });
    assert!(
        result.completed && !absent::is_absent(&result.result),
        "{:?}",
        result.result
    );
    path
}

fn square_tool(sources: &Sources<'_>, names: &Binders) -> Tool {
    let result = ::grap::evaluate(&names["square_tool"].into(), sources, 1000);
    assert!(result.completed);
    Tool::read(&result.result).unwrap()
}

#[test]
fn chamfer_groups_cover_each_edge_once_with_the_shared_square_tool_dimensions() {
    let (original, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    for (size, chamfer, diameter, length) in [(1.0, 0.1, 0.125, 0.22), (0.9, 0.14, 0.16, 0.3)] {
        let mut doc = original.clone();
        geometry::set_parameter(&mut doc, &names, "size", size);
        geometry::set_parameter(&mut doc, &names, "chamfer", chamfer);
        doc.cells
            .set_value(names["square_diameter"], f64::value(diameter));
        doc.cells
            .set_value(names["cutting_length"], f64::value(length));
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let tool = square_tool(&sources, &names);
        let cutting = &tool.sections[0];
        assert_eq!(
            cutting,
            &Section::taper(
                Point::new(diameter / 2.0, 0.0),
                Point::new(diameter / 2.0, length),
                SectionKind::Cutting,
            )
        );
        let mut normals = Vec::new();
        for (name, count) in [("op1_chamfers", 8), ("op2_chamfers", 4)] {
            let path = program(&sources, &names, name);
            assert_eq!(path.commands().len(), count * 2);
            assert_eq!(path.segments().count(), count);
            assert!((path.length().unwrap() - size * count as f64).abs() < 1e-12);
            for (a, b, axis, active) in path.tool_segments() {
                assert_eq!(active, Some(&tool));
                let feed = (Vector3::from(b) - Vector3::from(a)).normalize();
                let axis = Vector3::from(axis.vector());
                let normal = feed.cross(&axis);
                assert!((normal.norm() - 1.0).abs() < 1e-12);
                assert!(normal.dot(&axis).abs() < 1e-12);
                let signature: [i32; 3] =
                    std::array::from_fn(|i| (normal[i] * 2.0_f64.sqrt()).round() as i32);
                assert_eq!(signature.iter().filter(|&&v| v != 0).count(), 2);
                assert!(
                    !normals.contains(&signature),
                    "duplicate edge {signature:?}"
                );
                normals.push(signature);
                assert_eq!(signature[2] < 0, name == "op2_chamfers");
                assert!((axis.z - normal.z).abs() < 1e-12);
                let contact = (Vector3::from(a) + Vector3::from(b)) / 2.0 + axis * (length / 2.0)
                    - normal * (diameter / 2.0);
                let expected =
                    Vector3::from(signature.map(|sign| sign as f64 * (size - chamfer) / 2.0));
                assert!(
                    (contact - expected).norm() < 1e-12,
                    "{contact:?} != {expected:?}"
                );
            }
        }
        assert_eq!(normals.len(), 12);
    }
}

#[test]
fn square_contours_subtract_the_twelve_chamfer_planes_without_gouging() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let mut stock = Stock::block([-0.5; 3], [0.5; 3]).unwrap();
    for name in ["op1_chamfers", "op2_chamfers"] {
        let path = program(&sources, &names, name);
        for (a, b, axis, tool) in path.tool_segments() {
            stock.cut(tool.unwrap(), a, b, axis, 0.001).unwrap();
        }
    }
    let shape = VmShape::from(stock.into_field());
    let tape = shape.ez_float_slice_tape();
    let mut eval = VmShape::new_float_slice_eval();
    // Offset the grid to avoid testing a sign exactly on a surface.
    let points: Vec<[f32; 3]> = (-20..=20)
        .flat_map(|x| {
            (-20..=20).flat_map(move |y| {
                (-20..=20).map(move |z| {
                    [
                        x as f32 * 0.026 + 0.001,
                        y as f32 * 0.026 + 0.002,
                        z as f32 * 0.026 + 0.003,
                    ]
                })
            })
        })
        .collect();
    let xyz: [Vec<f32>; 3] = std::array::from_fn(|i| points.iter().map(|p| p[i]).collect());
    let actual = eval.eval(&tape, &xyz[0], &xyz[1], &xyz[2]).unwrap();
    for (point, &actual) in points.iter().zip(actual) {
        let [x, y, z] = point.map(f32::abs);
        let expected = (x - 0.5)
            .max(y - 0.5)
            .max(z - 0.5)
            .max(x + y - 0.9)
            .max(x + z - 0.9)
            .max(y + z - 0.9);
        if expected.abs() > 1e-5 {
            assert_eq!(
                actual < 0.0,
                expected < 0.0,
                "{point:?}: actual {actual}, expected {expected}"
            );
        }
    }
}
