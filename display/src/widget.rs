//! Native widget continuations and their editor-facing inputs/outputs.
//! Layout measures and places these without interpreting a widget description.

use crate::{ActionHandler, Layout, LineEdit};
pub use frame::{Fragment, HoverContext, HoverInput, HoverPass, Ink, Probe};
use gid::Value;
pub use measured::Extent;
use measured::Measured;
pub use measured::place;
use puri::Placement;
use puri::edit::{EditOperation, LineEditState};
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
pub type Edit<World> = Rc<dyn Fn(&mut World, &LineEdit, &EditOperation<'_>) -> bool>;
pub type Pick<World> = Rc<dyn Fn(&mut World, Value) -> bool>;
/// Interpret a Grap handler with caller-supplied capabilities at this site.
pub type EventInterpreter<World> = Rc<dyn Fn(&mut World, &Value, Value) -> bool>;
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
pub type CompletionControl<'a, World, Hover> = &'a dyn Fn(
    &mut TextCtx,
    crate::CompletionKind,
    Option<&crate::CompletionProvider>,
) -> Measured<HoverPass<World, Hover>>;
pub type DrawingControl<'a, World, Hover> =
    &'a dyn Fn(Extent, usize, Value) -> Measured<HoverPass<World, Hover>>;

pub struct Context<'a, 'fonts, World, Hover> {
    pub project: &'a dyn project::Project<World, Hover>,
    pub completion: CompletionControl<'a, World, Hover>,
    pub drawing: DrawingControl<'a, World, Hover>,
    pub text: &'a mut TextCtx<'fonts>,
    pub styles: &'a style::Styles,
    pub site: &'a dyn Fn() -> Site<'a, World, Hover>,
    pub event_interpreter: &'a dyn Fn() -> EventInterpreter<World>,
    pub annotate: &'a dyn Fn() -> Annotate<World>,
    pub start_gesture: &'a dyn Fn() -> gesture::Start<World>,
    pub value_edit: &'a dyn Fn() -> Option<gesture::BeginEdit<World>>,
    pub drag_threshold: f64,
    pub command: fn(&puri::handler::Modifiers) -> bool,
    pub pick: Pick<World>,
    pub picking: fn(&puri::handler::PointerButtonEvent) -> bool,
    pub same_target: fn(&Hover, &Hover) -> bool,
    pub primary_edit: fn(&puri::handler::PointerButtonEvent) -> bool,
}

/// Site state and capabilities are requested only by document-aware widgets.
pub struct Site<'a, World, Hover> {
    pub writable: bool,
    pub selected: bool,
    pub editing: Option<&'a LineEditState>,
    pub initial_text: &'a dyn Fn(&str) -> LineEditState,
    pub spelling: Option<&'a str>,
    pub target: Hover,
    pub value: Option<&'a Value>,
    pub select: ActionHandler<World>,
    pub edit: Edit<World>,
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
            let stroke = puri::Stroke::new(context.styles.scale);
            let brush = context.styles.dim.brush.clone();
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

pub fn selectable<World: 'static, Hover: Clone + PartialEq + 'static>(
    context: &Context<'_, '_, World, Hover>,
) -> impl FnOnce(Measured<HoverPass<World, Hover>>) -> Measured<HoverPass<World, Hover>> + 'static {
    let site = (context.site)();
    let target = site.target;
    let select = site.select;
    let pick = context.pick.clone();
    let value = site.value.cloned();
    let picking = context.picking;
    let same_target = context.same_target;
    move |child| {
        crate::widget::before_hover(
            child,
            move |placement, output: &mut HoverContext<'_, World, Hover>| {
                if !placement.clipped_out() {
                    output.claim(Probe::retaining(placement, target.clone()));
                    output
                        .handler()
                        .on_pointer_down_with(move |world, event, hovered| {
                            puri::interact::is_primary_contact(event)
                                && hovered
                                    .hovered()
                                    .is_some_and(|hovered| same_target(hovered, &target))
                                && if picking(event) {
                                    value
                                        .as_ref()
                                        .is_some_and(|value| pick(world, value.clone()))
                                } else {
                                    select(world)
                                }
                        });
                }
            },
        )
    }
}

pub fn selectable_widget<World: 'static, Hover: Clone + PartialEq + 'static>(
    widget: Widget<World, Hover>,
) -> Widget<World, Hover> {
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
            pass.push(move |output| {
                let start = output.output.renders.len();
                hover(output, placement);
                debug_geometry(placement, output);
                if placement.clipped_out() {
                    output.output.renders.truncate(start);
                }
            });
        },
    )
}

fn debug_geometry<W: 'static, H: 'static>(
    placement: Placement,
    output: &mut HoverContext<'_, W, H>,
) {
    if output.input.debug_geometry && !placement.clipped_out() {
        output.ink(move |canvas, ink| {
            if ink.debug_geometry {
                canvas.stroke_shape(
                    placement.rect.into(),
                    puri::Stroke::new(0.75),
                    puri::Color::new([0.0, 0.65, 1.0, 0.36]).into(),
                    puri::Affine::IDENTITY,
                );
            }
        });
    }
}

