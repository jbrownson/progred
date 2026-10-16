use super::*;

#[test]
fn read_only_line_needs_no_selection_or_editing_capabilities() {
    crate::display::test_support::with_context::<crate::Editor, crate::frame::Hovered, _>(
        &crate::display::test_support::NoProject,
        |context| {
            let mut cx = context.inputs.clone();
            cx.edits = cx.edits.detached(vec![]);
            let mut context = Context {
                inputs: &cx,
                project: context.project,
                path: context.path,
                value: context.value,
                text: &mut *context.text,
            };
            let measured = view(
                &mut context,
                LineEdit {
                    text: "read only".into(),
                    placeholder: None,
                    update: Rc::new(|_, _, _| panic!("read-only line cannot write")),
                    prefix: String::new(),
                    suffix: String::new(),
                    family: TextFamily::default(),
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
