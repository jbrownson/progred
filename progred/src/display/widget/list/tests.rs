use super::*;
use crate::display::recording::{Recorded, record};
use crate::display::widget::HoverInput;
use crate::display::widget::frame::{Attribution, HoverOutput};
use kurbo::{Point, Rect};
use puri::draw::{DrawCmd, DrawList, Shape};
use uig::Placement;

fn gap(active: bool) -> HoverOutput<(), u32> {
    let layout = insertion_gap(16.0, active.then_some(7));
    let Recorded::Widget(widget) = record(&layout) else {
        panic!("native gap")
    };
    let measured = crate::display::test_support::with_context(
        &crate::display::test_support::NoProject,
        |context| widget(context),
    );
    assert_eq!(measured.extent.width, 0.0);
    assert_eq!(measured.extent.height(), 16.0);
    super::super::frame::place(
        measured,
        Placement::new(
            Rect::new(10.0, 20.0, 100.0, 36.0),
            Rect::new(30.0, 0.0, 80.0, 100.0),
        )
        .with_available_rect(Rect::new(10.0, 20.0, 100.0, 36.0)),
        &HoverInput::default(),
    )
}

#[test]
fn insertion_gap_only_paints_feedback_and_respects_the_clip() {
    let output = gap(true);
    assert!(output.handler.is_none());
    assert!(
        output
            .hover_geometry
            .probe(Some(Point::new(50.0, 28.0)), None, 0.0)
            .is_none()
    );
    for hovered in [None, Some(7), Some(8)] {
        let mut drawing = DrawList::new();
        puri::frame::render(
            gap(true)
                .bind(Attribution {
                    hovered,
                    ..Default::default()
                })
                .renders,
            &mut drawing,
        );
        if hovered == Some(7) {
            assert!(
                matches!(&drawing.0[..], [DrawCmd::Stroke { shape: Shape::Line(line), .. }]
                if line.p0 == Point::new(30.0, 28.0) && line.p1 == Point::new(80.0, 28.0))
            );
        } else {
            assert!(drawing.0.is_empty());
        }
    }
}

#[test]
fn inactive_insertion_gap_is_only_spacing() {
    let gap = gap(false);
    assert!(
        gap.hover_geometry
            .probe(Some(Point::new(50.0, 28.0)), None, 0.0)
            .is_none()
    );
    assert!(gap.handler.is_none());
    assert!(gap.after_hover.is_empty());
}
