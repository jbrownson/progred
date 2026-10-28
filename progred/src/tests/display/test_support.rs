//! The display layer's test harness: placed frames resolved for dispatch; a
//! recording interpreter for projection calls (production programs retain no
//! parallel request encoding; tests obtain one by running the same
//! function); widget contexts over small views and the fonts they measure
//! with; placed decorations; painted commands and landmark lookup; and
//! pointer events.

use puri::handler::Handler;

pub trait ResolveForDispatch<C, H> {
    fn resolve_for_dispatch(self) -> Handler<C, widget::frame::DispatchContext<C, H>>;
    /// The handler, and a dispatch context holding the frame's landmarks.
    fn resolve_with_context(
        self,
    ) -> (
        Handler<C, widget::frame::DispatchContext<C, H>>,
        widget::frame::DispatchContext<C, H>,
    );
    /// Dispatch the navigation an arrow nothing claims becomes, straight to
    /// the frame's handler: unlike the shell, this gives no key handler the
    /// arrow first and doesn't reveal the selection.
    fn navigate(self, world: &mut C, direction: puri::handler::NavigationDirection) -> bool;
}

impl<C: 'static, H: Clone + 'static> ResolveForDispatch<C, H> for widget::frame::HoverOutput<C, H> {
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

    fn navigate(self, world: &mut C, direction: puri::handler::NavigationDirection) -> bool {
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
use ::grap::RuntimeValue;
use gid::{CellId, Step, Value};
use measured::Extent;
use measured::choices::{ChoiceBuild, ChoiceLayout};
use puri::text::{LayoutContext, TextCache, TextCtx};
use std::cell::RefCell;

/// A host that applies and evaluates nothing: a projection that asks fails
/// its test.
pub struct NoEval;

impl crate::display::Env for NoEval {
    fn apply_expression_runtime(
        &self,
        _: &RuntimeValue,
        _: &[(CellId, RuntimeValue)],
    ) -> RuntimeValue {
        panic!("unexpected application")
    }

    fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
        panic!("unexpected evaluation")
    }

    fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
        panic!("unexpected evaluation")
    }

    fn apply_runtime_scoped(
        &self,
        _: &RuntimeValue,
        _: &[(CellId, RuntimeValue)],
        _: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation {
        panic!("unexpected application")
    }

    fn apply_runtime_memo(&self, _: &Value, _: &[(CellId, Value)], _: usize) -> RuntimeValue {
        panic!("unexpected application")
    }
}

/// A host whose every application and evaluation answers its value.
pub struct EvaluatesTo(pub Value);

impl crate::display::Env for EvaluatesTo {
    fn apply_expression_runtime(
        &self,
        _: &RuntimeValue,
        _: &[(CellId, RuntimeValue)],
    ) -> RuntimeValue {
        self.0.clone().into()
    }

    fn evaluate_runtime(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
        self.0.clone().into()
    }

    fn evaluate_runtime_memo(&self, _: &RuntimeValue, _: usize, _: &[Step]) -> RuntimeValue {
        self.0.clone().into()
    }

