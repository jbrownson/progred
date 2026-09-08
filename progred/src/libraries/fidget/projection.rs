use super::{one_marker, parameters, vocabulary::*};
use crate::display::{
    Delim, Face, Layout, ProjectionInput, activatable, alternatives, at, col, dim, faced, hug, row,
    selectable_bracket, shared,
};
use crate::libraries::name;
use gid::{CellId, Step, Value};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Sum,
    Product,
}

fn precedence(marker: CellId) -> Option<Precedence> {
    match marker {
        SUM | SUBTRACT => Some(Precedence::Sum),
        MULTIPLY | DIVIDE => Some(Precedence::Product),
        _ => None,
    }
}

fn form(value: &Value) -> Option<(CellId, &Value)> {
    let fields = value.as_record()?;
    let marker = one_marker(fields)?;
    (marker != TRANSLATE).then_some(())?;
    fields
        .keys()
        .all(|field| *field == marker || *field == name::vocabulary::NAME)
        .then_some(())?;
    let content = fields.get(&marker)?;
    if marker == AXIS {
        content.as_cell()?;
    } else {
        let expected = parameters(marker)?;
        let content = content.as_record()?;
        (content.len() == expected.len() && expected.iter().all(|key| content.contains_key(key)))
            .then_some(())?;
    }
    Some((marker, content))
}

fn operand(
    marker: CellId,
    key: CellId,
    value: &Value,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    let child = at([Step::Key(marker), Step::Key(key)], value);
    let needs_group = value
        .as_record()
        .is_some_and(|fields| fields.contains_key(&name::vocabulary::NAME))
        || match (
            precedence(marker),
            form(value).and_then(|(marker, _)| precedence(marker)),
        ) {
            (Some(parent), Some(nested)) => nested < parent || (nested == parent && key == RIGHT),
            _ => false,
        };
    if needs_group {
        selectable_bracket(Delim::Paren, child)
    } else {
        child
    }
}

fn infix(
    marker: CellId,
    left: &Value,
    right: &Value,
    operator: Layout<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    let left = shared(operand(marker, LEFT, left));
    let right = shared(operand(marker, RIGHT, right));
    let operator = shared(operator);
    alternatives([
        row(6.0, [left.clone(), operator.clone(), right.clone()]),
        col(0, 2.0, [left, row(6.0, [operator, right])]),
    ])
}

fn arguments(
    children: impl IntoIterator<Item = Layout<crate::Editor, crate::frame::Hovered>>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
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

pub(super) fn field(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    // An active new field needs the structural record's insertion control.
    input.pending.is_none().then_some(())?;
    let (marker, content) = form(input.value?)?;
    let body = if marker == AXIS {
        crate::libraries::grap::shallow_at([Step::Key(AXIS)], content, &input.default_projection)
    } else {
        let target = input.targets.current();
        let (spelling, face) = match input.env.name(marker) {
            Some(name) => (name.to_owned(), Face::Label),
            None => (name::short_id(marker), Face::Id),
        };
        let operator = activatable(faced(spelling, face), target.hover, target.select);
        let fields = content.as_record()?;
        if precedence(marker).is_some() {
            infix(marker, fields.get(&LEFT)?, fields.get(&RIGHT)?, operator)
        } else {
            hug(
                operator,
                arguments(
                    parameters(marker)?
                        .iter()
                        .map(|key| Some(at([Step::Key(marker), Step::Key(*key)], fields.get(key)?)))
                        .collect::<Option<Vec<_>>>()?,
                ),
                0.0,
                20.0,
            )
        }
    };
    Some(
        match input.value?.as_record()?.get(&name::vocabulary::NAME) {
            Some(name) => hug(
                row(
                    6.0,
                    [at([Step::Key(name::vocabulary::NAME)], name), dim("=")],
                ),
                body,
                6.0,
                20.0,
            ),
            None => body,
        },
    )
}
