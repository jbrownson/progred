//! Outline entries own their UI state; their bodies jump to shared record fields.

use super::vocabulary::OUTLINE;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::{self as d, col};
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

pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Layout<W>> {
    let keys = sections(input.value?)?;
    let fields = input.value?.as_record()?.clone();
    let value = ::grap::RuntimeValue::from(input.value?.clone());
    let runtime_input = input.with_value(Some(&value));
    let footer = d::structure::Fields::default()
        .rest(&runtime_input, |key| key == OUTLINE || keys.contains(&key));
    let label = d::structure::field_label(
        &runtime_input,
        OUTLINE,
        d::structure::field_name(input.env, OUTLINE),
        true,
    );
    Some(Layout::program(Rc::new(move |context, build| {
        let here = context.site();
        let entries = d::runtime_partial({
            let fields = fields.clone();
            move |input| {
                let elements = input.value?;
                elements.list_len()?;
                let heading = d::runtime_partial(heading);
                d::structure::list_column_with(input, 24.0, Some(heading), |position, heading| {
                    match elements.list_element(&position).and_then(|v| v.as_cell()) {
                        Some(key) => section(
                            position,
                            key,
                            heading,
                            &here,
                            &fields,
                            &input.default_projection,
                        ),
                        None => heading,
                    }
                })
            }
        });
        col(
            0,
            24.0,
            [d::col(
                0,
                8.0,
                [
                    label.clone(),
                    d::descend([Step::Key(OUTLINE)], d::Projected::by(Some(entries), None)),
                ],
            )]
            .into_iter()
            .chain(footer.clone()),
        )
        .measure(context, build)
    })))
}

/// A section whose body is the outline's field `key`, drawn where the
/// outline is stored and edited there.
fn section<W: Host>(
    position: gid::Position,
    key: CellId,
    heading: Layout<W>,
    outline: &crate::editing::Site,
    fields: &gid::Record,
    default_projection: &Partial<W>,
) -> Layout<W> {
    let steps = [Step::Element(position), Step::Key(key)];
    let body_projection = Some(d::compose_partials([
        d::structure::list_column(16.0, None),
        default_projection.clone(),
    ]));
    let unstored = match fields.get(&key) {
        Some(value) => d::at(
            steps.clone(),
            value,
            d::Projected::by(body_projection.clone(), None),
        ),
        None => d::slot(),
    };
    // The body stored there is edited there, along with what's stored below
    // it; one with nowhere stored draws as it is.
    let body = match outline.below(&[Step::Key(key)]).location() {
        Some(stored) => d::jump(
            steps.clone(),
            stored,
            d::Projected::by(body_projection, None),
        ),
        None => unstored,
    };
    Layout::program(Rc::new(move |context, build| {
        let visible = open(context.viewing(), &steps);
        col(
            0,
            8.0,
            [heading.clone()]
                .into_iter()
                .chain(visible.then(|| d::indent(18.0, body.clone()))),
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

fn heading<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let key = input.value?.as_cell()?;
    let site = input.site();
    crate::libraries::name::shallow_cell_with(input, move |label| {
        Layout::program(Rc::new(move |context, build| {
            let visible = open(context.viewing(), &[Step::Key(key)]);
            let section = context.site().below(&[Step::Key(key)]);
            let heading = site.clone();
            d::on_activate(
                d::row(
                    6.0,
                    [d::dim(if visible { "▾" } else { "▸" }), label.clone()],
                ),
                W::Hover::from(site.hover()),
                Rc::new(move |session| {
                    section.open(session).fold(CLOSED, Some(visible));
                    heading.open(session).select();
                    true
                }),
            )
            .measure(context, build)
        }))
    })
}

#[cfg(test)]
mod tests;
