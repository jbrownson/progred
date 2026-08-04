//! Progred's boxes with baselines: the TeX/pict model. A box is (width, ascent,
//! descent) plus a way to place itself; rows compose on baselines,
//! columns stack with a chosen child's baseline.
//!
//! Invariants the future pretty-printing layer relies on: extents are
//! known at construction (before placement), construction has no side
//! effects so alternative layouts can be built and discarded, and
//! placement is the single traversal that touches the context `P`.

use puri::draw::Canvas;
use puri::edit::{EditCtx, LineEditDescription};
use puri::geometry::Placement;
use puri::handler::{Handler, HasHandler, capture};
use puri::text::{TextCtx, TextMetrics, TextStyle};
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};
use vello::kurbo::{Affine, Insets, Point, Rect, Size, Vec2};

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

impl From<TextMetrics> for Extent {
    fn from(metrics: TextMetrics) -> Self {
        Self {
            width: metrics.width,
            ascent: metrics.ascent,
            descent: metrics.descent,
        }
    }
}

pub struct Node<P> {
    pub extent: Extent,
    kind: Kind<P>,
}

pub struct PlaceInner<P> {
    child: Node<P>,
    placement: Placement,
}

fn child_placement(parent: Placement, rect: Rect) -> Placement {
    Placement::new(rect, parent.clip_rect)
}

fn clipped_placement(placement: Placement, bounds: Rect) -> Placement {
    Placement::new(placement.rect, placement.clip_rect.intersect(bounds))
}

impl<P> PlaceInner<P> {
    pub fn place(self, ctx: &mut P) {
        place(self.child, ctx, self.placement);
    }
}

type PlaceAround<P> = Box<dyn FnOnce(&mut P, Placement, PlaceInner<P>)>;
type PlaceLeaf<P> = Box<dyn FnOnce(&mut P, Placement)>;

enum Kind<P> {
    Leaf(PlaceLeaf<P>),
    Row {
        children: Vec<Node<P>>,
        gap: f64,
    },
    Col {
        children: Vec<Node<P>>,
        gap: f64,
    },
    Pad {
        child: Box<Node<P>>,
        insets: Insets,
    },
    Around {
        child: Box<Node<P>>,
        place: PlaceAround<P>,
    },
}

pub fn leaf<P>(extent: Extent, place: impl FnOnce(&mut P, Placement) + 'static) -> Node<P> {
    Node {
        extent,
        kind: Kind::Leaf(Box::new(place)),
    }
}

/// Children on one baseline: ascent and descent are the maxima.
pub fn row<P>(gap: f64, children: Vec<Node<P>>) -> Node<P> {
    let width = children.iter().map(|c| c.extent.width).sum::<f64>()
        + gap * children.len().saturating_sub(1) as f64;
    let ascent = children
        .iter()
        .map(|c| c.extent.ascent)
        .fold(0.0_f64, f64::max);
    let descent = children
        .iter()
        .map(|c| c.extent.descent)
        .fold(0.0_f64, f64::max);
    Node {
        extent: Extent {
            width,
            ascent,
            descent,
        },
        kind: Kind::Row { children, gap },
    }
}

/// Children stacked; the column's baseline is child `baseline`'s.
pub fn col<P>(baseline: usize, gap: f64, children: Vec<Node<P>>) -> Node<P> {
    let extent = if children.is_empty() {
        Extent::default()
    } else {
        assert!(baseline < children.len());
        let width = children
            .iter()
            .map(|c| c.extent.width)
            .fold(0.0_f64, f64::max);
        let total = children.iter().map(|c| c.extent.height()).sum::<f64>()
            + gap * (children.len() - 1) as f64;
        let ascent = children[..baseline]
            .iter()
            .map(|c| c.extent.height())
            .sum::<f64>()
            + gap * baseline as f64
            + children[baseline].extent.ascent;
        Extent {
            width,
            ascent,
            descent: total - ascent,
        }
    };
    Node {
        extent,
        kind: Kind::Col { children, gap },
    }
}

pub fn pad<P>(insets: Insets, child: Node<P>) -> Node<P> {
    let e = child.extent;
    Node {
        extent: Extent {
            width: e.width + insets.x0 + insets.x1,
            ascent: e.ascent + insets.y0,
            descent: e.descent + insets.y1,
        },
        kind: Kind::Pad {
            child: Box::new(child),
            insets,
        },
    }
}

