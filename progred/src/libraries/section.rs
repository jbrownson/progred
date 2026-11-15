//! Sections: a name over a thing, which folds away under it,
//! `{isa: section, name, body}`. A section goes wherever a value does: a
//! notebook's entry, a record's field, a list's element, another section's
//! body. It only labels what it holds.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{self as d, col, row};
use crate::libraries::{Library, name};
use ::grap::RuntimeValue;
use gid::{CellId, Step};
use std::rc::Rc;

pub const ID: CellId = CellId::from_u128(0x7f7fc66001a018ed214e5251a9bd4e02);

pub mod vocabulary {
    use gid::CellId;

    /// A section's kind: `{isa: section, name, body}`.
    pub const SECTION: CellId = CellId::from_u128(0x9d30242da6ce9d5736c24e2adbd820ff);
    /// What a section's name is over.
    pub const BODY: CellId = CellId::from_u128(0xac7cc432940c7cc64b315a5581108930);
}
use vocabulary::*;

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "section",
        [(SECTION, "section"), (BODY, "body")],
        ::grap::ForeignFunctions::default(),
    )
    .with_projections_for([(SECTION, d::runtime_partial(display))])
}

/// A section of `body` named `spelling`.
pub fn record(spelling: &str, body: gid::Value) -> gid::Value {
    ::grap::isa::record(
        SECTION,
        [
            (
                name::vocabulary::NAME,
                crate::libraries::text::value(spelling),
            ),
            (BODY, body),
        ],
    )
}

/// Whether a section starts closed.
const CLOSED: bool = false;

/// The section's name beside a disclosure mark, and under them, unless it's
/// folded, its body, drawn as it would be anywhere. The mark folds the body
/// and selects the section; the name is edited where it's drawn.
fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    d::structure::kind_form(input, SECTION, [name::vocabulary::NAME, BODY], |_| {
        let site = input.site();
        let name = d::descend(
            [Step::Key(name::vocabulary::NAME)],
            Some(d::first(input, name::projection(Some("section".into())))),
            None,
        );
        let body = d::indent(18.0, d::descend([Step::Key(BODY)], None, None));
        Some(Layout::program(Rc::new(move |context, build| {
            let open = open(context.viewing());
            let folding = context.site().below(&[Step::Key(BODY)]);
            let section = site.clone();
            let target = W::Hover::from(site.hover());
            let mark = d::widget::hover::on_hover(
                d::on_activate(
                    d::dim(if open { "▾" } else { "▸" }),
                    target.clone(),
                    Rc::new(move |session| {
                        folding.open(session).fold(CLOSED, Some(open));
                        section.open(session).select();
                        true
                    }),
                ),
                target,
            );
            col(
                0,
                8.0,
                [row(6.0, [mark, name.clone()])]
                    .into_iter()
                    .chain(open.then(|| body.clone())),
            )
            .measure(context, build)
        })))
    })
}

/// Whether the section here shows its body: unless it's been folded, and
/// always around the selection.
fn open(viewing: &dyn d::Viewing) -> bool {
    let body = [Step::Key(BODY)];
    viewing.selected_below(&body) || !crate::annotations::folded(viewing.state_below(&body), CLOSED)
}

#[cfg(test)]
#[path = "tests/section.rs"]
mod tests;
