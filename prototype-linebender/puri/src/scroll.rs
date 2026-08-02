//! A scroll viewport: the child, clipped to a viewport and shifted by
//! an offset. Pure in Puri's sense — the offsets are the CALLER's
//! state (like an editor's [`crate::edit::LineEditState`], custody
//! never lives here), extents are known before placement, and the
//! caller derives the clamp from the child's extent it already holds.
//! It remains a placement entry because the viewport's extent and
//! origin are caller chrome; nested entries derive their effective
//! clips from explicit parent placements. Scroll bars remain a later,
//! separate widget.

use crate::draw::Canvas;
use crate::handler::{Handler, HasHandler, capture};
use crate::layout::{Extent, Node, Placement, place};
use kurbo::{Affine, Point, Size, Vec2};
use ui_events::pointer::{PointerButtonEvent, PointerScrollEvent};

/// How far `content` can scroll within `viewport`, per axis.
pub fn max_offset(content: Extent, viewport: Size) -> Vec2 {
    Vec2::new(
        (content.width - viewport.width).max(0.0),
        (content.height() - viewport.height).max(0.0),
    )
}

/// Places `child` shifted up-left by `offset` inside the viewport
/// `placement`, clipped to its rect. The
/// caller clamps the offset (against [`max_offset`]) before placing.
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
    let child_placement = placement.clipped_by(rect).child(child_rect);
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
    use crate::draw::{DrawCmd, DrawList, GlyphRun, Shape};
    use crate::handler::Handler;
    use crate::layout::leaf;
    use kurbo::{Rect, Stroke};
    use peniko::Brush;
    use ui_events::ScrollDelta;
    use ui_events::pointer::{
        PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
        PointerUpdate,
    };

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

    #[test]
    fn content_shifts_by_the_offset_inside_a_clip() {
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
                    peniko::Color::WHITE,
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
    fn max_offset_is_the_overflow_per_axis() {
        let content = Extent {
            width: 300.0,
            ascent: 100.0,
            descent: 150.0,
        };
        assert_eq!(
            max_offset(content, Size::new(200.0, 60.0)),
            Vec2::new(100.0, 190.0)
        );
        assert_eq!(max_offset(content, Size::new(400.0, 400.0)), Vec2::ZERO);
    }

    #[test]
    fn viewport_bounds_starts_while_moves_and_releases_remain_unbounded() {
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
