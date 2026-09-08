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
    let site = (context.line)();
    if let Some(spelling) = site.spelling {
        line.text = spelling.to_owned();
    }
    let active = site.input.as_ref().filter(|input| input.selected);
    let default = active
        .filter(|input| input.editing.is_none())
        .map(|input| (input.initial_text)(&line.text));
    let editing = active.and_then(|input| input.editing.or(default.as_ref()));
    let style = context.styles.line_style(&line);
    let placeholder_style = TextStyle {
        family: style.family,
        ..context.styles.dim.clone()
    };
    let content = match active.zip(editing) {
        Some((input, state)) => {
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
            let edit = input.edit.clone();
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
    let active = active.is_some();
    if let Some(input) = site.input {
        let select = input.select.clone();
        let edit = input.edit.clone();
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
        let select = input.select;
        let edit = input.edit;
        let target = input.target;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_line_needs_no_selection_or_editing_capabilities() {
        crate::test_support::with_context::<(), (), _>(
            &crate::test_support::NoProject,
            |context| {
                context.line = &|| crate::widget::LineSite {
                    spelling: None,
                    input: None,
                };
                let measured = view(
                    context,
                    LineEdit {
                        text: "read only".into(),
                        placeholder: None,
                        update: Rc::new(|_, _, _| panic!("read-only line cannot write")),
                        prefix: String::new(),
                        suffix: String::new(),
                        family: TextFamily::default(),
                    },
                );
                let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
                let output = crate::widget::place(measured, placement).run(&Default::default());
                assert!(output.handler.is_none());
                assert!(output.claim.is_none());
                assert!(output.landmark_select.is_none());
                assert_eq!(output.renders.len(), 1);
            },
        );
    }
}
