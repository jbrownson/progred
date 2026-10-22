//! A view's identity, and the widgets that mark where a view is drawn.

use super::HoverPass;
use super::container;
use super::frame::ViewRegion;
use gid::{CellId, Path};
use kurbo::Vec2;
use measured::Measured;
use puri::handler::ScrollOutcome;
use std::rc::Rc;
use ui_events::pointer::PointerScrollEvent;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Document,
    /// The part of the document's at `path`, which its root splits into.
    Pane {
        path: Path,
    },
    /// A pane the editor opened beside the document.
    Opened {
        id: CellId,
    },
}

/// A view: the document's, one of its parts', or a pane the editor opened.
/// A view is what it targets, and its occurrences' paths say which that is,
/// so a path is enough to find its view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Root(Rc<Target>);

impl Root {
    pub fn document() -> Self {
        Self(Rc::new(Target::Document))
    }

    pub fn pane(path: Path) -> Self {
        Self(Rc::new(Target::Pane { path }))
    }

    pub fn opened(id: CellId) -> Self {
        Self(Rc::new(Target::Opened { id }))
    }

    pub fn target(&self) -> &Target {
        &self.0
    }
}

/// A scroll viewport, tagged as the editor view `root`. The settled region
/// is frame output, not retained widget state; input and keyboard reveal use
/// it to update the same caller-owned view.
pub fn scrolled_at<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    offset: Vec2,
    root: Root,
    scale: f64,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<C, H>> {
    let extent = child.extent;
    let scrolled = container::scrolled(child, offset, scale, on_scroll);
    super::before_place(scrolled, move |placement, output| {
        output.view_region(ViewRegion {
            root,
            rect: placement.rect,
            maximum: super::scroll::maximum(extent, placement.rect.size(), scale),
            content: Some(Vec2::new(extent.width, extent.height()) / scale),
        });
    })
}

/// An assigned-size editor view, with no content scrolling or padding.
pub fn viewport<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    root: Root,
) -> Measured<HoverPass<C, H>> {
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
pub fn in_view<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    root: Root,
) -> Measured<HoverPass<C, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.in_view(root, |pass| inner.place_at_into(placement, pass));
    })
}
