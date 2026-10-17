//! Real projection and dispatch fixtures, without a window.

use super::*;
use peniko::ImageData;
use puri::draw::{DrawCmd, DrawList, Shape};
use puri::hover::Claim;

type World = crate::Editor;

/// The keyboard-navigation fixture, opened from disk rather than the
/// Examples menu.
const NAVIGATION_EXAMPLE: &str = include_str!("../../../../../examples/navigation.gid");

#[test]
fn plain_reads_text_numbers_and_absences_but_keeps_calls_records() {
    use crate::libraries::{absent, f64};
    use crate::workspace::Projection;
    let (words, number, sum, missing) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let doc = Document {
        root: Some(Value::record([
            (words, text::value("hello")),
            (number, f64::value(3.0)),
            (sum, grap::call(f64::vocabulary::SUM.into(), [])),
            (
                missing,
                absent::with_reason(absent::vocabulary::NO_ALTERNATIVE),
            ),
        ])),
        cells: Cells::new(),
    };
    // How many glyphs each field draws, and whether the call's function
    // field is drawn as a field of its own.
    let drawn = |projection| {
        let mut editor = crate::test_editor_with_stack(doc.clone(), crate::stack::load());
        editor.model.workspace.document.projection = projection;
        let driver = crate::driver::Driver::new(editor, kurbo::Size::new(900.0, 600.0));
        let glyphs = |field| {
            let rect = driver.rect(&[Step::Key(field)]);
            crate::display::test_support::painted(&driver.painted().0)
                .filter_map(|command| match command {
                    DrawCmd::GlyphRun(run) => Some(run),
                    _ => None,
                })
                .flat_map(|run| {
                    run.glyphs
                        .iter()
                        .map(|glyph| run.transform * Point::new(glyph.x.into(), glyph.y.into()))
                        .collect::<Vec<_>>()
                })
                .filter(|glyph| rect.contains(*glyph))
                .count()
        };
        (
            glyphs(words),
            glyphs(number),
            driver.drawn(&[Step::Key(sum), Step::Key(grap::vocabulary::FUNCTION)]),
            driver.drawn(&[Step::Key(missing), Step::Key(absent::vocabulary::ABSENT)]),
        )
    };
    let (words_plain, number_plain, function_plain, reason_plain) = drawn(Projection::Plain);
    let (words_raw, number_raw, _, reason_raw) = drawn(Projection::Raw);
    // Plain reads "hello" and 3; Raw spells out their bytes.
    assert!(words_plain >= "hello".len(), "{words_plain}");
    assert!(words_plain < words_raw, "{words_plain} < {words_raw}");
    assert!(number_plain < number_raw, "{number_plain} < {number_raw}");
    assert!(function_plain, "a call stays a record");
    // Plain reads an absent value as its reason; Raw draws the reason's field.
    assert!(!reason_plain, "Plain draws the reason as the value itself");
    assert!(reason_raw, "Raw draws the reason as a field");
}

#[test]
fn an_absent_value_reads_as_its_reason_then_its_fields_all_on_red() {
    use crate::libraries::{absent, f64};
    let washes = |root: Value| {
        let doc = Document {
            root: Some(root),
            cells: Cells::new(),
        };
        let error = peniko::Brush::from(crate::styles::Theme::Light.palette().error);
        place(&doc, None, 600.0)
            .0
            .list
            .0
            .into_iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(wash),
                    brush,
                    ..
                } if brush == error => Some(wash.rect()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Each cause of a failed alternative is washed too, inside its failure's.
    let failure = washes(absent::from_causes([
        absent::with_reason(absent::vocabulary::DECLINED),
        ::grap::absent::with_detail(
            ::grap::absent::MISSING_ARGUMENT,
            absent::vocabulary::VALUE,
            f64::value(3.0),
        ),
    ]));
    assert_eq!(failure.len(), 3, "{failure:?}");
    for cause in &failure[1..] {
        assert_eq!(failure[0].union(*cause), failure[0], "{failure:?}");
    }
    assert!(washes(Value::record([(new_cell_id(), f64::value(3.0))])).is_empty());
}

fn click_at(world: &mut crate::Editor, point: Point) -> Option<Hovered> {
    let frame = editing_frame(world, false);
    let Some((_, Claim::Direct(target))) = frame.hover_geometry.probe(Some(point), None, 0.0)
    else {
        return None;
    };
    let mut input =
        crate::placed::DispatchContext::new(Some(crate::test_root()), Some(target.clone()));
    frame.resolve_for_dispatch().dispatch_pointer_down_with(
        world,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (point.x, point.y).into(),
                ..Default::default()
            },
        },
        &mut input,
    );
    Some(target)
}

/// Configure the editor's projection slots as a tutorial page does.
fn tutorial_slots(world: &mut crate::Editor, slots: &[CellId]) {
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(CellId::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
        &world.stack.libraries,
    )
    .unwrap();
}

mod completion;
#[cfg(not(target_arch = "wasm32"))]
mod compositor;
mod conject;
mod custom_navigation;
mod declarations;
mod document_libraries;
mod drawing;
mod fidget_shapes;
mod fidget_source;
mod focus;
mod folds;
mod interaction;
mod iop_tree_native;
mod layout;
mod logical_navigation;
mod navigation;
mod outline;
mod profile;
mod svg;
mod tutorial;
