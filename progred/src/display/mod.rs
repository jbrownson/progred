//! Projection composition, box descriptions, and editor-facing native widgets.
//! Document recursion is a projection operation; boxes compose opaque continuations.

use gid::{CellId, Resolution, Step, Value};
use peniko::Brush;
use puri::Leaf;
pub use puri::delim::Delim;
#[cfg(test)]
pub use puri::delim::Side;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub mod builder;
mod measure;
#[cfg(all(test, feature = "layout-profile"))]
pub mod profile;
#[cfg(test)]
pub mod recording;
pub mod structure;
pub mod widget;

#[cfg(test)]
pub mod test_support;

static NEXT_SHARED_LAYOUT: AtomicUsize = AtomicUsize::new(0);

/// Editor-mapped paint face a Puri leaf asks for. Libraries pick a role,
/// not a color.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Name,
    String,
    Dim,
    Label,
    Id,
    AccentWash,
    Ink,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextFamily {
    #[default]
    SystemUi,
    Monospace,
}

/// The paint a Progred projection supplies to Puri. Faces defer to
/// the editor theme; a literal brush belongs to the projected
/// content itself.
#[derive(Clone)]
pub enum Paint {
    Face(Face),
    Brush(Brush),
}

pub use widget::line::{LineEdit, LineUpdate, layout as line_edit};

pub use widget::delimiter::{bracket, selectable_bracket};

pub use measured::RowAlignment;

/// A coordinate-free action. Interaction combinators decide which input
/// invokes it, in the same handler chain as raw pointer events.
pub type ActionHandler<World> = Rc<dyn Fn(&mut World) -> bool>;

pub use widget::gesture::{
    PointEvent, PointHandler, PointUpdate, ScrubEvent, ScrubGesture, ScrubUpdate, StateDragEvent,
    StateDragGesture, StateDragHandler,
};

#[cfg(test)]
pub use widget::gesture::ScrubHandler;

/// One value a projection suggests at an explicit completion control.
/// The host owns filtering and presentation; the projection owns the
/// contextual vocabulary and the value ultimately inserted.
#[derive(Clone)]
pub struct Completion {
    pub display: CompletionText,
    pub aliases: Vec<String>,
    pub detail: Option<CompletionText>,
    pub value: CompletionValue,
    /// Grap callable run at the committed location. Its selection and
    /// annotation effects are staged with the insertion.
    /// No continuation means no selection change; standard completion
    /// combinators supply their own selection transitions explicitly.
    pub on_commit: Option<Value>,
}

/// Names are resolved from the current sources when the picker is built,
/// before filtering, rather than copied into a retained offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionText {
    Literal(String),
    Name(CellId),
}

impl From<String> for CompletionText {
    fn from(text: String) -> Self {
        Self::Literal(text)
    }
}

impl From<&str> for CompletionText {
    fn from(text: &str) -> Self {
        Self::Literal(text.into())
    }
}

impl From<CellId> for CompletionText {
    fn from(cell: CellId) -> Self {
        Self::Name(cell)
    }
}

#[derive(Clone)]
pub enum CompletionValue {
    Literal(Value),
    Create(Rc<dyn Fn() -> Value>),
}

impl CompletionValue {
    pub fn literal(&self) -> Option<&Value> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Create(_) => None,
        }
    }

    pub fn instantiate(&self) -> Value {
        match self {
            Self::Literal(value) => value.clone(),
            Self::Create(create) => create(),
        }
    }
}

impl Completion {
    pub fn new(display: impl Into<CompletionText>, value: Value) -> Self {
        Self::with_value(display, CompletionValue::Literal(value))
    }

    /// Construct a value only when activated, for offers that mint fresh identities.
    pub fn generated(
        display: impl Into<CompletionText>,
        create: impl Fn() -> Value + 'static,
    ) -> Self {
        Self::with_value(display, CompletionValue::Create(Rc::new(create)))
    }

