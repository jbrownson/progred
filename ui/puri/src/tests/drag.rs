use super::*;

#[test]
fn threshold_uses_logical_radial_distance_and_preserves_the_first_motion() {
    let mut drag = Drag::new(Point::new(100.0, 200.0), 2.0, 5.0);
    assert_eq!(drag.advance(Point::new(104.0, 206.0)), None);
    assert_eq!(
        drag.advance(Point::new(106.0, 208.0)),
        Some(Motion {
            movement: Vec2::new(3.0, 4.0),
            distance: Vec2::new(3.0, 4.0),
        })
    );
    assert_eq!(
        drag.advance(Point::new(100.0, 200.0)),
        Some(Motion {
            movement: Vec2::new(-3.0, -4.0),
            distance: Vec2::ZERO,
        })
    );
    assert_eq!(
        drag.advance(Point::new(-100.0, 200.0)),
        Some(Motion {
            movement: Vec2::new(-100.0, 0.0),
            distance: Vec2::new(-100.0, 0.0),
        })
    );
}
