use super::{one_marker, parameters, vocabulary::*};
use crate::display::Projected;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::hug;
use crate::display::{
    Delim, alternatives, col, descend, dim, row, selectable, selectable_bracket, shared,
};
use crate::libraries::name;
use crate::libraries::representation::{self, Precedence};
use gid::{CellId, Step, Value};

#[cfg(test)]
mod tests;

fn precedence(marker: CellId) -> Option<Precedence> {
    let (_, symbol) = super::ARITHMETIC
        .into_iter()
        .find(|(cell, _)| *cell == marker)?;
    Precedence::of_symbol(symbol)
}

fn form(value: &Value) -> Option<(CellId, &Value)> {
    let fields = value.as_record()?;
    let marker = one_marker(fields)?;
    let content = fields.get(&marker)?;
    if marker == AXIS {
        content.as_cell()?;
    } else {
        let expected = parameters(marker)?;
        let content = content.as_record()?;
        expected
            .iter()
            .all(|key| content.contains_key(key))
            .then_some(())?;
    }
    Some((marker, content))
}

/// Whether `value`, under `marker`'s `key`, needs parentheses: when it's
/// named, so reads as `name = …`, or binds too loosely to stand there bare.
fn grouped(marker: CellId, key: CellId, value: &Value) -> bool {
    value
        .as_record()
        .is_some_and(|fields| fields.contains_key(&name::vocabulary::NAME))
        || match (
            precedence(marker),
            form(value).and_then(|(marker, _)| precedence(marker)),
        ) {
            (Some(parent), Some(nested)) => parent.groups(nested, key == RIGHT),
            _ => false,
        }
}

fn arguments<W: Host>(children: impl IntoIterator<Item = Layout<W>>) -> Layout<W> {
    let children: Vec<_> = children.into_iter().map(shared).collect();
    selectable_bracket(
        Delim::Paren,
        match children.as_slice() {
            [child] => child.clone(),
            _ => alternatives([
                row(
                    0.0,
                    children.iter().enumerate().flat_map(|(i, child)| {
                        (i > 0)
                            .then(|| dim(", "))
                            .into_iter()
                            .chain([child.clone()])
                    }),
                ),
                col(
                    0,
                    2.0,
                    children.iter().enumerate().map(|(i, child)| {
                        if i == 0 {
                            child.clone()
                        } else {
                            row(0.0, [dim(", "), child.clone()])
                        }
                    }),
                ),
            ]),
        },
    )
}

pub(super) fn field<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    // An active new field needs the structural record's insertion control.
    input.pending.is_none().then_some(())?;
    super::one_marker_by(|marker| input.value.is_some_and(|v| v.contains_field(marker)))?;
    let input = input.with_value(input.value.map(::grap::RuntimeValue::as_value));
    let (marker, content) = form(input.value?)?;
    let body = if marker == AXIS {
        crate::libraries::grap::shallow_path([Step::Key(AXIS)], &input.default_projection)
    } else {
        let site = input.site();
        let operator = selectable(
            crate::display::structure::field_name(input.env, marker),
            &site,
        );
        let fields = content.as_record()?;
        if precedence(marker).is_some() {
            representation::infix(operator, |key| {
                (
                    descend([Step::Key(marker), Step::Key(key)], Projected::ordinary()),
                    fields
                        .get(&key)
                        .is_some_and(|value| grouped(marker, key, value)),
                )
            })
        } else {
            hug(
                operator,
                arguments(
                    parameters(marker)?
                        .iter()
                        .map(|key| {
                            fields.get(key)?;
                            Some(descend(
                                [Step::Key(marker), Step::Key(*key)],
                                Projected::ordinary(),
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?,
                ),
                0.0,
            )
        }
    };
    Some(
        match input.value?.as_record()?.get(&name::vocabulary::NAME) {
            Some(_) => hug(
                row(
                    6.0,
                    [
                        crate::display::descend(
                            [Step::Key(name::vocabulary::NAME)],
                            Projected::here(name::projection(None), &input.default_projection),
                        ),
                        dim("="),
                    ],
                ),
                body,
                6.0,
            ),
            None => body,
        },
    )
}
