use crate::libraries::{f64, layout as layout_data};
use crate::projection::test_support::place;
use gid::{CellId, Cells, Document, Value};
use puri::draw::DrawCmd;
#[test]
fn failed_toolpath_preview_discards_the_model_and_partial_paths() {
    use crate::libraries::{absent, color, control, f32, fidget, presentation, toolpath};
    use toolpath::vocabulary as t;
    let point = |function: CellId, x| {
        grap::call(
            function.into(),
            [
                (t::X, f64::value(x)),
                (t::Y, f64::value(0.0)),
                (t::Z, f64::value(0.0)),
            ],
        )
    };
    for preview in [t::PREVIEW_3D, t::PREVIEW_MESH] {
        for (result, fuel, should_draw) in [
            (Value::record([]), 1000, true),
            (absent::with_reason(t::INVALID_INPUT), 1000, false),
            (Value::record([]), 0, false),
        ] {
            let program = Value::record([
                (grap::vocabulary::PARAMS, Value::list([])),
                (
                    grap::vocabulary::BODY,
                    grap::call(
                        control::vocabulary::DO.into(),
                        [(
                            control::vocabulary::EXPRESSIONS,
                            Value::list([point(t::START_AT, 0.0), point(t::LINE_TO, 1.0), result]),
                        )],
                    ),
                ),
            ]);
            let expression = grap::call(
                preview.into(),
                [
                    (presentation::vocabulary::VALUE, f32::value(-1.0)),
                    (t::PROGRAM, program),
                    (t::LINE_RADIUS, f64::value(0.01)),
                    (
                        fidget::vocabulary::COLOR,
                        Value::record([(color::vocabulary::RGB, vec![255, 128, 0].into())]),
                    ),
                    (layout_data::vocabulary::WIDTH, f64::value(32.0)),
                    (layout_data::vocabulary::HEIGHT, f64::value(32.0)),
                    (layout_data::vocabulary::FUEL, f64::value(fuel as f64)),
                ],
            );
            let stack = crate::stack::load();
            let result = grap::evaluate_value(&expression, &stack.libraries, 1000);
            assert!(result.completed && !absent::is_absent(&result.result));
            let doc = Document {
                root: Some(result.result),
                cells: Cells::new(),
            };
            let (bench, _) = place(&doc, None, 200.0);
            assert_eq!(
                bench
                    .list
                    .0
                    .iter()
                    .any(|cmd| matches!(cmd, DrawCmd::Image { .. } | DrawCmd::Mesh { .. })),
                should_draw
            );
        }
    }
}
