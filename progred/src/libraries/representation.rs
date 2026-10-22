//! What a representation's projections share, whether it is a number type or
//! fidget's fields: its tag after a value, operators drawn infix, and how
//! tightly infix binds. An operator's own name carries its tag.

use crate::display::Pending;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{Delim, Face, alternatives, col, row, selectable_bracket, shared, subscript};
use crate::libraries::grap::numbers::vocabulary::{LEFT, RIGHT};
use ::grap::RuntimeValue;
use ::grap::vocabulary::FUNCTION;
use gid::{CellId, Step};

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
                crate::identity::name_or_short_id(
                    input.env.name(representation).map(|name| name.spelling()),
                    representation,
                ),
                Face::Dim,
            ),
        ],
    )
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
        crate::libraries::grap::expression_path(input, [Step::Key(FUNCTION)]),
        |field| {
            (
                crate::libraries::grap::expression_path(input, [Step::Key(field)]),
                fields
                    .field(field)
                    .as_ref()
                    .and_then(binds)
                    .is_some_and(|operand| precedence.groups(operand, field == RIGHT)),
            )
        },
    );
    Some(crate::display::structure::Fields::default().after(
        input,
        |key| key == FUNCTION || key == LEFT || key == RIGHT,
        layout,
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
