use super::completion::{completion_card, completion_placement};
use super::*;
use crate::annotations::Annotations;
use crate::completion::Entry;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::hover::hover_secondary;
use crate::identity::short_id;
use crate::libraries::layout as layout_data;
use crate::libraries::{f64, fidget, name, text};
use crate::library::Libraries;
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::payload as selection_payload;
use crate::selection::{break_edit_run, delete_edge, pending_edge, pending_into, pending_value};
use crate::test_support::{
    arrow, completion_entries_with, core_libraries, doc_of, library_offers, make_selection,
    positions, root_completions, src,
};
use crate::test_values::key;
use gid::{Cells, Document, new_cell_id};
use kurbo::Rect;
use measured::Extent;
use peniko::Brush;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};

type EditingWorld = crate::Editor;

fn editing_world(doc: &Document, libraries: &Libraries) -> EditingWorld {
    let mut world = crate::test_editor(doc.clone());
    world.stack.libraries = libraries.clone();
    world
}

fn editing_frame(world: &mut EditingWorld, raw: bool) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_with_projection(world, raw, None)
}

fn editing_frame_with_projection(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_at(world, raw, projection, None)
}

fn editing_frame_at(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
    pointer: Option<Point>,
) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_with_annotations(world, raw, projection, pointer, &Annotations::default())
}

fn editing_frame_with_annotations(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
    pointer: Option<Point>,
    annotations: &Annotations,
) -> crate::placed::HoverOutput<EditingWorld> {
    let stack = &world.stack;
    let styles = crate::styles::editor(world.palette, 1.0);
    let mut tcx = TextCtx {
        fonts: &mut world.font_cx,
        layouts: &mut world.layout_cx,
        scale: 1.0,
        cache: &mut world.text_cache,
    };
    let measured = project(
        ProjectDescription {
            command_modifier: world.command_modifier,
            focused: world.focused,
            computations: None,
            view: &crate::test_root(),
            completions: Some(&stack.completions),
            sources: Sources {
                doc: &world.model.doc,
                libraries: &world.stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection: world.model.selection.as_ref(),
            source_selection: world.model.selection.as_ref(),
            annotations,
            raw,
            styles: &styles,
            width: 500.0,

            projection: (!raw).then_some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
    );
    let height = measured.extent.height().max(1.0);
    crate::display::widget::frame::place(
        measured,
        // A short document still has a viewport below it for completion popups.
        Placement::new(
            Rect::new(0.0, 0.0, 500.0, height),
            Rect::new(0.0, 0.0, 500.0, height.max(500.0)),
        ),
        &placed::HoverInput {
            command_modifier: world.command_modifier,
            pointer,
            ..Default::default()
        },
    )
}

/// Select through the current projection, without an editing interaction.
fn make_projected_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    let mut world = editing_world(doc, libraries);
    let placed = editing_frame(&mut world, false);
    if let Some(target) = placed.descends.landmark(&path) {
        (target.select)(&mut world, None);
    }
    world
        .model
        .selection
        .unwrap_or_else(|| make_selection(path))
}

fn make_projected_editing_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut world = editing_world(doc, libraries);
    world.model.selection = Some(make_projected_selection(doc, libraries, path));
    editing_frame(&mut world, false)
        .resolve_for_dispatch()
        .dispatch_key(&mut world, &arrow(NamedKey::End));
    world.model.selection.unwrap()
}

fn write_text(doc: &mut Rc<Document>, libraries: &Libraries, selected: &mut Selection) -> bool {
    write_with(doc, libraries, selected, text::edit)
}

fn write_with(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    selected: &mut Selection,
    update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static,
) -> bool {
    line_control::commit(
        doc,
        libraries,
        selected,
        &crate::libraries::line_edit::native(update),
    )
}

/// Three unnamed record slots, empty until filled.
const EMPTY_SLOTS: &str = r#"{
  "binders": {
    "first": 9940ece27410c72a5308a544890ccc71,
    "second": f717b766d250a7b86c5eb842885c4417,
    "third": 5e716c07490849f072b4e9017dd6230d,
  },
  "cells": {},
  "root": {},
}"#;

const FRUIT: &str = r#"{"root": ["apples", "pears", "plums"]}"#;

mod completion;
mod display;
mod editing;
mod events;
mod frame;

fn measured_rect(width: f64) -> kurbo::Rect {
    kurbo::Rect::new(0.0, 0.0, width, 100.0)
}
