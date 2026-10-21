//! Documents projected through the frame harness and written as SVG, with a
//! few checks of what they draw.

use super::*;
use crate::editing::Read;
use crate::libraries::{f64, name};
use crate::projection::test_support::make_projected_selection;
use crate::sample::sample_document;
use crate::selection::{pending_edge, pending_value};
use crate::sources::Sources;
use crate::test_support::{core_libraries, src};
use crate::test_values::key;
use gid::{Cells, Step, Value, new_cell_id};
#[test]
fn svg_bench_renders_the_sample_projection() {
    let doc = sample_document();
    render(&doc, None, 900.0, "raw_projection.svg");
    render(&doc, None, 560.0, "raw_projection_narrow.svg");
    // The deep-fallback regime: hugging fails at most levels, so
    // layout cost blowing up when width is scarce shows as this test
    // slowing down.
    render(&doc, None, 320.0, "raw_projection_tight.svg");
}

#[test]
fn svg_bench_renders_the_grap_demo() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/grap-demo.gid"
    )))
    .expect("the Grap demo parses");
    render(&doc, None, 900.0, "grap_demo.svg");
    render(&doc, None, 560.0, "grap_demo_narrow.svg");
}

#[test]
fn svg_bench_renders_numeric_type_labels() {
    let mut cells = Cells::new();
    for label in ["radius", "offset", "count"] {
        cells.set_value(crate::test_values::label(label), name::record(label, []));
    }
    let doc = Document {
        root: Some(Value::record([
            (
                crate::test_values::label("radius"),
                crate::libraries::f32::value(24.5),
            ),
            (crate::test_values::label("offset"), f64::value(-2.75)),
            (
                crate::test_values::label("count"),
                crate::libraries::u64::value(12),
            ),
        ])),
        cells,
    };
    render(&doc, None, 400.0, "numeric_type_labels.svg");
    let selection = make_projected_selection(
        &doc,
        &core_libraries(),
        crate::occurrence::of(&[key("radius")]),
    );
    render(
        &doc,
        Some(&selection),
        400.0,
        "numeric_type_labels_editing.svg",
    );
}

#[test]
fn svg_bench_renders_numeric_operation_labels() {
    let doc = Document {
        root: Some(Value::list(
            crate::test_values::numeric_calls()
                .into_iter()
                .map(|(_, _, call)| call),
        )),
        cells: Cells::new(),
    };
    render(&doc, None, 440.0, "numeric_operation_labels.svg");
}

#[test]
fn svg_bench_renders_toolpath_source_and_preview() {
    use crate::libraries::presentation;
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    render(&doc, None, 760.0, "toolpaths_source.svg");
    let libraries = core_libraries();
    let sources = src(&doc, &libraries);
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let declaration = sources.resolve_path(&pane.path).unwrap();
    let (value, _) = presentation::viewport(declaration).unwrap();
    assert_eq!(value.as_cell(), Some(names["program_tree"]));
    let preview =
        presentation::tests::viewport_output(declaration, &sources, 700.0, 500.0).unwrap();
    assert!(
        preview
            .as_record()
            .unwrap()
            .contains_key(&crate::libraries::controls::vocabulary::WITH_CONTROLS)
    );
    let doc = Document {
        root: Some(preview),
        cells: doc.cells,
    };
    let (bench, _) = place(&doc, None, 760.0);
    let image = bench.image();
    let mut model = 0;
    let mut paths = 0;
    for pixel in image.data.as_ref().chunks_exact(4).filter(|p| p[3] > 0) {
        model += usize::from(pixel[2] > pixel[1] && pixel[1] > pixel[0]);
        paths += usize::from(u16::from(pixel[0]) > 2 * u16::from(pixel[2]));
    }
    assert!(model > 100 && paths > 100, "model {model}, paths {paths}");
    write_svg(&bench.list, 760.0, 548.0, "toolpaths.svg");
}

#[test]
fn svg_bench_renders_fidget_source() {
    for (source, file) in [
        (crate::test_examples::TORUS, "fidget_torus.svg"),
        (crate::test_examples::TANGLECUBE, "fidget_tanglecube.svg"),
        (crate::test_examples::GYROID, "fidget_gyroid.svg"),
        (crate::test_examples::CUBE, "fidget_cube.svg"),
    ] {
        let (doc, _) = crate::gid_text::parse(source).unwrap();
        render(&doc, None, 560.0, file);
    }
}

