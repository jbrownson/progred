use super::*;

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
