//! A document-aware line widget, composed from Puri text and input functions.

use crate::display::widget::HoverPass;

use super::navigation::Select;
use super::{Context, extent, leaf};
use crate::display::{Host, Layout, TextFamily};
use gid::Value;
use kurbo::Point;
use measured::Measured;
use puri::edit::LineEditDescription;
use puri::handler::{HasHandler, NavigationDirection};
use std::rc::Rc;
use uig::Placement;

/// Current props, captured by this frame's handlers; never persistent state.
#[derive(Clone)]
pub struct LineEdit {
    pub text: String,
    pub update: LineUpdate,
    pub prefix: String,
    pub suffix: String,
    pub family: TextFamily,
    /// Drawn dim where the text goes while there's none, the caret before
    /// it; typing replaces it. Never stored.
    pub placeholder: Option<String>,
}

pub type LineUpdate = Rc<dyn Fn(&str, Option<&Value>) -> Option<Value>>;

impl LineEdit {
    /// This line, drawn for a record, editing the record's field `key`: it
    /// reads that field and writes what it makes of the text back into the
    /// record, the record's other fields kept, or into a new record where
    /// there's none. The record and its field are then one stop, the field
    /// no occurrence of its own; applied again, it reaches a field further
    /// in.
    pub fn of_field(self, key: gid::CellId) -> Self {
        let update = self.update;
        Self {
            update: Rc::new(move |text, record| {
                let fields = record.and_then(Value::as_record);
                let field = update(text, fields.and_then(|fields| fields.get(&key)))?;
                Some(Value::Record(
                    fields.cloned().unwrap_or_default().update(key, field),
                ))
            }),
            ..self
        }
    }
}

pub fn line_edit<W: Host>(line: LineEdit) -> crate::display::host::Layout<W> {
    Layout::widget(Rc::new(move |context| view(context, line.clone())))
}

/// The line `line` describes. Drawn where its value is missing, it's that
/// place's hole too, a Tab stop typed into as the line: the first edit writes
/// the value, and Enter with nothing typed writes the line's reading of
/// nothing. With nothing to show and no placeholder, it shows an empty frame.
pub fn view<W: Host>(
    context: &mut Context<'_, '_, W, W::Hover>,
    line: LineEdit,
) -> Measured<HoverPass<W, W::Hover>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::LineEdit);
    let toolkit = context.toolkit;
    let path = context.path();
    let writable = toolkit.writable();
    let missing = context.value.is_none();
    let active = toolkit.selection().filter(|selection| {
        writable
            && *path == selection.path()
            && match selection.stage(&toolkit.sources()) {
                crate::selection::Stage::Edge => true,
                crate::selection::Stage::Pending => missing,
                crate::selection::Stage::Label => false,
            }
    });
    let default = active
        .filter(|selection| selection.edit().is_none())
        .map(|selection| selection.initial_line(&line.text));
    let editing = active.and_then(|selection| selection.edit().or(default.as_ref()));
    let site = context.site();
    let path = path.clone();
    let content = match editing {
        Some(state) => {
            let widget = puri::edit::text_edit(
                LineEditDescription {
                    state,
                    focused: toolkit.focused(),
                    presentation: context.toolkit.styles().line_presentation(&line),
                    style: &context.toolkit.styles().edit,
                },
                context.text,
            );
            let empty = state.text().is_empty() && state.preedit_parts().is_none();
            let site = site.clone();
            let edit_line = line.clone();
            let edit = leaf(extent(widget.metrics()), move |output, placement| {
                output.line_edit(widget, placement, move |world: &mut W, operation| {
                    site.open(&mut world.session())
                        .edit_line(&edit_line, operation)
                });
            });
            match line.placeholder.as_deref().filter(|_| empty) {
                Some(placeholder) => measured::row(
                    0.0,
                    vec![edit, placeholder_text(context, &line, placeholder)],
                ),
                None => edit,
            }
        }
        None => match line.placeholder.as_deref().filter(|_| line.text.is_empty()) {
            Some(placeholder) => placeholder_text(context, &line, placeholder),
            None if line.prefix.is_empty() && line.text.is_empty() && line.suffix.is_empty() => {
                crate::display::widget::empty(context.text, context.toolkit.styles())
            }
            None => {
                let text = puri::text::text(
                    context.text,
                    &format!("{}{}{}", line.prefix, line.text, line.suffix),
                    &context.toolkit.styles().line_style(&line),
                );
                leaf(extent(text.metrics()), move |output, placement| {
                    output.render(move |canvas, _| text.place(canvas, placement));
                })
            }
        },
    };
    let blank = editing.is_some_and(|state| state.text().is_empty());
    let active = active.is_some();
    let selected = toolkit.selected();
    if writable {
        let nav_site = site.clone();
        let nav_line = line.clone();
        let navigation: Select<W> = Rc::new(move |world, direction| {
            let mut session = world.session();
            let mut edit = nav_site.open(&mut session);
            edit.select();
            if matches!(
                direction,
                Some(NavigationDirection::Left | NavigationDirection::Right)
            ) {
                edit.edit_line(&nav_line, &|edit| {
                    match direction {
                        Some(NavigationDirection::Right) => edit.state.cursor_to_start(),
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
        let target = W::Hover::from(site.hover());
        let empty_site = site.clone();
        let update = line.update.clone();
        let content = crate::display::widget::placed::on_key(content, move |world, event| {
            event.state.is_down()
                && event.key
                    == ui_events::keyboard::Key::Named(ui_events::keyboard::NamedKey::Enter)
                && missing
                && blank
                && world
                    .session()
                    .pending()
                    .filter(|picking| picking.is_at(&empty_site))
                    .zip(update("", None))
                    .is_some_and(|(picking, value)| {
                        picking.commit(
                            value,
                            Some(crate::selection::at(&[], crate::selection::payload::edge())),
                        )
                    })
        });
        crate::display::widget::before_place(content, move |placement: Placement, output| {
            if missing {
                output.tab_stop(stop.clone());
            }
            output.navigation_stop(stop.clone(), selected);
            // Arriving from the left or right places the caret at that edge.
            output.on_arrival(stop.clone(), navigation);
            if !placement.clipped_out() {
                output.claim(puri::hover::Probe::retaining(placement, target));
            }
            let command = output.input.command_modifier;
            output.handler().on_pointer_down(move |world, event| {
                crate::modifiers::primary_edit(command, event)
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
                                event,
                                placement.rect.origin(),
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

/// `placeholder` drawn as the line's text would be, in the dim color.
fn placeholder_text<W: Host>(
    context: &mut Context<'_, '_, W, W::Hover>,
    line: &LineEdit,
    placeholder: &str,
) -> Measured<HoverPass<W, W::Hover>> {
    let styles = context.toolkit.styles();
    let style = puri::text::TextStyle {
        brush: styles.dim.brush.clone(),
        ..styles.line_style(line)
    };
    let text = puri::text::text(context.text, placeholder, &style);
    leaf(extent(text.metrics()), move |output, placement| {
        output.render(move |canvas, _| text.place(canvas, placement));
    })
}

#[cfg(test)]
mod tests;
