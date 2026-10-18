//! A recording interpreter for projection calls. Production programs retain
//! no parallel request encoding; tests obtain one by running the same function.

use puri::handler::Handler;

pub trait ResolveForDispatch<C, H> {
    fn resolve_for_dispatch(self) -> Handler<C, widget::frame::DispatchContext<C, H>>;
    /// The handler with the frame's landmarks, as the shell dispatches it.
    fn resolve_with_context(
        self,
    ) -> (
        Handler<C, widget::frame::DispatchContext<C, H>>,
        widget::frame::DispatchContext<C, H>,
    );
    /// Dispatch an arrow's navigation as the shell does.
    fn navigate(self, world: &mut C, direction: widget::Direction) -> bool;
}

impl<C: 'static, H: Clone + 'static> ResolveForDispatch<C, H> for widget::HoverOutput<C, H> {
    fn resolve_for_dispatch(self) -> Handler<C, widget::frame::DispatchContext<C, H>> {
        self.resolve_with_context().0
    }

    fn resolve_with_context(
        self,
    ) -> (
        Handler<C, widget::frame::DispatchContext<C, H>>,
        widget::frame::DispatchContext<C, H>,
    ) {
        let hovered = self.claim.as_ref().and_then(|(_, claim)| match claim {
            puri::hover::Claim::Direct(target) | puri::hover::Claim::Extended(target) => {
                Some(target.clone())
            }
            puri::hover::Claim::Occludes => None,
        });
        let frame = self.bind(widget::frame::Attribution {
            hovered,
            ..Default::default()
        });
        (
            frame.handler.expect("resolved frame handler"),
            widget::frame::DispatchContext {
                landmarks: frame.landmarks.into(),
                ..Default::default()
            },
        )
    }

    fn navigate(self, world: &mut C, direction: widget::Direction) -> bool {
        let (handler, mut context) = self.resolve_with_context();
        handler
            .dispatch(
                world,
                puri::handler::Event::Navigate(direction),
                &mut context,
            )
            .handled()
    }
}

pub use crate::display::recording::{Recordable, Recorded, record};
use crate::display::{Partial, widget};
use gid::{CellId, Step, Value};
use measured::Extent;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::{LayoutContext, TextCache, TextCtx};
use std::cell::RefCell;

/// A host that applies and evaluates nothing: a projection that asks fails
/// its test.
pub struct NoEval;

