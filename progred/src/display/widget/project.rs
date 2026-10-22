//! Projection recursion is a caller-supplied operation, not a layout node.

use crate::display::widget::HoverPass;

use crate::display::Partial;
use gid::Step;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::TextCtx;

/// What a walk makes of the value it reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reached {
    /// A stop of its own.
    Stop,
    /// A stop that what holds it requires: deleting it leaves a hole, so the
    /// shape keeps its field.
    Required,
    /// Part of the occurrence that walked to it, joining its stop.
    Part,
}

pub trait Project<World, Hover> {
    /// Draw what's at `steps` below here, as `reached` says.
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: &[Step],
        overriding: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
        reached: Reached,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    fn jump(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: Vec<Step>,
        document: crate::location::Location<'static>,
        overriding: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    /// Draw `value` computed at `steps` below here, the result of
    /// `evaluated` when it came from one.
    #[allow(clippy::too_many_arguments)]
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: Vec<Step>,
        value: grap::RuntimeValue,
        evaluated: Option<std::rc::Rc<crate::display::Evaluated>>,
        overriding: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    /// Descend showing the child as Raw does: no value projections and no
    /// naming convention, whatever the enclosing view shows.
    #[cfg(any(test, target_arch = "wasm32"))]
    fn descend_raw(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        step: Step,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
}
