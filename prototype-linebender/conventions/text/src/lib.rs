//! A UTF-8 text convention over ordinary graph data. Text is a
//! positively recognized record facet, not a graph-core atom.

use progred_display::{Env, Layout, LineEdit, editable_line, overlay};
use progred_graph::Value;

pub mod vocabulary {
    use progred_graph::CellId;

    pub const UTF8: CellId = CellId::from_u128(0x332529b8ea83a7ba10fd7f6d942e5016);
}

pub fn value(text: impl Into<String>) -> Value {
    Value::record([(
        vocabulary::UTF8,
        Value::from(text.into().into_bytes()),
    )])
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

pub fn display(_: &dyn Env, value: &Value) -> Option<Layout> {
    line(value).map(editable_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_graph::new_cell_id;

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

    impl Env for Unused {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (Value::record([]), 0)
        }
    }

    #[test]
    fn display_is_an_editable_line() {
        use progred_display::Click;
        assert!(matches!(
            display(&Unused, &value("hi")),
            Some(Layout::OnClick {
                click: Click::Line(line),
                ..
            }) if line.text == "hi"
        ));
    }
}
