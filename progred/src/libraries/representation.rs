//! What a representation's projections share, whether it is a number type or
//! fidget's fields: its tag after a value or operator, operators drawn infix
//! or as calls headed by the tagged operator, and how tightly infix binds.

use crate::display::projection::group;
use crate::display::{
    Delim, Face, Layout, Partial, ProjectionInput, alternatives, col, row, selectable_bracket,
    shared, subscript,
};
use crate::libraries::number::vocabulary::{LEFT, RIGHT};
use ::grap::RuntimeValue;
use gid::{CellId, Step};
use std::rc::Rc;

/// `content` followed by the representation's name as a subscript.
pub(crate) fn tagged<V>(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, V>,
    content: Layout<crate::Editor, crate::frame::Hovered>,
    representation: CellId,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    row(
        2.0,
        [
            content,
            subscript(
                input
                    .env
                    .name(representation)
                    .map(str::to_owned)
                    .unwrap_or_else(|| crate::identity::short_id(representation)),
                Face::Dim,
            ),
        ],
    )
}

pub(crate) fn operation(representation: CellId) -> Partial<crate::Editor, crate::frame::Hovered> {
    crate::display::runtime_partial(move |input| {
        crate::libraries::grap::shallow_cell_with(input, |label| {
            tagged(input, label, representation)
        })
    })
}

/// How tightly an infix operator binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Precedence {
    Comparison,
    Sum,
    Product,
}

impl Precedence {
    /// The arithmetic symbols; other spellings don't read infix.
    pub(crate) fn of_symbol(spelling: &str) -> Option<Self> {
        match spelling {
            "+" | "-" => Some(Self::Sum),
            "*" | "/" => Some(Self::Product),
            _ => None,
        }
    }

    /// Whether an operand bound as `operand` needs parentheses under this:
    /// when it binds looser, or as tightly on the right or in a comparison.
    pub(crate) fn groups(self, operand: Self, right: bool) -> bool {
        operand < self || (operand == self && (right || self == Self::Comparison))
    }
}

/// `left op right` for one representation's operators.
pub(crate) fn infix_display(
    representation: CellId,
    operators: &[(CellId, Precedence)],
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    input.pending.is_none().then_some(())?;
    let binds = |call: &RuntimeValue| {
        call.field(LEFT)?;
        call.field(RIGHT)?;
        let function = call.field(::grap::vocabulary::FUNCTION)?.as_cell()?;
        operators
            .iter()
            .find(|(operator, _)| *operator == function)
            .map(|(_, precedence)| *precedence)
    };
    let fields = input.value?;
    let precedence = binds(fields)?;
    let operand = |field| {
        let child =
            crate::libraries::grap::expression_descend(Step::Key(field), &input.default_projection);
        let grouped = fields
            .field(field)
            .as_ref()
            .and_then(binds)
            .is_some_and(|operand| precedence.groups(operand, field == RIGHT));
        if grouped {
            selectable_bracket(Delim::Paren, child)
        } else {
            child
        }
    };
    let left = shared(operand(LEFT));
    let operator = shared(crate::display::descend_local(
        Step::Key(::grap::vocabulary::FUNCTION),
        operation(representation),
        &input.default_projection,
    ));
    let right = shared(operand(RIGHT));
    // Too narrow for one line, the operator starts the second.
    Some(group(alternatives([
        row(6.0, [left.clone(), operator.clone(), right.clone()]),
        col(0, 2.0, [left, row(6.0, [operator, right])]),
    ])))
}

/// Calls of `operations`, headed by the operator with its tag.
pub(crate) fn calls(
    representation: CellId,
    operations: impl Into<Rc<[CellId]>>,
) -> Partial<crate::Editor, crate::frame::Hovered> {
    let operations = operations.into();
    let function_projection = operation(representation);
    crate::display::runtime_partial(move |input| {
        let function = input
            .value?
            .field(::grap::vocabulary::FUNCTION)?
            .as_cell()?;
        operations.contains(&function).then_some(())?;
        crate::libraries::grap::call_with_function(input, Some(function_projection.clone()))
    })
}
