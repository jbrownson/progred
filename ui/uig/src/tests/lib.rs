use super::*;

#[test]
fn visibility_and_hits_use_both_rectangles() {
    let placement = Placement::new(
        Rect::new(20.0, 50.0, 60.0, 90.0),
        Rect::new(40.0, 20.0, 120.0, 80.0),
    );
    assert_eq!(placement.visible_rect(), Rect::new(40.0, 50.0, 60.0, 80.0));
    assert!(!placement.contains(Point::new(30.0, 60.0)));
    assert!(placement.contains(Point::new(50.0, 60.0)));
    let hidden = Placement::new(Rect::new(70.0, 0.0, 90.0, 10.0), placement.clip_rect);
    assert!(hidden.clipped_out());
}

#[test]
fn available_space_is_not_a_hit_target_or_a_clip() {
    let placement = Placement::new(
        Rect::new(10.0, 20.0, 15.0, 30.0),
        Rect::new(0.0, 25.0, 100.0, 50.0),
    )
    .with_available_rect(Rect::new(10.0, 0.0, 15.0, 100.0));
    assert!(!placement.contains(Point::new(12.0, 40.0)));
    let filled = placement.fill_height();
    assert_eq!(filled.rect, placement.available_rect);
    assert_eq!(filled.clip_rect, placement.clip_rect);
    assert!(filled.contains(Point::new(12.0, 40.0)));
    assert!(!filled.contains(Point::new(12.0, 60.0)));
}
