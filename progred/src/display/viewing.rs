//! What a widget reads about the view it's built in: the occurrence it
//! draws and the view's reading context. A library's widgets read only
//! [`Viewing`]; the toolkit's own also read the theme, the selection and
//! the sources in full, through [`Toolkit`].

use crate::display::widget::source::SourceTrace;
use crate::display::widget::view::Root;
use crate::editing::Site;
use crate::location::Location;
use crate::selection::Selection;
use crate::sources::Sources;
use gid::{Step, Value};
use puri::edit::LineEditState;

pub trait Viewing {
    /// Physical pixels per logical point, for what's drawn in pixels.
    fn scale(&self) -> f64;
    /// Whether an edit to this occurrence would be stored.
    fn writable(&self) -> bool;
    /// The document read over its libraries, for evaluating Grap.
    fn host(&self) -> &dyn ::grap::Host;
    /// The editor's memo roots and background work.
    fn computations(&self) -> &crate::computations::Computations;
    /// The per-path state `steps` below this occurrence.
    fn state_below(&self, steps: &[Step]) -> Option<&Value>;
    /// Whether the selection is `steps` below this occurrence, or deeper.
    fn selected_below(&self, steps: &[Step]) -> bool;
    /// Whether this occurrence sits inside `cell`'s value, so drawing
    /// `cell` here would draw that value again inside itself.
    fn inside(&self, cell: gid::CellId) -> bool;
    /// Whether this occurrence sits inside some cell's value, rather than
    /// being reached directly by the view or being a cell's whole value.
    fn inside_a_cell(&self) -> bool;
}

/// What the toolkit's own widgets read besides: the theme, the occurrence's
/// place in its view, and the selection and sources in full.
pub(crate) trait Toolkit: Viewing {
    /// This occurrence's path in its view.
    fn path(&self) -> &[crate::occurrence::Step];
    /// The view this occurrence is drawn in.
    fn view(&self) -> &Root;
    /// This occurrence, as a place to act.
    fn site(&self) -> Site;
    /// The theme, at the view's scale.
    fn styles(&self) -> &crate::display::widget::style::Styles;
    /// The document read over its libraries.
    fn sources(&self) -> Sources<'_>;
    /// Where this occurrence is stored.
    fn location(&self) -> Option<Location<'static>>;
    /// Whether the window has focus.
    fn focused(&self) -> bool;
    /// The selection, while this view has it.
    fn selection(&self) -> Option<&Selection>;
    /// Whether the primary highlight is on this occurrence.
    fn selected(&self) -> bool;
    /// The label being typed for a new field on the record here.
    fn label_query(&self) -> Option<&LineEditState>;
    /// What the loaded libraries offer holes.
    fn completions(&self) -> Option<&crate::display::CompletionProvider>;
    /// `cell`'s name as the document names it, Raw's view or not.
    fn name(&self, cell: gid::CellId) -> Option<crate::display::Name<'_>>;
    /// The selected structural source, however it's projected.
    fn selected_trace(&self) -> Option<&SourceTrace>;
    /// Where `steps` below here is stored, as a source trace.
    fn trace_below(&self, steps: &[Step]) -> Option<SourceTrace>;
}
