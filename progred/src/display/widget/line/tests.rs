use super::*;

#[test]
fn read_only_line_needs_no_selection_or_editing_capabilities() {
    crate::display::test_support::with_empty_view::<crate::Editor, crate::frame::Hovered, _>(
        &crate::display::test_support::NoProject,
        |cx, text, project| {
            let measured = crate::projection::test_support::at_path(
                cx,
                text,
                project,
                &[crate::occurrence::Step::Computed],
                None,
                |context| {
                    view(
                        context,
                        LineEdit {
                            text: "read only".into(),
                            update: Rc::new(|_, _| panic!("read-only line cannot write")),
                            prefix: String::new(),
                            suffix: String::new(),
                            family: TextFamily::default(),
                        },
                    )
                },
            );
            let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
            let output =
                crate::display::widget::frame::place(measured, placement, &Default::default());
            assert!(output.handler.is_none());
            assert!(output.claim.is_none());
            assert!(output.landmark_select.is_none());
            assert_eq!(output.after_hover.len(), 1);
        },
    );
}

#[test]
fn sample_text_line_click_mounts_its_own_editor() {
    use crate::test_values::key;
    let doc = crate::sample::sample_document();
    let mut driver = crate::driver::Driver::new(
        crate::test_editor(doc),
        // Tall enough to show the style's color without scrolling.
        kurbo::Size::new(900.0, 1200.0),
    );
    let path = vec![
        key("style"),
        gid::Step::Follow(gid::Resolution::Document),
        key("color"),
    ];
    driver.click(&path);
    assert_eq!(driver.selection(), path);

    // The next frame's focused editor still owns pointer-down: a double
    // click selects its word, and a later single click can collapse that
    // selection to a new caret. This is distinct from the first click
    // above, which mounts the editor.
    let line = driver.rect(&path);
    driver.clicks_at(line.center(), 2);
    let (anchor, focus) = driver.line_selection().unwrap();
    assert_ne!(anchor, focus);
    driver.click_at(Point::new(line.x0 + 1.0, line.center().y));
    let (anchor, focus) = driver.line_selection().unwrap();
    assert_eq!(anchor, focus);
}
