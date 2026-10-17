use super::*;
use kurbo::Shape;

#[test]
fn fits_extents_and_preserves_disconnected_passes() {
    let mut lines = projected();
    lines.start_at([0.0, 0.0, 0.0], Axis::Z).unwrap();
    lines.line_to([1.0, 1.0, 0.0]).unwrap();
    lines.start_at([1.0, 0.0, 0.0], Axis::Z).unwrap();
    lines.line_to([0.0, 1.0, 0.0]).unwrap();
    let path = fitted(lines.sink, Size::new(600.0, 300.0)).unwrap();
    assert_eq!(
        path.elements()
            .iter()
            .filter(|p| matches!(p, kurbo::PathEl::MoveTo(_)))
            .count(),
        2
    );
    let bounds = path.bounding_box();
    assert!(bounds.x0 >= 15.99 && bounds.y0 >= 15.99);
    assert!(bounds.x1 <= 584.01 && bounds.y1 <= 284.01);
}

mod frames;
