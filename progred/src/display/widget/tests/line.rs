use super::*;
use kurbo::Point;

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
                        &Rc::new(LineEdit {
                            text: "read only".into(),
                            update: Rc::new(|_, _| panic!("read-only line cannot write")),
                            prefix: String::new(),
                            suffix: String::new(),
                            family: TextFamily::default(),
                            placeholder: None,
                        }),
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
    let path = vec![key("style"), gid::Step::Follow, key("color")];
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

#[test]
fn starting_to_type_a_missing_name_moves_nothing_after_it() {
    use crate::driver::Driver;
    use crate::libraries::def::vocabulary::{DEFS, LIBRARY};
    use gid::Step;
    let mut driver = Driver::of(::grap::isa::record(LIBRARY, [(DEFS, gid::Value::list([]))]));
    let name = [Step::Key(crate::libraries::name::vocabulary::NAME)];
    let [before, after] = [false, true].map(|editing| {
        if editing {
            driver.select(&name);
        }
        [driver.rect(&name), driver.rect(&[Step::Key(DEFS)])]
    });
    assert_eq!(before, after, "the caret goes over the placeholder");
}

#[test]
fn selecting_an_unnamed_lambdas_name_moves_nothing_after_it() {
    use crate::driver::Driver;
    use gid::Step;
    let parameter = gid::new_cell_id();
    let mut cells = gid::Cells::new();
    cells.set_value(parameter, crate::libraries::name::record("y", []));
    let mut driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(::grap::lambda([parameter], parameter.into())),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let params = [Step::Key(::grap::vocabulary::PARAMS)];
    let before = driver.rect(&params);
    // Its λ is the name's placeholder, which the caret goes over.
    driver.keystroke(
        ui_events::keyboard::Key::Named(ui_events::keyboard::NamedKey::Tab),
        ui_events::keyboard::Modifiers::empty(),
    );
    assert_eq!(
        driver.selection(),
        crate::occurrence::of(&[Step::Key(crate::libraries::name::vocabulary::NAME)])
    );
    assert_eq!(driver.rect(&params), before);
}
