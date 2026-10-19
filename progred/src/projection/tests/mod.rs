use super::test_support::*;
use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::display::widget::navigation::Landmark;
use crate::libraries::layout as layout_data;
use crate::libraries::{f64, fidget, name, text};
use crate::library::Libraries;
use crate::location::delete_edge;
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::payload as selection_payload;
use crate::selection::pending_value;
use crate::test_support::{
    arrow, core_libraries, doc_of, make_selection, positions, root_completions, src,
};
use crate::test_values::key;
use gid::{Cells, Document, new_cell_id};
use kurbo::{Point, Rect};
use measured::Extent;
use peniko::Brush;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyState;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType,
};
use uig::Placement;

mod completion;
mod display;
mod editing;
mod events;
mod frame;

fn measured_rect(width: f64) -> kurbo::Rect {
    kurbo::Rect::new(0.0, 0.0, width, 100.0)
}