#[test]
fn svg_bench_renders_the_placeholder_notation() {
    let empty = Document::default();
    let (_, root) = render(&empty, None, 320.0, "raw_placeholder_root.svg");
    // The engaged twin: same slot, same rect, selection blue.
    let (_, engaged) = render(
        &empty,
        Some(&pending_value(Vec::new())),
        320.0,
        "raw_placeholder_engaged.svg",
    );
    assert_eq!(engaged, root);
    let cells = Cells::new();
    let bare = new_cell_id();
    render(
        &Document {
            root: Some(Value::from(bare)),
            cells,
        },
        None,
        320.0,
        "raw_placeholder_cell.svg",
    );
    // The commit transition pair: the same spelling typed in the
    // slot and committed as the string, its glyphs where they were.
    let (typed, typed_extent) = render(
        &empty,
        Some(&crate::selection::pending_with_query(
            Vec::new(),
            "\"asdf\"",
        )),
        320.0,
        "raw_placeholder_typed.svg",
    );
    let (committed, committed_extent) = render(
        &Document {
            root: Some(crate::libraries::text::value("asdf")),
            cells: Cells::new(),
        },
        Some(&crate::selection::Selection::edge(Vec::new())),
        320.0,
        "raw_placeholder_committed.svg",
    );
    assert_eq!(committed_extent, typed_extent);
    let (typed, committed) = (glyphs(&typed.list), glyphs(&committed.list));
    assert_eq!(committed.len(), "\"asdf\"".len());
    assert!(
        committed.iter().all(|(id, at)| typed
            .iter()
            .any(|(other, there)| other == id && there.distance(*at) < 1e-6)),
        "{committed:?} moved from {typed:?}"
    );
    // The empty string under its write-through editor: quotes
    // stay snug, no slot minimum applies to string literals.
    let empty_string = Document {
        root: Some(crate::libraries::text::value("")),
        cells: Cells::new(),
    };
    let sel = Selection::edge(Vec::new());
    let (_, string) = render(
        &empty_string,
        Some(&sel),
        320.0,
        "raw_empty_string_editing.svg",
    );
    assert!(
        string.width < engaged.width,
        "{} < {}",
        string.width,
        engaged.width
    );
}

/// Each glyph `list` draws, by its id, and where.
fn glyphs(list: &DrawList) -> Vec<(u32, Point)> {
    crate::display::test_support::painted(&list.0)
        .filter_map(|command| match command {
            DrawCmd::GlyphRun(run) => Some(run),
            _ => None,
        })
        .flat_map(|run| {
            run.glyphs.iter().map(|glyph| {
                (
                    glyph.id,
                    run.transform * Point::new(glyph.x.into(), glyph.y.into()),
                )
            })
        })
        .collect()
}

/// `pending` with `query` typed into it, the caret at its end.
fn typed(mut pending: Selection, query: &str) -> Selection {
    pending.edit_query(|line| {
        *line = puri::edit::LineEditState::new(query).with_cursor_at_end();
        true
    });
    pending
}

#[test]
fn svg_bench_renders_a_pending_edge() {
    let doc = sample_document();
    let library = crate::stack::load().libraries;
    let edge = pending_edge(
        &Read {
            sources: Sources {
                doc: &doc,
                libraries: &library,
            },
        },
        crate::occurrence::of(&[Step::Key(crate::test_values::label("shape"))]),
    )
    .unwrap();
    assert_eq!(
        edge.stage(&src(&doc, &library)),
        crate::selection::Stage::Label
    );
    let typing = typed(edge, "na");
    render(&doc, Some(&typing), 560.0, "raw_pending_edge.svg");

    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let typing = typed(
        pending_edge(
            &Read {
                sources: Sources {
                    doc: &doc,
                    libraries: &library,
                },
            },
            Vec::new(),
        )
        .unwrap(),
        "field",
    );
    render(&doc, Some(&typing), 320.0, "pending_field_slot.svg");
}
