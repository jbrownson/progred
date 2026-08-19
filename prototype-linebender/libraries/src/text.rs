//! A UTF-8 text convention over ordinary GID data. Text is a
//! positively recognized record facet, not a GID-core atom.

use crate::{Library, name};
use gid::{Cells, Value};
use grap_runtime::{ForeignFunction, ForeignFunctions};
use progred_display::{Layout, LineEdit, ProjectionInput, editable_line, line_update, overlay};

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
        ForeignFunction {
            call: |context, call, environment| {
                let Some(current) = context.field(call, line_update::CURRENT) else {
                    return Ok(context.missing_argument(line_update::CURRENT));
                };
                let Some(input) = context.field(call, line_update::INPUT) else {
                    return Ok(context.missing_argument(line_update::INPUT));
                };
                let current = context.eval(current, environment)?;
                let input = context.eval(input, environment)?;
                Ok(match read(&input) {
                    Some(text) => overlay(&current, value(text)),
                    None => crate::absent::value(),
                })
            },
        },
    )
}

pub fn line(value: &Value) -> Option<LineEdit> {
    read(value).map(|text| LineEdit {
        text: text.to_string(),
        update: grap_runtime::ffi(vocabulary::UPDATE),
        prefix: "\"".into(),
        suffix: "\"".into(),
    })
}

pub fn display<World, Hover>(
    input: ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    line(input.value).map(editable_line)
}

pub fn library<World, Hover>() -> Library<World, Hover> {
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
        let edit = line(&enriched).unwrap();
        assert_eq!(edit.text, "hello");
        assert_eq!(edit.prefix, "\"");
        let written = grap_runtime::evaluate(
            &grap_runtime::call(
                edit.update,
                [
                    (line_update::CURRENT, enriched.clone()),
                    (line_update::INPUT, value("hi")),
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

    struct Unused;

    impl progred_display::Env for Unused {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (Value::record([]), 0)
        }
    }

    #[test]
    fn display_is_an_editable_line() {
        assert!(matches!(
            display::<(), ()>(ProjectionInput {
                env: &Unused,
                value: &value("hi"),
                selection: None,
                state: None,
                select: std::rc::Rc::new(|_| false),
                hover: (),
            }),
            Some(Layout::Leaf(progred_display::Display::LineEdit(line))) if line.text == "hi"
        ));
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
