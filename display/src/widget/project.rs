//! Projection recursion is a caller-supplied operation, not a layout node.

use crate::widget::HoverPass;

use crate::Partial;
use gid::{Step, Value};
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::TextCtx;

pub trait Project<World, Hover> {
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        step: Step,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        steps: Vec<Step>,
        value: Value,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
    fn transient(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<World, Hover>>,
        value: Value,
        fuel: usize,
    ) -> ChoiceLayout<HoverPass<World, Hover>>;
}
