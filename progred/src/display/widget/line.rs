//! A document-aware line widget, composed from Puri text and input functions.

use crate::display::widget::HoverPass;

use super::{Context, Select, extent, leaf};
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

pub fn layout(line: LineEdit) -> crate::view::Layout {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::LineEdit);
    Layout::widget(Rc::new(move |context| view(context, line.clone())))
}

pub fn view(
    context: &mut Context<'_, '_, crate::Editor, crate::frame::Hovered>,
    line: LineEdit,
) -> Measured<HoverPass<crate::Editor, crate::frame::Hovered>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::LineEdit);
    let toolkit = context.toolkit;
    let path = toolkit.path();
    let writable = toolkit.writable();
    let active = toolkit.selection().filter(|selection| {
        writable
            && selection.path() == path
            && selection.stage(&toolkit.sources()) == crate::selection::Stage::Edge
    });
    let default = active
        .filter(|selection| selection.edit().is_none())
        .map(|selection| selection.initial_line(&line.text));
    let editing = active.and_then(|selection| selection.edit().or(default.as_ref()));
    let site = context.site();
    let path: Rc<[gid::Step]> = Rc::from(path);
    let style = context.toolkit.styles().line_style(&line);
    let placeholder_style = TextStyle {
        family: style.family,
        ..context.toolkit.styles().dim.clone()
    };
    let content = match active.zip(editing) {
        Some((_, state)) => {
            let widget = puri::edit::text_edit(
                LineEditDescription {
                    state,
                    focused: toolkit.focused(),
                    presentation: context.toolkit.styles().line_presentation(&line),
                    style: &context.toolkit.styles().edit,
                    placeholder: line
                        .placeholder
                        .as_deref()
                        .map(|text| (text, &placeholder_style)),
                },
                context.text,
            );
            let site = site.clone();
            let line = line.clone();
            leaf(extent(widget.metrics()), move |output, placement| {
                output.line_edit(
                    widget,
                    placement,
                    move |world: &mut crate::Editor, operation| {
                        site.open(&mut world.session()).edit_line(&line, operation)
                    },
                );
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
    let selected = toolkit.selected();
    if writable {
        let nav_site = site.clone();
        let nav_line = line.clone();
        let navigation: Select<crate::Editor> = Rc::new(move |world, direction| {
            let mut session = world.session();
            let mut edit = nav_site.open(&mut session);
            edit.select();
            if matches!(
                direction,
                Some(super::navigation::Direction::Left | super::navigation::Direction::Right)
            ) {
                edit.edit_line(&nav_line, &|edit| {
                    match direction {
                        Some(super::navigation::Direction::Right) => edit.state.cursor_to_start(),
                        _ => edit.state.cursor_to_end(),
                    }
                    true
                });
            }
            true
        });
        let stop = path.clone();
        let presentation = context.toolkit.styles().line_presentation(&line);
        let scale = context.toolkit.styles().scale as f32;
        let target = crate::frame::Hovered::Tree(site.hover());
        crate::display::widget::before_place(content, move |placement: Placement, output| {
            output.navigation_stop(stop.clone(), selected);
            // Arriving from the left or right places the caret at that edge.
            output.on_arrival(stop.clone(), navigation);
            if !placement.clipped_out() {
                output.claim(super::frame::Probe::retaining(placement, target));
            }
            let primary = crate::modifiers::primary_edit(output.input.command_modifier);
            output.handler().on_pointer_down(move |world, event| {
                primary(event)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && {
                        let mut session = world.session();
                        let mut edit = site.open(&mut session);
                        if !active {
                            edit.select();
                        }
                        edit.edit_line(&line, &|edit| {
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
        crate::display::widget::before_place(content, move |_, output| {
            output.navigation_stop(path, selected);
        })
    }
}

#[cfg(test)]
mod tests;
