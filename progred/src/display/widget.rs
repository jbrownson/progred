//! Native widget continuations and their editor-facing inputs/outputs.
//! Layout measures and places these without interpreting a widget description.

use crate::display::Layout;
#[cfg(test)]
pub use frame::ResolvedHover;
#[cfg(test)]
pub use frame::place;
pub use frame::{HoverContext, HoverInput, HoverOutput, HoverPass, Probe};
use gid::Value;
pub use measured::Extent;
use measured::Measured;
use puri::Placement;
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextMetrics};
use std::rc::Rc;

pub mod completion;
pub mod container;
pub mod delimiter;
pub mod drawing;
pub mod frame;
pub mod gesture;
pub mod hover;
pub mod interaction;
pub mod line;
pub mod navigation;
pub mod offers;
pub mod popover;
pub mod project;
pub mod scroll;
pub mod source;
pub mod style;
pub mod view;

pub use navigation::{Direction, Select};
pub type Annotate<World> = Rc<dyn Fn(&mut World, Value) -> bool>;
pub type HoverCallback<World, Hover> =
    Box<dyn FnOnce(&mut HoverContext<'_, World, Hover>, Placement)>;
pub type Decoration<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(&mut Context<'a, 'fonts, World, Hover>) -> HoverCallback<World, Hover>,
>;

pub type Program<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(
        &mut Context<'a, 'fonts, World, Hover>,
        &mut measured::choices::ChoiceBuild<HoverPass<World, Hover>>,
    ) -> measured::choices::ChoiceLayout<HoverPass<World, Hover>>,
>;
pub struct Context<'a, 'fonts, World, Hover> {
    pub project: &'a dyn project::Project<World, Hover>,
    pub text: &'a mut TextCtx<'fonts>,
    pub inputs: &'a crate::projection::Cx<'a>,
    pub path: &'a [gid::Step],
    pub value: Option<&'a Value>,
}

pub type Widget<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(
        &mut Context<'a, 'fonts, World, Hover>,
    ) -> Measured<HoverPass<World, Hover>>,
>;

/// Contribute a layer beneath the child, without inspecting its widget type.
pub fn before<World: 'static, Hover: 'static>(
    child: Layout<World, Hover>,
    before: Decoration<World, Hover>,
) -> Layout<World, Hover> {
    Layout::new(move |builder| {
        let child = child.run(builder);
        builder.before(child, before.clone())
    })
}

pub fn after<World: 'static, Hover: 'static>(
    child: Layout<World, Hover>,
    after: Decoration<World, Hover>,
) -> Layout<World, Hover> {
    Layout::new(move |builder| {
        let child = child.run(builder);
        builder.after(child, after.clone())
    })
}

pub fn border<World: 'static, Hover: 'static>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    after(
        child,
        Rc::new(|context| {
            let stroke = puri::Stroke::new(context.inputs.styles.scale);
            let brush = context.inputs.styles.dim.brush.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, _| {
                        canvas.stroke_shape(
                            placement.rect.inset(-stroke.width / 2.0).into(),
                            stroke,
                            brush,
                            puri::Affine::IDENTITY,
                        );
                    });
                }
            })
        }),
    )
}

pub fn selectable(
    context: &Context<'_, '_, crate::Editor, crate::frame::Hovered>,
) -> impl FnOnce(
    Measured<HoverPass<crate::Editor, crate::frame::Hovered>>,
) -> Measured<HoverPass<crate::Editor, crate::frame::Hovered>>
+ 'static {
    let path: Rc<[gid::Step]> = Rc::from(context.path);
    let target = crate::frame::Hovered::Tree(crate::hover::Hover::Value(path.clone()));
    let select = crate::projection::select_handler(path, context.inputs);
    let value = context.value.cloned();
    move |child| {
        crate::display::widget::before_place(
            child,
            move |placement,
                  output: &mut HoverContext<'_, crate::Editor, crate::frame::Hovered>| {
                if !placement.clipped_out() {
                    output.claim(Probe::retaining(placement, target.clone()));
                    output
                        .handler()
                        .on_pointer_down_with(move |world, event, hovered| {
                            puri::interact::is_primary_contact(event)
                                && hovered.hovered().is_some_and(|hovered| hovered == &target)
                                && if crate::modifiers::pick(&event.state.modifiers) {
                                    value
                                        .as_ref()
                                        .is_some_and(|value| world.pick_identity(value.clone()))
                                } else {
                                    select(world)
                                }
                        });
                }
            },
        )
    }
}

