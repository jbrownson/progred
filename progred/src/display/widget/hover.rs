//! Hover regions and feedback are ordinary placement decorators.

use super::frame::Probe;
use super::{before, style};
use crate::display::Layout;
use puri::handler::HasHandler;
use puri::{Affine, Point};
use std::rc::Rc;

pub fn on_hover<World: 'static, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |_| {
            let target = target.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.claim(Probe::retaining(placement, target));
                }
            })
        }),
    )
}

pub fn block_hover<World: 'static, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(|_| {
            Box::new(|output, placement| {
                if !placement.clipped_out() {
                    output.claim(Probe::occludes(placement));
                    output.handler().on_pointer_down(move |_, event| {
                        placement
                            .contains(Point::new(event.state.position.x, event.state.position.y))
                    });
                }
            })
        }),
    )
}

/// Paint feedback for this target without changing who claims hover.
pub fn hover_highlight<World: 'static, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |context| {
            let target = target.clone();
            let scale = context.inputs.styles.scale;
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, hovered| {
                        if hovered
                            .hovered
                            .as_ref()
                            .is_some_and(|hovered| hovered == &target)
                        {
                            canvas.fill_shape(
                                style::highlight_outline(scale, placement.rect).into(),
                                style::hover_wash().into(),
                                Affine::IDENTITY,
                            );
                        }
                    });
                }
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recorded, record};
    use crate::display::widget::HoverOutput;
    use crate::display::widget::{HoverContext, HoverInput};
    use puri::handler::{PointerButtonEvent, PointerInfo, PointerState, PointerType};
    use puri::hover::Claim;
    use puri::{DrawList, Placement, Rect};

    fn place(layout: Layout<(), u32>, placement: Placement) -> HoverOutput<(), u32> {
        place_at(layout, placement, HoverInput::default())
    }

    fn place_at(
        layout: Layout<(), u32>,
        placement: Placement,
        input: HoverInput<'_, u32>,
    ) -> HoverOutput<(), u32> {
        let before = match record(&layout) {
            Recorded::Before { before, .. } | Recorded::After { after: before, .. } => before,
            _ => panic!("native decorator"),
        };
        let mut output = HoverOutput::default();
        crate::display::test_support::with_context(
            &crate::display::test_support::NoProject,
            |context| before(context),
        )(&mut HoverContext::new(input, &mut output), placement);
        output
    }

    #[test]
    fn a_border_uses_settled_geometry_without_requesting_site_or_event_capabilities() {
        let placement = Placement::root(Rect::new(10.0, 20.0, 40.0, 60.0));
        let mut output = place(
            crate::display::widget::border(crate::display::text("inside")),
            placement,
        );
        assert!(output.handler.is_none());
        assert!(output.claim.is_none());
        assert!(output.landmark_select.is_none());
        let mut drawing = DrawList::new();
        puri::frame::render(output.resolve(Default::default()), &mut drawing);
        assert!(
            matches!(&drawing.0[..], [puri::DrawCmd::Stroke { shape: puri::Shape::Rect(rect), style, .. }]
            if *rect == placement.rect.inset(-0.5) && style.width == 1.0)
        );
        let clipped = Placement::new(placement.rect, Rect::new(80.0, 80.0, 90.0, 90.0));
        assert!(
            place(
                crate::display::widget::border(crate::display::text("inside")),
                clipped
            )
            .after_hover
            .is_empty()
        );
    }

    #[test]
    fn claiming_and_painting_hover_are_independent() {
        let placement = Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        let claimed = place_at(
            on_hover(crate::display::text("target"), 7),
            placement,
            HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                reach: 4.0,
                ..Default::default()
            },
        );
        assert_eq!(
            claimed.claim.map(|(_, claim)| claim),
            Some(Claim::Direct(7))
        );
        assert_eq!(
            place_at(
                on_hover(crate::display::text("target"), 7),
                placement,
                HoverInput {
                    pointer: Some(Point::new(22.0, 5.0)),
                    prior: Some(&7),
                    reach: 4.0,
                    debug_geometry: false,
                }
            )
            .claim
            .map(|(_, claim)| claim),
            Some(Claim::Extended(7))
        );
        assert!(claimed.after_hover.is_empty());
        assert!(claimed.handler.is_none());
        for hovered in [None, Some(7), Some(8)] {
            let mut highlighted = place(
                hover_highlight(crate::display::text("feedback"), 7),
                placement,
            );
            assert!(highlighted.claim.is_none());
            assert!(highlighted.handler.is_none());
            let mut canvas = DrawList::new();
            puri::frame::render(
                highlighted.resolve(crate::display::widget::ResolvedHover {
                    hovered,
                    ..Default::default()
                }),
                &mut canvas,
            );
            assert_eq!(canvas.0.len(), usize::from(hovered == Some(7)));
        }
    }

    #[test]
    fn occlusion_blocks_starts_not_releases_and_respects_the_clip() {
        let placement = Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 10.0, 20.0),
        );
        let blocked = place(block_hover(crate::display::text("panel")), placement);
        let handler = blocked.handler.unwrap();
        for x in [5.0, 15.0, 25.0] {
            let event = PointerButtonEvent {
                button: None,
                pointer: PointerInfo {
                    pointer_id: None,
                    persistent_device_id: None,
                    pointer_type: PointerType::Touch,
                },
                state: PointerState {
                    position: (x, 5.0).into(),
                    ..Default::default()
                },
            };
            assert_eq!(handler.dispatch_pointer_down(&mut (), &event), x == 5.0);
            assert!(!handler.dispatch_pointer_up(&mut (), &event));
            assert_eq!(
                place_at(
                    block_hover(crate::display::text("panel")),
                    placement,
                    HoverInput {
                        pointer: Some(Point::new(x, 5.0)),
                        reach: 4.0,
                        ..Default::default()
                    }
                )
                .claim
                .map(|(_, claim)| claim),
                (x == 5.0).then_some(Claim::Occludes)
            );
        }
        let clipped = Placement::new(placement.rect, Rect::new(30.0, 0.0, 40.0, 20.0));
        for layout in [
            on_hover(crate::display::text("target"), 7),
            block_hover(crate::display::text("panel")),
            hover_highlight(crate::display::text("feedback"), 7),
        ] {
            let output = place(layout, clipped);
            assert!(
                output.claim.is_none() && output.handler.is_none() && output.after_hover.is_empty()
            );
        }
    }
}
