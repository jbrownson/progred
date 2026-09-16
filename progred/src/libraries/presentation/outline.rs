//! Outline entries own their UI state; their bodies jump to shared record fields.

use super::vocabulary::OUTLINE;
use crate::display::{self as d, Layout, ProjectionInput};
use crate::frame::Hovered;
use gid::{CellId, Path, Step, Value};
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

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
) -> Option<Layout<crate::Editor, Hovered>> {
    let keys = sections(input.value?)?;
    let fields = input.value?.as_record()?.clone();
    let footer = extras(input, &keys);
    let label = d::structure::record_label(input, OUTLINE);
    Some(Layout::program(Rc::new(move |context, build| {
        let source = context
            .inputs
            .edits
            .source(context.path)
            .map(|path| path.into_owned());
        let entries = d::partial({
            let fields = fields.clone();
            move |input| {
                let elements = input.value?.as_list()?;
                let heading = d::partial({
                    let fields = fields.clone();
                    move |input| heading(input, &fields)
                });
                d::structure::list_column_with(input, 24.0, Some(heading), |position, heading| {
                    let Some(key) = elements.get(&position).and_then(Value::as_cell) else {
                        return heading;
                    };
                    section(
                        position,
                        key,
                        heading,
                        source.as_deref(),
                        &fields,
                        &input.default_projection,
                    )
                })
            }
        });
        d::col(
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
        )
        .measure(context, build)
    })))
}

fn section(
    position: gid::Position,
    key: CellId,
    heading: Layout<crate::Editor, Hovered>,
    source: Option<&[Step]>,
    fields: &gid::Record,
    default_projection: &d::Partial<crate::Editor, Hovered>,
) -> Layout<crate::Editor, Hovered> {
    let value = fields.get(&key).cloned();
    let steps = [Step::Element(position), Step::Key(key)];
    let body_projection = Some(d::compose_partials([
        d::structure::list_column(16.0, None),
        default_projection.clone(),
    ]));
    let body = match source {
        Some(source) => d::jump_with_conject(
            steps.clone(),
            source
                .iter()
                .cloned()
                .chain([Step::Key(key)])
                .collect::<Path>(),
            d::Conject::descend(),
            body_projection,
            None,
        ),
        None => match &value {
            Some(value) => d::at_with_projection(steps.clone(), value, body_projection, None),
            None => d::slot(),
        },
    };
    Layout::program(Rc::new(move |context, build| {
        let path: Path = context.path.iter().cloned().chain(steps.clone()).collect();
        let (_, visible) = visibility(context.inputs, &path, value.as_ref());
        d::col(
            0,
            8.0,
            [heading.clone()]
                .into_iter()
                .chain(visible.then(|| d::pad(18.0, body.clone()))),
        )
        .measure(context, build)
    }))
}

fn visibility(
    cx: &crate::projection::Cx<'_>,
    path: &[Step],
    value: Option<&Value>,
) -> (bool, bool) {
    let default_closed = value
        .and_then(|value| crate::selection::collapse_default_for_value(&cx.sources, value, false))
        .unwrap_or(false);
    let selected = cx.selection.is_some_and(|s| s.path().starts_with(path));
    let visible = selected || !crate::annotations::collapsed(cx.annotations, path, default_closed);
    (default_closed, visible)
}

fn heading(
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
    fields: &gid::Record,
) -> Option<Layout<crate::Editor, Hovered>> {
    let key = input.value?.as_cell()?;
    let value = fields.get(&key).cloned();
    let target = input.targets.current();
    crate::libraries::grap::shallow_cell_with(input, move |label| {
        Layout::program(Rc::new(move |context, build| {
            let path: Path = context
                .path
                .iter()
                .cloned()
                .chain([Step::Key(key)])
                .collect();
            let (default_closed, visible) = visibility(context.inputs, &path, value.as_ref());
            let root = context.inputs.view.clone();
            let select = target.select.clone();
            d::on_activate(
                d::row(
                    6.0,
                    [d::dim(if visible { "▾" } else { "▸" }), label.clone()],
                ),
                target.hover.clone(),
                Rc::new(move |editor| {
                    editor.set_collapsed(&root, &path, default_closed, Some(visible));
                    select(editor);
                    true
                }),
            )
            .measure(context, build)
        }))
    })
}

fn extras(
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
    sections: &[CellId],
) -> Option<Layout<crate::Editor, Hovered>> {
    let fields = d::structure::record_keys(input)?
        .into_iter()
        .filter(|key| *key != OUTLINE && !sections.contains(key))
        .map(|key| d::structure::record_field(input, key, None))
        .collect::<Vec<_>>();
    let pending = d::structure::pending_field(input);
    if fields.is_empty() && pending.is_none() {
        None
    } else {
        Some(d::record_heads(fields, pending))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_order_preserves_repeated_occurrences_and_unrelated_fields() {
        let a = CellId::from_u128(1);
        let b = CellId::from_u128(2);
        let value = Value::record([
            (OUTLINE, Value::list([b.into(), a.into(), b.into()])),
            (a, Value::record([])),
        ]);
        assert_eq!(sections(&value), Some(vec![b, a, b]));
        assert!(!value.as_record().unwrap().contains_key(&b));
        assert!(
            sections(&Value::record([(
                OUTLINE,
                Value::list([Value::record([])])
            )]))
            .is_none()
        );
    }
}
