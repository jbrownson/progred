//! A UTF-8 text convention over ordinary GID data. Text is a
//! positively recognized record facet, not a GID-core atom.

use crate::{Library, name};
use gid::{Cells, Value};
use progred_display::{Layout, LineEdit, ProjectionInput, editable_line, overlay};

pub mod vocabulary {
    use gid::CellId;

    pub const UTF8: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);
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

fn update(current: &Value, text: &str) -> Option<Value> {
    Some(overlay(current, value(text)))
}

pub fn line(value: &Value) -> Option<LineEdit> {
    read(value).map(|text| LineEdit {
        text: text.to_string(),
        update,
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
    Library {
        cells,
        projections: vec![display::<World, Hover>],
        ..Library::default()
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
        assert_eq!(
            (edit.update)(&enriched, "hi"),
            Some(Value::record(
                value("hi")
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(vec![1])),
            ))
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
