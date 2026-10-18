//! Source following shared by generated drawings and ordinary widgets.
use super::frame::DispatchContext;
use crate::display::Host;
use crate::hover::{Hover, SourceTrace};
use crate::sources::Sources;
use puri::handler::{Event, EventOutcome, HasHandler};
use puri::{Placement, Point};
use std::rc::Rc;

pub(crate) fn decoration<W: Host>(
    source: SourceTrace,
) -> crate::display::widget::Decoration<W, W::Hover> {
    Rc::new(move |context| {
        let source = source.clone();
        let selected = context.toolkit.selected_trace() == Some(&source);
        let selection = context.toolkit.styles().selection_wash.clone();
        let hover = context.toolkit.styles().accent_wash.brush.clone();
        let scale = context.toolkit.styles().scale;
        Box::new(move |output, placement| {
            output.claim(puri::hover::Probe::exact(
                placement,
                W::Hover::from(Hover::Source(source.clone())),
            ));
            let command = output.input.command_modifier;
            handlers(output, placement, scale, command);
            output.render(move |canvas, resolved| {
                let brush = if selected {
                    Some(selection)
                } else if resolved.hovered_trace.as_ref() == Some(&source) {
                    Some(hover)
                } else {
                    None
                };
                if let Some(brush) = brush {
                    use puri::draw::Canvas;
                    // Avoid a pane-sized clip layer for every highlighted notch.
                    canvas.fill(
                        placement.rect.intersect(placement.clip_rect),
                        brush,
                        puri::Affine::IDENTITY,
                    );
                }
            });
        })
    })
}

/// Cmd+click on a source drawn in `placement` selects where it's drawn, and
/// holding Cmd over it reveals that.
pub(crate) fn handlers<W: Host>(
    output: &mut impl HasHandler<W, Input = DispatchContext<W, W::Hover>>,
    placement: Placement,
    scale: f64,
    command: puri::keyboard::CommandModifier,
) {
    if !placement.clipped_out() {
        output
            .handler()
            .on_pointer_down_with(move |world, event, input| {
                puri::interact::is_primary_contact(event)
                    && command.pressed(&event.state.modifiers)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && match input.hovered().and_then(W::tree) {
                        Some(hover @ (Hover::Source(_) | Hover::Calls(_))) => {
                            let target = {
                                let session = world.session();
                                let sources = session.sources();
                                let Some(source) = hover_source(&sources, &input.landmarks, hover)
                                else {
                                    return true;
                                };
                                source_landmark(&sources, &input.landmarks, &source)
                            };
                            if let Some(target) = target {
                                input.geometry(scale).arrive(world, target, None);
                            }
                            true
                        }
                        _ => false,
                    }
            });
        output.handler().on(move |world, event, input| {
            let handled = matches!(event, Event::HoverChanged | Event::ModifiersChanged(_))
                && !input.pressed
                && command.pressed(&input.modifiers)
                && input.pointer.is_some_and(|point| placement.contains(point))
                && match input.hovered().and_then(W::tree) {
                    Some(hover @ (Hover::Source(_) | Hover::Calls(_))) => {
                        let session = world.session();
                        let sources = session.sources();
                        let target = hover_source(&sources, &input.landmarks, hover)
                            .and_then(|source| source_landmark(&sources, &input.landmarks, &source))
                            .and_then(|landmark| {
                                landmark.root.clone().map(|root| (root, landmark.rect))
                            });
                        if let Some((root, rect)) = target {
                            input
                                .geometry(scale)
                                .reveal_rect(session.model, &root, rect);
                        }
                        true
                    }
                    _ => false,
                };
            EventOutcome::from_handled(event, handled)
        });
    }
}

/// Source availability is a property of this projection, not of evaluation or
/// viewport clipping. Preserve leaf attribution if no call has an occurrence.
pub(crate) fn hover_source<World>(
    sources: &Sources<'_>,
    landmarks: &[super::navigation::Landmark<World>],
    hover: &Hover,
) -> Option<SourceTrace> {
    match hover {
        Hover::Source(source) => Some(source.clone()),
        Hover::Calls(calls) => calls
            .sources()
            .find(|source| source_landmark(sources, landmarks, source).is_some())
            .or_else(|| calls.sources().next()),
        _ => None,
    }
}

pub(crate) fn source_landmark<'a, World>(
    sources: &Sources<'_>,
    landmarks: &'a [super::navigation::Landmark<World>],
    source: &SourceTrace,
) -> Option<&'a super::navigation::Landmark<World>> {
    landmarks.iter().find(|landmark| {
        landmark.root.is_some()
            && landmark
                .scope
                .location(&landmark.path)
                .is_some_and(|location| SourceTrace::from_location(sources, &location) == *source)
    })
}
