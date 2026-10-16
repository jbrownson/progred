//! Measured boxes with baselines: the TeX/pict model. A [`Measured`]
//! box is (width, ascent, descent) plus a way to place itself; rows
//! compose on baselines, columns stack with a chosen child's baseline.
//!
//! Invariants the pretty-printing layer relies on: extents are known
//! at construction (before placement), construction has no side
//! effects so alternative layouts can be built and discarded, and
//! placement is one pure traversal from settled geometry to the
//! caller's [`Output`].
//!
//! The engine is generic in what placement produces. Leaves either
//! yield an `Out` from their settled [`Placement`] or contribute
//! directly to the frame's accumulator. Containers visit children in
//! placement order, so the later-placed contribution is the one on top
//! — painted last, asked first.

use kurbo::{Insets, Point, Rect, Size};
use uig::Placement;

pub mod choices;

#[cfg(test)]
mod structure_tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowAlignment {
    Baseline,
    /// Align top edges, retaining the chosen child's baseline, as columns do.
    Top {
        baseline: usize,
    },
    Center,
}

/// The composition being placed, after choosing layout alternatives. Gaps and
/// geometry are intentionally absent: this describes structure, not distances.
#[derive(Clone, Copy)]
pub enum Composition {
    Row(RowAlignment),
    Column { baseline: usize },
    Overlay,
}

/// Observe the structure that actually places, without recording another tree.
///
/// Each callback encloses exactly its composition/child's placement. An omitted
/// subtree emits nothing inside its child boundary; a deferred subtree emits
/// when it is eventually placed. Empty children still have boundaries, so a
/// column's designated baseline retains its meaning.
/// Consumers uninterested in structure use the default, transparent methods.
/// Implementations must run the supplied continuation once: these hooks observe
/// composition, rather than deciding whether its contents should place.
pub trait ObserveLayout: Sized {
    fn layout(&mut self, _composition: Composition, children: impl FnOnce(&mut Self)) {
        children(self);
    }

    fn layout_child(&mut self, child: impl FnOnce(&mut Self)) {
        child(self);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extent {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
}

impl Extent {
    pub fn height(&self) -> f64 {
        self.ascent + self.descent
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height())
    }

    pub fn rect_at(&self, at: Point) -> Rect {
        Rect::from_origin_size(at, self.size())
    }
}

/// What a placement pass accumulates: a monoid whose combine names its
/// asymmetry. `base.over(above)` stacks `above` on top of `base`.
pub trait Output {
    fn empty() -> Self;
    fn over(self, above: Self) -> Self;
}

pub struct Measured<Out> {
    pub extent: Extent,
    place: Box<dyn FnOnce(Placement, &mut Out)>,
}

/// A child held back for its wrapper: place it, move it, or drop it.
pub struct PlaceInner<Out> {
    child: Measured<Out>,
    placement: Placement,
}

pub fn child_placement(parent: Placement, rect: Rect) -> Placement {
    Placement::new(rect, parent.clip_rect)
}

pub fn clipped_placement(placement: Placement, bounds: Rect) -> Placement {
    Placement {
        clip_rect: placement.clip_rect.intersect(bounds),
        ..placement
    }
}

impl<Out: Output> PlaceInner<Out> {
    pub fn place(self) -> Out {
        place(self.child, self.placement)
    }

    pub fn place_at(self, placement: Placement) -> Out {
        place(self.child, placement)
    }
}

impl<Out> PlaceInner<Out> {
    pub fn place_into(self, out: &mut Out) {
        place_into(self.child, self.placement, out)
    }

    pub fn place_at_into(self, placement: Placement, out: &mut Out) {
        place_into(self.child, placement, out)
    }

    pub fn extent(&self) -> Extent {
        self.child.extent
    }
}

pub fn leaf<Out: Output>(
    extent: Extent,
    place: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    leaf_into(extent, move |placement, out| {
        contribute(out, place(placement))
    })
}

pub fn leaf_into<Out>(
    extent: Extent,
    place: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    Measured {
        extent,
        place: Box::new(place),
    }
}

pub fn row<Out: ObserveLayout + 'static>(gap: f64, children: Vec<Measured<Out>>) -> Measured<Out> {
    row_aligned(gap, children, RowAlignment::Baseline)
}

pub fn centered_row<Out: ObserveLayout + 'static>(
    gap: f64,
    children: Vec<Measured<Out>>,
) -> Measured<Out> {
    row_aligned(gap, children, RowAlignment::Center)
}

