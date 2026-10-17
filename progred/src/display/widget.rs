//! Native widget continuations and their editor-facing inputs/outputs.
//! Layout measures and places these without interpreting a widget description.

use crate::display::Layout;
#[cfg(test)]
pub use frame::place;
#[cfg(test)]
pub use frame::{Attribution, HoverOutput};
pub use frame::{HoverContext, HoverInput, HoverPass, Probe};
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
pub mod image;
pub mod interaction;
pub mod line;
pub mod list;
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
    pub value: Option<&'a grap::RuntimeValue>,
}

pub type Widget<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(
        &mut Context<'a, 'fonts, World, Hover>,
    ) -> Measured<HoverPass<World, Hover>>,
>;

pub type Around<World, Hover> = Rc<
    dyn for<'a, 'fonts> Fn(
        &mut Context<'a, 'fonts, World, Hover>,
    ) -> Box<
        dyn FnOnce(Measured<HoverPass<World, Hover>>) -> Measured<HoverPass<World, Hover>>,
    >,
>;

pub fn around<W: 'static, H: 'static>(child: Layout<W, H>, wrap: Around<W, H>) -> Layout<W, H> {
    Layout::new(move |builder| {
        let child = child.run(builder);
        builder.around(child, wrap.clone())
    })
}

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

/// A wash of the palette's `color` under the child, its corners rounded.
pub fn ground<World: 'static, Hover: 'static>(
    child: Layout<World, Hover>,
    color: fn(&crate::styles::Palette) -> peniko::Color,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |context| {
            let scale = context.inputs.styles.scale;
            let color = color(&context.inputs.styles.palette);
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, _| {
                        canvas.fill_shape(
                            puri::RoundedRect::from_rect(placement.rect, 4.0 * scale).into(),
                            color.into(),
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
                    let command = output.input.command_modifier;
                    output.claim(Probe::retaining(placement, target.clone()));
                    output
                        .handler()
                        .on_pointer_down_with(move |world, event, hovered| {
                            puri::interact::is_primary_contact(event)
                                && hovered.hovered().is_some_and(|hovered| hovered == &target)
                                && if command.pressed(&event.state.modifiers) {
                                    value
                                        .as_ref()
                                        .is_some_and(|value| world.pick_identity(value.to_value()))
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

/// Expand before invoking the entire child's placement, including decorations.
pub fn fill_width<W: 'static, H: 'static>(child: Layout<W, H>) -> Layout<W, H> {
    Layout::program(Rc::new(move |context, build| {
        let child = child.measure(context, build);
        measured::choices::ChoiceLayout::map(child, 0.0, |child| {
            measured::around_into(child, |placement, inner, output| {
                inner.place_at_into(
                    Placement {
                        rect: puri::Rect::new(
                            placement.available_rect.x0,
                            placement.rect.y0,
                            placement.available_rect.x1,
                            placement.rect.y1,
                        ),
                        ..placement
                    },
                    output,
                );
            })
        })
    }))
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
mod tests;
