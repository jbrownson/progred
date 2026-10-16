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
    let display = (library().projection.unwrap())(&ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &NoEval,
        value: Some(&value("hi").into()),
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
    ) -> ::grap::Evaluation<gid::Value> {
        panic!("unexpected projection application")
    }

    fn evaluate(&self, _: &Value) -> Value {
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
