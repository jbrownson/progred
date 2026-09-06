//! A UTF-8 text convention over ordinary GID data. Text is a
//! positively recognized record facet, not a GID-core atom.
//!
//! When a real string-function library lands here, funnel argument
//! reads and result construction through shared helpers, the way f64
//! routes through `binary`/`unary` — then revisit a construction-side
//! `Rc<str>` runtime representation (reads already borrow cheaply, so
//! unlike f64 no literal decoding or registration is needed) once
//! string construction runs in loops and a profile can weigh it.

use crate::{Library, line_edit, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xeaaf309c36a65d2811083944da29aec9);
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
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context
                .field(call, line_edit::vocabulary::CURRENT)
                .map(|current| context.eval(current, environment))
                .transpose()?;
            let input = context.eval(input, environment)?;
            Ok(match read(&input) {
                Some(text) => current
                    .as_ref()
                    .map(|current| overlay_value(current, value(text)))
                    .unwrap_or_else(|| value(text)),
                None => crate::absent::value(),
            })
        }),
    )
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let content = read(input.value)?;
    Some(line_edit::layout(
        content,
        grap_runtime::ffi(vocabulary::UPDATE),
        "\"",
        "\"",
    ))
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UTF8, name::record("utf8", []));
    cells.set_value(vocabulary::UPDATE, name::record("text update", []));
    Library::named(
        ID,
        "text",
        crate::Definitions::from_parts(cells, functions()),
        progred_display::partial(display::<World, Hover>),
    )
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
        let written = crate::test_evaluate(
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

    #[test]
    fn display_is_an_editable_line() {
        let target = |_| progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: (),
        };
        let display = (library::<(), ()>().projection)(&ProjectionInput {
            env: &NoEval,
            value: &value("hi"),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&target),
        })
        .expect("text projection");
        let progred_display::Layout::LineEdit(line) = display else {
            panic!("text projects directly to a line editor")
        };
        assert_eq!(line.text, "hi");
        assert_eq!(line.prefix, "\"");
        assert_eq!(line.suffix, "\"");
    }

    struct NoEval;

    impl progred_display::Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("the Rust line editor does not evaluate while projecting")
        }
    }

    #[test]
    fn the_library_owns_its_vocabulary_and_projection() {
        let library = library::<(), ()>();
        assert_eq!(
            library.value(vocabulary::UTF8).and_then(name::read),
            Some("utf8")
        );
    }
}
