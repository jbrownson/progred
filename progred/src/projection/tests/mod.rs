use super::test_support::*;
use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::display::widget::offers::Entry;
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
use puri::geometry::Placement;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};

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
