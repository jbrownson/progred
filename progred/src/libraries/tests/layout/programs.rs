//! Layouts a Grap program emits through the scoped layout functions, against
//! the same layouts built as data.

use crate::annotations::Annotations;
use crate::frame::test_support::ProfileView;
use crate::libraries::{grap::control::vocabulary as control, layout, presentation, text};
use crate::perf::{distribution, iterations};
use crate::projection::test_support::BenchContext;
use crate::sources::Sources;
use crate::test_support::core_libraries;
use crate::test_values::call;
use gid::{CellId, Cells, Document, Value};
use layout::vocabulary as l;
use puri::draw::DrawCmd;
use std::time::Instant;

const ITEM: CellId = CellId::from_u128(9001);
const LABEL: CellId = CellId::from_u128(9002);

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
                (l::GAP, crate::libraries::f64::value(6.0)),
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
        quote(layout::test_support::row(
            6.0,
            [
                layout::test_support::layout_node(
                    l::kind::TEXT,
                    [
                        (l::PAINT, l::NAME_FACE.into()),
                        (
                            l::CONTENT,
                            Value::record([
                                (
                                    ::grap::isa::ISA,
                                    gid::Value::from(control::kind::UNQUOTE).into(),
                                ),
                                (control::UNQUOTE, LABEL.into()),
                            ]),
                        ),
                    ],
                ),
                layout::test_support::text_leaf("value", l::STRING_FACE),
            ],
        ))
    };
    let calls = (0..rows).map(|index| call(ITEM, [(LABEL, text::value(format!("field {index}")))]));
    let body = if emitting {
        let col = call(
            l::COL,
            [
                (l::GAP, crate::libraries::f64::value(2.0)),
                (l::CHILDREN, sequence(calls)),
            ],
        );
        let layout = if bordered {
            call(l::BORDER, [(l::CHILD, col)])
        } else {
            col
        };
        call(l::LAYOUT_PROGRAM, [(grap::vocabulary::EXPRESSION, layout)])
    } else {
        let col = layout::test_support::col(
            0,
            2.0,
            calls.map(|call| {
                Value::record([
                    (
                        ::grap::isa::ISA,
                        gid::Value::from(control::kind::UNQUOTE).into(),
                    ),
                    (control::UNQUOTE, call),
                ])
            }),
        );
        quote(if bordered {
            layout::test_support::bordered(col)
        } else {
            col
        })
    };
    let projection = grap::lambda([presentation::vocabulary::VALUE], body);
    let mut cells = Cells::new();
    cells.set_value(ITEM, grap::lambda([LABEL], item));
    Document {
        cells,
        root: Some(presentation::projected(Value::record([]), projection)),
    }
}

/// A window tall enough for every row.
fn view() -> ProfileView {
    ProfileView {
        size: kurbo::Size::new(1400.0, 10000.0),
        scale: 1.0,
        root: None,
    }
}

/// A bench whose root is drawn as a pane's, presenting the document's value
/// through its own projection field, or a render as its result.
fn presenting() -> BenchContext {
    let mut context = BenchContext::new();
    context.stack.projection = context.stack.pane_projection.clone();
    context
}

#[test]
fn scoped_layout_program_matches_value_layout_through_the_real_frame_with_and_without_a_border() {
    for bordered in [false, true] {
        let (view, mut context) = (view(), presenting());
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
fn returned_layout_program_uses_its_own_evaluation_allowance() {
    let doc = document(true, 100, false);
    let libraries = core_libraries();
    let sources = Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let function = doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&presentation::vocabulary::PROJECTION)
        .unwrap();
    let prepared = grap::Evaluator::new(&sources, 100_000).apply_expression(
        &function.into(),
        [(presentation::vocabulary::VALUE, Value::record([]).into())],
    );
    assert_eq!(prepared.halt, None);
    let function = prepared.result.field(l::PROGRAM).unwrap();
    for fuel in [crate::sources::DEFAULT_FUEL, 100_000] {
        let (evaluation, layout) = layout::scope::run::<crate::Editor>(
            || panic!("text-only layout needs no editing site"),
            |scope| {
                grap::Evaluator::new(&sources, fuel)
                    .functions(scope)
                    .apply(&function, [])
            },
        );
        if fuel == crate::sources::DEFAULT_FUEL {
            assert!(evaluation.halt.is_some());
            assert_eq!(
                grap::absent::reason(&evaluation.result.to_value()),
                Some(grap::absent::FUEL_EXHAUSTED)
            );
            assert!(layout.is_none());
        } else {
            assert_eq!(evaluation.halt, None);
            assert!(layout.is_some());
        }
    }
}

#[test]
#[ignore = "Grap layout descriptions versus layout-emitting FFIs"]
fn grap_layout_ffi_profile_loop() {
    // The returned program has the ordinary fresh evaluation allowance,
    // independent of the explicit budget used to construct it below.
    const ROWS: usize = 20;
    let docs = [false, true].map(|emitting| {
        let mut doc = document(emitting, ROWS, false);
        let function = doc
            .root
            .as_ref()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&presentation::vocabulary::PROJECTION)
            .unwrap()
            .clone();
        doc.root = Some(::grap::isa::record(
            presentation::vocabulary::kind::RENDER,
            [
                (
                    grap::vocabulary::EXPRESSION,
                    grap::call(
                        function,
                        [(presentation::vocabulary::VALUE, Value::record([]))],
                    ),
                ),
                (l::FUEL, crate::libraries::f64::value(100_000.0)),
            ],
        ));
        doc
    });
    // The root is a render, which a pane draws as its result.
    let (view, mut context) = (view(), presenting());
    let annotations = Annotations::default();
    let count = iterations();
    let mut samples = [Vec::new(), Vec::new()];
    for index in 0..count + 5 {
        // Reverse order each pair to reduce thermal/order bias.
        for variant in [index % 2, 1 - index % 2] {
            let start = Instant::now();
            let (bench, _) = context.frame(view.frame(&docs[variant], &annotations));
            let build = start.elapsed();
            assert_eq!(
                bench
                    .list
                    .0
                    .iter()
                    .filter(|command| matches!(command, DrawCmd::GlyphRun(_)))
                    .count(),
                ROWS * 2,
                "variant {variant}"
            );
            let prepare = bench.times.prepare;
            let cleanup = Instant::now();
            drop(bench);
            let disposal = cleanup.elapsed();
            if index >= 5 {
                samples[variant].push((build + disposal, prepare));
            }
        }
    }
    for ((label, metric), timings) in [
        ("GID layout construction + decoding", "gid"),
        ("Scoped layout FFIs", "ffi"),
    ]
    .into_iter()
    .zip(samples)
    {
        eprintln!("{label}; {ROWS} two-text rows, {count} interleaved warm frames");
        distribution("frame + disposal", timings.iter().map(|(total, _)| *total));
        distribution("prepare", timings.iter().map(|(_, prepare)| *prepare));
        crate::perf::record(metric, timings.iter().map(|(total, _)| *total));
    }
}