    fn apply_runtime_scoped(
        &self,
        _: &RuntimeValue,
        _: &[(CellId, RuntimeValue)],
        _: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation {
        ::grap::Evaluation {
            result: self.0.clone().into(),
            halt: None,
            remaining_fuel: 0,
        }
    }

    fn apply_runtime_memo(&self, _: &Value, _: &[(CellId, Value)], _: usize) -> RuntimeValue {
        self.0.clone().into()
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
        steps: Vec<Step>,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
        part: bool,
    },
    Jump {
        steps: Vec<Step>,
        document: crate::location::Location<'static>,
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

/// Records the walk a layout makes.
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
        part: bool,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::Descend {
            steps: steps.to_vec(),
            projection,
            default_projection,
            part,
        })
    }
    fn descend_raw(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Step,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("raw descents aren't recorded")
    }
    fn jump(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        steps: Vec<Step>,
        document: crate::location::Location<'static>,
        projection: Option<Partial<W, H>>,
        default_projection: Option<Partial<W, H>>,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        self.record(ProjectionCall::Jump {
            steps,
            document,
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
        _: Option<std::rc::Rc<crate::display::Evaluated>>,
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

/// [`with_sources`] over an empty document and no libraries.
pub fn with_context<W: 'static, H: 'static, R>(
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    with_empty(|sources| with_sources(sources, project, run))
}

/// A widget context at the root of a view reading through `sources`.
pub fn with_sources<W: 'static, H: 'static, R>(
    sources: crate::sources::Sources<'_>,
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    with_view(sources, project, |cx, text, project| {
        crate::projection::test_support::at_root(cx, text, project, None, run)
    })
}

/// [`with_context`], with a toolkit that fails the test when a widget asks
/// for its occurrence's site or per-path state.
pub fn with_siteless_context<W: 'static, H: 'static, R>(
    project: &dyn widget::project::Project<W, H>,
    run: impl FnOnce(&mut widget::Context<'_, '_, W, H>) -> R,
) -> R {
    with_context(project, |context| {
        let toolkit = NoSite(context.toolkit);
        run(&mut widget::Context {
            project: context.project,
            text: &mut *context.text,
            toolkit: &toolkit,
            value: context.value,
        })
    })
}

/// A toolkit reading as the one it wraps, except that it fails the test
/// when asked for the occurrence's site or per-path state.
struct NoSite<'a>(&'a dyn crate::display::Toolkit);

impl crate::display::Viewing for NoSite<'_> {
    fn scale(&self) -> f64 {
        self.0.scale()
    }
    fn writable(&self) -> bool {
        self.0.writable()
    }
    fn host(&self) -> &dyn ::grap::Host {
        self.0.host()
    }
    fn computations(&self) -> &crate::computations::Computations {
        self.0.computations()
    }
    fn state_below(&self, _: &[Step]) -> Option<&Value> {
        panic!("unexpected request for per-path state")
    }
    fn selected_below(&self, steps: &[Step]) -> bool {
        self.0.selected_below(steps)
    }
    fn inside(&self, cell: gid::CellId) -> bool {
        self.0.inside(cell)
    }
    fn inside_a_cell(&self) -> bool {
        self.0.inside_a_cell()
    }
}

impl crate::display::Toolkit for NoSite<'_> {
    fn path(&self) -> &crate::occurrence::Trail {
        self.0.path()
    }
    fn site(&self) -> crate::editing::Site {
        panic!("unexpected request for the occurrence's site")
    }
    fn styles(&self) -> &widget::style::Styles {
        self.0.styles()
    }
    fn sources(&self) -> crate::sources::Sources<'_> {
        self.0.sources()
    }
    fn location(&self) -> Option<crate::location::Location<'static>> {
        self.0.location()
    }
    fn focused(&self) -> bool {
        self.0.focused()
    }
    fn selection(&self) -> Option<&crate::selection::Selection> {
        self.0.selection()
    }
    fn selected(&self) -> bool {
        self.0.selected()
    }
    fn label_query(&self) -> Option<&puri::edit::LineEditState> {
        self.0.label_query()
    }
    fn completions(&self) -> Option<&crate::display::CompletionProvider> {
        self.0.completions()
    }
    fn name(&self, cell: CellId) -> Option<crate::display::Name<'_>> {
        self.0.name(cell)
    }
    fn selected_trace(&self) -> Option<&widget::source::SourceTrace> {
        self.0.selected_trace()
    }
    fn trace_below(&self, steps: &[Step]) -> Option<widget::source::SourceTrace> {
        self.0.trace_below(steps)
    }
}

/// What the decorator `layout` leads or ends with places at `placement`,
/// the pointer as `input` says; `None` when it isn't a decorator.
pub fn place_decoration<W: 'static, H: Clone + 'static>(
    layout: &impl Recordable<W, H>,
    placement: uig::Placement,
    input: widget::HoverInput<'_, H>,
) -> Option<widget::frame::HoverOutput<W, H>> {
    match layout.record() {
        Recorded::Before {
            before: decoration, ..
        }
        | Recorded::After {
            after: decoration, ..
        } => {
            let mut output = widget::frame::HoverOutput::default();
            with_context(&NoProject, |context| decoration(context))(
                &mut widget::HoverContext::new(input, &mut output, &mut Default::default()),
                placement,
            );
            Some(output)
        }
        _ => None,
    }
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
    with_empty(|sources| with_view(sources, project, run))
}

/// The fonts, layouts and shaping cache text is measured with, as a frame
/// lends them.
pub(crate) struct Fonts {
    fonts: parley::FontContext,
    layouts: LayoutContext<peniko::Brush>,
    cache: TextCache,
}

