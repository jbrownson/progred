use super::*;
use puri::draw::{DrawCmd, DrawList};

#[test]
fn item_rects_and_span_drags_use_the_same_notches_at_every_density() {
    for scale in [1.0, 2.0] {
        for count in [1, 4, 500] {
            let slider = RangeSlider::new(count, 0..count).unwrap();
            let rect = Rect::new(10.0, 30.0, 410.0, 30.0 + HEIGHT * scale);
            for item in 0..count {
                let center = slider.item_rect(rect, scale, item).unwrap().center();
                let drag = slider.begin(rect, scale, center);
                assert_eq!(slider.dragged(drag, rect, scale, center), item..item + 1);
                assert!(rect.contains(center));
            }
            assert_eq!(slider.item_rect(rect, scale, count), None);
            let empty = Rect::new(0.0, 0.0, 0.0, HEIGHT * scale);
            assert_eq!(slider.item_rect(empty, scale, 0), None);
        }
    }
}

#[test]
fn separators_fit_the_notches_and_use_physical_pixel_visibility() {
    let slider = RangeSlider::new(10, 2..8).unwrap();
    for scale in [1.0, 2.0, 3.0] {
        for spacing_px in [0.0, 1.5, 1.99, 2.0, 3.0, 8.0, 40.0] {
            let rect = Rect::new(
                0.25,
                0.0,
                0.25 + 2.0 * HANDLE * scale + 10.0 * spacing_px,
                HEIGHT * scale,
            );
            let mut list = DrawList::new();
            slider.draw(&mut list, rect, scale, None);
            let strokes: Vec<_> = list
                .0
                .iter()
                .filter_map(|cmd| match cmd {
                    DrawCmd::Stroke { style, .. } => Some(style.width),
                    _ => None,
                })
                .collect();
            if spacing_px < 2.0 {
                assert!(strokes.is_empty());
            } else {
                assert_eq!(strokes.len(), slider.count + 1);
                for width in strokes {
                    assert!((width - scale.min(spacing_px * 0.25)).abs() < 1e-12);
                }
            }
            assert_eq!(
                list.0
                    .iter()
                    .filter(|cmd| matches!(cmd, DrawCmd::Fill { .. }))
                    .count(),
                4
            );
        }
    }
}

#[test]
fn current_marker_uses_item_geometry_and_stays_visible_in_dense_rows() {
    for scale in [1.0, 2.0] {
        for count in [4, 500] {
            let slider = RangeSlider::new(count, 0..count).unwrap();
            let rect = Rect::new(10.0, 0.0, 110.0 * scale, HEIGHT * scale);
            let rail = RangeSlider::rail(rect, scale);
            let notch = rail.width() / count as f64;
            for current in [0, count / 2, count - 1] {
                let mut drawing = DrawList::new();
                slider.draw(&mut drawing, rect, scale, Some(current));
                let Some(DrawCmd::Fill {
                    shape: puri::Shape::Rect(marker),
                    ..
                }) = drawing.0.last()
                else {
                    panic!("current item marker");
                };
                let center = rail.x0 + (current as f64 + 0.5) * notch;
                assert!(marker.x0 <= center && marker.x1 >= center);
                assert!(marker.x0 >= rail.x0 && marker.x1 <= rail.x1);
                assert_eq!(marker.width(), notch.max(3.0 * scale));
                assert_eq!(marker.height(), 2.0 * scale);
                assert!(marker.y1 <= rect.y1);
            }
            let mut plain = DrawList::new();
            let mut invalid = DrawList::new();
            slider.draw(&mut plain, rect, scale, None);
            slider.draw(&mut invalid, rect, scale, Some(count));
            assert_eq!(plain.0.len(), invalid.0.len());
        }
    }
}

#[test]
fn hidden_separators_keep_individual_items_selectable() {
    let slider = RangeSlider::new(10, 0..10).unwrap();
    let rect = Rect::new(0.0, 0.0, 35.0, 48.0);
    let point = Point::new(15.25, 24.0);
    let drag = slider.begin(rect, 2.0, point);
    assert_eq!(slider.dragged(drag, rect, 2.0, point), 3..4);
    assert_eq!(
        slider.dragged(drag, rect, 2.0, Point::new(21.25, 24.0)),
        3..8
    );
}

#[test]
fn click_drag_and_handles_share_painted_geometry() {
    let slider = RangeSlider::new(4, 0..4).unwrap();
    let rect = Rect::new(0.0, 0.0, 410.0, 24.0);
    let point = Point::new(155.0, 12.0);
    let drag = slider.begin(rect, 1.0, point);
    assert_eq!(slider.dragged(drag, rect, 1.0, point), 1..2);
    assert_eq!(
        slider.dragged(drag, rect, 1.0, Point::new(355.0, -500.0)),
        1..4
    );
    assert_eq!(
        slider.dragged(drag, rect, 1.0, Point::new(-500.0, 0.0)),
        0..2
    );
    let drag = slider.begin(rect, 1.0, Point::new(405.0, 12.0));
    assert_eq!(
        slider.dragged(drag, rect, 1.0, Point::new(205.0, 0.0)),
        0..2
    );
    let drag = slider.begin(rect, 1.0, Point::new(5.0, 12.0));
    assert_eq!(
        slider.dragged(drag, rect, 1.0, Point::new(305.0, 0.0)),
        3..4
    );
    assert!(RangeSlider::new(0, 0..0).is_none());
}