/// Holds `child` to at least `min` wide by padding on the right: a
/// frame's minimum, not the child's — the child keeps its own extent
/// and placement.
pub fn min_width<P>(min: f64, child: Node<P>) -> Node<P> {
    let deficit = (min - child.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, deficit, 0.0), child)
}

/// Transparently wraps this node's placement. `place_inner` places
/// its content and descendants exactly once when invoked.
pub fn around<P>(
    child: Node<P>,
    place: impl FnOnce(&mut P, Placement, PlaceInner<P>) + 'static,
) -> Node<P> {
    Node {
        extent: child.extent,
        kind: Kind::Around {
            child: Box::new(child),
            place: Box::new(place),
        },
    }
}

/// Run `place_before` while entering this node, before its content
/// and descendants.
pub fn before<P>(
    child: Node<P>,
    place_before: impl FnOnce(&mut P, Placement) + 'static,
) -> Node<P> {
    around(child, move |ctx, placement, place_inner| {
        place_before(ctx, placement);
        place_inner.place(ctx);
    })
}

/// The historical leading decoration operation, retained as the
/// rectangle-only spelling of [`before`].
pub fn decorate<P>(
    child: Node<P>,
    draw: impl FnOnce(&mut P, Rect) + 'static,
) -> Node<P> {
    before(child, move |ctx, placement| draw(ctx, placement.rect))
}

pub fn place<P>(node: Node<P>, ctx: &mut P, placement: Placement) {
    let extent = node.extent;
    let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
    match node.kind {
        Kind::Leaf(f) => f(ctx, placement),
        Kind::Row { children, gap } => {
            let mut x = at.x;
            for child in children {
                let advance = child.extent.width + gap;
                let rect = Rect::new(
                    x,
                    at.y - child.extent.ascent,
                    x + child.extent.width,
                    at.y + child.extent.descent,
                );
                place(child, ctx, child_placement(placement, rect));
                x += advance;
            }
        }
        Kind::Col { children, gap } => {
            let mut y = at.y - extent.ascent;
            for child in children {
                let advance = child.extent.height() + gap;
                let child_baseline = y + child.extent.ascent;
                let rect = Rect::new(
                    at.x,
                    child_baseline - child.extent.ascent,
                    at.x + child.extent.width,
                    child_baseline + child.extent.descent,
                );
                place(child, ctx, child_placement(placement, rect));
                y += advance;
            }
        }
        Kind::Pad { child, insets } => {
            let child_at = Point::new(at.x + insets.x0, at.y);
            let rect = Rect::new(
                child_at.x,
                child_at.y - child.extent.ascent,
                child_at.x + child.extent.width,
                child_at.y + child.extent.descent,
            );
            place(*child, ctx, child_placement(placement, rect));
        }
        Kind::Around { child, place: wrap } => {
            wrap(
                ctx,
                placement,
                PlaceInner {
                    child: *child,
                    placement,
                },
            );
        }
    }
}

/// `at` is the top-left corner of the node.
#[cfg(test)]
pub fn place_top_left<P>(node: Node<P>, ctx: &mut P, at: Point) {
    let placement = Placement::root(node.extent.rect_at(at));
    place(node, ctx, placement);
}

pub fn text<P: Canvas>(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Node<P> {
    let text = puri::text::text(ctx, s, style);
    leaf(
        text.metrics().into(),
        move |canvas, placement| text.place(canvas, placement),
    )
}

pub fn text_edit<C: 'static, P: Canvas + HasHandler<C>>(
    description: LineEditDescription<'_>,
    tcx: &mut TextCtx,
    with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Node<P> {
    let edit = puri::edit::text_edit(description, tcx);
    leaf(
        edit.metrics().into(),
        move |p, placement| edit.place(p, placement, with),
    )
}

pub fn on_primary_pointer_down<C: 'static, P: HasHandler<C>>(
    node: Node<P>,
    accepts: impl Fn(&PointerButtonEvent) -> bool + 'static,
    action: impl Fn(&mut C, &PointerButtonEvent) -> bool + 'static,
) -> Node<P> {
    before(node, move |p, placement| {
        puri::interact::on_primary_pointer_down(p, placement, accepts, action);
    })
}

