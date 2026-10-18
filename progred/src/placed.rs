//! Progred adapters for the placement → hover → paint/dispatch chain.
//! The shared after-hover boundary keeps paint optional and resolves targets
//! before either painting or dispatch sees the frame.

use crate::display::widget::container::{self, Layers};
use crate::frame::Hovered;
use crate::workspace::Root;
use kurbo::Vec2;
use measured::Measured;
use puri::handler::ScrollOutcome;
#[cfg(test)]
use puri::hover::Claim;
use ui_events::pointer::PointerScrollEvent;

pub type HoverPass<C> = crate::display::widget::HoverPass<C, Hovered>;
#[cfg(test)]
pub type HoverOutput<C> = crate::display::widget::HoverOutput<C, Hovered>;
#[cfg(test)]
pub use crate::display::widget::HoverContext;
pub use crate::display::widget::HoverInput;
pub type DispatchContext<C> = crate::display::widget::frame::DispatchContext<C, Hovered>;
pub type Attribution = crate::display::widget::frame::Attribution<Hovered>;
pub use puri::frame::Render;
#[cfg(test)]
pub type Probe = crate::display::widget::frame::Probe<Hovered>;
pub use crate::display::widget::frame::{Cursor, ViewRegion};

/// App-facing construction inside the hover continuation, over the frame's
/// hover type.
#[cfg(test)]
pub(crate) type Builder<'a, 'input, C> =
    crate::display::widget::placed::Builder<'a, 'input, C, Hovered>;
pub use crate::display::widget::placed::{before, decorate, leaf, text};

/// Add `content` as an out-of-flow subtree without contributing its
/// extent to `base`. The completed frame raises all such subtrees
/// together.
pub use crate::display::widget::container::floating;

/// A scroll viewport, tagged as an editor view when it has an owner. The
/// settled region is frame output, not retained widget state; input and
/// keyboard reveal use it to update the same caller-owned view.
pub fn scrolled_at<C: 'static>(
    child: Measured<HoverPass<C>>,
    offset: Vec2,
    owner: Option<(Root, f64)>,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<C>> {
    let extent = child.extent;
    let scale = owner.as_ref().map(|(_, scale)| *scale).unwrap_or(1.0);
    let scrolled = container::scrolled(child, offset, scale, on_scroll);
    before(scrolled, move |output, placement| {
        if let Some((root, scale)) = owner {
            output.placed.view_region(ViewRegion {
                root,
                rect: placement.rect,
                maximum: Vec2::new(
                    ((extent.width - placement.rect.width()) / scale).max(0.0),
                    ((extent.height() - placement.rect.height()) / scale).max(0.0),
                ),
                content: Some(Vec2::new(extent.width, extent.height()) / scale),
            });
        }
    })
}

/// An assigned-size editor view, with no content scrolling or padding.
pub fn viewport<C: 'static>(child: Measured<HoverPass<C>>, root: Root) -> Measured<HoverPass<C>> {
    measured::around_into(child, move |placement, inner, pass| {
        let child_rect = inner.extent().rect_at(placement.rect.origin());
        let child_placement = measured::child_placement(
            measured::clipped_placement(placement, placement.rect),
            child_rect,
        );
        pass.clipped(placement, |pass| inner.place_at_into(child_placement, pass));
        pass.visit(|placed| {
            placed.view_region(ViewRegion {
                root,
                rect: placement.rect,
                maximum: Vec2::ZERO,
                content: None,
            });
        });
    })
}

/// Associate every navigation occurrence produced by `child` with
/// one editor view. This happens after projection and placement, so
/// the projection language remains unaware of panes.
pub fn in_view<C: 'static>(child: Measured<HoverPass<C>>, root: Root) -> Measured<HoverPass<C>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.in_view(root, |pass| inner.place_at_into(placement, pass));
    })
}

#[cfg(test)]
mod tests;