pub fn before_hover<W: 'static, H: 'static>(
    child: Measured<HoverPass<W, H>>,
    before: impl FnOnce(Placement, &mut HoverContext<'_, W, H>) + 'static,
) -> Measured<HoverPass<W, H>> {
    measured::before_into(child, move |placement, pass| {
        pass.push(move |output| before(placement, output));
    })
}

pub fn after_hover<W: 'static, H: 'static>(
    child: Measured<HoverPass<W, H>>,
    after: impl FnOnce(Placement, &mut HoverContext<'_, W, H>) + 'static,
) -> Measured<HoverPass<W, H>> {
    measured::after_into(child, move |placement, pass| {
        pass.push(move |output| after(placement, output));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::{Recorded, record};
    use puri::draw::Shape;
    use puri::{Affine, Color, DrawCmd, DrawList, Point, Rect, Stroke};

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
                output.ink(move |canvas, hover| {
                    during_render.borrow_mut().push("render");
                    assert_eq!(hover.hovered, Some(&7));
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
        let pass = place(measured, placement);
        assert!(calls.borrow().is_empty());
        let mut output = pass.run(&HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        });
        assert_eq!(&*calls.borrow(), &["hover"]);
        assert_eq!(
            output.claim.map(|(_, claim)| claim),
            Some(puri::hover::Claim::Direct(7))
        );
        let mut canvas = DrawList::new();
        for render in std::mem::take(&mut output.renders) {
            render(
                &mut canvas,
                Ink {
                    hovered: Some(&7),
                    ..Ink::default()
                },
            );
        }
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
        let mut frame = Fragment::default();
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
        assert_eq!(output.renders.len(), 1);
        let mut canvas = DrawList::new();
        for render in std::mem::take(&mut output.renders) {
            render(&mut canvas, Default::default());
        }
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
        let mut frame = Fragment::default();
        let mut output = crate::widget::HoverContext::<(), ()>::new(Default::default(), &mut frame);
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
        assert_eq!(output.renders.len(), 1);
        let mut canvas = DrawList::new();
        for render in std::mem::take(&mut output.renders) {
            render(&mut canvas, Default::default());
        }
        assert_eq!(calls.get(), 100);
        assert_eq!(canvas.0.len(), 100);
    }

    #[test]
    fn inert_widgets_and_pointer_wrappers_do_not_request_site_state() {
        let mut fonts = puri::text::FontContext::new();
        let mut layouts = puri::text::LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut context = Context::<(), ()> {
            project: &crate::test_support::NoProject,
            completion: &|_, _, _| panic!("unexpected completion control"),
            drawing: &|_, _, _| panic!("unexpected drawing control"),
            text: &mut TextCtx {
                fonts: &mut fonts,
                layouts: &mut layouts,
                cache: &mut cache,
                scale: 1.0,
            },
            styles: &style::editor(1.0),
            site: &|| panic!("unrelated site input requested"),
            event_interpreter: &|| panic!("unrelated Grap interpreter requested"),
            annotate: &|| panic!("unrelated annotation capability requested"),
            start_gesture: &|| panic!("unexpected gesture startup request"),
            value_edit: &|| panic!("unexpected value edit request"),
            drag_threshold: 3.0,
            command: |_| false,
            pick: Rc::new(|_, _| true),
            picking: |_| false,
            same_target: PartialEq::eq,
            primary_edit: |_| true,
        };
        let measured = delimiter::side(crate::Delim::Bracket, crate::Side::Open)(&mut context);
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        assert_eq!(
            place(measured, placement)
                .run(&Default::default())
                .renders
                .len(),
            1
        );
        for layout in [
            interaction::on_click(crate::text("click"), Rc::new(|_| true)),
            interaction::on_activate(crate::text("activate"), (), Rc::new(|_| true)),
            interaction::pickable(crate::text("pick"), (), Value::record([])),
        ] {
            let Recorded::Before { before, .. } = record(&layout) else {
                panic!("leading callback");
            };
            let mut output = Fragment::default();
            before(&mut context)(
                &mut HoverContext::new(Default::default(), &mut output),
                placement,
            );
            assert!(output.handler.is_some());
        }
        for layout in [
            hover::on_hover(crate::text("claim"), ()),
            hover::block_hover(crate::text("occluder")),
            hover::hover_highlight(crate::text("highlight"), ()),
        ] {
            let Recorded::Before { before, .. } = record(&layout) else {
                panic!("leading callback");
            };
            before(&mut context)(
                &mut HoverContext::new(Default::default(), &mut Fragment::default()),
                placement,
            );
        }
        assert_eq!(
            place(empty::<(), ()>(context.text, context.styles), placement)
                .run(&Default::default())
                .renders
                .len(),
            1
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
        let output = place(measured, placement).run(&HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        });
        assert_eq!(
            output.claim.map(|(_, claim)| claim),
            Some(puri::hover::Claim::Direct(2))
        );
    }
}
