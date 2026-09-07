use super::*;
use crate::command::Example;
use progred_libraries::{control, f32};

fn cube(size: f32, chamfer: f32, depth: f32) -> Value {
    let (mut doc, names) = crate::gid_text::parse(Example::Cube.source()).unwrap();
    let id = names["cube"];
    let mut definition = doc.cells.value(id).unwrap().clone();
    for (name, value) in [
        ("size", size),
        ("chamfer", chamfer),
        ("control_depth", depth),
    ] {
        let bindings = definition
            .as_record()
            .unwrap()
            .get(&grap::vocabulary::BODY)
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
        definition = crate::spine::set(
            Some(&definition),
            &[
                Step::Key(grap::vocabulary::BODY),
                Step::Key(control::vocabulary::BINDINGS),
                Step::Element(position),
                Step::Key(control::vocabulary::VALUE),
            ],
            f32::value(value),
        )
        .unwrap();
    }
    doc.cells.set_value(id, definition);
    let libraries = core_libraries();
    let evaluated = grap::evaluate(&grap::call(id.into(), []), &src(&doc, &libraries), 10_000);
    assert!(evaluated.completed);
    assert!(
        !progred_libraries::absent::is_absent(&evaluated.result),
        "{:?}",
        evaluated.result
    );
    evaluated.result
}

// Independent scalar interpretation of the generated field; rendering tests
// also lower this document through the real Fidget backend.
fn sample(field: &Value, point: [f32; 3]) -> f32 {
    use fidget::vocabulary::*;
    if let Some(number) = f32::read(field) {
        return number;
    }
    let fields = field.as_record().unwrap();
    if let Some(axis) = fields.get(&AXIS).and_then(Value::as_cell) {
        return point[[X, Y, Z].iter().position(|id| *id == axis).unwrap()];
    }
    let (operation, arguments) = fields.iter().next().unwrap();
    let arguments = arguments.as_record().unwrap();
    let read = |key| sample(arguments.get(&key).unwrap(), point);
    match *operation {
        SUM => read(LEFT) + read(RIGHT),
        SUBTRACT => read(LEFT) - read(RIGHT),
        MULTIPLY => read(LEFT) * read(RIGHT),
        DIVIDE => read(LEFT) / read(RIGHT),
        MAX => read(LEFT).max(read(RIGHT)),
        ABS => read(OPERAND).abs(),
        SQUARE => read(OPERAND).powi(2),
        _ => panic!("unexpected field operation: {operation:?}"),
    }
}

fn rhino_patch(size: f32, chamfer: f32, depth: f32, u: f32, v: f32) -> [f32; 3] {
    let bernstein = |t: f32| [(1.0 - t).powi(2), 2.0 * t * (1.0 - t), t.powi(2)];
    let half_face = size / 2.0 - chamfer;
    bernstein(u)
        .into_iter()
        .enumerate()
        .flat_map(|(i, bu)| {
            bernstein(v).into_iter().enumerate().map(move |(j, bv)| {
                let control = [
                    half_face * (1.0 - i as f32),
                    half_face * (j as f32 - 1.0),
                    size / 2.0 - if i == 1 && j == 1 { depth } else { 0.0 },
                ];
                control.map(|coordinate| coordinate * bu * bv)
            })
        })
        .fold([0.0; 3], |sum, point| {
            std::array::from_fn(|i| sum[i] + point[i])
        })
}

fn assert_boundary(field: &Value, point: [f32; 3], outward: [f32; 3], size: f32) {
    let residual = sample(field, point);
    assert!(residual.abs() < size * 2e-6, "{point:?}: {residual}");
    let offset = |sign| std::array::from_fn(|i| point[i] + sign * outward[i] * size * 1e-4);
    assert!(sample(field, offset(-1.0)) < 0.0, "inside {point:?}");
    assert!(sample(field, offset(1.0)) > 0.0, "outside {point:?}");
}

#[test]
fn fidget_cube_matches_all_six_rhino_control_point_surfaces() {
    for (size, chamfer, depth) in [(1.0, 0.1, 0.5), (2.0, 0.2, 1.0), (1.0, 0.15, 0.2)] {
        let field = cube(size, chamfer, depth);
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let outward = std::array::from_fn(|i| if i == axis { sign } else { 0.0 });
                for u in 0..=12 {
                    for v in 0..=12 {
                        let local =
                            rhino_patch(size, chamfer, depth, u as f32 / 12.0, v as f32 / 12.0);
                        let mut point = [0.0; 3];
                        point[axis] = sign * local[2];
                        point[(axis + 1) % 3] = local[0];
                        point[(axis + 2) % 3] = local[1];
                        assert_boundary(&field, point, outward, size);
                    }
                }
            }
        }
    }
}

#[test]
fn fidget_cube_chamfers_meet_the_face_edges_and_three_way_corners() {
    let field = cube(1.0, 0.1, 0.5);
    let hexagon = [
        [0.5, 0.4, -0.4],
        [0.5, 0.4, 0.4],
        [0.45, 0.45, 0.45],
        [0.4, 0.5, 0.4],
        [0.4, 0.5, -0.4],
        [0.45, 0.45, -0.45],
    ];
    for (a, b, c) in [(0, 1, 2), (0, 2, 1), (1, 2, 0)] {
        for sa in [-1.0, 1.0] {
            for sb in [-1.0, 1.0] {
                for index in 0..6 {
                    for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        let edge: [f32; 3] = std::array::from_fn(|i| {
                            hexagon[index][i] * (1.0 - t) + hexagon[(index + 1) % 6][i] * t
                        });
                        for radial in [0.0, 0.5, 1.0] {
                            let local: [f32; 3] = std::array::from_fn(|i| {
                                [0.45, 0.45, 0.0][i] * (1.0 - radial) + edge[i] * radial
                            });
                            let mut point = [0.0; 3];
                            point[a] = sa * local[0];
                            point[b] = sb * local[1];
                            point[c] = local[2];
                            let mut outward = [0.0; 3];
                            outward[a] = sa;
                            outward[b] = sb;
                            assert_boundary(&field, point, outward, 1.0);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fidget_cube_center_depth_is_a_quarter_of_the_control_point_displacement() {
    let field = cube(1.0, 0.1, 0.5);
    assert_boundary(&field, [0.0, 0.0, 0.375], [0.0, 0.0, 1.0], 1.0);
    assert!(sample(&field, [0.0; 3]) < 0.0);
    assert!(sample(&field, [0.5; 3]) > 0.0);
    let flat = cube(1.0, 0.1, 0.0);
    assert_boundary(&flat, [0.0, 0.0, 0.5], [0.0, 0.0, 1.0], 1.0);
}