fn row_aligned<Out: ObserveLayout + 'static>(
    gap: f64,
    children: Vec<Measured<Out>>,
    alignment: RowAlignment,
) -> Measured<Out> {
    let extent = row_extent(gap, alignment, children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out: &mut Out| {
        out.layout(Composition::Row(alignment), |out| {
            place_row(
                extent,
                placement,
                gap,
                alignment,
                children,
                |child| child.extent,
                |child, placement| out.layout_child(|out| place_into(child, placement, out)),
            );
        });
    })
}

pub fn col<Out: ObserveLayout + 'static>(
    baseline: usize,
    gap: f64,
    children: Vec<Measured<Out>>,
) -> Measured<Out> {
    let extent = col_extent(baseline, gap, children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out: &mut Out| {
        out.layout(Composition::Column { baseline }, |out| {
            place_col(
                placement,
                gap,
                children,
                |child| child.extent,
                |child, placement| out.layout_child(|out| place_into(child, placement, out)),
            );
        });
    })
}

pub fn layers<Out: ObserveLayout + 'static>(children: Vec<Measured<Out>>) -> Measured<Out> {
    let extent = overlay_extent(children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out: &mut Out| {
        out.layout(Composition::Overlay, |out| {
            place_layers(
                extent,
                placement,
                children,
                |child| child.extent,
                |child, placement| out.layout_child(|out| place_into(child, placement, out)),
            );
        });
    })
}

pub fn pad<Out: 'static>(insets: Insets, child: Measured<Out>) -> Measured<Out> {
    let extent = padded_extent(insets, child.extent);
    leaf_into(extent, move |placement, out| {
        let placement = padded_placement(placement, insets, child.extent);
        place_into(child, placement, out)
    })
}

pub fn min_width<Out: 'static>(min: f64, child: Measured<Out>) -> Measured<Out> {
    let deficit = (min - child.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, deficit, 0.0), child)
}

pub fn fill_height<Out: 'static>(child: Measured<Out>) -> Measured<Out> {
    around_into(child, |placement, inner, out| {
        inner.place_at_into(placement.fill_height(), out)
    })
}

pub fn fill_width<Out: 'static>(child: Measured<Out>) -> Measured<Out> {
    around_into(child, |placement, inner, out| {
        inner.place_at_into(placement.fill_width(), out)
    })
}

/// A wrapper controls when and where its child continuation runs.
pub fn around<Out: Output + 'static>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>) -> Out + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        contribute(out, place(placement, inner))
    })
}

pub fn around_into<Out: 'static>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>, &mut Out) + 'static,
) -> Measured<Out> {
    leaf_into(child.extent, move |placement, out| {
        place(placement, PlaceInner { child, placement }, out)
    })
}

pub fn overlay_into<Out: 'static>(
    base: Measured<Out>,
    layer: Measured<Out>,
    position: impl FnOnce(Placement, Extent) -> Option<Placement> + 'static,
) -> Measured<Out> {
    let extent = layer.extent;
    around_into(base, move |placement, base, output| {
        base.place_into(output);
        if let Some(placement) = position(placement, extent) {
            place_into(layer, placement, output);
        }
    })
}

pub fn overlay<Out: Output + 'static>(
    base: Measured<Out>,
    layer: Measured<Out>,
    position: impl FnOnce(Placement, Extent, &Out) -> Option<Placement> + 'static,
) -> Measured<Out> {
    let extent = layer.extent;
    around(base, move |placement, inner| {
        let out = inner.place();
        match position(placement, extent, &out) {
            Some(at) => out.over(place(layer, at)),
            None => out,
        }
    })
}

pub fn before<Out: Output + 'static>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        place_before(placement).over(inner.place())
    })
}

pub fn after<Out: Output + 'static>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        inner.place().over(place_after(placement))
    })
}

pub fn before_into<Out: 'static>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        place_before(placement, out);
        inner.place_into(out);
    })
}

pub fn after_into<Out: 'static>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        inner.place_into(out);
        place_after(placement, out);
    })
}

pub fn decorate<Out: Output + 'static>(
    child: Measured<Out>,
    draw: impl FnOnce(Rect) -> Out + 'static,
) -> Measured<Out> {
    before(child, move |placement| draw(placement.rect))
}

pub fn place<Out: Output>(layout: Measured<Out>, placement: Placement) -> Out {
    let mut out = Out::empty();
    place_into(layout, placement, &mut out);
    out
}

fn contribute<Out: Output>(out: &mut Out, above: Out) {
    let base = std::mem::replace(out, Out::empty());
    *out = base.over(above);
}

