//! Booleans: literals of the bool kind, `{isa: bool, bytes}`, one byte, 1
//! for true and 0 for false. Grap holds them natively, as it does numbers;
//! this library names the kind, draws it, keeps the constants `true` and
//! `false`, which code refers to by name, and adds `require`.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};
use crate::libraries::{Library, isa, name};
use ::grap::{ForeignFunction, ForeignFunctions, RuntimeValue};
use gid::Value;
use std::rc::Rc;

pub const ID: gid::CellId = gid::CellId::from_u128(0xf76a2ef341541a5c955fc23094e2df52);

pub mod vocabulary {
    use gid::CellId;

    pub use ::grap::logic::{BOOL, NOT_BOOL};
    /// The constants, `true` and `false`, holding their literals.
    pub const TRUE: CellId = CellId::from_u128(0x04831f9b704935059231eb111770e62e);
    pub const FALSE: CellId = CellId::from_u128(0x8fcc0a2e5f26c72efd9e917a28919801);
    pub const REQUIRE: CellId = CellId::from_u128(0xcadca87ed76939f9c1ba3bb94495ad06);
    pub const CONDITION: CellId = CellId::from_u128(0x91c786935eb5794b005582b8e7941bc2);
    pub const CONDITION_NOT_MET: CellId = CellId::from_u128(0x1d6c6b09f182a3a036e19eb06cdf1ffc);
}

fn functions() -> ForeignFunctions<'static> {
    ForeignFunctions::default().register(
        vocabulary::REQUIRE,
        ForeignFunction::with_arguments([vocabulary::CONDITION], |_, (condition,): (bool,)| {
            if condition {
                Value::record([])
            } else {
                ::grap::absent::with_detail(
                    vocabulary::CONDITION_NOT_MET,
                    vocabulary::CONDITION,
                    ::grap::logic::value(false),
                )
            }
        })
        .cacheable(),
    )
}

/// How a boolean is spelled.
fn spelling(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// The boolean `spelling` spells.
fn spelled(spelling: &str) -> Option<bool> {
    match spelling.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// A boolean reads as its spelling, edited in place, after any name, then
/// any other fields.
fn display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let value = input.value?.as_bool()?;
    let line = crate::display::line_edit(crate::display::LineEdit {
        text: spelling(value).into(),
        update: Rc::new(|spelling: &str, current: Option<&Value>| {
            let value = ::grap::logic::value(spelled(spelling)?);
            Some(super::keeping_fields(current, value))
        }),
        prefix: String::new(),
        suffix: String::new(),
        family: crate::display::TextFamily::SystemUi,
        placeholder: None,
    });
    Some(crate::display::structure::Fields::default().after(
        input,
        |key| isa::literal_field(key) || key == name::vocabulary::NAME,
        name::with_name(input, line),
    ))
}

/// What's typed, the start of `true` or `false`: read as a new boolean, and
/// where the libraries' vocabulary is offered, as the constant by name.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Value).then_some(())?;
    let query = request.query.trim().to_lowercase();
    let spelled = [(vocabulary::TRUE, true), (vocabulary::FALSE, false)]
        .into_iter()
        .filter(move |(_, value)| !query.is_empty() && spelling(*value).starts_with(&query));
    match request.scope {
        CompletionScope::Literal => Some(
            spelled
                .map(|(_, value)| {
                    crate::libraries::completion::make(
                        spelling(value),
                        Some(vocabulary::BOOL.into()),
                        ::grap::logic::value(value),
                        Some(crate::selection::at(&[], crate::selection::payload::edge())),
                    )
                })
                .collect(),
        ),
        CompletionScope::Everything => Some(
            spelled
                .map(|(constant, _)| {
                    crate::libraries::completion::select(constant, constant.into()).with_detail(ID)
                })
                .collect(),
        ),
        CompletionScope::Start => None,
    }
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "logic",
        [
            (vocabulary::BOOL, "bool"),
            (vocabulary::REQUIRE, "require"),
            (vocabulary::CONDITION, "condition"),
            (vocabulary::NOT_BOOL, "not a boolean"),
            (vocabulary::CONDITION_NOT_MET, "condition not met"),
        ],
        functions(),
    )
    .with_constants([
        (vocabulary::TRUE, spelling(true), ::grap::logic::value(true)),
        (
            vocabulary::FALSE,
            spelling(false),
            ::grap::logic::value(false),
        ),
    ])
    .with_absents([vocabulary::NOT_BOOL, vocabulary::CONDITION_NOT_MET])
    .with_kinds([(vocabulary::BOOL, vec![isa::vocabulary::BYTES])])
    .with_projections_for([(vocabulary::BOOL, crate::display::runtime_partial(display))])
    .with_completions(completions)
}

#[cfg(test)]
#[path = "tests/logic.rs"]
mod tests;
