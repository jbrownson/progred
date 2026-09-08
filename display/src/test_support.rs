//! A recording interpreter for projection calls. Production programs retain
//! no parallel request encoding; tests obtain one by running the same function.

use measured::Output;

pub use crate::recording::{Recordable, Recorded, record};
use crate::{Partial, widget};
use gid::{Step, Value};
use measured::Extent;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::{FontContext, LayoutContext, TextCache, TextCtx};
use std::{cell::RefCell, rc::Rc};

pub enum ProjectionCall<W, H> {
    Descend {
        step: Step,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    },
    At {
        steps: Vec<Step>,
        value: Value,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    },
    Transient {
        value: Value,
        fuel: usize,
    },
    Other,
}

pub fn delimited<W, H>(
    layout: &Recorded<W, H>,
) -> (
    &widget::Widget<W, H>,
    &Recorded<W, H>,
    &widget::Widget<W, H>,
) {
    match layout {
        Recorded::Row { gap, children, .. } if *gap == 0.0 => match children.as_slice() {
            [Recorded::Widget(left), child, Recorded::Widget(right)] => (left, child, right),
            _ => panic!("expected two ordinary delimiter widgets around a child"),
        },
        _ => panic!("expected a delimiter row"),
    }
}

struct Recorder<W, H>(RefCell<ProjectionCall<W, H>>);

impl<W: 'static, H: 'static> Recorder<W, H> {
    fn record(&self, call: ProjectionCall<W, H>) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.0.replace(call);
        ChoiceLayout::fixed(measured::leaf(Extent::default(), |_| {
            widget::HoverPass::empty()
        }))
    }
}

impl<W: 'static, H: 'static> widget::project::Project<W, H> for Recorder<W, H> {
    fn descend(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        step: Step,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::Descend {
            step,
            projection,
            default_projection,
        })
    }
    fn at(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        steps: Vec<Step>,
        value: Value,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::At {
            steps,
            value,
            projection,
            default_projection,
        })
    }
    fn transient(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        value: Value,
        fuel: usize,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::Transient { value, fuel })
    }
}

pub fn with_context<W: 'static, H: 'static, R>(
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    run(&mut widget::Context {
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        styles: &widget::style::editor(1.0),
        project,
        completion: &|_, _, _| panic!("unexpected completion control"),
        drawing: &|_, _, _| panic!("unexpected drawing control"),
        site: &|| panic!("unexpected document site"),
        event_interpreter: &|| panic!("unexpected event interpreter"),
        annotate: &|| panic!("unexpected annotation"),
        start_gesture: &|| panic!("unexpected gesture"),
        value_edit: &|| panic!("unexpected edit"),
        drag_threshold: 3.0,
        command: |_| false,
        pick: Rc::new(|_, _| panic!("unexpected pick")),
        picking: |_| false,
        same_target: |_, _| false,
        primary_edit: |_| false,
    })
}

pub fn inspect<W: 'static, H: 'static>(layout: &impl Recordable<W, H>) -> ProjectionCall<W, H> {
    let recorder = Recorder(RefCell::new(ProjectionCall::Other));
    if let Recorded::Program(program) = record(layout) {
        with_context(&recorder, |context| {
            program(context, &mut ChoiceBuild::default());
        });
    }
    recorder.0.into_inner()
}

/// Native-only widget tests intentionally provide no document recursion.
pub struct NoProject;
impl<W, H> widget::project::Project<W, H> for NoProject {
    fn descend(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Step,
        _: Option<Partial<W, H>>,
        _: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected descent")
    }
    fn at(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Vec<Step>,
        _: Value,
        _: Option<Partial<W, H>>,
        _: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected projection")
    }
    fn transient(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Value,
        _: usize,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected computed root")
    }
}
