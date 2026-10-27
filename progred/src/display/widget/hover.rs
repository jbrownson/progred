//! Hover regions and feedback are ordinary placement decorators.

use super::{before, style};
use crate::display::Layout;
use puri::hover::Probe;
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
        Rc::new(|_| Box::new(|output, placement| output.occlude(placement))),
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
            let scale = context.toolkit.styles().scale;
            let palette = context.toolkit.styles().palette;
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, hovered| {
                        if hovered
                            .hovered
                            .as_ref()
                            .is_some_and(|hovered| hovered == &target)
                        {
                            style::hover_highlight(
                                palette,
                                scale,
                                canvas,
                                style::highlight_outline(scale, placement.rect),
                            );
                        }
                    });
                }
            })
        }),
    )
}

#[cfg(test)]
#[path = "tests/hover.rs"]
mod tests;