/// Place `child` shifted up-left by `offset` inside a clipped
/// viewport. The caller owns and clamps the offset.
pub fn place_scrolled<C: 'static, P: Canvas + HasHandler<C>>(
    child: Node<P>,
    ctx: &mut P,
    placement: Placement,
    offset: Vec2,
    on_scroll: impl Fn(&mut C, &PointerScrollEvent) -> bool + 'static,
) {
    let rect = placement.rect;
    if !placement.clipped_out() {
        ctx.handler().on_scroll(move |state, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && on_scroll(state, event)
        });
    }
    let child_rect = child
        .extent
        .rect_at(Point::new(rect.x0 - offset.x, rect.y0 - offset.y));
    let child_placement = child_placement(clipped_placement(placement, rect), child_rect);
    let child_handler = capture(ctx, |ctx| {
        ctx.clip(rect, Affine::IDENTITY, |ctx| {
            place(child, ctx, child_placement);
        });
    });
    install_child(ctx.handler(), child_handler, placement);
}

fn install_child<C: 'static>(outer: &mut Handler<C>, child: Handler<C>, placement: Placement) {
    let Handler {
        pointer_down,
        pointer_move,
        pointer_up,
        scroll,
        key,
        ime,
    } = child;
    if !placement.clipped_out() {
        outer.on_pointer_down(move |ctx, event: &PointerButtonEvent| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && pointer_down(ctx, event)
        });
        outer.on_scroll(move |ctx, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && scroll(ctx, event)
        });
    }
    outer.on_pointer_move(pointer_move);
    outer.on_pointer_up(pointer_up);
    outer.on_key(key);
    outer.on_ime(ime);
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
    use ui_events::ScrollDelta;
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
        PointerUpdate,
    };
    use vello::kurbo::Stroke;
    use vello::peniko::Brush;

    struct Frame<C> {
        list: DrawList,
        handler: Handler<C>,
    }

    impl<C> Canvas for Frame<C> {
        fn fill(
            &mut self,
            shape: impl Into<Shape>,
            brush: impl Into<Brush>,
            transform: Affine,
        ) {
            self.list.fill(shape, brush, transform);
        }

        fn stroke(
            &mut self,
            shape: impl Into<Shape>,
            style: Stroke,
            brush: impl Into<Brush>,
            transform: Affine,
        ) {
            self.list.stroke(shape, style, brush, transform);
        }

        fn glyph_run(&mut self, run: GlyphRun) {
            self.list.glyph_run(run);
        }

        fn clip(
            &mut self,
            shape: impl Into<Shape>,
            transform: Affine,
            content: impl FnOnce(&mut Self),
        ) {
            let shape = shape.into();
            let mut child = Frame {
                list: DrawList::new(),
                handler: std::mem::take(&mut self.handler),
            };
            content(&mut child);
            self.handler = child.handler;
            self.list.0.push(DrawCmd::Clip {
                shape,
                transform,
                children: child.list.0,
            });
        }
    }

    impl<C> HasHandler<C> for Frame<C> {
        fn handler(&mut self) -> &mut Handler<C> {
            &mut self.handler
        }
    }

    fn pointer() -> PointerInfo {
        PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn state_at(x: f64, y: f64) -> PointerState {
        let mut state = PointerState::default();
        state.position.x = x;
        state.position.y = y;
        state
    }

    fn down_at(x: f64, y: f64) -> PointerButtonEvent {
        PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: pointer(),
            state: state_at(x, y),
        }
    }

    fn move_at(x: f64, y: f64) -> PointerUpdate {
        PointerUpdate {
            pointer: pointer(),
            current: state_at(x, y),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }
    }

    fn scroll_at(x: f64, y: f64) -> PointerScrollEvent {
        PointerScrollEvent {
            pointer: pointer(),
            delta: ScrollDelta::LineDelta(0.0, 1.0),
            state: state_at(x, y),
        }
    }

    fn probe(extent: Extent) -> Node<Vec<Placement>> {
        leaf(extent, move |placed: &mut Vec<Placement>, placement| {
            placed.push(placement)
        })
    }

    fn ext(width: f64, ascent: f64, descent: f64) -> Extent {
        Extent {
            width,
            ascent,
            descent,
        }
    }

    #[test]
    fn children_inherit_the_enclosing_clip_until_a_container_narrows_it() {
        let root = Placement::new(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(40.0, 20.0, 120.0, 80.0),
        );
        let child = child_placement(root, Rect::new(20.0, 50.0, 60.0, 90.0));
        assert_eq!(child.clip_rect, root.clip_rect);
        assert_eq!(child.visible_rect(), Rect::new(40.0, 50.0, 60.0, 80.0));
        let clipped = clipped_placement(child, Rect::new(50.0, 0.0, 80.0, 100.0));
        assert_eq!(clipped.clip_rect, Rect::new(50.0, 20.0, 80.0, 80.0));
    }

    #[test]
    fn row_places_children_on_one_baseline() {
        let r = row(4.0, vec![probe(ext(10.0, 8.0, 2.0)), probe(ext(20.0, 12.0, 4.0))]);
        assert_eq!(r.extent, ext(34.0, 12.0, 4.0));

        let mut placed = Vec::new();
        place(
            r,
            &mut placed,
            Placement::root(Rect::new(0.0, 88.0, 34.0, 104.0)),
        );
        assert_eq!(
            placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
            vec![
                Rect::new(0.0, 92.0, 10.0, 102.0),
                Rect::new(14.0, 88.0, 34.0, 104.0),
            ]
        );
    }

    #[test]
    fn col_takes_the_chosen_childs_baseline() {
        let c = col(
            1,
            2.0,
            vec![
                probe(ext(10.0, 8.0, 2.0)),
                probe(ext(4.0, 4.0, 0.0)),
                probe(ext(10.0, 8.0, 2.0)),
            ],
        );
        assert_eq!(c.extent, ext(10.0, 16.0, 12.0));

        let mut placed = Vec::new();
        place(
            c,
            &mut placed,
            Placement::root(Rect::new(0.0, 84.0, 10.0, 112.0)),
        );
        assert_eq!(
            placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
            vec![
                Rect::new(0.0, 84.0, 10.0, 94.0),
                Rect::new(0.0, 96.0, 4.0, 100.0),
                Rect::new(0.0, 102.0, 10.0, 112.0),
            ]
        );
    }

    #[test]
    fn pad_grows_extent_and_offsets_the_child() {
        let p = pad(Insets::new(3.0, 5.0, 7.0, 1.0), probe(ext(10.0, 8.0, 2.0)));
        assert_eq!(p.extent, ext(20.0, 13.0, 3.0));

        let mut placed = Vec::new();
        place(
            p,
            &mut placed,
            Placement::root(Rect::new(0.0, 87.0, 20.0, 103.0)),
        );
        assert_eq!(placed[0].rect, Rect::new(3.0, 92.0, 13.0, 102.0));
    }

    #[test]
    fn min_width_pads_narrow_children_and_leaves_wide_ones() {
        let narrow = min_width(25.0, probe(ext(10.0, 8.0, 2.0)));
        assert_eq!(narrow.extent, ext(25.0, 8.0, 2.0));
        let mut placed = Vec::new();
        place_top_left(narrow, &mut placed, Point::ZERO);
        assert_eq!(placed[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));

        let wide = min_width(25.0, probe(ext(30.0, 8.0, 2.0)));
        assert_eq!(wide.extent, ext(30.0, 8.0, 2.0));
    }

    #[test]
    fn decorate_receives_the_subtree_rect() {
        struct Ctx {
            rects: Vec<Rect>,
            placed: Vec<Placement>,
        }
        let child = leaf(ext(10.0, 8.0, 2.0), |ctx: &mut Ctx, placement| {
            ctx.placed.push(placement)
        });
        let d = decorate(child, |ctx: &mut Ctx, rect| ctx.rects.push(rect));

        let mut ctx = Ctx {
            rects: Vec::new(),
            placed: Vec::new(),
        };
        place(
            d,
            &mut ctx,
            Placement::root(Rect::new(5.0, 92.0, 15.0, 102.0)),
        );
        assert_eq!(ctx.rects, vec![Rect::new(5.0, 92.0, 15.0, 102.0)]);
        assert_eq!(ctx.placed[0].rect, Rect::new(5.0, 92.0, 15.0, 102.0));
    }

    #[test]
    fn around_controls_the_inner_placement_and_sees_the_clip_rect() {
        struct Ctx {
            events: Vec<&'static str>,
        }
        let child = leaf(ext(20.0, 10.0, 10.0), |ctx: &mut Ctx, _| {
            ctx.events.push("inner");
        });
        let wrapped = around(child, |ctx: &mut Ctx, placement, place_inner| {
            assert_eq!(placement.rect, Rect::new(0.0, 0.0, 20.0, 20.0));
            assert_eq!(placement.clip_rect, Rect::new(5.0, -5.0, 25.0, 15.0));
            ctx.events.push("before");
            place_inner.place(ctx);
            ctx.events.push("after");
        });
        let mut ctx = Ctx {
            events: Vec::new(),
        };
        place(
            wrapped,
            &mut ctx,
            Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(5.0, -5.0, 25.0, 15.0),
            ),
        );
        assert_eq!(ctx.events, ["before", "inner", "after"]);
    }

    #[test]
    fn around_may_discard_the_inner_placement() {
        struct Ctx {
            placed: bool,
        }
        let child = leaf(ext(10.0, 5.0, 5.0), |ctx: &mut Ctx, _| ctx.placed = true);
        let wrapped = around(child, |_: &mut Ctx, _, _| {});
        let mut ctx = Ctx {
            placed: false,
        };
        place_top_left(wrapped, &mut ctx, Point::ZERO);
        assert!(!ctx.placed);
    }

    #[test]
    fn scrolled_content_shifts_inside_the_viewport_clip() {
        let probe = leaf(
            Extent {
                width: 100.0,
                ascent: 0.0,
                descent: 300.0,
            },
            |frame: &mut Frame<()>, placement| {
                assert_eq!(placement.clip_rect, Rect::new(10.0, 20.0, 90.0, 70.0));
                frame.fill(
                    Rect::new(
                        placement.rect.x0,
                        placement.rect.y0,
                        placement.rect.x0 + 1.0,
                        placement.rect.y0 + 1.0,
                    ),
                    vello::peniko::Color::WHITE,
                    Affine::IDENTITY,
                );
            },
        );
        let mut frame: Frame<()> = Frame {
            list: DrawList::new(),
            handler: Handler::new(),
        };
        place_scrolled(
            probe,
            &mut frame,
            Placement::new(
                Rect::new(10.0, 20.0, 90.0, 70.0),
                Rect::new(0.0, 0.0, 100.0, 100.0),
            ),
            Vec2::new(5.0, 40.0),
            |_, _| false,
        );
        let [DrawCmd::Clip {
            shape: Shape::Rect(clip),
            children,
            ..
        }] = &frame.list.0[..]
        else {
            panic!("expected one clip");
        };
        assert_eq!(*clip, Rect::new(10.0, 20.0, 90.0, 70.0));
        let [DrawCmd::Fill {
            shape: Shape::Rect(dot),
            ..
        }] = &children[..]
        else {
            panic!("expected the probe inside the clip");
        };
        assert_eq!((dot.x0, dot.y0), (5.0, -20.0));
    }

    #[test]
    fn scroll_viewport_bounds_starts_and_not_active_motion_or_release() {
        let child = leaf(
            Extent {
                width: 30.0,
                ascent: 0.0,
                descent: 30.0,
            },
            |frame: &mut Frame<Vec<&'static str>>, _| {
                frame.handler.on_pointer_down(|log, _| {
                    log.push("down");
                    true
                });
                frame.handler.on_pointer_move(|log, _| {
                    log.push("move");
                    true
                });
                frame.handler.on_pointer_up(|log, _| {
                    log.push("up");
                    true
                });
            },
        );
        let mut frame = Frame {
            list: DrawList::new(),
            handler: Handler::new(),
        };
        place_scrolled(
            child,
            &mut frame,
            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
            Vec2::ZERO,
            |log, _| {
                log.push("scroll");
                true
            },
        );
        let mut log = Vec::new();
        assert!(!frame.handler.dispatch_pointer_down(&mut log, &down_at(20.0, 5.0)));
        assert!(!frame.handler.dispatch_scroll(&mut log, &scroll_at(20.0, 5.0)));
        assert!(frame.handler.dispatch_pointer_down(&mut log, &down_at(5.0, 5.0)));
        assert!(frame.handler.dispatch_scroll(&mut log, &scroll_at(5.0, 5.0)));
        assert!(frame.handler.dispatch_pointer_move(&mut log, &move_at(20.0, 5.0)));
        assert!(frame.handler.dispatch_pointer_up(&mut log, &down_at(20.0, 5.0)));
        assert_eq!(log, ["down", "scroll", "move", "up"]);
    }

}
