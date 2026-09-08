//! A recording interpreter for projection calls. Production programs retain
//! no parallel request encoding; tests obtain one by running the same function.

use puri::handler::Handler;

pub trait ResolveForDispatch<C, H> {
    fn resolve_for_dispatch(&mut self) -> &Handler<C, widget::frame::DispatchContext<C, H>>;
}

impl<C: 'static, H: Clone + 'static> ResolveForDispatch<C, H> for widget::HoverOutput<C, H> {
    fn resolve_for_dispatch(&mut self) -> &Handler<C, widget::frame::DispatchContext<C, H>> {
        let hovered = self.claim.as_ref().and_then(|(_, claim)| match claim {
            puri::hover::Claim::Direct(target) | puri::hover::Claim::Extended(target) => {
                Some(target.clone())
            }
            puri::hover::Claim::Occludes => None,
        });
        drop(self.resolve(widget::frame::ResolvedHover {
            hovered,
            ..Default::default()
        }));
        self.handler.as_ref().expect("resolved frame handler")
    }
}

pub use crate::display::recording::{Recordable, Recorded, record};
use crate::display::{Partial, widget};
use gid::{Step, Value};
use measured::Extent;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::{FontContext, LayoutContext, TextCache, TextCtx};
use std::cell::RefCell;

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

#[test]
fn recorder_preserves_local_and_descendant_projection_inputs() {
    use crate::display::{at_with_projection, descend, partial};
    let local = partial::<(), ()>(|_| None);
    let default = partial::<(), ()>(|_| None);
    let step = Step::Key(gid::new_cell_id());
    for layout in [
        descend(step.clone(), Some(local.clone()), Some(default.clone())),
        at_with_projection(
            [step],
            &Value::record([]),
            Some(local.clone()),
            Some(default.clone()),
        ),
    ] {
        let (actual, descendants) = match inspect(&layout) {
            ProjectionCall::Descend {
                projection,
                default_projection,
                ..
            }
            | ProjectionCall::At {
                projection,
                default_projection,
                ..
            } => (projection, default_projection),
            _ => panic!("a scoped projection call"),
        };
        assert!(std::rc::Rc::ptr_eq(actual.as_ref().unwrap(), &local));
        assert!(std::rc::Rc::ptr_eq(descendants.as_ref().unwrap(), &default));
    }
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
        ChoiceLayout::fixed(measured::leaf_into(Extent::default(), |_, _| {}))
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
    let document = gid::Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let libraries = crate::libraries::Libraries::default();
    let root = crate::test_root();
    let annotations = crate::annotations::Annotations::default();
    let styles = widget::style::editor(1.0);
    let cx = crate::projection::Cx {
        view: &root,
        completions: None,
        sources: crate::sources::Sources {
            doc: &document,
            libraries: &libraries,
        },
        raw: false,
        annotations: &annotations,
        styles: &styles,
        selection: None,
        secondary: None,
        selected_trace: None,
        source: crate::projection::Source::Stored,
        fuel: std::cell::Cell::new(grap::DEFAULT_FUEL),
    };
    run(&mut widget::Context {
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        project,
        inputs: &cx,
        path: &[],
        value: None,
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
