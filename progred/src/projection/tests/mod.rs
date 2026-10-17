use super::test_support::*;
use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::display::widget::offers::Entry;
use crate::libraries::layout as layout_data;
use crate::libraries::{f64, fidget, name, text};
use crate::library::Libraries;
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::payload as selection_payload;
use crate::selection::{break_edit_run, delete_edge, pending_edge, pending_value};
use crate::test_support::{
    EMPTY_SLOTS, arrow, completion_entries_with, core_libraries, doc_of, library_offers,
    make_selection, positions, root_completions, set_tutorial_slots, src,
};
use crate::test_values::key;
use gid::{Cells, Document, new_cell_id};
use kurbo::Rect;
use measured::Extent;
use peniko::Brush;
use puri::geometry::Placement;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType,
};

const FRUIT: &str = r#"{"root": ["apples", "pears", "plums"]}"#;

mod completion;
mod display;
mod editing;
mod events;
mod frame;

fn measured_rect(width: f64) -> kurbo::Rect {
    kurbo::Rect::new(0.0, 0.0, width, 100.0)
}