impl crate::display::Env for NoEval {
    fn apply_scoped(
        &self,
        _: &Value,
        _: &[(CellId, Value)],
        _: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation<Value> {
        panic!("unexpected application")
    }

    fn evaluate(&self, _: &Value) -> Value {
        panic!("unexpected evaluation")
    }
}

/// A host whose every application and evaluation answers its value.
pub struct EvaluatesTo(pub Value);

impl crate::display::Env for EvaluatesTo {
    fn apply_scoped(
        &self,
        _: &Value,
        _: &[(CellId, Value)],
        _: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation<Value> {
        ::grap::Evaluation {
            result: self.0.clone(),
            completed: true,
            remaining_fuel: 0,
        }
    }

    fn evaluate(&self, _: &Value) -> Value {
        self.0.clone()
    }
}

/// What `layout` records inside the wrappers that share or decorate it.
pub fn unshared<W, H>(mut layout: &Recorded<W, H>) -> &Recorded<W, H> {
    while let Recorded::Shared { child, .. } = layout.content() {
        layout = child.as_ref();
    }
    layout.content()
}

pub enum ProjectionCall<W, H> {
    Descend {
        step: Step,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    },
    DescendPath {
        steps: Vec<Step>,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    },
    Jump {
        steps: Vec<Step>,
        document: crate::location::Location<'static>,
        conject: crate::display::Conject,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    },
    At {
        steps: Vec<Step>,
        value: Value,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
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
            Value::record([]),
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
    match layout.content() {
        Recorded::Row { gap, children, .. } if *gap == 0.0 => match children.as_slice() {
            [Recorded::Widget(left), child, Recorded::Widget(right)] => {
                (left, child.content(), right)
            }
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
        steps: &[Step],
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(match steps {
            [step] => ProjectionCall::Descend {
                step: step.clone(),
                projection,
                default_projection,
            },
            _ => ProjectionCall::DescendPath {
                steps: steps.to_vec(),
                projection,
                default_projection,
            },
        })
    }
    fn jump(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        steps: Vec<Step>,
        document: crate::location::Location<'static>,
        conject: crate::display::Conject,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::Jump {
            steps,
            document,
            conject,
            projection,
            default_projection,
        })
    }
    fn at(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        steps: Vec<Step>,
        value: grap::RuntimeValue,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::At {
            steps,
            value: value.into_value(),
            projection,
            default_projection,
        })
    }
}

pub fn with_context<W: 'static, H: 'static, R>(
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    with_empty_view(project, |cx, text, project| {
        run(&mut widget::Context {
            text,
            project,
            toolkit: &crate::projection::Occurrence { cx, path: &[] },
            value: None,
        })
    })
}

/// [`with_context`] reading through `sources`.
pub fn with_sources<W: 'static, H: 'static, R>(
    sources: crate::sources::Sources<'_>,
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    with_view(sources, project, |cx, text, project| {
        run(&mut widget::Context {
            text,
            project,
            toolkit: &crate::projection::Occurrence { cx, path: &[] },
            value: None,
        })
    })
}

/// [`with_view`] over an empty document and no libraries.
pub(crate) fn with_empty_view<W: 'static, H: 'static, R>(
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(
        &crate::projection::Cx<'_>,
        &mut TextCtx<'_>,
        &dyn widget::project::Project<W, H>,
    ) -> R,
) -> R {
    let document = gid::Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let libraries = crate::library::Libraries::default();
    with_view(
        crate::sources::Sources {
            doc: &document,
            libraries: &libraries,
        },
        project,
        run,
    )
}

/// The view a widget is built in, reading through `sources`, for a test to
/// change before building a widget context over it.
pub(crate) fn with_view<W: 'static, H: 'static, R>(
    sources: crate::sources::Sources<'_>,
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(
        &crate::projection::Cx<'_>,
        &mut TextCtx<'_>,
        &dyn widget::project::Project<W, H>,
    ) -> R,
) -> R {
    let mut fonts = crate::fonts::bundled_font_context();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    let root = crate::test_root();
    let annotations = crate::annotations::Annotations::default();
    let styles = widget::style::editor(widget::style::Theme::Light.palette(), 1.0);
    let computations = crate::computations::Computations::from_sources(sources);
    let cx = crate::projection::Cx {
        focused: true,
        edits: Default::default(),
        computations: &computations,
        view: &root,
        completions: None,
        sources,
        raw: false,
        annotations: &annotations,
        styles: &styles,
        selection: None,
        selected_trace: None,
    };
    run(
        &cx,
        &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        project,
    )
}

pub fn inspect<W: 'static, H: 'static>(layout: &impl Recordable<W, H>) -> ProjectionCall<W, H> {
    let recorder = Recorder(RefCell::new(ProjectionCall::Other));
    if let Recorded::Program(program) = record(layout).content() {
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
        _: &[Step],
        _: Option<Partial<W, H>>,
        _: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected descent")
    }
    fn jump(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Vec<Step>,
        _: crate::location::Location<'static>,
        _: crate::display::Conject,
        _: Option<Partial<W, H>>,
        _: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected jump")
    }
    fn at(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Vec<Step>,
        _: grap::RuntimeValue,
        _: Option<Partial<W, H>>,
        _: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected projection")
    }
}

/// Every painted command in painting order, looking through clips. Each
/// command carries its own complete transform, so nothing is lost.
pub fn painted(
    commands: &[puri::draw::DrawCmd],
) -> Box<dyn Iterator<Item = &puri::draw::DrawCmd> + '_> {
    Box::new(commands.iter().flat_map(|command| match command {
        puri::draw::DrawCmd::Clip { children, .. } => painted(children),
        command => Box::new(std::iter::once(command)),
    }))
}

/// Look up a frame's selectable stops by where they lead.
pub trait Landmarks<World> {
    /// The stop drawn at `path`.
    fn landmark(&self, path: &[gid::Step]) -> Option<&widget::navigation::Landmark<World>>;
}

impl<World> Landmarks<World> for [widget::navigation::Landmark<World>] {
    fn landmark(&self, path: &[gid::Step]) -> Option<&widget::navigation::Landmark<World>> {
        self.iter().find(|landmark| landmark.path.as_ref() == path)
    }
}

/// The mouse, as a pointer event names it.
pub fn mouse() -> ui_events::pointer::PointerInfo {
    ui_events::pointer::PointerInfo {
        pointer_id: None,
        persistent_device_id: None,
        pointer_type: ui_events::pointer::PointerType::Mouse,
    }
}