    fn with_value(display: impl Into<CompletionText>, value: CompletionValue) -> Self {
        Self {
            display: display.into(),
            aliases: Vec::new(),
            detail: None,
            value,
            on_commit: None,
        }
    }

    pub fn with_aliases(mut self, aliases: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_detail(mut self, detail: impl Into<CompletionText>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn on_commit(mut self, function: Value) -> Self {
        self.on_commit = Some(function);
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompletionScope {
    Suggested,
    Everything,
}

/// Read-only definition metadata; native implementations are never exposed here.
#[derive(Clone, Copy)]
pub struct ResolvedCell<'a> {
    pub source: Resolution,
    pub value: &'a Value,
    pub native: bool,
}

/// The path names the missing value, or the record receiving a new label.
/// Reads use that same source context, including source-qualified Follow steps.
pub struct CompletionRequest<'a> {
    pub query: &'a str,
    pub kind: CompletionKind,
    pub scope: CompletionScope,
    pub path: &'a [Step],
    pub value_at: &'a dyn Fn(&[Step]) -> Option<&'a Value>,
    pub resolve: &'a dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
}

impl CompletionRequest<'_> {
    pub fn value(&self) -> Option<&Value> {
        (self.value_at)(self.path)
    }
}

/// Invoked only for an active picker. None leaves vocabulary unspecified;
/// Some(empty) deliberately offers nothing except the Everything escape.
pub type CompletionProvider = Rc<dyn Fn(&CompletionRequest<'_>) -> Option<Vec<Completion>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Value,
    Field,
}

/// Pending editor structure immediately beneath the value being
/// projected. Projections use this to place the corresponding control;
/// they never need the host's absolute path or selection encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Field,
    Child(Step),
}

/// Selection and hover behavior for a location relative to the value
/// currently being projected. The host resolves the relative steps;
/// libraries never receive its absolute document path.
pub struct ProjectionTarget<World, Hover> {
    pub select: ActionHandler<World>,
    /// Select this location with an explicit data payload. Controls use
    /// it for transient modes without exposing the host's path.
    pub select_with: Rc<dyn Fn(&mut World, Value) -> bool>,
    pub hover: Hover,
}

/// A borrowed resolver for the current projection target and relative
/// targets beneath it. Resolving owns the returned interaction data;
/// merely trying a partial projection allocates nothing.
#[derive(Clone, Copy)]
pub struct ProjectionTargets<'a, World, Hover> {
    at: &'a dyn Fn(Vec<Step>) -> ProjectionTarget<World, Hover>,
    insert_after: Option<&'a dyn Fn(gid::Position) -> (Hover, ActionHandler<World>)>,
}

impl<'a, World, Hover> ProjectionTargets<'a, World, Hover> {
    pub fn new(at: &'a dyn Fn(Vec<Step>) -> ProjectionTarget<World, Hover>) -> Self {
        Self {
            at,
            insert_after: None,
        }
    }

    pub fn with_insert_after(
        mut self,
        insert: &'a dyn Fn(gid::Position) -> (Hover, ActionHandler<World>),
    ) -> Self {
        self.insert_after = Some(insert);
        self
    }

    pub fn insert_after(&self, position: gid::Position) -> Option<(Hover, ActionHandler<World>)> {
        self.insert_after.map(|insert| insert(position))
    }

    pub fn at(&self, steps: impl Into<Vec<Step>>) -> ProjectionTarget<World, Hover> {
        (self.at)(steps.into())
    }

    pub fn current(&self) -> ProjectionTarget<World, Hover> {
        (self.at)(Vec::new())
    }
}

/// A reusable layout program. Running it calls a builder directly; the
/// production builder retains only the graph needed for alternative selection.
pub struct Layout<World, Hover>(
    Rc<dyn Fn(&mut dyn builder::Builder<World, Hover>) -> builder::Node>,
);

impl<World, Hover> Clone for Layout<World, Hover> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<World: 'static, Hover: 'static> Layout<World, Hover> {
    pub fn new(
        run: impl Fn(&mut dyn builder::Builder<World, Hover>) -> builder::Node + 'static,
    ) -> Self {
        Self(Rc::new(run))
    }

