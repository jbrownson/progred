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
