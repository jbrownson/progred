//! Projection recursion is a caller-supplied operation, not a layout node.

use super::Fragment;
use crate::Partial;
use gid::{Step, Value};
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::TextCtx;

pub trait Project<World, Hover> {
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Fragment<World, Hover>>,
        step: Step,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<Fragment<World, Hover>>;
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Fragment<World, Hover>>,
        steps: Vec<Step>,
        value: Value,
        current: Option<Partial<World, Hover>>,
        default: Option<Partial<World, Hover>>,
    ) -> ChoiceLayout<Fragment<World, Hover>>;
    fn transient(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Fragment<World, Hover>>,
        value: Value,
        fuel: usize,
    ) -> ChoiceLayout<Fragment<World, Hover>>;
}
