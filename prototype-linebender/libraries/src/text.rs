//! A UTF-8 text convention over ordinary GID data. Text is a
//! positively recognized record facet, not a GID-core atom.

use crate::{Library, absent, layout, line_edit, name};
use gid::{Cells, Value};
use grap_runtime::{ForeignFunction, ForeignFunctions};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const UTF8: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);
    /// The text line's write-back rule: overlay the typed spelling
    /// onto the current record, other fields carried.
    pub const UPDATE: CellId = CellId::from_u128(0x27c58b96e1f4d03a8d17b62c94e05fa3);
}

pub fn value(text: impl Into<String>) -> Value {
    Value::record([(vocabulary::UTF8, Value::from(text.into().into_bytes()))])
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::UTF8)?
        .as_blob()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::new(|context, call, environment| {
            let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
            };
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context.eval(current, environment)?;
            let input = context.eval(input, environment)?;
            Ok(match read(&input) {
                Some(text) => overlay_value(&current, value(text)),
                None => crate::absent::value(),
            })
        }),
    )
}

pub fn display<World, Hover: Clone>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let content = read(input.value)?;
    let expression = line_edit::call(
        value(content),
        grap_runtime::ffi(vocabulary::UPDATE),
        value("\""),
        value("\""),
        input.selection.cloned().unwrap_or_else(absent::value),
    );
    let (display, _) = input.env.evaluate(&expression);
    layout::decode(&display, &input.select, &input.hover)
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UTF8, name::record("utf8", []));
    cells.set_value(vocabulary::UPDATE, name::record("text update", []));
    Library {
        cells,
        functions: functions(),
        projections: vec![display::<World, Hover>],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn utf8_is_an_open_convention_over_bytes() {
        assert_eq!(read(&value("hello")), Some("hello"));
        assert_eq!(read(&Value::from(vec![0xff])), None);

        let extra = new_cell_id();
        let enriched = Value::record(
            value("hello")
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(vec![1])),
        );
        assert_eq!(read(&enriched), Some("hello"));
        let written = grap_runtime::evaluate(
            &grap_runtime::call(
                grap_runtime::ffi(vocabulary::UPDATE),
                [
                    (line_edit::vocabulary::CURRENT, enriched.clone()),
                    (line_edit::vocabulary::INPUT, value("hi")),
                ],
            ),
            |_| None,
            &functions(),
            100,
        );
        assert!(written.diagnostics.is_empty());
        assert_eq!(
            written.result,
            Value::record(
                value("hi")
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(vec![1])),
            )
        );
    }

    struct TestEnv {
        cells: Cells,
        functions: ForeignFunctions,
    }

    impl progred_display::Env for TestEnv {
        fn evaluate(&self, expression: &Value) -> (Value, usize) {
            let evaluation = grap_runtime::evaluate(
                expression,
                |cell| self.cells.value(cell).cloned(),
                &self.functions,
                500,
            );
            (evaluation.result, evaluation.remaining_fuel)
        }
    }

    fn env() -> TestEnv {
        let library = Library::<(), ()>::merge_all([
            name::library(),
            crate::control::library(),
            crate::selection::library(),
            crate::layout::library(),
            crate::line_edit::library(),
        ]);
        TestEnv {
            cells: library.cells,
            functions: library
                .functions
                .merge(crate::line_edit::test_geometry_functions()),
        }
    }

    #[test]
    fn display_is_an_editable_line() {
        let select = std::rc::Rc::new(|_: &mut ()| false);
        let display = display::<(), ()>(ProjectionInput {
                env: &env(),
                value: &value("hi"),
                selection: None,
                state: None,
                select: select.clone(),
                hover: (),
                targets: progred_display::ProjectionTargets::fixed(select, ()),
            })
            .expect("text projection");
        let Layout::OnEvent { child, .. } = display else {
            panic!("inactive editor installs pointer-down");
        };
        let Layout::OnHover { child, .. } = *child else {
            panic!("line editor claims hover");
        };
        assert!(matches!(*child, Layout::Overlay { .. }));
    }

    #[test]
    fn the_library_owns_its_vocabulary_and_projection() {
        let library = library::<(), ()>();
        assert_eq!(
            library.cells.value(vocabulary::UTF8).and_then(name::read),
            Some("utf8")
        );
        assert_eq!(library.projections.len(), 1);
    }
}