    pub fn run(&self, builder: &mut dyn builder::Builder<World, Hover>) -> builder::Node {
        (self.0)(builder)
    }

    pub fn widget(widget: widget::Widget<World, Hover>) -> Self {
        Self::new(move |builder| builder.widget(widget.clone()))
    }

    pub fn program(program: widget::Program<World, Hover>) -> Self {
        Self::new(move |builder| builder.program(program.clone()))
    }
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    /// Apply a callable to values without evaluating those arguments as expressions.
    fn apply(&self, function: &Value, arguments: &[(CellId, Value)]) -> (Value, usize) {
        let evaluation = self.apply_scoped(function, arguments, None);
        (evaluation.result, evaluation.remaining_fuel)
    }

    /// Borrow capabilities for this application only. Their output remains host-owned.
    fn apply_scoped(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation;

    /// Remaining fuel is the evaluator budget left after this call,
    /// so a grap-shaped result can continue the same allowance.
    fn evaluate(&self, expression: &Value) -> (Value, usize);

    /// Evaluate with an explicit allowance. Hosts that do not expose
    /// fuel may keep their ordinary behavior.
    fn evaluate_with_fuel(&self, expression: &Value, _fuel: usize) -> (Value, usize) {
        self.evaluate(expression)
    }

    /// The selected definition's conventional human name.
    fn name(&self, _cell: CellId) -> Option<&str> {
        None
    }

    /// The selected definition and its source, without evaluation.
    fn resolve(&self, _cell: CellId) -> Option<ResolvedCell<'_>> {
        None
    }
}

/// Everything a partial projection receives for one value. The
/// default selection callback and matching hover claim may be used,
/// wrapped, ignored, or replaced; no central action vocabulary is
/// involved. Editor state arrives POSITIONALLY and as DATA: the
/// selection's payload when this value's path is the selected one,
/// and this path's annotation record — never an address, never a
/// store.
pub struct ProjectionInput<'a, World, Hover> {
    pub env: &'a dyn Env,
    /// The composed default for child projections. Clone and compose it
    /// explicitly when choosing a local override or a subtree scope.
    pub default_projection: Partial<World, Hover>,
    /// `None` is a missing location, distinct from every stored GID value.
    pub value: Option<&'a Value>,
    /// Physical pixels per logical display unit for this projection pass.
    pub scale_factor: f64,
    /// Whether this projected location can accept a document write.
    /// This exposes capability without exposing its host-owned path.
    pub writable: bool,
    /// The selection payload — stage, query, choice — iff this
    /// value's path is the selected one.
    pub selection: Option<&'a Value>,
    /// A new field or missing child currently being authored directly
    /// beneath this value, if any.
    pub pending: Option<Pending>,
    /// This path's annotation record (fold state and whatever joins it).
    pub state: Option<&'a Value>,
    /// Derive this value's interaction target, or one below it,
    /// without exposing the host's full path. The host work is lazy:
    /// a declining projection need never construct either target.
    pub targets: ProjectionTargets<'a, World, Hover>,
}

/// A partial borrows the shared projection input. Ordered composition
/// can therefore try declining projections without cloning interaction
/// targets that only the successful projection retains.
pub type Partial<World, Hover> = Rc<
    dyn for<'a, 'input> Fn(
        &'input ProjectionInput<'a, World, Hover>,
    ) -> Option<Layout<World, Hover>>,
>;

pub fn partial<World, Hover>(
    projection: impl for<'a, 'input> Fn(
        &'input ProjectionInput<'a, World, Hover>,
    ) -> Option<Layout<World, Hover>>
    + 'static,
) -> Partial<World, Hover> {
    Rc::new(projection)
}

