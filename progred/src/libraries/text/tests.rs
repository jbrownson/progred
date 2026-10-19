use super::*;
use crate::display::test_support::NoEval;
use crate::libraries::test_widgets::projection_input;
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
    let display = (library().projection.unwrap())(&projection_input(&NoEval, &value("hi").into()))
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

#[test]
fn the_library_owns_its_vocabulary_and_projection() {
    let library = library::<crate::Editor>();
    assert_eq!(
        library
            .definitions
            .value(vocabulary::UTF8)
            .and_then(name::read),
        Some("utf8")
    );
}
