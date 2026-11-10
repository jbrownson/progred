//! Projection recursion is a caller-supplied operation, not a layout node.

use crate::display::widget::HoverPass;

use crate::display::Partial;
use gid::Step;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::TextCtx;

/// Where the selection stops for what a walk reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// At what's reached, a stop of its own.
    Own,
    /// At the occurrence that walked to it, which it is part of.
    Joined,
}

pub trait Project<World, Hover> {
    /// Draw what's at `steps` below here, stopping the selection as `stop`
    /// says.
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: &[Step],
        overriding: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
        stop: Stop,
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