/// First successful partial wins. An empty composition always declines.
pub fn compose_partials<World: 'static, Hover: 'static>(
    partials: impl IntoIterator<Item = Partial<World, Hover>>,
) -> Partial<World, Hover> {
    let partials: Vec<_> = partials.into_iter().collect();
    match partials.as_slice() {
        [] => partial(|_| None),
        [only] => only.clone(),
        _ => partial(move |input| partials.iter().find_map(|projection| projection(input))),
    }
}

#[cfg(test)]
pub fn text<World: 'static, Hover: 'static>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Name)
}

pub fn dim<World: 'static, Hover: 'static>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Dim)
}

pub fn id<World: 'static, Hover: 'static>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Id)
}

pub fn faced<World: 'static, Hover: 'static>(
    text: impl Into<String>,
    face: Face,
) -> Layout<World, Hover> {
    leaf(Leaf::Text {
        text: text.into(),
        paint: Paint::Face(face),
        script: puri::text::Script::Normal,
    })
}

pub fn subscript<World: 'static, Hover: 'static>(
    text: impl Into<String>,
    face: Face,
) -> Layout<World, Hover> {
    leaf(Leaf::Text {
        text: text.into(),
        paint: Paint::Face(face),
        script: puri::text::Script::Subscript,
    })
}

pub fn completion(
    kind: CompletionKind,
    provider: Option<CompletionProvider>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    Layout::widget(Rc::new(move |context| {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Completion);
        crate::projection::completion::control(context, kind, provider.as_ref())
    }))
}

pub fn slot<World: 'static, Hover: 'static>() -> Layout<World, Hover> {
    Layout::widget(Rc::new(|context| {
        widget::empty(context.text, context.inputs.styles)
    }))
}

pub fn leaf<World: 'static, Hover: 'static>(leaf: Leaf<Paint>) -> Layout<World, Hover> {
    Layout::new(move |builder| builder.leaf(&leaf))
}

pub use widget::hover::{block_hover, hover_highlight, on_hover};
pub use widget::interaction::{on_activate, on_click, pickable};

pub fn activatable<World: 'static, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    on_hover(on_activate(child, target.clone(), handler), target)
}

pub use widget::gesture::{on_point, on_scrub, on_state_drag};

pub fn row<World: 'static, Hover: 'static>(
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    aligned_row(RowAlignment::Baseline, gap, children)
}

pub fn centered_row<World: 'static, Hover: 'static>(
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    aligned_row(RowAlignment::Center, gap, children)
}

pub fn aligned_row<World: 'static, Hover: 'static>(
    alignment: RowAlignment,
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    let children: Vec<_> = children.into_iter().collect();
    Layout::new(move |builder| {
        let children = children.iter().map(|child| child.run(builder)).collect();
        builder.row(alignment, gap, children)
    })
}

pub fn col<World: 'static, Hover: 'static>(
    baseline: usize,
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    let children: Vec<_> = children.into_iter().collect();
    Layout::new(move |builder| {
        let children = children.iter().map(|child| child.run(builder)).collect();
        builder.col(baseline, gap, children)
    })
}

pub fn overlay<World: 'static, Hover: 'static>(
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    let children: Vec<_> = children.into_iter().collect();
    Layout::new(move |builder| {
        let children = children.iter().map(|child| child.run(builder)).collect();
        builder.overlay(children)
    })
}

pub type FloatingPosition =
    Rc<dyn Fn(f64, puri::Placement, measured::Extent) -> Option<puri::Placement>>;

pub fn floating<World: 'static, Hover: 'static>(
    base: Layout<World, Hover>,
    content: Layout<World, Hover>,
    position: impl Fn(f64, puri::Placement, measured::Extent) -> Option<puri::Placement> + 'static,
) -> Layout<World, Hover> {
    let position: FloatingPosition = Rc::new(position);
    Layout::new(move |builder| {
        let base = base.run(builder);
        let content = content.run(builder);
        builder.floating(base, content, position.clone())
    })
}

pub use widget::popover::popover;