pub fn selectable_widget(
    widget: Widget<crate::Editor, crate::frame::Hovered>,
) -> Widget<crate::Editor, crate::frame::Hovered> {
    Rc::new(move |context| {
        let decorate = selectable(context);
        decorate(widget(context))
    })
}

pub fn fill_height<World: 'static, Hover: 'static>(
    widget: Widget<World, Hover>,
) -> Widget<World, Hover> {
    Rc::new(move |context| measured::fill_height(widget(context)))
}

pub fn extent(metrics: TextMetrics) -> Extent {
    Extent {
        width: metrics.width,
        ascent: metrics.ascent,
        descent: metrics.descent,
    }
}

pub fn empty<World: 'static, Hover: 'static>(
    text: &mut TextCtx,
    styles: &style::Styles,
) -> Measured<HoverPass<World, Hover>> {
    let frame = puri_widgets::text_frame::empty(text, &styles.label, styles.dim.brush.clone());
    leaf(extent(frame.metrics()), move |output, placement| {
        output.render(move |canvas, _| frame.place(canvas, placement));
    })
}

pub fn leaf<World: 'static, Hover: 'static>(
    extent: Extent,
    hover: impl FnOnce(&mut HoverContext<'_, World, Hover>, Placement) + 'static,
) -> Measured<HoverPass<World, Hover>> {
    measured::leaf_into(
        extent,
        move |placement, pass: &mut HoverPass<World, Hover>| {
            pass.visit(move |output| {
                let start = output.output.after_hover.len();
                hover(output, placement);
                debug_geometry(placement, output);
                if placement.clipped_out() {
                    let after = output.output.after_hover.split_off(start);
                    output.after_hover(move |hover, effects| {
                        let start = effects.renders.len();
                        after.bind(hover, effects);
                        effects.renders.truncate(start);
                    });
                }
            });
        },
    )
}

/// A paint-only leaf; geometry is settled before drawing, and hover needs no ink.
pub fn paint<World: 'static, Hover: 'static>(
    extent: Extent,
    paint: impl FnOnce(&mut dyn puri::draw::CanvasSink, Placement) + 'static,
) -> Measured<HoverPass<World, Hover>> {
    leaf(extent, move |output, placement| {
        output.render(move |canvas, _| paint(canvas, placement));
    })
}

fn debug_geometry<W: 'static, H: 'static>(
    placement: Placement,
    output: &mut HoverContext<'_, W, H>,
) {
    if output.input.debug_geometry && !placement.clipped_out() {
        output.render(move |canvas, _| {
            canvas.stroke_shape(
                placement.rect.into(),
                puri::Stroke::new(0.75),
                puri::Color::new([0.0, 0.65, 1.0, 0.36]).into(),
                puri::Affine::IDENTITY,
            );
        });
    }
}

pub fn before_place<W: 'static, H: 'static>(
    child: Measured<HoverPass<W, H>>,
    before: impl FnOnce(Placement, &mut HoverContext<'_, W, H>) + 'static,
) -> Measured<HoverPass<W, H>> {
    measured::before_into(child, move |placement, pass| {
        pass.visit(move |output| before(placement, output));
    })
}

pub fn after_place<W: 'static, H: 'static>(
    child: Measured<HoverPass<W, H>>,
    after: impl FnOnce(Placement, &mut HoverContext<'_, W, H>) + 'static,
) -> Measured<HoverPass<W, H>> {
    measured::after_into(child, move |placement, pass| {
        pass.visit(move |output| after(placement, output));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recorded, record};
    use puri::draw::Shape;
    use puri::{Affine, Color, DrawCmd, DrawList, Point, Rect, Stroke};

    #[test]
    fn paint_only_leaves_defer_ink_and_skip_clipped_or_discarded_paint() {
        let extent = Extent {
            width: 20.0,
            ascent: 8.0,
            descent: 2.0,
        };
        let rect = Rect::new(30.0, 40.0, 50.0, 50.0);
        for (clip, render) in [(rect, false), (rect, true), (Rect::ZERO, true)] {
            let calls = Rc::new(std::cell::Cell::new(0));
            let drawing = calls.clone();
            let leaf = paint::<(), ()>(extent, move |canvas, placement| {
                drawing.set(drawing.get() + 1);
                canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
            });
            assert_eq!(leaf.extent, extent);
            let mut placement = Placement::root(rect);
            placement.clip_rect = clip;
            let mut fragment =
                crate::display::widget::frame::place(leaf, placement, &Default::default());
            assert_eq!(calls.get(), 0);
            assert!(fragment.claim.is_none());
            assert!(fragment.handler.is_none());
            let mut canvas = DrawList::new();
            if render {
                puri::frame::render(fragment.resolve(Default::default()), &mut canvas);
            }
            let painted = render && clip == rect;
            assert_eq!(calls.get(), usize::from(painted));
            assert_eq!(canvas.0.len(), usize::from(painted));
            if painted {
                assert!(
                    matches!(&canvas.0[0], DrawCmd::Fill { shape: Shape::Rect(drawn), .. } if *drawn == rect)
                );
            }
        }
    }

    #[test]
    fn placement_hover_render_and_dispatch_are_distinct_stages() {
        let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
        let during_hover = calls.clone();
        let measured = leaf(
            Extent {
                width: 20.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |output: &mut HoverContext<'_, Vec<&str>, usize>, placement| {
                during_hover.borrow_mut().push("hover");
                output.claim(Probe::retaining(placement, 7));
                let during_render = during_hover.clone();
                output.render(move |canvas, hover| {
                    during_render.borrow_mut().push("render");
                    assert_eq!(hover.hovered, Some(7));
                    canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
                });
                output.handler().on_key(|state, _| {
                    state.push("key");
                    true
                });
                output.on_arrival(Some(Rc::new(|state, _| {
                    state.push("select");
                    true
                })));
            },
        );
        assert!(calls.borrow().is_empty());
        let placement = Placement::root(Rect::new(10.0, 20.0, 30.0, 30.0));
        let mut output = place(
            measured,
            placement,
            &HoverInput {
                pointer: Some(placement.rect.center()),
                ..Default::default()
            },
        );
        assert_eq!(&*calls.borrow(), &["hover"]);
        assert_eq!(
            output.claim.clone().map(|(_, claim)| claim),
            Some(puri::hover::Claim::Direct(7))
        );
        let mut canvas = DrawList::new();
        puri::frame::render(
            output.resolve(ResolvedHover {
                hovered: Some(7),
                ..Default::default()
            }),
            &mut canvas,
        );
        assert_eq!(&*calls.borrow(), &["hover", "render"]);
        assert!(
            matches!(&canvas.0[..], [DrawCmd::Fill { shape: Shape::Rect(rect), .. }] if *rect == placement.rect)
        );
        let mut state = vec![];
        assert!(
            output
                .handler
                .unwrap()
                .dispatch_key(&mut state, &puri::handler::KeyboardEvent::default())
        );
        assert!(output.landmark_select.unwrap()(
            &mut state,
            Some(Direction::Left)
        ));
        assert_eq!(state, ["key", "select"]);
    }

    #[test]
    fn native_canvas_clip_streams_to_the_selected_interpreter() {
        let mut frame = HoverOutput::default();
        let mut output: HoverContext<'_, (), ()> =
            HoverContext::new(Default::default(), &mut frame);
        let clip = Rect::new(0.0, 0.0, 20.0, 10.0);
        output.render(move |canvas, _| {
            canvas.with_clip(
                clip.into(),
                Affine::IDENTITY,
                Box::new(move |canvas| {
                    canvas.fill_shape(clip.into(), Color::BLACK.into(), Affine::IDENTITY);
                    canvas.with_clip(
                        clip.into(),
                        Affine::translate((2.0, 0.0)),
                        Box::new(move |canvas| {
                            canvas.stroke_shape(
                                clip.into(),
                                Stroke::new(1.0),
                                Color::WHITE.into(),
                                Affine::IDENTITY,
                            );
                        }),
                    );
                }),
            )
        });
        let mut output = frame;
        assert_eq!(output.after_hover.len(), 1);
        let mut canvas = DrawList::new();
        puri::frame::render(output.resolve(Default::default()), &mut canvas);
        let [DrawCmd::Clip { children, .. }] = &canvas.0[..] else {
            panic!("outer clip");
        };
        let [DrawCmd::Fill { .. }, DrawCmd::Clip { children, .. }] = &children[..] else {
            panic!("nested clip");
        };
        assert!(matches!(&children[..], [DrawCmd::Stroke { .. }]));
    }

    #[test]
    fn render_is_one_continuation_regardless_of_drawing_count() {
        let calls = Rc::new(std::cell::Cell::new(0));
        let during_render = calls.clone();
        let mut frame = HoverOutput::default();
        let mut output =
            crate::display::widget::HoverContext::<(), ()>::new(Default::default(), &mut frame);
        output.render(move |canvas, _| {
            for _ in 0..100 {
                during_render.set(during_render.get() + 1);
                canvas.fill_shape(
                    Rect::new(0.0, 0.0, 1.0, 1.0).into(),
                    Color::BLACK.into(),
                    Affine::IDENTITY,
                );
            }
        });
        assert_eq!(calls.get(), 0);
        let mut output = frame;
        assert_eq!(output.after_hover.len(), 1);
        let mut canvas = DrawList::new();
        puri::frame::render(output.resolve(Default::default()), &mut canvas);
        assert_eq!(calls.get(), 100);
        assert_eq!(canvas.0.len(), 100);
    }

    #[test]
    fn inert_widgets_and_pointer_wrappers_do_not_request_site_state() {
        crate::display::test_support::with_context::<crate::Editor, crate::frame::Hovered, _>(
            &crate::display::test_support::NoProject,
            |context| {
                let measured = delimiter::side(
                    crate::display::Delim::Bracket,
                    crate::display::Side::Open,
                )(context);
                let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
                assert_eq!(
                    crate::display::widget::frame::place(measured, placement, &Default::default())
                        .after_hover
                        .len(),
                    1
                );
                for layout in [
                    interaction::on_click(crate::display::text("click"), Rc::new(|_| true)),
                    interaction::on_activate(
                        crate::display::text("activate"),
                        crate::libraries::test_widgets::hover(vec![]),
                        Rc::new(|_| true),
                    ),
                    interaction::pickable(
                        crate::display::text("pick"),
                        crate::libraries::test_widgets::hover(vec![]),
                        Value::record([]),
                    ),
                ] {
                    let Recorded::Before { before, .. } = record(&layout) else {
                        panic!("leading callback");
                    };
                    let mut output = HoverOutput::default();
                    before(context)(
                        &mut HoverContext::new(Default::default(), &mut output),
                        placement,
                    );
                    assert!(output.handler.is_some());
                }
                for layout in [
                    hover::on_hover(
                        crate::display::text("claim"),
                        crate::libraries::test_widgets::hover(vec![]),
                    ),
                    hover::block_hover(crate::display::text("occluder")),
                    hover::hover_highlight(
                        crate::display::text("highlight"),
                        crate::libraries::test_widgets::hover(vec![]),
                    ),
                ] {
                    let Recorded::Before { before, .. } = record(&layout) else {
                        panic!("leading callback");
                    };
                    before(context)(
                        &mut HoverContext::new(Default::default(), &mut HoverOutput::default()),
                        placement,
                    );
                }
                assert_eq!(
                    crate::display::widget::frame::place(
                        empty::<(), ()>(context.text, context.inputs.styles),
                        placement,
                        &Default::default()
                    )
                    .after_hover
                    .len(),
                    1
                );
            },
        );
    }

    #[test]
    fn discarded_layout_alternatives_do_not_place_widget_outputs() {
        use measured::choices::{ChoiceBuild, ChoiceLayout, resolve_choices};
        let widget = |width, target| {
            ChoiceLayout::fixed(leaf(
                Extent {
                    width,
                    ascent: 10.0,
                    descent: 0.0,
                },
                move |output: &mut HoverContext<'_, (), usize>, placement| {
                    output.claim(Probe::retaining(placement, target))
                },
            ))
        };
        let mut choices = ChoiceBuild::default();
        let layout = choices.alternatives(vec![widget(100.0, 1), widget(20.0, 2)]);
        let measured = resolve_choices(choices.finish(layout), 50.0, false);
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        let output = crate::display::widget::frame::place(
            measured,
            placement,
            &HoverInput {
                pointer: Some(placement.rect.center()),
                ..Default::default()
            },
        );
        assert_eq!(
            output.claim.clone().map(|(_, claim)| claim),
            Some(puri::hover::Claim::Direct(2))
        );
    }
}
