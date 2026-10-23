//! A view's identity, and the widgets that mark where a view is drawn.

use super::HoverPass;
use super::container;
use super::frame::ViewRegion;
use gid::{CellId, Path, Step};
use kurbo::Vec2;
use measured::Measured;
use puri::handler::ScrollOutcome;
use std::rc::Rc;
use ui_events::pointer::PointerScrollEvent;

/// Which view something is in: the document's, or a pane the editor opened
/// beside it. It's the first tier of where something is drawn; the path
/// within the view, a document path, is the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewId {
    Document,
    Opened(CellId),
}

/// An area of the window that draws one value of a view: the view's whole
/// area, or, where its root value is a row or column, the area of the part
/// at `part`, a path in the view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Area {
    pub view: ViewId,
    part: Rc<[Step]>,
}

impl Area {
    /// `view`'s whole area.
    pub fn whole(view: ViewId) -> Self {
        Self {
            view,
            part: Rc::from([]),
        }
    }

    /// The area of `view`'s part at `path`.
    pub fn part(view: ViewId, path: Path) -> Self {
        Self {
            view,
            part: Rc::from(path),
        }
    }

    /// Where in its view the area's part is: no steps for the whole view.
    pub fn path(&self) -> &[Step] {
        &self.part
    }
}

/// A scroll viewport, tagged as the window's `area`. The settled region is
/// frame output, not retained widget state; input and keyboard reveal use it
/// to update the same caller-owned view.
pub fn scrolled_at<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    offset: Vec2,
    area: Area,
    scale: f64,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<HoverPass<C, H>> {
    let extent = child.extent;
    let scrolled = container::scrolled(child, offset, scale, on_scroll);
    super::before_place(scrolled, move |placement, output| {
        output.view_region(ViewRegion {
            area,
            rect: placement.rect,
            maximum: super::scroll::maximum(extent, placement.rect.size(), scale),
            content: Some(Vec2::new(extent.width, extent.height()) / scale),
        });
    })
}

/// An assigned-size area, with no content scrolling or padding.
pub fn viewport<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    area: Area,
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
                area,
                rect: placement.rect,
                maximum: Vec2::ZERO,
                content: None,
            });
        });
    })
}

/// Associate every navigation occurrence produced by `child` with one area
/// of the window. This happens after projection and placement, so the
/// projection language remains unaware of areas.
pub fn in_view<C: 'static, H: 'static>(
    child: Measured<HoverPass<C, H>>,
    area: Area,
) -> Measured<HoverPass<C, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.in_view(area, |pass| inner.place_at_into(placement, pass));
    })
}