pub fn padding<World: 'static, Hover: 'static>(
    insets: peniko::kurbo::Insets,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    Layout::new(move |builder| {
        let child = child.run(builder);
        builder.pad(insets, child)
    })
}

pub fn pad<World: 'static, Hover: 'static>(
    left: f64,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    padding((left, 0.0, 0.0, 0.0).into(), child)
}

pub use widget::border;

/// Project record-shaped fields in a caller-supplied order. The
/// caller owns each field's meaning — label behavior, child
/// projection, and edit policy — while this combinator owns the
/// braced flat and column forms.
pub struct RecordField<World, Hover> {
    pub label: Layout<World, Hover>,
    pub value: Layout<World, Hover>,
}

#[cfg(test)]
pub fn record<'a>(
    fields: impl IntoIterator<Item = (CellId, &'a Value)>,
    order: impl FnMut(&CellId, &CellId) -> Ordering,
    field: impl FnMut(CellId, &'a Value) -> RecordField<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    record_with(fields, order, field, [])
}

/// The record layout with explicit trailing fields, such as the one
/// pending field currently being authored. They participate in both
/// responsive forms but not in sorting the stored fields.
pub fn record_with<'a>(
    fields: impl IntoIterator<Item = (CellId, &'a Value)>,
    mut order: impl FnMut(&CellId, &CellId) -> Ordering,
    mut field: impl FnMut(CellId, &'a Value) -> RecordField<crate::Editor, crate::frame::Hovered>,
    trailing: impl IntoIterator<Item = RecordField<crate::Editor, crate::frame::Hovered>>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.sort_by(|(left, _), (right, _)| order(left, right));
    record_heads(
        fields
            .into_iter()
            .map(|(key, value)| field(key, value))
            .chain(trailing)
            .map(|field| RecordField {
                label: row(0.0, [field.label, dim(":")]),
                value: field.value,
            }),
        [],
    )
}

/// Shared record geometry. Heads already include punctuation and its
/// interaction target; trailing rows can be incomplete field editors.
pub fn record_heads(
    fields: impl IntoIterator<Item = RecordField<crate::Editor, crate::frame::Hovered>>,
    trailing: impl IntoIterator<Item = Layout<crate::Editor, crate::frame::Hovered>>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    let fields = fields
        .into_iter()
        .map(|field| RecordField {
            label: shared(field.label),
            value: shared(field.value),
        })
        .collect::<Vec<_>>();
    let mut flat = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        flat.push(row(
            0.0,
            [field.label.clone(), dim(" "), field.value.clone()],
        ));
    }
    let mut rows = fields
        .into_iter()
        .map(|field| hug(field.label, field.value, 6.0, 20.0))
        .collect::<Vec<_>>();
    for tail in trailing {
        let tail = shared(tail);
        if !flat.is_empty() {
            flat.push(dim(", "));
        }
        flat.push(tail.clone());
        rows.push(tail);
    }
    selectable_bracket(
        Delim::Brace,
        alternatives([row(0.0, flat), col(0, 2.0, rows)]),
    )
}

pub fn hug<World: 'static, Hover: Clone + 'static>(
    head: Layout<World, Hover>,
    child: Layout<World, Hover>,
    gap: f64,
    tab: f64,
) -> Layout<World, Hover> {
    let head = shared(head);
    let child = shared(child);
    alternatives([
        row(gap, [head.clone(), child.clone()]),
        col(0, 2.0, [head, pad(tab, child)]),
    ])
}

/// Share one projected child between mutually exclusive alternatives.
/// This describes an explicit edge in the layout DAG, not a cache:
/// the child is projected and measured once per frame. A selected
/// concrete form must contain at most one use.
pub fn shared<World: 'static, Hover: 'static>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    let id = NEXT_SHARED_LAYOUT.fetch_add(1, AtomicOrdering::Relaxed);
    Layout::new(move |builder| builder.shared(id, &child))
}

