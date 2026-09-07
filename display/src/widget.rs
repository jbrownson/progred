//! Native widget continuations and their editor-facing inputs/outputs.
//! Layout measures and places these without interpreting a widget description.

use crate::{ActionHandler, Layout, LineEdit};
use gid::Value;
pub use measured::place;
use measured::{Extent, Measured, Output};
use puri::Placement;
use puri::draw::CanvasSink;
use puri::edit::{EditOperation, LineEditState};
use puri::handler::{Handler, HasHandler};
use puri::hover::Probe;
use puri::text::{TextCtx, TextMetrics};
use std::rc::Rc;

pub mod completion;
pub mod container;
pub mod delimiter;
pub mod gesture;
pub mod hover;
pub mod interaction;
pub mod line;
pub mod scroll;
pub mod style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;
pub type Edit<World> = Rc<dyn Fn(&mut World, &LineEdit, &EditOperation<'_>) -> bool>;
pub type Pick<World> = Rc<dyn Fn(&mut World, Value) -> bool>;
/// Interpret a Grap handler with caller-supplied capabilities at this site.
pub type EventInterpreter<World> = Rc<dyn Fn(&mut World, &Value, Value) -> bool>;
pub type Annotate<World> = Rc<dyn Fn(&mut World, Value) -> bool>;
pub type Render<Hover> = Box<dyn FnOnce(&mut dyn CanvasSink, Option<&Hover>)>;
pub type Place<World, Hover> = Box<dyn FnOnce(&mut Fragment<World, Hover>, Placement)>;
pub type Before<World, Hover> =
    Rc<dyn for<'a, 'fonts> Fn(&mut Context<'a, 'fonts, World, Hover>) -> Place<World, Hover>>;

pub struct Context<'a, 'fonts, World, Hover> {
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
    ) -> Measured<Fragment<World, Hover>>,
>;

/// Contribute outputs before the child places, without inspecting its widget type.
pub fn before<World, Hover>(
    child: Layout<World, Hover>,
    before: Before<World, Hover>,
) -> Layout<World, Hover> {
    Layout::Before {
        child: Box::new(child),
        before,
    }
}

/// A side box whose final measurement depends on the enclosed box's span.
/// Prepare borrowed inputs now; measure against the chosen child later.
pub type Side<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(&mut Context<'a, 'fonts, World, Hover>) -> MeasuredSide<World, Hover>,
>;

pub struct MeasuredSide<World, Hover> {
    pub maximum_width: f64,
    pub measure: Box<dyn FnOnce(Extent) -> Measured<Fragment<World, Hover>>>,
}

pub fn selectable<World: 'static, Hover: Clone + 'static>(
    context: &Context<'_, '_, World, Hover>,
) -> impl FnOnce(Measured<Fragment<World, Hover>>) -> Measured<Fragment<World, Hover>> + 'static {
    let site = (context.site)();
    let target = site.target;
    let select = site.select;
    let pick = context.pick.clone();
    let value = site.value.cloned();
    let picking = context.picking;
    let same_target = context.same_target;
    move |child| {
        measured::before_into(
            child,
            move |placement, output: &mut Fragment<World, Hover>| {
                if !placement.clipped_out() {
                    output
                        .claims
                        .push(Probe::retaining(placement, target.clone()));
                    output
                        .handler()
                        .on_pointer_down_with(move |world, event, hovered| {
                            puri::interact::is_primary_contact(event)
                                && hovered
                                    .as_ref()
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

pub fn selectable_side<World: 'static, Hover: Clone + 'static>(
    side: Side<World, Hover>,
) -> Side<World, Hover> {
    Rc::new(move |context| {
        let side = side(context);
        let decorate = selectable(context);
        MeasuredSide {
            maximum_width: side.maximum_width,
            measure: Box::new(move |span| decorate((side.measure)(span))),
        }
    })
}

pub struct Fragment<World, Hover> {
    pub renders: Vec<Render<Hover>>,
    pub handler: Option<Handler<World, Option<Hover>>>,
    pub claims: Vec<Probe<Hover>>,
    pub select: Option<Select<World>>,
    pub floaters: Vec<Box<Self>>,
}

impl<World, Hover> Default for Fragment<World, Hover> {
    fn default() -> Self {
        Self {
            renders: vec![],
            handler: None,
            claims: vec![],
            select: None,
            floaters: vec![],
        }
    }
}

impl<World: 'static, Hover: 'static> Output for Fragment<World, Hover> {
    fn empty() -> Self {
        Self::default()
    }

    fn over(mut self, mut above: Self) -> Self {
        self.renders.append(&mut above.renders);
        self.claims.append(&mut above.claims);
        self.floaters.append(&mut above.floaters);
        self.handler = match (self.handler, above.handler) {
            (base, None) => base,
            (None, above) => above,
            (Some(base), Some(above)) => Some(base.over(above)),
        };
        self.select = above.select.or(self.select);
        self
    }
}

impl<World, Hover: 'static> HasHandler<World> for Fragment<World, Hover> {
    type Input = Option<Hover>;
    fn handler(&mut self) -> &mut Handler<World, Option<Hover>> {
        self.handler.get_or_insert_with(Handler::new)
    }
}

impl<World: 'static, Hover: 'static> container::Layers for Fragment<World, Hover> {
    fn clipped(mut self, placement: Placement) -> Self {
        let renders = std::mem::take(&mut self.renders);
        self.render(move |canvas, hovered| {
            canvas.with_clip(
                placement.rect.into(),
                puri::Affine::IDENTITY,
                Box::new(move |canvas| {
                    for render in renders {
                        render(canvas, hovered);
                    }
                }),
            );
        });
        self.handler = self
            .handler
            .map(|handler| container::gate_starts(handler, placement));
        self
    }

    fn float(&mut self, above: Self) {
        self.floaters.push(Box::new(above));
    }
}

impl<World, Hover: 'static> Fragment<World, Hover> {
    pub fn render(&mut self, render: impl FnOnce(&mut dyn CanvasSink, Option<&Hover>) + 'static) {
        self.renders.push(Box::new(render));
    }
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
) -> Measured<Fragment<World, Hover>> {
    let frame = puri_widgets::text_frame::empty(text, &styles.label, styles.dim.brush.clone());
    leaf(extent(frame.metrics()), move |output, placement| {
        output.render(move |canvas, _| frame.place(canvas, placement));
    })
}

pub fn leaf<World: 'static, Hover: 'static>(
    extent: Extent,
    place: impl FnOnce(&mut Fragment<World, Hover>, Placement) + 'static,
) -> Measured<Fragment<World, Hover>> {
    measured::leaf_into(
        extent,
        move |placement, output: &mut Fragment<World, Hover>| {
            let start = output.renders.len();
            place(output, placement);
            if placement.clipped_out() {
                output.renders.truncate(start);
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use puri::draw::Shape;
    use puri::{Affine, Color, DrawCmd, DrawList, Point, Rect, Stroke};

    #[test]
    fn placement_hover_render_and_dispatch_are_distinct_stages() {
        let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
        let during_place = calls.clone();
        let measured = leaf(
            Extent {
                width: 20.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |output: &mut Fragment<Vec<&str>, usize>, placement| {
                during_place.borrow_mut().push("place");
                output.claims.push(Probe::retaining(placement, 7));
                let during_render = during_place.clone();
                output.renders.push(Box::new(move |canvas, hover| {
                    during_render.borrow_mut().push("render");
                    assert_eq!(hover, Some(&7));
                    canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
                }));
                output.handler().on_key(|state, _| {
                    state.push("key");
                    true
                });
                output.select = Some(Rc::new(|state, _| {
                    state.push("select");
                    true
                }));
            },
        );
        assert!(calls.borrow().is_empty());
        let placement = Placement::root(Rect::new(10.0, 20.0, 30.0, 30.0));
        let output = place(measured, placement);
        assert_eq!(&*calls.borrow(), &["place"]);
        assert_eq!(output.claims.len(), 1);
        assert_eq!(
            output.claims[0].answer(placement.rect.center(), None, 0.0),
            Some(puri::hover::Claim::Direct(7))
        );
        let mut canvas = DrawList::new();
        for render in output.renders {
            render(&mut canvas, Some(&7));
        }
        assert_eq!(&*calls.borrow(), &["place", "render"]);
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
        assert!(output.select.unwrap()(&mut state, Some(Direction::Left)));
        assert_eq!(state, ["key", "select"]);
    }

    #[test]
    fn native_canvas_clip_streams_to_the_selected_interpreter() {
        let mut output: Fragment<(), ()> = Fragment::empty();
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
        assert_eq!(output.renders.len(), 1);
        let mut canvas = DrawList::new();
        for render in output.renders {
            render(&mut canvas, None);
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
        let mut output = Fragment::<(), ()>::empty();
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
        assert_eq!(output.renders.len(), 1);
        let mut canvas = DrawList::new();
        for render in output.renders {
            render(&mut canvas, None);
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
        let side = delimiter::side(crate::Delim::Bracket, crate::Side::Open)(&mut context);
        let measured = (side.measure)(Extent {
            width: 20.0,
            ascent: 10.0,
            descent: 2.0,
        });
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        assert_eq!(place(measured, placement).renders.len(), 1);
        for layout in [
            interaction::on_click(crate::text("click"), Rc::new(|_| true)),
            interaction::on_activate(crate::text("activate"), (), Rc::new(|_| true)),
            interaction::pickable(crate::text("pick"), (), Value::record([])),
        ] {
            let Layout::Before { before, .. } = layout else {
                panic!("leading callback");
            };
            let mut output = Fragment::empty();
            before(&mut context)(&mut output, placement);
            assert!(output.handler.is_some());
        }
        for layout in [
            hover::on_hover(crate::text("claim"), ()),
            hover::block_hover(crate::text("occluder")),
            hover::hover_highlight(crate::text("highlight"), ()),
        ] {
            let Layout::Before { before, .. } = layout else {
                panic!("leading callback");
            };
            before(&mut context)(&mut Fragment::empty(), placement);
        }
        assert_eq!(
            place(empty::<(), ()>(context.text, context.styles), placement)
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
                move |output: &mut Fragment<(), usize>, placement| {
                    output.claims.push(Probe::retaining(placement, target))
                },
            ))
        };
        let mut choices = ChoiceBuild::default();
        let layout = choices.alternatives(vec![widget(100.0, 1), widget(20.0, 2)]);
        let measured = resolve_choices(choices.finish(layout), 50.0, false);
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        let output = place(measured, placement);
        assert_eq!(output.claims.len(), 1);
        assert_eq!(
            output.claims[0].answer(placement.rect.center(), None, 0.0),
            Some(puri::hover::Claim::Direct(2))
        );
    }
}
