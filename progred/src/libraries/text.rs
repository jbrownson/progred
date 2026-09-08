//! A UTF-8 text convention over ordinary GID data. Text is a
//! positively recognized record facet, not a GID-core atom.
//!
//! When a real string-function library lands here, funnel argument
//! reads and result construction through shared helpers, the way f64
//! routes through `binary`/`unary` — then revisit a construction-side
//! `Rc<str>` runtime representation (reads already borrow cheaply, so
//! unlike f64 no literal decoding or registration is needed) once
//! string construction runs in loops and a profile can weigh it.

use crate::libraries::{Library, line_edit, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xeaaf309c36a65d2811083944da29aec9);
use crate::display::{Layout, ProjectionInput, overlay_value};
use ::grap::{ForeignFunction, ForeignFunctions};

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

pub fn edit(spelling: &str, current: Option<&Value>) -> Option<Value> {
    Some(
        current
            .map(|current| overlay_value(current, value(spelling)))
            .unwrap_or_else(|| value(spelling)),
    )
}

pub fn completion(spelling: &str) -> crate::display::Completion {
    crate::libraries::completion::select(crate::display::Completion::new(
        format!("\"{spelling}\""),
        value(spelling),
    ))
}

pub fn query_spelling(query: &str) -> &str {
    query
        .trim()
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(query)
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
            Ok(read(&input)
                .and_then(|text| edit(text, current.as_ref()))
                .unwrap_or_else(crate::libraries::absent::value))
        }),
    )
}

pub fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    editor(input.value?).map(crate::display::line_edit)
}

pub(crate) fn editor(value: &Value) -> Option<crate::display::LineEdit> {
    Some(line_edit::description(
        read(value)?,
        None::<String>,
        line_edit::native(edit),
        "\"",
        "\"",
        crate::display::TextFamily::SystemUi,
    ))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UTF8, name::record("utf8", []));
    cells.set_value(vocabulary::UPDATE, name::record("text update", []));
    Library::named(
        ID,
        "text",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(display),
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
        let written = crate::libraries::test_evaluate(
            &::grap::call(
                ::grap::ffi(vocabulary::UPDATE),
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
        let target = |_| crate::display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            select_with: std::rc::Rc::new(|_, _| false),
            hover: crate::libraries::test_widgets::hover(vec![]),
        };
        let display = (library().projection)(&ProjectionInput {
            default_projection: crate::display::partial(|_| None),
            env: &NoEval,
            value: Some(&value("hi")),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: crate::display::ProjectionTargets::new(&target),
        })
        .expect("text projection");
        assert!(matches!(
            crate::display::recording::record(&display),
            crate::display::recording::Recorded::Widget(_)
        ));
        let line = editor(&value("hi")).unwrap();
        assert_eq!(line.text, "hi");
        assert_eq!(line.prefix, "\"");
        assert_eq!(line.suffix, "\"");
    }

    struct NoEval;

    impl crate::display::Env for NoEval {
        fn apply_scoped(
            &self,
            _: &gid::Value,
            _: &[(gid::CellId, gid::Value)],
            _scope: Option<&::grap::ForeignOverlay<'_>>,
        ) -> ::grap::Evaluation {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("the Rust line editor does not evaluate while projecting")
        }
    }

    #[test]
    fn the_library_owns_its_vocabulary_and_projection() {
        let library = library();
        assert_eq!(
            library.value(vocabulary::UTF8).and_then(name::read),
            Some("utf8")
        );
    }
}