pub fn descend<World: 'static, Hover: 'static>(
    step: Step,
    projection: Option<Partial<World, Hover>>,
    default_projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::program(Rc::new(move |context, build| {
        context.project.descend(
            context.text,
            build,
            step.clone(),
            projection.clone(),
            default_projection.clone(),
        )
    }))
}

pub fn at<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
) -> Layout<World, Hover> {
    at_with_projection(steps, value, None, None)
}

pub fn at_with_projection<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    projection: Option<Partial<World, Hover>>,
    default_projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    let steps = steps.into();
    let value = value.clone();
    Layout::program(Rc::new(move |context, build| {
        context.project.at(
            context.text,
            build,
            steps.clone(),
            value.clone(),
            projection.clone(),
            default_projection.clone(),
        )
    }))
}

/// Try a specialization here, keeping the default for descendants.
pub fn descend_local<World: 'static, Hover: 'static>(
    step: Step,
    projection: Partial<World, Hover>,
    default: &Partial<World, Hover>,
) -> Layout<World, Hover> {
    descend(
        step,
        Some(compose_partials([projection, default.clone()])),
        Some(default.clone()),
    )
}

pub fn at_local<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    projection: Partial<World, Hover>,
    default: &Partial<World, Hover>,
) -> Layout<World, Hover> {
    at_with_projection(
        steps,
        value,
        Some(compose_partials([projection, default.clone()])),
        Some(default.clone()),
    )
}

/// Extend the default for both this value and its descendants.
pub fn at_scoped<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    projection: Partial<World, Hover>,
    default: &Partial<World, Hover>,
) -> Layout<World, Hover> {
    let projection = compose_partials([projection, default.clone()]);
    at_with_projection(steps, value, Some(projection.clone()), Some(projection))
}

pub fn transient<World: 'static, Hover: 'static>(
    value: &Value,
    fuel: usize,
) -> Layout<World, Hover> {
    let value = value.clone();
    Layout::program(Rc::new(move |context, build| {
        context
            .project
            .transient(context.text, build, value.clone(), fuel)
    }))
}

pub fn alternatives<World: 'static, Hover: 'static>(
    options: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    let options: Vec<_> = options.into_iter().collect();
    Layout::new(move |builder| {
        let options = options.iter().map(|option| option.run(builder)).collect();
        builder.alternatives(options)
    })
}

