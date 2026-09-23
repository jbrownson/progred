//! Source following shared by generated drawings and ordinary widgets.
use crate::{
    Editor,
    frame::Hovered,
    hover::{Hover, SourceTrace},
    sources::Sources,
};
use puri::handler::{Event, EventOutcome, HasHandler};
use puri::{Placement, Point};
use std::rc::Rc;

pub(crate) fn decoration(
    source: SourceTrace,
) -> crate::display::widget::Decoration<Editor, Hovered> {
    Rc::new(move |context| {
        let source = source.clone();
        let selected = context.inputs.selected_trace.as_ref() == Some(&source);
        let selection = context.inputs.styles.selection_wash.clone();
        let hover = context.inputs.styles.accent_wash.brush.clone();
        let scale = context.inputs.styles.scale;
        Box::new(move |output, placement| {
            output.claim(puri::hover::Probe::exact(
                placement,
                Hovered::Tree(Hover::Source(source.clone())),
            ));
            handlers(output, placement, scale);
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

pub(crate) fn handlers(
    output: &mut impl HasHandler<Editor, Input = crate::placed::DispatchContext<Editor>>,
    placement: Placement,
    scale: f64,
) {
    if !placement.clipped_out() {
        output
            .handler()
            .on_pointer_down_with(move |world, event, input| {
                puri::interact::is_primary_contact(event)
                    && world.command_modifier.pressed(&event.state.modifiers)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && match input.hovered() {
                        Some(Hovered::Tree(hover @ (Hover::Source(_) | Hover::Calls(_)))) => {
                            let Some(source) =
                                hover_source(&world.sources(), &input.descends, hover)
                            else {
                                return true;
                            };
                            if let Some(target) =
                                source_descend(&world.sources(), &input.descends, &source)
                            {
                                input.geometry(scale).arrive(world, target, None);
                            }
                            true
                        }
                        _ => false,
                    }
            });
        output.handler().on(move |world, event, input| {
            let handled = matches!(event, Event::HoverChanged | Event::ModifiersChanged(_))
                && !world.pressed
                && world.command_modifier.pressed(&world.modifiers)
                && world.pointer.is_some_and(|point| placement.contains(point))
                && match input.hovered() {
                    Some(Hovered::Tree(hover @ (Hover::Source(_) | Hover::Calls(_)))) => {
                        let target = hover_source(&world.sources(), &input.descends, hover)
                            .and_then(|source| {
                                source_descend(&world.sources(), &input.descends, &source)
                            })
                            .and_then(|descend| {
                                descend.root.clone().map(|root| (root, descend.rect))
                            });
                        if let Some((root, rect)) = target {
                            input.geometry(scale).reveal_rect(world, &root, rect);
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
    descends: &[crate::navigate::Descend<World>],
    hover: &Hover,
) -> Option<SourceTrace> {
    match hover {
        Hover::Source(source) => Some(source.clone()),
        Hover::Calls(calls) => calls
            .sources()
            .find(|source| source_descend(sources, descends, source).is_some())
            .or_else(|| calls.sources().next()),
        _ => None,
    }
}

pub(crate) fn source_descend<'a, World>(
    sources: &Sources<'_>,
    descends: &'a [crate::navigate::Descend<World>],
    source: &SourceTrace,
) -> Option<&'a crate::navigate::Descend<World>> {
    descends.iter().find(|descend| {
        descend.root.is_some()
            && descend.scope.source(&descend.path).is_some_and(|path| {
                SourceTrace::from_path(sources, Rc::from(path.as_ref())) == *source
            })
    })
}
