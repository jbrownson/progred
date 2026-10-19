use super::*;
use crate::libraries::{f64, text};

use crate::selection::Selection;
use crate::test_support::{core_libraries, src};
use gid::{Document, new_cell_id};
use peniko::Color;

#[test]
fn native_handler_captures_the_current_line_description() {
    let update = native(|spelling, _| Some(crate::libraries::text::value(spelling)));
    let line = description(
        "42",
        None::<String>,
        update.clone(),
        "(",
        ")",
        TextFamily::SystemUi,
    );
    assert_eq!(line.text, "42");
    assert_eq!(line.placeholder, None);
    assert!(Rc::ptr_eq(&line.update, &update));
    assert_eq!(line.prefix, "(");
    assert_eq!(line.suffix, ")");
    assert_eq!(line.family, TextFamily::SystemUi);
}

#[test]
fn layout_can_request_a_monospace_editor() {
    let line = description(
        "b4e0fe",
        None::<String>,
        native(|_, _| None),
        "#",
        "",
        TextFamily::Monospace,
    );
    assert_eq!(line.family, TextFamily::Monospace);
}

#[test]
fn layout_exposes_a_placeholder() {
    let line = description(
        "",
        Some("λ"),
        native(|_, _| None),
        "",
        "",
        TextFamily::SystemUi,
    );
    assert_eq!(line.placeholder.as_deref(), Some("λ"));
}

#[test]
fn native_line_conversions_agree_with_their_grap_entry_points() {
    use crate::libraries::{blob, color, f32, line_edit, number, u64};

    let libraries = core_libraries();
    let metadata = new_cell_id();
    for (current, function, native) in [
        (
            text::value("old"),
            text::vocabulary::UPDATE,
            line_edit::native(text::edit),
        ),
        (
            Value::from(vec![0xab]),
            blob::vocabulary::UPDATE,
            line_edit::native(blob::edit),
        ),
        (
            f32::value(1.0),
            f32::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, f32::value)),
        ),
        (
            f64::value(1.0),
            f64::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, f64::value)),
        ),
        (
            u64::value(1),
            u64::vocabulary::UPDATE,
            line_edit::native(|s, c| number::edit(s, c, u64::value)),
        ),
        (
            color::value(Color::new([0.2, 0.4, 0.6, 1.0])),
            color::vocabulary::UPDATE,
            line_edit::native(color::edit),
        ),
    ] {
        let current = match current {
            Value::Record(mut fields) => {
                fields.insert(metadata, text::value("preserve me"));
                Value::Record(fields)
            }
            value => value,
        };
        let doc = Document {
            root: Some(current.clone()),
            cells: Cells::new(),
        };
        let grap = line_edit::grap(grap::ffi(function));
        let sources = src(&doc, &libraries);
        for spelling in ["", "invalid", "1.5", "12", "-", "0", "ff", "123456"] {
            assert_eq!(
                native(&sources, spelling, Some(&current)),
                grap(&sources, spelling, Some(&current)),
                "conversion {function} at {spelling:?}",
            );
        }
    }
}

#[test]
fn custom_update_can_bind_an_absent_and_return_a_value() {
    use crate::libraries::{control::vocabulary as c, line_edit::vocabulary as l};
    let missing = new_cell_id();
    let update = grap::lambda(
        [l::INPUT, l::CURRENT],
        grap::call(
            Value::from(c::LET),
            [
                (
                    c::BINDINGS,
                    Value::list([Value::record([
                        (c::BIND, Value::from(new_cell_id())),
                        (c::VALUE, Value::from(missing)),
                    ])]),
                ),
                (grap::vocabulary::EXPRESSION, Value::from(l::INPUT)),
            ],
        ),
    );
    let libraries = core_libraries();
    let mut doc = Rc::new(Document {
        root: Some(text::value("before")),
        cells: Cells::new(),
    });
    let function = grap::evaluate_value(&update, &src(&doc, &libraries), 1000).result;
    let evaluated = grap::apply_value(
        &function,
        [
            (l::INPUT, text::value("after")),
            (l::CURRENT, doc.root.clone().unwrap()),
        ],
        &src(&doc, &libraries),
        None,
        1000,
    );
    assert_eq!(text::read(&evaluated.result), Some("after"));
    let line = crate::display::LineEdit {
        text: "after".into(),
        placeholder: None,
        update: crate::libraries::line_edit::grap(function),
        prefix: String::new(),
        suffix: String::new(),
        family: Default::default(),
    };
    let mut selected = Selection::edge(&crate::test_root(), vec![]);
    selected.edit_line_mut(&line.text);
    assert!(crate::editing::commit_line(
        &mut doc,
        &libraries,
        &mut selected,
        &line.update,
    ));
    assert_eq!(text::read(doc.root.as_ref().unwrap()), Some("after"));
}