impl Fonts {
    pub(crate) fn new() -> Self {
        Self {
            fonts: crate::fonts::bundled_font_context(),
            layouts: LayoutContext::new(),
            cache: TextCache::default(),
        }
    }

    /// A text context over these at `scale`.
    pub(crate) fn text(&mut self, scale: f64) -> TextCtx<'_> {
        TextCtx {
            fonts: &mut self.fonts,
            layouts: &mut self.layouts,
            cache: &mut self.cache,
            scale: scale as f32,
        }
    }
}

/// An empty document read over no libraries.
fn with_empty<R>(run: impl FnOnce(crate::sources::Sources<'_>) -> R) -> R {
    let document = gid::Document::default();
    let libraries = crate::library::Libraries::default();
    run(crate::sources::Sources {
        doc: &document,
        libraries: &libraries,
    })
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
    let mut fonts = Fonts::new();
    let styles = widget::style::editor(widget::style::Theme::Light.palette(), 1.0);
    let computations = crate::computations::Computations::from_sources(sources);
    let cx = crate::projection::Cx {
        focused: true,
        computations: &computations,
        completions: None,
        sources,
        names: true,
        styles: &styles,
        selection: None,
        selected_trace: None,
    };
    run(&cx, &mut fonts.text(1.0), project)
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

/// `text` in the name face.
pub fn text<W: 'static, H: 'static>(text: impl Into<String>) -> crate::display::Layout<W, H> {
    crate::display::faced(text, crate::display::Face::Name)
}

/// Records drawn with the structural record layout, each field's value by
/// `child`'s projection where it has one.
pub fn record_partial<W: crate::display::Host>(
    child: impl Fn(CellId) -> Option<crate::display::host::Partial<W>> + 'static,
) -> crate::display::host::Partial<W> {
    let child = std::rc::Rc::new(child);
    crate::display::runtime_partial(move |input| {
        let child = child.clone();
        crate::display::structure::Fields::default()
            .child(move |key| child(key))
            .record(input)
    })
}

/// No pointer, nothing hovered before, and the platform's command key.
impl<H> Default for widget::HoverInput<'_, H> {
    fn default() -> Self {
        Self {
            command_modifier: crate::modifiers::native(),
            picking: false,
            pointer: None,
            prior: None,
            reach_px: 0.0,
        }
    }
}

/// No landmarks or views, at scale 1.
impl<W> Default for widget::navigation::Geometry<'_, W> {
    fn default() -> Self {
        Self {
            landmarks: &[],
            view_regions: &[],
            scale: 1.0,
        }
    }
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
        _: bool,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected descent")
    }
    fn descend_raw(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Step,
    ) -> ChoiceLayout<widget::HoverPass<W, H>> {
        panic!("unexpected descent")
    }
    fn jump(
        &self,
        _: &mut TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<W, H>>,
        _: Vec<Step>,
        _: crate::location::Location<'static>,
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
        _: Option<std::rc::Rc<crate::display::Evaluated>>,
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
    /// The stop drawn at `path`, named by occurrence or document steps.
    fn landmark<S>(&self, path: &[S]) -> Option<&widget::navigation::Landmark<World>>
    where
        crate::occurrence::Step: PartialEq<S>;
}

impl<World> Landmarks<World> for [widget::navigation::Landmark<World>] {
    fn landmark<S>(&self, path: &[S]) -> Option<&widget::navigation::Landmark<World>>
    where
        crate::occurrence::Step: PartialEq<S>,
    {
        self.iter().find(|landmark| landmark.path == *path)
    }
}

/// The mouse, as a pointer event names it.
pub fn mouse() -> ui_events::pointer::PointerInfo {
    ui_events::pointer::PointerInfo {
        pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: ui_events::pointer::PointerType::Mouse,
    }
}

/// The mouse's primary button pressed at `point`, `modifiers` held.
pub fn primary_press(
    point: kurbo::Point,
    modifiers: ui_events::keyboard::Modifiers,
) -> ui_events::pointer::PointerButtonEvent {
    ui_events::pointer::PointerButtonEvent {
        button: Some(ui_events::pointer::PointerButton::Primary),
        pointer: mouse(),
        state: ui_events::pointer::PointerState {
            position: (point.x, point.y).into(),
            modifiers,
            ..Default::default()
        },
    }
}
