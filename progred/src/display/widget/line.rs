//! A document-aware line widget, composed from Puri text and input functions.

use crate::display::widget::HoverPass;

use super::{Context, Direction, Select, extent, leaf};
use crate::display::{Env, Layout, TextFamily};
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

pub fn layout(line: LineEdit) -> Layout<crate::Editor, crate::frame::Hovered> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::LineEdit);
    Layout::widget(Rc::new(move |context| view(context, line.clone())))
}

pub fn view(
    context: &mut Context<'_, '_, crate::Editor, crate::frame::Hovered>,
    mut line: LineEdit,
) -> Measured<HoverPass<crate::Editor, crate::frame::Hovered>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::LineEdit);
    #[cfg(test)]
    crate::libraries::test_widgets::observe_line(&line);
    let cx = context.inputs;
    let path = context.path;
    let writable = !cx.source.transient() && crate::selection::writable_at(&cx.sources, path);
    if let Some((_, spelling)) = cx.scrub_spelling.filter(|(site, _)| *site == path) {
        line.text = spelling.to_owned();
    }
    let active = cx.selection.filter(|selection| {
        writable
            && selection.path() == path
            && selection.stage(&cx.sources) == crate::selection::Stage::Edge
    });
    let default = active
        .filter(|selection| selection.edit().is_none())
        .map(|selection| selection.initial_line(&line.text));
    let editing = active.and_then(|selection| selection.edit().or(default.as_ref()));
    let root = cx.view.clone();
    let path: Rc<[gid::Step]> = Rc::from(path);
    let style = context.inputs.styles.line_style(&line);
    let placeholder_style = TextStyle {
        family: style.family,
        ..context.inputs.styles.dim.clone()
    };
    let content = match active.zip(editing) {
        Some((_, state)) => {
            let widget = puri::edit::text_edit(
                LineEditDescription {
                    state,
                    focused: true,
                    presentation: context.inputs.styles.line_presentation(&line),
                    style: &context.inputs.styles.edit,
                    placeholder: line
                        .placeholder
                        .as_deref()
                        .map(|text| (text, &placeholder_style)),
                },
                context.text,
            );
            let root = root.clone();
            let path = path.clone();
            let line = line.clone();
            leaf(extent(widget.metrics()), move |output, placement| {
                widget.install(output, placement, move |world, operation| {
                    crate::editing::edit_line(world, &root, &path, &line, operation)
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
    if writable {
        let nav_root = root.clone();
        let nav_path = path.clone();
        let description = line.clone();
        let navigation: Select<crate::Editor> = Rc::new(move |world, direction| {
            crate::editing::select(world, &nav_root, &nav_path);
            if direction == Some(Direction::Left) {
                crate::editing::edit_line(world, &nav_root, &nav_path, &description, &|edit| {
                    edit.state.cursor_to_start();
                    true
                });
            }
            true
        });
        let presentation = context.inputs.styles.line_presentation(&line);
        let scale = context.inputs.styles.scale as f32;
        let target = crate::frame::Hovered::Tree(crate::hover::Hover::Value(path.clone()));
        crate::display::widget::before_hover(content, move |placement: Placement, output| {
            output.on_arrival(Some(navigation));
            if !placement.clipped_out() {
                output.claim(super::frame::Probe::retaining(placement, target));
            }
            output.handler().on_pointer_down(move |world, event| {
                crate::editing::primary_edit(event)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && {
                        if !active {
                            crate::editing::select(world, &root, &path);
                        }
                        crate::editing::edit_line(world, &root, &path, &line, &|edit| {
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
        crate::display::test_support::with_context::<crate::Editor, crate::frame::Hovered, _>(
            &crate::display::test_support::NoProject,
            |context| {
                let mut cx = context.inputs.clone();
                cx.source = crate::projection::Source::Transient { owner: &[] };
                let mut context = Context {
                    inputs: &cx,
                    project: context.project,
                    path: context.path,
                    value: context.value,
                    text: &mut *context.text,
                };
                let measured = view(
                    &mut context,
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
                let output =
                    crate::display::widget::place(measured, placement).run(&Default::default());
                assert!(output.handler.is_none());
                assert!(output.claim.is_none());
                assert!(output.landmark_select.is_none());
                assert_eq!(output.renders.len(), 1);
            },
        );
    }
}