pub fn place_into<Out>(layout: Measured<Out>, placement: Placement, out: &mut Out) {
    (layout.place)(placement, out)
}

pub fn place_top_left<Out: Output>(layout: Measured<Out>, at: Point) -> Out {
    let placement = Placement::root(layout.extent.rect_at(at));
    place(layout, placement)
}

pub(crate) fn row_extent(
    gap: f64,
    alignment: RowAlignment,
    children: impl Iterator<Item = Extent>,
) -> Extent {
    let (mut extent, mut count) = (Extent::default(), 0usize);
    let mut height: f64 = 0.0;
    for child in children {
        extent.width += child.width;
        match alignment {
            RowAlignment::Baseline => {
                extent.ascent = extent.ascent.max(child.ascent);
                extent.descent = extent.descent.max(child.descent);
            }
            RowAlignment::Center => {
                if count == 0 || child.height() > extent.height() {
                    extent.ascent = child.ascent;
                    extent.descent = child.descent;
                }
            }
            RowAlignment::Top { baseline } => {
                height = height.max(child.height());
                if count == baseline {
                    extent.ascent = child.ascent;
                }
                extent.descent = height - extent.ascent;
            }
        }
        count += 1;
    }
    if let RowAlignment::Top { baseline } = alignment {
        assert!(count == 0 || baseline < count);
    }
    extent.width += gap * count.saturating_sub(1) as f64;
    extent
}

pub(crate) fn col_extent(
    baseline: usize,
    gap: f64,
    children: impl ExactSizeIterator<Item = Extent>,
) -> Extent {
    assert!(children.len() == 0 || baseline < children.len());
    let mut extent = Extent::default();
    let mut height = 0.0;
    for (index, child) in children.enumerate() {
        if index > 0 {
            height += gap;
        }
        extent.width = extent.width.max(child.width);
        if index == baseline {
            extent.ascent = height + child.ascent;
        }
        height += child.height();
    }
    extent.descent = height - extent.ascent;
    extent
}

pub(crate) fn overlay_extent(children: impl Iterator<Item = Extent>) -> Extent {
    children.fold(Extent::default(), |extent, child| Extent {
        width: extent.width.max(child.width),
        ascent: extent.ascent.max(child.ascent),
        descent: extent.descent.max(child.descent),
    })
}

pub(crate) fn padded_extent(insets: Insets, child: Extent) -> Extent {
    Extent {
        width: child.width + insets.x0 + insets.x1,
        ascent: child.ascent + insets.y0,
        descent: child.descent + insets.y1,
    }
}

pub(crate) fn place_row<T>(
    extent: Extent,
    placement: Placement,
    gap: f64,
    alignment: RowAlignment,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    let mut x = placement.rect.x0;
    for child in children {
        let size = extent_of(&child);
        let y = placement.rect.y0
            + match alignment {
                RowAlignment::Baseline => extent.ascent - size.ascent,
                RowAlignment::Top { .. } => 0.0,
                RowAlignment::Center => (extent.height() - size.height()) / 2.0,
            };
        let rect = size.rect_at(Point::new(x, y));
        place(
            child,
            child_placement(placement, rect).with_available_rect(Rect::new(
                rect.x0,
                placement.rect.y0,
                rect.x1,
                placement.rect.y1,
            )),
        );
        x += size.width + gap;
    }
}

pub(crate) fn place_col<T>(
    placement: Placement,
    gap: f64,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    let mut y = placement.rect.y0;
    for child in children {
        let size = extent_of(&child);
        let rect = size.rect_at(Point::new(placement.rect.x0, y));
        place(
            child,
            child_placement(placement, rect).with_available_rect(Rect::new(
                placement.rect.x0,
                rect.y0,
                placement.rect.x1,
                rect.y1,
            )),
        );
        y += size.height() + gap;
    }
}

pub(crate) fn place_layers<T>(
    extent: Extent,
    placement: Placement,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    for child in children {
        let size = extent_of(&child);
        let rect = size.rect_at(Point::new(
            placement.rect.x0,
            placement.rect.y0 + extent.ascent - size.ascent,
        ));
        place(
            child,
            child_placement(placement, rect).with_available_rect(placement.rect),
        );
    }
}

pub(crate) fn padded_placement(placement: Placement, insets: Insets, child: Extent) -> Placement {
    let rect = child.rect_at(Point::new(
        placement.rect.x0 + insets.x0,
        placement.rect.y0 + insets.y0,
    ));
    child_placement(placement, rect).with_available_rect(placement.available_rect.inset(-insets))
}

#[cfg(test)]
mod tests;
