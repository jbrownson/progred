//! Projection recursion is a caller-supplied operation, not a layout node.

use crate::display::widget::HoverPass;

use crate::display::Partial;
use gid::Step;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::TextCtx;

pub trait Project<World, Hover> {
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: &[Step],
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    fn jump(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: Vec<Step>,
        document: Vec<Step>,
        conject: crate::display::Conject,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: Vec<Step>,
        value: grap::RuntimeValue,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
}
