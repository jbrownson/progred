//! Progred's built-in libraries. Each module owns one conceptual
//! library and exports its complete contribution to the editor.

use gid::Cells;
use grap_runtime::ForeignFunctions;
use progred_display::Partial;

pub mod absent;
pub mod color;
pub mod control;
pub mod f64;
pub mod geometry;
pub mod grap;
pub mod layout;
pub mod line_edit;
pub mod list;
pub mod logic;
pub mod name;
pub mod presentation;
pub mod random;
pub mod selection;
pub mod site;
pub mod text;
pub mod u64;

pub struct Library<World, Hover> {
    pub cells: Cells,
    pub functions: ForeignFunctions,
    pub projections: Vec<Partial<World, Hover>>,
}

impl<World, Hover> Default for Library<World, Hover> {
    fn default() -> Self {
        Self {
            cells: Cells::new(),
            functions: ForeignFunctions::default(),
            projections: Vec::new(),
        }
    }
}

impl<World, Hover> Library<World, Hover> {
    pub fn merge(self, other: Self) -> Self {
        Self {
            cells: self.cells.merged(other.cells),
            functions: self.functions.merge(other.functions),
            projections: self
                .projections
                .into_iter()
                .chain(other.projections)
                .collect(),
        }
    }

    pub fn merge_all(libraries: impl IntoIterator<Item = Self>) -> Self {
        libraries.into_iter().fold(Self::default(), Self::merge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, Value};
    use grap_runtime::{Environment, Expression, ForeignFunction, Halt};
    use progred_display::{Env, Layout, ProjectionInput, text as text_layout};
    use puri::Leaf;
    use std::rc::Rc;

    const SHARED_CELL: CellId = CellId::from_u128(1);
    const SHARED_FUNCTION: CellId = CellId::from_u128(2);

    fn left_function(
        _: &mut grap_runtime::Context,
        _: Expression,
        _: &Environment,
    ) -> Result<Value, Halt> {
        Ok(Value::from(b"left".to_vec()))
    }

    fn right_function(
        _: &mut grap_runtime::Context,
        _: Expression,
        _: &Environment,
    ) -> Result<Value, Halt> {
        Ok(Value::from(b"right".to_vec()))
    }

    fn left_projection(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
        Some(text_layout("left"))
    }

    fn right_projection(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
        Some(text_layout("right"))
    }

    struct NoEval;

    impl Env for NoEval {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (Value::record([]), 0)
        }
    }

    #[test]
    fn library_is_the_product_of_its_three_monoids() {
        let mut left_cells = Cells::new();
        left_cells.set_value(SHARED_CELL, Value::from(b"left".to_vec()));
        let mut right_cells = Cells::new();
        right_cells.set_value(SHARED_CELL, Value::from(b"right".to_vec()));
        let merged = Library::merge_all([
            Library {
                cells: left_cells,
                functions: ForeignFunctions::default().register(
                    SHARED_FUNCTION,
                    ForeignFunction::new(left_function),
                ),
                projections: vec![left_projection],
            },
            Library {
                cells: right_cells,
                functions: ForeignFunctions::default().register(
                    SHARED_FUNCTION,
                    ForeignFunction::new(right_function),
                ),
                projections: vec![right_projection],
            },
        ]);

        assert_eq!(
            merged.cells.value(SHARED_CELL),
            Some(&Value::from(b"left".to_vec()))
        );
        assert_eq!(
            grap_runtime::evaluate(
                &grap_runtime::call(Value::from(SHARED_FUNCTION), []),
                |_| None,
                &merged.functions,
                10,
            )
            .result,
            Value::from(b"right".to_vec())
        );
        let value = Value::record([]);
        assert_eq!(
            merged
                .projections
                .iter()
                .map(|projection| {
                    let target = |_| progred_display::ProjectionTarget {
                        select: Rc::new(|_: &mut ()| false),
                        hover: (),
                    };
                    projection(&ProjectionInput {
                        env: &NoEval,
                        value: &value,
                        selection: None,
                        state: None,
                        targets: progred_display::ProjectionTargets::new(&target),
                    })
                    .and_then(|layout| match layout {
                        Layout::Leaf(Leaf::Text { text, .. }) => Some(text),
                        _ => None,
                    })
                    .unwrap()
                })
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
    }
}
