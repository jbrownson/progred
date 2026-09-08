//! A document-aware line widget, composed from Puri text and input functions.

use crate::widget::HoverPass;

use super::{Context, Direction, Select, extent, leaf};
use crate::{Env, Layout, TextFamily};
use gid::Value;
use measured::Measured;
use puri::edit::{LineEditDescription, LineEditPointerDown};
use puri::handler::HasHandler;
use puri::text::TextStyle;
use puri::{Placement, Point};
use std::rc::Rc;

/// Current props, captured by this frame's handlers; never persistent state.
#[derive(Clone)]
pub struct LineEdit {
    pub text: String,
    pub placeholder: Option<String>,
    pub update: LineUpdate,
    pub prefix: String,
    pub suffix: String,
    pub family: TextFamily,
}

pub type LineUpdate = Rc<dyn Fn(&dyn Env, &str, Option<&Value>) -> Option<Value>>;

pub fn layout<World: 'static, Hover: Clone + PartialEq + 'static>(
    line: LineEdit,
) -> Layout<World, Hover> {
    #[cfg(feature = "profile")]
    let _profile = crate::profile::enter(crate::profile::Kind::LineEdit);
    Layout::widget(Rc::new(move |context| view(context, line.clone())))
}

pub fn view<World: 'static, Hover: Clone + PartialEq + 'static>(
    context: &mut Context<'_, '_, World, Hover>,
    mut line: LineEdit,
) -> Measured<HoverPass<World, Hover>> {
    #[cfg(feature = "profile")]
    let _profile = crate::profile::enter(crate::profile::Kind::LineEdit);
    let site = (context.site)();
    if let Some(spelling) = site.spelling {
        line.text = spelling.to_owned();
    }
    let active = site.writable && site.selected;
    let default = (active && site.editing.is_none()).then(|| (site.initial_text)(&line.text));
    let editing = active
        .then_some(site.editing.or(default.as_ref()))
        .flatten();
    let style = context.styles.line_style(&line);
    let placeholder_style = TextStyle {
        family: style.family,
        ..context.styles.dim.clone()
    };
    let content = match editing {
        Some(state) => {
            let widget = puri::edit::text_edit(
                LineEditDescription {
                    state,
                    focused: true,
                    presentation: context.styles.line_presentation(&line),
                    style: &context.styles.edit,
                    placeholder: line
                        .placeholder
                        .as_deref()
                        .map(|text| (text, &placeholder_style)),
                },
                context.text,
            );
            let edit = site.edit.clone();
            let line = line.clone();
            leaf(extent(widget.metrics()), move |output, placement| {
                widget.install(output, placement, move |world, operation| {
                    edit(world, &line, operation)
                });
                output.render(move |canvas, _| widget.draw(canvas, placement));
            })
        }
        None => {
            let placeholder = line.placeholder.as_deref().filter(|_| line.text.is_empty());
            let text = puri::text::text(
                context.text,
                &format!(
                    "{}{}{}",
                    line.prefix,
                    placeholder.unwrap_or(&line.text),
                    line.suffix
                ),
                if placeholder.is_some() {
                    &placeholder_style
                } else {
                    &style
                },
            );
            leaf(extent(text.metrics()), move |output, placement| {
                output.render(move |canvas, _| text.place(canvas, placement));
            })
        }
    };
    if site.writable {
        let select = site.select.clone();
        let edit = site.edit.clone();
        let description = line.clone();
        let navigation: Select<World> = Rc::new(move |world, direction| {
            select(world);
            if direction == Some(Direction::Left) {
                edit(world, &description, &|edit| {
                    edit.state.cursor_to_start();
                    true
                });
            }
            true
        });
        let presentation = context.styles.line_presentation(&line);
        let scale = context.styles.scale as f32;
        let select = site.select;
        let edit = site.edit;
        let target = site.target;
        let primary_edit = context.primary_edit;
        crate::widget::before_hover(content, move |placement: Placement, output| {
            output.on_arrival(Some(navigation));
            if !placement.clipped_out() {
                output.claim(super::frame::Probe::retaining(placement, target));
            }
            output.handler().on_pointer_down(move |world, event| {
                primary_edit(event)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && {
                        if !active {
                            select(world);
                        }
                        edit(world, &line, &|edit| {
                            edit.state.pointer_down(
                                &presentation,
                                edit.fonts,
                                edit.layouts,
                                scale,
                                LineEditPointerDown {
                                    point: Point::new(
                                        event.state.position.x - placement.rect.x0,
                                        event.state.position.y - placement.rect.y0,
                                    ),
                                    shift: event.state.modifiers.shift(),
                                    count: event.state.count.max(1),
                                },
                            );
                            true
                        }) || !active
                    }
            });
        })
    } else {
        content
    }
}
