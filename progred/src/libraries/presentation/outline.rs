//! Outline entries own their UI state; their bodies jump to shared record fields.

use super::vocabulary::OUTLINE;
use crate::display::projection::group;
use crate::display::{self as d, col};
use crate::view::{Layout, Partial, ProjectionInput};
use gid::{CellId, Step, Value};
use std::rc::Rc;

fn sections(value: &Value) -> Option<Vec<CellId>> {
    value
        .as_record()?
        .get(&OUTLINE)?
        .as_list()?
        .values()
        .map(Value::as_cell)
        .collect()
}

pub(super) fn display(input: &ProjectionInput<'_>) -> Option<Layout> {
    let keys = sections(input.value?)?;
    let fields = input.value?.as_record()?.clone();
    let value = ::grap::RuntimeValue::from(input.value?.clone());
    let runtime_input = input.with_value(Some(&value));
    let footer =
        d::structure::extra_fields(&runtime_input, |key| key == OUTLINE || keys.contains(&key));
    let label = d::structure::record_label(&runtime_input, OUTLINE);
    Some(Layout::program(Rc::new(move |context, build| {
        let here = context.site();
        let entries = d::runtime_partial({
            let fields = fields.clone();
            move |input| {
                let elements = input.value?;
                elements.list_len()?;
                let heading = d::runtime_partial(heading);
                d::structure::list_column_with(input, 24.0, Some(heading), |position, heading| {
                    let Some(key) = elements.list_element(&position).and_then(|v| v.as_cell())
                    else {
                        return heading;
                    };
                    section(
                        position,
                        key,
                        heading,
                        &here,
                        &fields,
                        &input.default_projection,
                    )
                })
            }
        });
        group(col(
            0,
            24.0,
            [d::col(
                0,
                8.0,
                [
                    label.clone(),
                    d::descend(Step::Key(OUTLINE), Some(entries), None),
                ],
            )]
            .into_iter()
            .chain(footer.clone()),
        ))
        .measure(context, build)
    })))
}

/// A section whose body is the outline's field `key`, drawn where the
/// outline is stored and edited there.
fn section(
    position: gid::Position,
    key: CellId,
    heading: Layout,
    outline: &crate::editing::Site,
    fields: &gid::Record,
    default_projection: &Partial,
) -> Layout {
    let steps = [Step::Element(position), Step::Key(key)];
    let body_projection = Some(d::compose_partials([
        d::structure::list_column(16.0, None),
        default_projection.clone(),
    ]));
    let unstored = match fields.get(&key) {
        Some(value) => d::at_with_projection(steps.clone(), value, body_projection.clone(), None),
        None => d::slot(),
    };
    let body = d::jump_to(
        steps.clone(),
        outline.below(&[Step::Key(key)]),
        body_projection,
        unstored,
    );
    Layout::program(Rc::new(move |context, build| {
        let visible = open(context.viewing(), &steps);
        col(
            0,
            8.0,
            [heading.clone()]
                .into_iter()
                .chain(visible.then(|| d::pad(18.0, body.clone()))),
        )
        .measure(context, build)
    }))
}

/// Whether the section `steps` below here shows its body. Sections start
/// open, and stay open around the selection.
fn open(viewing: &dyn crate::display::Viewing, steps: &[Step]) -> bool {
    viewing.selected_below(steps) || !crate::annotations::folded(viewing.state_below(steps), CLOSED)
}

/// Whether a section starts closed.
const CLOSED: bool = false;

fn heading(input: &ProjectionInput<'_, ::grap::RuntimeValue>) -> Option<Layout> {
    let key = input.value?.as_cell()?;
    let target = input.targets.current();
    crate::libraries::grap::shallow_cell_with(input, move |label| {
        Layout::program(Rc::new(move |context, build| {
            let visible = open(context.viewing(), &[Step::Key(key)]);
            let section = context.site().below(&[Step::Key(key)]);
            let select = target.select.clone();
            d::on_activate(
                d::row(
                    6.0,
                    [d::dim(if visible { "▾" } else { "▸" }), label.clone()],
                ),
                target.hover.clone(),
                Rc::new(move |session| {
                    section.open(session).fold(CLOSED, Some(visible));
                    select(session);
                    true
                }),
            )
            .measure(context, build)
        }))
    })
}

#[cfg(test)]
mod tests;
