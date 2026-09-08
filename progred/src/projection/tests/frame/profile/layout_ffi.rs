use super::*;
use layout::vocabulary as l;
use progred_libraries::{control::vocabulary as control, layout, presentation, text};

const ITEM: CellId = CellId::from_u128(9001);
const LABEL: CellId = CellId::from_u128(9002);

fn call(function: CellId, arguments: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    grap::call(function.into(), arguments)
}

fn quote(value: Value) -> Value {
    call(control::QUOTE, [(grap::vocabulary::EXPRESSION, value)])
}

fn sequence(expressions: impl IntoIterator<Item = Value>) -> Value {
    call(
        control::DO,
        [(control::EXPRESSIONS, Value::list(expressions))],
    )
}

fn emitted_text(content: Value, face: CellId) -> Value {
    call(
        l::TEXT,
        [(l::CONTENT, content), (l::PAINT, quote(face.into()))],
    )
}

fn document(emitting: bool, rows: usize, bordered: bool) -> Document {
    let item = if emitting {
        call(
            l::ROW,
            [
                (l::GAP, progred_libraries::f64::value(6.0)),
                (
                    l::CHILDREN,
                    sequence([
                        emitted_text(LABEL.into(), l::NAME_FACE),
                        emitted_text(text::value("value"), l::STRING_FACE),
                    ]),
                ),
            ],
        )
    } else {
        quote(layout::row(
            6.0,
            [
                Value::record([(
                    l::TEXT,
                    Value::record([
                        (l::PAINT, l::NAME_FACE.into()),
                        (
                            l::CONTENT,
                            Value::record([(control::UNQUOTE, LABEL.into())]),
                        ),
                    ]),
                )]),
                layout::text_leaf("value", l::STRING_FACE),
            ],
        ))
    };
    let calls = (0..rows).map(|index| call(ITEM, [(LABEL, text::value(format!("field {index}")))]));
    let body = if emitting {
        call(
            l::LAYOUT_PROGRAM,
            [(
                grap::vocabulary::EXPRESSION,
                call(
                    l::COL,
                    [
                        (l::GAP, progred_libraries::f64::value(2.0)),
                        (l::CHILDREN, sequence(calls)),
                    ],
                ),
            )],
        )
    } else {
        quote(layout::col(
            0,
            2.0,
            calls.map(|call| Value::record([(control::UNQUOTE, call)])),
        ))
    };
    let projection = grap::lambda([presentation::vocabulary::VALUE], body);
    let projection = if bordered {
        call(
            l::BORDER,
            [(presentation::vocabulary::PROJECTION, projection)],
        )
    } else {
        projection
    };
    let mut cells = Cells::new();
    cells.set_value(ITEM, grap::lambda([LABEL], item));
    Document {
        cells,
        root: Some(Value::record([
            (presentation::vocabulary::VALUE, Value::record([])),
            (presentation::vocabulary::PROJECTION, projection),
        ])),
    }
}

fn setup() -> (ProfileView, BenchContext) {
    let mut context = BenchContext::new();
    context.stack.projection = context
        .stack
        .projection
        .with_entry(progred_display::partial(presentation::projected_display));
    (
        ProfileView {
            size: kurbo::Size::new(1400.0, 10000.0),
            scale: 1.0,
            root: None,
        },
        context,
    )
}

#[test]
fn scoped_layout_program_matches_value_layout_through_the_real_frame_and_border_combinator() {
    for bordered in [false, true] {
        let (view, mut context) = setup();
        let annotations = Annotations::default();
        let (data, data_extent) =
            context.frame(view.frame(&document(false, 6, bordered), &annotations));
        let (native, native_extent) =
            context.frame(view.frame(&document(true, 6, bordered), &annotations));
        assert_eq!(native_extent, data_extent);
        assert_eq!(format!("{:?}", native.list.0), format!("{:?}", data.list.0));
        assert!(native.list.0.len() >= 12);
    }
}

#[test]
#[ignore]
fn grap_layout_ffi_profile_loop() {
    let docs = [false, true].map(|emitting| {
        let mut doc = document(emitting, 100, false);
        let function = doc
            .root
            .as_ref()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&presentation::vocabulary::PROJECTION)
            .unwrap()
            .clone();
        doc.root = Some(Value::record([(
            presentation::vocabulary::RENDER,
            Value::record([
                (
                    grap::vocabulary::EXPRESSION,
                    grap::call(
                        function,
                        [(presentation::vocabulary::VALUE, Value::record([]))],
                    ),
                ),
                (l::FUEL, progred_libraries::f64::value(100_000.0)),
            ]),
        )]));
        doc
    });
    let (view, mut context) = setup();
    context.stack.projection = context.stack.projection.without_entry();
    let annotations = Annotations::default();
    let count = iterations();
    let mut samples = [Vec::new(), Vec::new()];
    for index in 0..count + 5 {
        // Reverse order each pair to reduce thermal/order bias.
        for variant in [index % 2, 1 - index % 2] {
            let start = Instant::now();
            let (bench, _) = context.frame(view.frame(&docs[variant], &annotations));
            let build = start.elapsed();
            assert_eq!(bench.list.0.len(), 200);
            let phases = bench.times;
            let cleanup = Instant::now();
            drop(bench);
            let disposal = cleanup.elapsed();
            if index >= 5 {
                samples[variant].push(Timing {
                    total: build + disposal,
                    phases,
                    disposal,
                });
            }
        }
    }
    for (label, timings) in ["GID layout construction + decoding", "Scoped layout FFIs"]
        .into_iter()
        .zip(samples)
    {
        eprintln!("{label}; 100 two-text rows, {count} interleaved warm frames");
        distribution("frame + disposal", timings.iter().map(|t| t.total));
        distribution("prepare", timings.iter().map(|t| t.phases.prepare));
    }
}
