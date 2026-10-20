//! What a representation's projections share, whether it is a number type or
//! fidget's fields: its tag after a value or operator, operators drawn infix
//! or as calls headed by the tagged operator, and how tightly infix binds.

use crate::display::Pending;
use crate::display::group;
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::{Delim, Face, alternatives, col, row, selectable_bracket, shared, subscript};
use crate::libraries::grap::numbers::vocabulary::{LEFT, RIGHT};
use ::grap::RuntimeValue;
use ::grap::vocabulary::FUNCTION;
use gid::{CellId, Step};
use std::rc::Rc;

/// `content` followed by the representation's name as a subscript.
pub(crate) fn tagged<W: Host, V>(
    input: &ProjectionInput<'_, W, V>,
    content: Layout<W>,
    representation: CellId,
) -> Layout<W> {
    row(
        2.0,
        [
            content,
            subscript(
                crate::identity::name_or_short_id(input.env.name(representation), representation),
                Face::Dim,
            ),
        ],
    )
}

pub(crate) fn operation<W: Host>(representation: CellId) -> Partial<W> {
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

/// `left op right` for one representation's operators, from the moment the
/// operator is chosen: a missing operand is an empty slot, and other fields,
/// including one being added, follow in a record.
pub(crate) fn infix_display<W: Host>(
    representation: CellId,
    operators: &[(CellId, Precedence)],
    input: &ProjectionInput<'_, W, RuntimeValue>,
) -> Option<Layout<W>> {
    matches!(
        &input.pending,
        None | Some(Pending::Field | Pending::Child(Step::Key(_)))
    )
    .then_some(())?;
    let binds = |call: &RuntimeValue| {
        let function = call.field(FUNCTION)?.as_cell()?;
        operators
            .iter()
            .find(|(operator, _)| *operator == function)
            .map(|(_, precedence)| *precedence)
    };
    let fields = input.value?;
    let precedence = binds(fields)?;
    let layout = infix(
        crate::display::descend_local(
            [Step::Key(FUNCTION)],
            operation(representation),
            &input.default_projection,
        ),
        |field| {
            (
                crate::libraries::grap::expression_path(
                    [Step::Key(field)],
                    &input.default_projection,
                ),
                fields
                    .field(field)
                    .as_ref()
                    .and_then(binds)
                    .is_some_and(|operand| precedence.groups(operand, field == RIGHT)),
            )
        },
    );
    Some(crate::display::structure::with_extra_fields(
        input,
        |key| key == FUNCTION || key == LEFT || key == RIGHT,
        group(layout),
    ))
}

/// `left operator right`, with `operand` giving the layout under each of
/// `LEFT` and `RIGHT` and whether it needs parentheses. Too narrow for one
/// line, the operator starts the second.
pub(crate) fn infix<W: Host>(
    operator: Layout<W>,
    operand: impl Fn(CellId) -> (Layout<W>, bool),
) -> Layout<W> {
    let [left, right] = [LEFT, RIGHT].map(|field| {
        let (child, grouped) = operand(field);
        shared(if grouped {
            selectable_bracket(Delim::Paren, child)
        } else {
            child
        })
    });
    let operator = shared(operator);
    alternatives([
        row(6.0, [left.clone(), operator.clone(), right.clone()]),
        col(0, 2.0, [left, row(6.0, [operator, right])]),
    ])
}

/// Calls of `operations`, headed by the operator with its tag.
pub(crate) fn calls<W: Host>(
    representation: CellId,
    operations: impl Into<Rc<[CellId]>>,
) -> Partial<W> {
    let operations = operations.into();
    let function_projection = operation(representation);
    crate::display::runtime_partial(move |input| {
        let function = input.value?.field(FUNCTION)?.as_cell()?;
        operations.contains(&function).then_some(())?;
        crate::libraries::grap::call_with_function(input, Some(function_projection.clone()))
    })
}
