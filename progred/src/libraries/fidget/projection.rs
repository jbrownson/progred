use super::{one_marker, parameters, vocabulary::*};
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{
    Delim, Face, activatable, alternatives, col, descend, dim, faced, row, selectable_bracket,
    shared,
};
use crate::display::{group, group_hug, hug};
use crate::libraries::name;
use crate::libraries::representation::Precedence;
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
    (marker != TRANSLATE).then_some(())?;
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

fn operand<W: Host>(marker: CellId, key: CellId, value: &Value) -> Layout<W> {
    let child = descend([Step::Key(marker), Step::Key(key)], None, None);
    let needs_group = value
        .as_record()
        .is_some_and(|fields| fields.contains_key(&name::vocabulary::NAME))
        || match (
            precedence(marker),
            form(value).and_then(|(marker, _)| precedence(marker)),
        ) {
            (Some(parent), Some(nested)) => parent.groups(nested, key == RIGHT),
            _ => false,
        };
    if needs_group {
        selectable_bracket(Delim::Paren, child)
    } else {
        child
    }
}

fn infix<W: Host>(marker: CellId, left: &Value, right: &Value, operator: Layout<W>) -> Layout<W> {
    let left = shared(operand(marker, LEFT, left));
    let right = shared(operand(marker, RIGHT, right));
    let operator = shared(operator);
    alternatives([
        row(6.0, [left.clone(), operator.clone(), right.clone()]),
        col(0, 2.0, [left, row(6.0, [operator, right])]),
    ])
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
        let (spelling, face) = match input.env.name(marker) {
            Some(name) => (name.to_owned(), Face::Label),
            None => (crate::identity::short_id(marker), Face::Id),
        };
        let operator = activatable(faced(spelling, face), &site);
        let fields = content.as_record()?;
        if precedence(marker).is_some() {
            infix(marker, fields.get(&LEFT)?, fields.get(&RIGHT)?, operator)
        } else {
            hug(
                operator,
                arguments(
                    parameters(marker)?
                        .iter()
                        .map(|key| {
                            fields.get(key)?;
                            Some(descend([Step::Key(marker), Step::Key(*key)], None, None))
                        })
                        .collect::<Option<Vec<_>>>()?,
                ),
                0.0,
            )
        }
    };
    Some(
        match input.value?.as_record()?.get(&name::vocabulary::NAME) {
            Some(_) => group_hug(
                row(
                    6.0,
                    [
                        descend([Step::Key(name::vocabulary::NAME)], None, None),
                        dim("="),
                    ],
                ),
                body,
                6.0,
            ),
            None => group(body),
        },
    )
}
