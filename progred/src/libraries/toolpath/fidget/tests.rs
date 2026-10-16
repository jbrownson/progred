use super::*;
use fidget_engine::{shape::EzShape, vm::VmShape};

fn sample(tree: Tree, point: [f32; 3]) -> f32 {
    let shape = VmShape::from(tree);
    let mut evaluator = VmShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    evaluator
        .eval(&tape, &[point[0]], &[point[1]], &[point[2]])
        .unwrap()[0]
}

#[test]
fn capsule_has_round_ends_and_the_requested_radius() {
    let tree = capsule([0.0; 3], [1.0, 1.0, 1.0], 0.1).unwrap();
    assert!(sample(tree.clone(), [0.5; 3]) < 0.0);
    assert!(sample(tree.clone(), [0.5, 0.7, 0.5]) > 0.0);
    let end = 1.0 + 0.1 / 3.0_f32.sqrt();
    assert!(sample(tree, [end; 3]).abs() < 1e-6);
    let tree = capsule([0.0; 3], [1.0, 0.0, 0.0], 0.1).unwrap();
    assert!(sample(tree.clone(), [0.5, 0.1, 0.0]).abs() < 1e-6);
    assert!(sample(tree, [-0.1, 0.0, 0.0]).abs() < 1e-6);
}

#[test]
fn tubes_preserve_breaks_and_do_not_draw_isolated_starts() {
    let mut tubes = Tubes::new(0.1).unwrap();
    tubes.start_at([100.0; 3], Axis::Z).unwrap();
    assert!(tubes.path.is_none());
    assert!(tubes.paths.is_empty());
    tubes.start_at([0.0; 3], Axis::Z).unwrap();
    tubes.line_to([1.0, 0.0, 0.0]).unwrap();
    tubes.line_to([2.0, 0.0, 0.0]).unwrap();
    tubes.start_at([0.0, 2.0, 0.0], Axis::Z).unwrap();
    tubes.line_to([1.0, 2.0, 0.0]).unwrap();
    tubes.start_at([100.0; 3], Axis::Z).unwrap();
    let paths = tubes
        .scene()
        .into_iter()
        .map(|object| object.tree)
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2);
    assert!(sample(paths[0].clone(), [0.5, 0.0, 0.0]) < 0.0);
    assert!(sample(paths[0].clone(), [1.5, 0.0, 0.0]) < 0.0);
    assert!(sample(paths[0].clone(), [0.5, 2.0, 0.0]) > 0.0);
    assert!(sample(paths[1].clone(), [0.5, 2.0, 0.0]) < 0.0);
    for path in paths {
        assert!(sample(path, [0.5, 1.0, 0.0]) > 0.0, "no connecting move");
    }
}

#[test]
fn duplicate_points_are_spheres_and_invalid_emissions_leave_the_sink_unchanged() {
    let mut tubes = Tubes::new(0.1).unwrap();
    assert_eq!(tubes.line_to([0.0; 3]), Err(InvalidPath::MissingStart));
    tubes.start_at([0.0; 3], Axis::Z).unwrap();
    tubes.line_to([0.0; 3]).unwrap();
    assert_eq!(
        tubes.start_at([f64::NAN; 3], Axis::Z),
        Err(InvalidPath::NonFinitePoint)
    );
    assert_eq!(
        tubes.line_to([f64::MAX; 3]),
        Err(InvalidPath::CoordinateRange)
    );
    tubes.line_to([0.0; 3]).unwrap();
    let paths = tubes
        .scene()
        .into_iter()
        .map(|object| object.tree)
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    assert!(sample(paths[0].clone(), [0.1, 0.0, 0.0]).abs() < 1e-6);
    for radius in [
        0.0,
        -1.0,
        f64::INFINITY,
        f64::NAN,
        f64::MAX,
        f64::MIN_POSITIVE,
    ] {
        assert!(Tubes::new(radius).is_none());
    }
}

#[test]
fn direct_and_recorded_paths_produce_the_same_fields() {
    fn draw(sink: &mut impl Sink<Error = InvalidPath>) {
        sink.start_at([0.0; 3], Axis::Z).unwrap();
        sink.line_to([1.0, 0.5, 0.1]).unwrap();
        sink.start_at([-1.0, 0.0, 0.5], Axis::Z).unwrap();
        sink.line_to([0.0, 0.5, 0.2]).unwrap();
    }
    let mut direct = Tubes::new(0.1).unwrap();
    draw(&mut direct);
    let mut recording = Recording::default();
    draw(&mut recording);
    let mut replayed = Tubes::new(0.1).unwrap();
    recording.replay(&mut replayed).unwrap();
    assert!(direct.scene() == replayed.scene());
}
