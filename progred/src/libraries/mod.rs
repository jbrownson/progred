//! Progred's built-in libraries. Each module owns one conceptual
//! library and exports its complete contribution to the editor, through
//! the interface in [`crate::library`].

pub use crate::library::Definitions;
/// A built-in library's contribution, over a world that hosts the toolkit.
pub type Library<W> = crate::library::Library<W, <W as crate::display::Host>::Hover>;
#[cfg(test)]
use ::grap::{Definition, ForeignFunctions};

pub mod absent;
pub mod blob;
pub mod color;
pub mod completion;
#[cfg(test)]
mod conformance;
pub mod control;
pub mod controls;
pub mod conversion;
pub mod f32;
pub mod f64;
pub mod fidget;
pub mod geometry;
pub mod grap;
pub mod layout;
pub mod line_edit;
pub mod list;
pub mod logic;
pub mod name;
pub mod number;
pub mod path;
pub mod png;
pub mod presentation;
pub mod random;
pub mod representation;
pub mod selection;
pub mod sequence;
pub mod site;
pub mod text;
pub mod toolpath;
pub mod tree;
pub mod u64;
pub mod workspace;

#[cfg(test)]
pub(crate) struct TestHost<F>(pub F);

#[cfg(test)]
impl<F: Fn(gid::CellId) -> Vec<(gid::Resolution, ::grap::Definition)>> ::grap::Host
    for TestHost<F>
{
    fn resolve(&self, cell: gid::CellId) -> Option<(gid::Resolution, ::grap::Definition)> {
        (self.0)(cell).into_iter().next()
    }
}

#[cfg(test)]
fn test_host<'a>(
    resolve: impl Fn(gid::CellId) -> Option<gid::Value> + 'a,
    foreign: &'a ForeignFunctions,
) -> impl ::grap::Host + 'a {
    TestHost(move |cell| {
        match (resolve(cell), foreign.get(cell)) {
            (value, Some(function)) => Some(Definition::foreign(
                value.unwrap_or_else(|| gid::Value::record([])),
                function.clone(),
            )),
            (Some(value), None) => Some(Definition::Value(value)),
            (None, None) => None,
        }
        .map(|definition| (gid::Resolution::Document, definition))
        .into_iter()
        .collect()
    })
}

#[cfg(test)]
pub(crate) fn test_evaluate(
    expression: &gid::Value,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> ::grap::Evaluation<gid::Value> {
    ::grap::evaluate_value(expression, &test_host(resolve, foreign), fuel)
}

#[cfg(test)]
pub(crate) fn test_apply(
    function: &gid::Value,
    arguments: impl IntoIterator<Item = (gid::CellId, gid::Value)>,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> ::grap::Evaluation<gid::Value> {
    ::grap::apply_value(function, arguments, &test_host(resolve, foreign), fuel)
}

#[cfg(test)]
pub(crate) mod test_widgets;
