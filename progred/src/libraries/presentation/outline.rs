//! An opt-in outline of an ordinary record. Ordering names fields, not
//! copied values; all editing continues through their original locations.

use super::vocabulary::OUTLINE;
use crate::display::{self as d, Layout, ProjectionInput, widget};
use crate::frame::Hovered;
use gid::{CellId, Step, Value};
use puri::Canvas;
use std::rc::Rc;

/// Repeated references describe one section. Unlisted fields remain in
/// the ordinary record footer; the outline itself is the editable tab list.
fn sections(value: &Value) -> Option<Vec<CellId>> {
    let order = value.as_record()?.get(&OUTLINE)?.as_list()?;
    let mut keys = Vec::new();
    for (_, value) in order {
        let key = value.as_cell()?;
        if key != OUTLINE && !keys.contains(&key) {
            keys.push(key);
        }
    }
    Some(keys)
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
) -> Option<Layout<crate::Editor, Hovered>> {
    let keys = sections(input.value?)?;
    let footer = extras(input, &keys);
    let bodies = keys
        .iter()
        .map(|key| {
            let field = d::structure::record_field(
                input,
                *key,
                Some(d::structure::vertical_list(16.0, None)),
            );
            d::col(0, 8.0, [field.label, d::pad(18.0, field.value)])
        })
        .collect::<Vec<_>>();

    Some(Layout::program(Rc::new(move |context, build| {
        let root = context.inputs.view.clone();
        let sections: Rc<[Section]> = keys
            .iter()
            .map(|key| {
                let path: Rc<[Step]> = context
                    .path
                    .iter()
                    .cloned()
                    .chain([Step::Key(*key)])
                    .collect();
                let default_closed = context
                    .value
                    .and_then(Value::as_record)
                    .and_then(|fields| fields.get(key))
                    .and_then(|value| {
                        crate::selection::collapse_default_for_value(
                            &context.inputs.sources,
                            value,
                            false,
                        )
                    })
                    .unwrap_or(false);
                let selected = context
                    .inputs
                    .selection
                    .is_some_and(|selection| selection.path().starts_with(&path));
                Section {
                    key: *key,
                    visible: selected
                        || !crate::annotations::collapsed(
                            context.inputs.annotations,
                            &path,
                            default_closed,
                        ),
                    path,
                    default_closed,
                }
            })
            .collect();
        let tabs = d::structure::list(Some(d::partial({
            let sections = sections.clone();
            move |input| {
                let key = input.value?.as_cell()?;
                match sections.iter().find(|section| section.key == key) {
                    Some(section) => section_tab(input, section, &root),
                    None => crate::libraries::grap::shallow_cell(input),
                }
            }
        })));
        let strip = d::descend(Step::Key(OUTLINE), Some(tabs), None);
        let visible = sections
            .iter()
            .zip(&bodies)
            .filter(|(section, _)| section.visible)
            .map(|(_, body)| body.clone());
        d::col(
            0,
            24.0,
            [strip].into_iter().chain(visible).chain(footer.clone()),
        )
        .measure(context, build)
    })))
}

/// These are frame-local props for real sibling locations, not tab identities
/// or a second store of visibility state.
#[derive(Clone)]
struct Section {
    key: CellId,
    path: Rc<[Step]>,
    default_closed: bool,
    visible: bool,
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

fn section_tab(
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
    section: &Section,
    root: &crate::workspace::Root,
) -> Option<Layout<crate::Editor, Hovered>> {
    let target = input.targets.current();
    let hover = target.hover;
    let select = target.select;
    let section = section.clone();
    let root = root.clone();
    crate::libraries::grap::shallow_cell_with(input, move |label| {
        let label = d::pad(4.0, label);
        let label = if section.visible {
            widget::before(
                label,
                Rc::new(move |context| {
                    let brush = context.inputs.styles.accent_wash.brush.clone();
                    Box::new(move |output, placement| {
                        output.render(move |canvas, _| {
                            canvas.fill(placement.rect, brush, puri::Affine::IDENTITY);
                        });
                    })
                }),
            )
        } else {
            label
        };
        d::on_activate(
            label,
            hover,
            Rc::new(move |editor| {
                editor.finish_gesture();
                let before = editor.model.snapshot();
                let Some(view) = editor.model.workspace.view_mut(&root) else {
                    return false;
                };
                // A section may contain any value, including a scalar
                // or a missing field. Its visibility is still one fold
                // at that real location, not a new UI-state convention.
                crate::annotations::set_collapsed(
                    &mut view.annotations,
                    &section.path,
                    section.default_closed,
                    section.visible,
                );
                // The click selects the actual outline element, keeping
                // structural editing available and any hidden caret out.
                select(editor);
                crate::selection::break_edit_run(editor.model.selection.as_mut());
                editor.model.history.record(before);
                true
            }),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_order_is_explicit_and_open_to_unrelated_fields() {
        let a = CellId::from_u128(1);
        let b = CellId::from_u128(2);
        let value = Value::record([
            (
                OUTLINE,
                Value::list([b.into(), a.into(), b.into(), OUTLINE.into()]),
            ),
            (a, Value::record([])),
        ]);
        assert_eq!(sections(&value), Some(vec![b, a]));
        // A missing section remains a real editable location, not a fabricated value.
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