/// If both values are records, `patch` fields win on shared keys.
/// Otherwise `patch`.
pub fn overlay_value(current: &Value, patch: Value) -> Value {
    match (current.as_record(), patch.as_record()) {
        (Some(current), Some(patch)) => {
            let mut overlaid = current.clone();
            for (field, value) in patch {
                overlaid.insert(*field, value.clone());
            }
            Value::Record(overlaid)
        }
        _ => patch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recorded, record};
    use crate::display::test_support::{ProjectionCall, inspect};

    fn unshared<World, Hover>(mut layout: &Recorded<World, Hover>) -> &Recorded<World, Hover> {
        while let Recorded::Shared { child, .. } = layout {
            layout = child.as_ref();
        }
        layout
    }

    fn probe(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
        None
    }

    fn run_partial(projection: &Partial<(), ()>) -> Option<Layout<(), ()>> {
        struct NoEval;
        impl Env for NoEval {
            fn apply_scoped(
                &self,
                _: &Value,
                _: &[(CellId, Value)],
                _scope: Option<&grap::ForeignOverlay<'_>>,
            ) -> grap::Evaluation {
                panic!("unexpected application")
            }

            fn evaluate(&self, _: &Value) -> (Value, usize) {
                panic!("unexpected evaluation")
            }
        }

        projection(&ProjectionInput {
            default_projection: partial(|_| None),
            env: &NoEval,
            value: Some(&Value::record([])),
            scale_factor: 1.0,
            writable: false,
            selection: None,
            pending: None,
            state: None,
            targets: ProjectionTargets::new(&|_| panic!("unexpected target lookup")),
        })
    }

    #[test]
    fn empty_partial_composition_is_an_identity() {
        let empty = compose_partials([]);
        assert!(run_partial(&empty).is_none());
        let success = partial(|_| Some(slot()));
        for projection in [
            compose_partials([empty.clone(), success.clone()]),
            compose_partials([success, empty]),
        ] {
            assert!(run_partial(&projection).is_some());
        }
    }

    #[test]
    fn partial_composition_preserves_order_and_stops_at_success() {
        let visited = Rc::new(std::cell::RefCell::new(Vec::new()));
        let projection = |index, succeeds: bool| {
            let visited = visited.clone();
            partial(move |_| {
                visited.borrow_mut().push(index);
                succeeds.then(slot)
            })
        };
        let first = projection(1, false);
        let second = projection(2, false);
        let third = projection(3, true);
        let unreachable = partial(|_| panic!("later partial must not run"));
        for combined in [
            compose_partials([
                first.clone(),
                second.clone(),
                third.clone(),
                unreachable.clone(),
            ]),
            compose_partials([
                compose_partials([first.clone(), second.clone()]),
                compose_partials([third.clone(), unreachable.clone()]),
            ]),
            compose_partials([first, compose_partials([second, third, unreachable])]),
        ] {
            visited.borrow_mut().clear();
            assert!(run_partial(&combined).is_some());
            assert_eq!(*visited.borrow(), [1, 2, 3]);
        }
    }

    #[test]
    fn all_declining_partials_leave_fallback_to_the_caller() {
        assert!(run_partial(&compose_partials([partial(probe), partial(probe)])).is_none());
    }

    #[test]
    fn record_uses_the_supplied_order_in_both_responsive_forms() {
        const FIRST: CellId = CellId::from_u128(1);
        const SECOND: CellId = CellId::from_u128(2);
        let first = Value::from(vec![1]);
        let second = Value::from(vec![2]);
        let layout: Layout<crate::Editor, crate::frame::Hovered> = crate::display::record(
            [(FIRST, &first), (SECOND, &second)],
            |left, right| right.cmp(left),
            |key, value| RecordField {
                label: dim("field"),
                value: at([Step::Key(key)], value),
            },
        );
        let layout = record(&layout);
        let (_, child, _) = crate::display::test_support::delimited(&layout);
        let Recorded::Alternatives(forms) = child else {
            panic!("a record has responsive forms");
        };
        let Recorded::Row { children, .. } = &forms[0] else {
            panic!("the first form is flat");
        };
        let Recorded::Row {
            children: first, ..
        } = &children[0]
        else {
            panic!("a flat field keeps its label and value together");
        };
        assert!(matches!(&inspect(&(unshared(&first[2]))),
            ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
        let Recorded::Row {
            children: second, ..
        } = &children[2]
        else {
            panic!("a flat field keeps its label and value together");
        };
        assert!(matches!(&inspect(&(unshared(&second[2]))),
            ProjectionCall::At { steps, .. } if *steps == [Step::Key(FIRST)]
        ));
        let Recorded::Col { children, .. } = &forms[1] else {
            panic!("the second form is a column");
        };
        let Recorded::Alternatives(first) = &children[0] else {
            panic!("a column field may break after its label");
        };
        let Recorded::Row {
            children: inline, ..
        } = &first[0]
        else {
            panic!("a field first stays inline");
        };
        assert!(matches!(&inspect(&(unshared(&inline[1]))),
            ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
        let Recorded::Col {
            children: broken, ..
        } = &first[1]
        else {
            panic!("a field may put its value below its label");
        };
        let Recorded::Pad { child, .. } = &broken[1] else {
            panic!("a broken value is indented");
        };
        assert!(matches!(&inspect(&(unshared(child))),
            ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
    }
}

pub fn drawing_program(
    extent: measured::Extent,
    fuel: usize,
    program: Value,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    Layout::widget(Rc::new(move |context| {
        crate::projection::drawing::program_leaf(
            context.inputs,
            context.path,
            extent.width,
            extent.ascent,
            extent.descent,
            fuel,
            program.clone(),
        )
    }))
}
