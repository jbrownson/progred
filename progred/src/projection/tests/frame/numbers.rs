use super::*;
use crate::libraries::{f32, number, u64};

pub(super) fn calls() -> Vec<(CellId, CellId, Value)> {
    [
        (f64::vocabulary::F64, f64::vocabulary::SUM, f64::value(1.0)),
        (f32::vocabulary::F32, f32::vocabulary::SUM, f32::value(2.0)),
        (u64::vocabulary::U64, u64::vocabulary::LESS, u64::value(3)),
    ]
    .into_iter()
    .map(|(representation, function, value)| {
        (
            representation,
            function,
            grap::call(
                function.into(),
                [
                    (number::vocabulary::LEFT, value.clone()),
                    (number::vocabulary::RIGHT, value),
                ],
            ),
        )
    })
    .chain([(
        f64::vocabulary::F64,
        f64::vocabulary::SIN,
        grap::call(
            f64::vocabulary::SIN.into(),
            [(number::vocabulary::OPERAND, f64::value(0.5))],
        ),
    )])
    .collect()
}

#[test]
fn numeric_operation_labels_preserve_function_hover_and_selection() {
    let function_path = [Step::Key(grap::vocabulary::FUNCTION)];
    for (_, _, call) in calls() {
        let doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let (bench, _) = place(&doc, None, 900.0);
        let head = bench
            .descends
            .iter()
            .find(|d| d.path.as_ref() == function_path)
            .unwrap();
        assert!(
            !bench
                .descends
                .iter()
                .any(|d| d.path.iter().any(|s| matches!(s, Step::Follow(_))))
        );
        for x in [head.rect.x0 + 0.5, head.rect.x1 - 0.5] {
            let (hovered, _) =
                place_with_pointer(&doc, None, 900.0, Some(Point::new(x, head.rect.center().y)));
            assert!(matches!(hovered.hit,
                Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found.as_ref() == function_path));
        }
        let mut editor = crate::test_editor(doc.clone());
        let original = editor.model.doc.clone();
        assert!((head.select)(&mut editor, None));
        assert_eq!(
            editor.model.selection.as_ref().unwrap().path(),
            function_path
        );
        assert!(Rc::ptr_eq(&editor.model.doc, &original));
    }
}

#[test]
fn numeric_operation_labels_read_both_names_from_the_graph() {
    let function_path = [Step::Key(grap::vocabulary::FUNCTION)];
    for (representation, function, call) in calls() {
        let mut doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let width = |doc: &Document| {
            let (bench, _) = place(doc, None, 900.0);
            bench
                .descends
                .iter()
                .find(|d| d.path.as_ref() == function_path)
                .unwrap()
                .rect
                .width()
        };
        let original = width(&doc);
        doc.cells.set_value(
            representation,
            name::record("a longer representation name", []),
        );
        let renamed_type = width(&doc);
        assert!(renamed_type > original);
        doc.cells
            .set_value(function, name::record("a longer operation name", []));
        assert!(width(&doc) > renamed_type);
    }
}

#[test]
fn numeric_call_decoration_does_not_hide_extra_arguments() {
    let extra = new_cell_id();
    for (_, _, call) in calls() {
        let call = Value::record(
            call.as_record()
                .unwrap()
                .clone()
                .update(extra, text::value("extra")),
        );
        let doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let (bench, _) = place(&doc, None, 900.0);
        for field in doc.root.as_ref().unwrap().as_record().unwrap().keys() {
            assert!(
                bench
                    .descends
                    .iter()
                    .any(|d| d.path.as_ref() == [Step::Key(*field)])
            );
        }
    }
}
