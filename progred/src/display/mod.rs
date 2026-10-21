//! Projection composition, box descriptions, and editor-facing native widgets.
//! Document recursion is a projection operation; boxes compose opaque continuations.

use gid::{CellId, Resolution, Step, Value};
use peniko::Brush;
pub use puri::delim::Delim;
use puri::draw::Leaf;
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub mod builder;
pub mod fold;
pub mod host;
pub use host::Host;
mod measure;
#[cfg(all(test, feature = "layout-profile"))]
pub mod profile;
pub mod raw;
#[cfg(test)]
pub mod recording;
pub mod structure;
mod viewing;
pub(crate) use viewing::Toolkit;
pub use viewing::Viewing;
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

pub use widget::line::{LineEdit, LineUpdate, line_edit};

pub use widget::delimiter::{bracket, selectable_bracket};

pub use widget::offers::ranked;
pub use widget::picker::{Picker, line_hole, picker};

pub use measured::RowAlignment;

/// A coordinate-free action, run through the session a dispatch lends.
/// Interaction combinators decide which input invokes it, in the same handler
/// chain as raw pointer events.
pub type Action = Rc<dyn Fn(&mut crate::editing::Session<'_>) -> bool>;

pub use widget::gesture::{PointEvent, PointHandler, PointUpdate, StateDragHandler};

/// An offer at an explicit completion control. Activation, not highlighting,
/// runs its handler. The handler owns any edits or selection transitions.
#[derive(Clone)]
pub struct Completion {
    pub display: CompletionText,
    pub aliases: Vec<String>,
    pub detail: Option<CompletionText>,
    pub activate: Rc<dyn Fn(&mut crate::editing::Session<'_>)>,
    pub face: Face,
    /// Optional advertised value, for source attribution and contextual filtering.
    /// Activation never derives its behavior from this metadata.
    pub preview: Option<Value>,
}

/// Names are resolved from the current sources when the picker is built,
/// before filtering, rather than copied into a retained offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionText {
    Literal(String),
    Name(CellId),
    /// A name given here rather than a cell's, as a computed one is.
    Named(Name<'static>),
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

/// A cell's name: plain text, or a formatted name's runs, each plain or a
/// subscript, with any cell it names spelled as that cell's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Name<'a> {
    Plain(&'a str),
    Formatted(Rc<[(String, puri::text::Script)]>),
}

impl<'a> Name<'a> {
    /// The name as one string: what's typed matches it, and names sort by it.
    pub fn spelling(&self) -> std::borrow::Cow<'a, str> {
        match self {
            Self::Plain(text) => (*text).into(),
            Self::Formatted(runs) => runs.iter().map(|(text, _)| text.as_str()).collect(),
        }
    }

    /// Where in its [spelling](Self::spelling) the name's subscripts are.
    pub fn subscripts(&self) -> Vec<std::ops::Range<usize>> {
        let Self::Formatted(runs) = self else {
            return Vec::new();
        };
        let mut at = 0;
        runs.iter()
            .filter_map(|(text, script)| {
                let range = at..at + text.len();
                at = range.end;
                (*script == puri::text::Script::Subscript).then_some(range)
            })
            .collect()
    }
}

/// `name` drawn in `face`, its subscripts small and dim.
pub fn named<World: 'static, Hover: 'static>(name: &Name<'_>, face: Face) -> Layout<World, Hover> {
    match name {
        Name::Plain(text) => faced(*text, face),
        Name::Formatted(runs) => row(
            2.0,
            runs.iter().map(|(text, script)| match script {
                puri::text::Script::Normal => faced(text.as_str(), face),
                puri::text::Script::Subscript => subscript(text.as_str(), Face::Dim),
            }),
        ),
    }
}

impl Completion {
    pub fn new(
        display: impl Into<CompletionText>,
        activate: impl Fn(&mut crate::editing::Session<'_>) + 'static,
    ) -> Self {
        Self {
            display: display.into(),
            aliases: Vec::new(),
            detail: None,
            activate: Rc::new(activate),
            face: Face::Label,
            preview: None,
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
}

/// What a library is asked.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompletionScope {
    /// The values and calls it offers anywhere, such as a new color, for a
    /// projection that wants them among its own, as Grap's slots do.
    Everything,
    /// What was typed, read as one of a library's values, like a number or a
    /// string, or as a field's label, like a new cell with that name. Raw asks
    /// on every request, so a provider answers it cheaply; the picker lists it
    /// where a typed value goes instead of ranking it.
    Literal,
    /// What a new document can start as, such as a Grap program, for the
    /// hole of a document's empty root.
    Start,
}

/// Read-only definition metadata; native implementations are never exposed here.
#[derive(Clone, Copy)]
pub struct ResolvedCell<'a> {
    /// Whether the document or a library defines it.
    pub resolution: Resolution,
    pub value: &'a Value,
    pub native: bool,
}

/// The path names the missing value, or the record receiving a new label,
/// from where its location starts: the document's root, or the definition of
/// the cell it's in. Reads use that same context, so a cell's definition is
/// read as its own, however it was reached.
pub struct CompletionRequest<'a> {
    pub query: &'a str,
    pub kind: CompletionKind,
    pub scope: CompletionScope,
    pub path: &'a [Step],
    pub value_at: &'a dyn Fn(&[Step]) -> Option<&'a Value>,
    pub resolve: &'a dyn Fn(CellId) -> Option<ResolvedCell<'a>>,
    pub cells: &'a dyn Fn() -> Vec<CellId>,
    /// A cell's name as the document names it, for offers named by their
    /// cell. Raw's offers are named too, though Raw shows values' ids.
    pub name: &'a dyn Fn(CellId) -> Option<Name<'a>>,
    /// The loaded libraries' offers, for holes that list some of them, as
    /// Raw's list their readings of what's typed.
    pub libraries: Option<&'a CompletionProvider>,
}

impl CompletionRequest<'_> {
    pub fn value(&self) -> Option<&Value> {
        (self.value_at)(self.path)
    }
}

/// Invoked only for an active picker. A projection's provider is the whole
/// vocabulary of the holes it draws; a library's answers None where it has
/// nothing to add.
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

/// A library the document defines: the keys it owns, and the projection of a
/// record carrying any of them.
pub struct DocumentLibrary {
    pub keys: Vec<CellId>,
    pub projection: grap::RuntimeValue,
}

/// A pure evaluation a computed value came from: what ran, where it's from,
/// and its allowance, so it can be run again, with the same answer, to ask how
/// it got there.
#[derive(Clone, Debug)]
pub struct Evaluated {
    pub expression: grap::RuntimeValue,
    pub origin: Option<grap::SourceOrigin>,
    pub fuel: usize,
}

impl Evaluated {
    pub fn same(&self, other: &Self) -> bool {
        self.expression.same_result(&other.expression)
            && self.origin == other.origin
            && self.fuel == other.fuel
    }
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    /// Expression-facing application, without materializing the callable or
    /// its arguments: Grap parameters receive values, while foreign functions
    /// may interpret argument syntax themselves.
    fn apply_expression_runtime(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
    ) -> grap::RuntimeValue;

    /// Evaluate `expression`, stored `steps` below this occurrence, with an
    /// explicit allowance.
    fn evaluate_runtime(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[Step],
    ) -> grap::RuntimeValue;

    /// [`Env::evaluate_runtime`], reusing a completed evaluation when its
    /// observed inputs are unchanged. Hosts without a computation runtime may
    /// simply evaluate. Untracked reads, effects, and halts prevent reuse in
    /// hosts that provide memoization.
    fn evaluate_runtime_memo(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[Step],
    ) -> grap::RuntimeValue;

    /// The evaluation of `expression`, stored `steps` below this occurrence,
    /// with `fuel`, as [`Env::evaluate_runtime`] runs it.
    fn evaluation(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        _steps: &[Step],
    ) -> Evaluated {
        Evaluated {
            expression: expression.clone(),
            origin: None,
            fuel,
        }
    }

    /// The evaluation the computed value here came from, if it can be run
    /// again.
    fn evaluated(&self) -> Option<&Evaluated> {
        None
    }

    /// The calls under which a call at `call` answered `value`, itself first,
    /// found by running again the evaluation the computed value here came
    /// from. None where it didn't come from one, or no such call answered it.
    fn stack(&self, _call: &grap::SourceOrigin, _value: &Value) -> Option<Vec<grap::SourceOrigin>> {
        None
    }

    /// Borrow capabilities for this application only. Their output remains host-owned.
    fn apply_runtime_scoped(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation;

    /// Reuse an application with explicit data arguments, under the same
    /// dependency/effect contract as [`Env::evaluate_runtime_memo`].
    fn apply_runtime_memo(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue;

    /// The selected definition's conventional human name.
    fn name(&self, _cell: CellId) -> Option<Name<'_>> {
        None
    }

    /// The selected definition and its source, without evaluation.
    fn resolve(&self, _cell: CellId) -> Option<ResolvedCell<'_>> {
        None
    }

    /// The libraries the document declares at its root.
    fn libraries(&self) -> Vec<DocumentLibrary> {
        Vec::new()
    }

    /// Where the value being projected occurs.
    fn occurrence(&self) -> &crate::occurrence::Trail {
        crate::occurrence::Trail::ROOT
    }

    /// The stored value `path` reaches from `start`: what a projection reads
    /// above where its own value is stored, as a picker does to find what's
    /// in scope.
    fn read_from(&self, _start: crate::location::Start, _path: &[Step]) -> Option<&Value> {
        None
    }

    /// The value `path` reaches in the computed value the value being
    /// projected lies in, as data: [`Env::read_from`] where nothing is stored.
    fn read_computed(&self, _path: &[Step]) -> Option<&Value> {
        None
    }
}

/// Everything a partial projection receives for one value. It lends its
/// occurrence as a site to act on, which [`ProjectionInput::site`] gives; no
/// central action vocabulary is involved. Editor state arrives POSITIONALLY
/// and as DATA: the selection's payload when this value's path is the
/// selected one, and this path's annotation record — never an address,
/// never a store.
pub struct ProjectionInput<'a, World, Hover, V = Value> {
    pub env: &'a dyn Env,
    /// The composed default for child projections. Clone and compose it
    /// explicitly when choosing a local override or a subtree scope.
    pub default_projection: Partial<World, Hover>,
    /// `None` is a missing location, distinct from every stored GID value.
    pub value: Option<&'a V>,
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
    /// Builds this occurrence's site, which [`ProjectionInput::site`] lends.
    /// The host work is lazy: a declining projection need never build it.
    pub(crate) occurrence: &'a dyn Fn() -> crate::editing::Site,
}

/// A partial borrows the shared projection input. Ordered composition
/// can therefore try declining projections without cloning interaction
/// targets that only the successful projection retains.
pub type Partial<World, Hover> = Rc<
    dyn for<'a, 'input> Fn(
        &'input ProjectionInput<'a, World, Hover, grap::RuntimeValue>,
    ) -> Option<Layout<World, Hover>>,
>;

pub fn partial<World, Hover>(
    projection: impl for<'a, 'input> Fn(
        &'input ProjectionInput<'a, World, Hover>,
    ) -> Option<Layout<World, Hover>>
    + 'static,
) -> Partial<World, Hover> {
    Rc::new(move |input| {
        projection(&input.with_value(input.value.map(grap::RuntimeValue::as_value)))
    })
}

impl<'a, World, Hover, V> ProjectionInput<'a, World, Hover, V> {
    pub fn with_value<'b, T>(
        &'b self,
        value: Option<&'b T>,
    ) -> ProjectionInput<'b, World, Hover, T> {
        ProjectionInput {
            env: self.env,
            default_projection: self.default_projection.clone(),
            value,
            scale_factor: self.scale_factor,
            writable: self.writable,
            selection: self.selection,
            pending: self.pending.clone(),
            state: self.state,
            occurrence: self.occurrence,
        }
    }

    /// This occurrence, as a site to act on; [`crate::editing::Site::below`]
    /// gives one beneath it.
    pub(crate) fn site(&self) -> crate::editing::Site {
        (self.occurrence)()
    }
}

pub fn runtime_partial<World, Hover>(
    projection: impl for<'a, 'input> Fn(
        &'input ProjectionInput<'a, World, Hover, grap::RuntimeValue>,
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
        [] => runtime_partial(|_| None),
        [only] => only.clone(),
        _ => runtime_partial(move |input| partials.iter().find_map(|projection| projection(input))),
    }
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

pub fn slot<World: 'static, Hover: 'static>() -> Layout<World, Hover> {
    Layout::widget(Rc::new(|context| {
        widget::empty(context.text, context.toolkit.styles())
    }))
}

pub fn leaf<World: 'static, Hover: 'static>(leaf: Leaf<Paint>) -> Layout<World, Hover> {
    Layout::new(move |builder| builder.leaf(&leaf))
}

pub use widget::hover::{block_hover, hover_highlight, on_hover};
pub use widget::interaction::{on_activate, pickable};
pub use widget::navigation::{group, stop};

/// `child`, which hovers `site` and selects it when activated.
pub(crate) fn activatable<W: Host>(
    child: host::Layout<W>,
    site: &crate::editing::Site,
) -> host::Layout<W> {
    let target = W::Hover::from(site.hover());
    let select = widget::navigation::selecting(site.clone());
    on_hover(on_activate(child, target.clone(), select), target)
}

/// `child` drawn as the label `key` of the field at `site`. It hovers apart
/// from the field's value, so a Cmd+click picks the label, not the value; a
/// click selects the field if `select`.
pub(crate) fn field_label_at<W: Host>(
    child: host::Layout<W>,
    site: &crate::editing::Site,
    key: CellId,
    select: bool,
) -> host::Layout<W> {
    let target = W::Hover::from(site.label_hover());
    let select = select.then(|| widget::navigation::selecting(site.clone()));
    on_hover(
        widget::interaction::activate_or_pick(child, target.clone(), select, Value::from(key)),
        target,
    )
}

/// `child` drawn for the whole `value` at `site`: a navigation stop that a
/// click selects and a Cmd+click picks.
pub(crate) fn selectable_value<W: Host>(
    child: host::Layout<W>,
    site: &crate::editing::Site,
    value: &grap::RuntimeValue,
) -> host::Layout<W> {
    let child = stop(child);
    activatable(
        pickable(child, W::Hover::from(site.hover()), value.clone()),
        site,
    )
}

pub use widget::gesture::{drag_outlined, on_point, on_state_drag};

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

/// How far a body below its head is indented.
const TAB: f64 = 20.0;

/// Prefer head beside body; otherwise put an indented body below the head.
/// Navigation follows only the chosen presentation, never both alternatives.
pub fn hug<World: 'static, Hover: Clone + 'static>(
    head: Layout<World, Hover>,
    child: Layout<World, Hover>,
    gap: f64,
) -> Layout<World, Hover> {
    let head = shared(head);
    let child = shared(child);
    alternatives([
        row(gap, [head.clone(), child.clone()]),
        col(0, 2.0, [head, indent(TAB, child)]),
    ])
}

/// A selectable whole around either head/body presentation.
pub fn group_hug<World: 'static, Hover: Clone + 'static>(
    head: Layout<World, Hover>,
    child: Layout<World, Hover>,
    gap: f64,
) -> Layout<World, Hover> {
    group(hug(head, child, gap))
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

/// Where floating content goes, from the scale, its anchor's placement, its
/// extent, and the bounds floats may use.
pub type FloatingPosition =
    Rc<dyn Fn(f64, uig::Placement, measured::Extent, kurbo::Rect) -> Option<uig::Placement>>;

pub fn floating<World: 'static, Hover: 'static>(
    base: Layout<World, Hover>,
    content: Layout<World, Hover>,
    position: impl Fn(f64, uig::Placement, measured::Extent, kurbo::Rect) -> Option<uig::Placement>
    + 'static,
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
    insets: kurbo::Insets,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    Layout::new(move |builder| {
        let child = child.run(builder);
        builder.pad(insets, child)
    })
}

/// Place a child in the middle of the width its container offers it.
pub fn center<World: 'static, Hover: 'static>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    widget::around(
        child,
        Rc::new(|_| {
            Box::new(
                |child: measured::Measured<widget::HoverPass<World, Hover>>| {
                    let extent = child.extent;
                    measured::around_into(child, move |placement, inner, out| {
                        let offered = placement.available_rect;
                        let x = offered.x0 + ((offered.width() - extent.width) / 2.0).max(0.0);
                        let rect = extent.rect_at(kurbo::Point::new(x, placement.rect.y0));
                        inner.place_at_into(
                            measured::child_placement(placement, rect).with_available_rect(offered),
                            out,
                        )
                    })
                },
            )
        }),
    )
}

/// A right arrow drawn to the size of the dim text beside it, rather than
/// the font's `→`, which many faces draw tiny. It has the delimiters' stem
/// and glyph box.
pub fn arrow<World: 'static, Hover: 'static>() -> Layout<World, Hover> {
    Layout::widget(Rc::new(|context| {
        let em = f64::from(context.toolkit.styles().dim.size) * context.toolkit.styles().scale;
        let ink = context.toolkit.styles().dim.brush.clone();
        let (ascent, descent) = puri::delim::minimum_span(em);
        widget::paint(
            widget::Extent {
                width: em,
                ascent,
                descent,
            },
            move |canvas, placement| {
                let axis = placement.rect.y0 + ascent - 0.27 * em;
                let (tail, tip) = (placement.rect.x0 + 0.12 * em, placement.rect.x1 - 0.12 * em);
                let head = 0.24 * em;
                let mut path = kurbo::BezPath::new();
                path.move_to((tail, axis));
                path.line_to((tip, axis));
                path.move_to((tip - head, axis - 0.85 * head));
                path.line_to((tip, axis));
                path.line_to((tip - head, axis + 0.85 * head));
                canvas.stroke_shape(
                    puri::draw::Shape::Path(path),
                    kurbo::Stroke::new(0.075 * em)
                        .with_caps(kurbo::Cap::Round)
                        .with_join(kurbo::Join::Round),
                    ink,
                    kurbo::Affine::IDENTITY,
                );
            },
        )
    }))
}

const RULE_SPACE: f64 = 3.0;
const RULE_THICKNESS: f64 = 1.0;

/// A line across the width its container offers it, with space above and
/// below. Its baseline is its bottom, so a column aligned on it holds the
/// line just above the surrounding text's baseline.
pub fn rule<World: 'static, Hover: 'static>() -> Layout<World, Hover> {
    Layout::widget(Rc::new(|context| {
        let scale = context.toolkit.styles().scale;
        let ink = context.toolkit.styles().ink.brush.clone();
        measured::fill_width(widget::paint(
            widget::Extent {
                width: 0.0,
                ascent: (2.0 * RULE_SPACE + RULE_THICKNESS) * scale,
                descent: 0.0,
            },
            move |canvas, placement| {
                let top = placement.rect.y0 + RULE_SPACE * scale;
                canvas.fill_shape(
                    puri::draw::Shape::Rect(kurbo::Rect::new(
                        placement.rect.x0,
                        top,
                        placement.rect.x1,
                        top + RULE_THICKNESS * scale,
                    )),
                    ink,
                    kurbo::Affine::IDENTITY,
                );
            },
        ))
    }))
}

pub fn indent<World: 'static, Hover: 'static>(
    left: f64,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    padding((left, 0.0, 0.0, 0.0).into(), child)
}

pub use widget::{border, ground};

/// Project record-shaped fields in a caller-supplied order. The
/// caller owns each field's meaning — label behavior, child
/// projection, and edit policy — while this combinator owns the
/// braced flat and column forms.
pub struct RecordField<World, Hover> {
    pub label: Layout<World, Hover>,
    pub value: Layout<World, Hover>,
}

/// Ordered fields without a whole-value stop; the enclosing form owns it.
/// Trailing fields, such as one being authored, follow in both responsive
/// forms but take no part in sorting the stored fields.
pub fn record_fragment_with<W: Host, T>(
    fields: impl IntoIterator<Item = (CellId, T)>,
    order: impl FnMut(&CellId, &CellId) -> Ordering,
    field: impl FnMut(CellId, T) -> RecordField<W, W::Hover>,
    trailing: impl IntoIterator<Item = RecordField<W, W::Hover>>,
) -> host::Layout<W> {
    record_fragment(
        ordered_record_fields(fields, order, field)
            .chain(trailing)
            .map(record_colon),
        [],
    )
}

fn ordered_record_fields<W, H, T>(
    fields: impl IntoIterator<Item = (CellId, T)>,
    mut order: impl FnMut(&CellId, &CellId) -> Ordering,
    mut field: impl FnMut(CellId, T) -> RecordField<W, H>,
) -> impl Iterator<Item = RecordField<W, H>> {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.sort_by(|(left, _), (right, _)| order(left, right));
    fields
        .into_iter()
        .map(move |(key, value)| field(key, value))
}

fn record_colon<W: 'static, H: 'static>(field: RecordField<W, H>) -> RecordField<W, H> {
    RecordField {
        label: row(0.0, [field.label, dim(":")]),
        value: field.value,
    }
}

/// Shared record geometry. Heads already include punctuation and its
/// interaction target; trailing rows can be incomplete field editors.
pub fn record_heads<W: Host>(
    fields: impl IntoIterator<Item = RecordField<W, W::Hover>>,
    trailing: impl IntoIterator<Item = host::Layout<W>>,
) -> host::Layout<W> {
    let [horizontal, vertical] = record_forms(fields, trailing);
    selectable_bracket(Delim::Brace, group(alternatives([horizontal, vertical])))
}

/// A record-shaped fragment of a larger projection. Its fields compose their
/// navigation, but the enclosing projection owns the whole-value stop.
pub fn record_fragment<W: Host>(
    fields: impl IntoIterator<Item = RecordField<W, W::Hover>>,
    trailing: impl IntoIterator<Item = host::Layout<W>>,
) -> host::Layout<W> {
    selectable_bracket(Delim::Brace, alternatives(record_forms(fields, trailing)))
}

fn record_forms<W: 'static, H: Clone + 'static>(
    fields: impl IntoIterator<Item = RecordField<W, H>>,
    trailing: impl IntoIterator<Item = Layout<W, H>>,
) -> [Layout<W, H>; 2] {
    let fields = fields
        .into_iter()
        .map(|field| RecordField {
            label: shared(field.label),
            value: shared(field.value),
        })
        .collect::<Vec<_>>();
    let trailing = trailing.into_iter().map(shared).collect::<Vec<_>>();
    let flat = fields
        .iter()
        .map(|field| row(0.0, [field.label.clone(), dim(" "), field.value.clone()]))
        .chain(trailing.iter().cloned())
        .enumerate()
        .flat_map(|(index, item)| (index > 0).then(|| dim(", ")).into_iter().chain([item]))
        .collect::<Vec<_>>();
    let rows = fields
        .into_iter()
        .map(|field| hug(field.label, field.value, 6.0))
        .chain(trailing);
    [row(0.0, flat), col(0, 2.0, rows)]
}

/// Share one projected child between mutually exclusive alternatives.
/// This describes an explicit edge in the layout DAG, not a cache:
/// the child is projected and measured once per frame. A selected
/// concrete form must contain at most one use.
pub fn shared<World: 'static, Hover: 'static>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    let id = NEXT_SHARED_LAYOUT.fetch_add(1, AtomicOrdering::Relaxed);
    Layout::new(move |builder| builder.shared(id, &child))
}

/// Descend `steps` through stored structure without projecting intermediate
/// containers. `projection` draws what's there and `default_projection` its
/// descendants; each is the enclosing one where it's `None`.
pub fn descend<World: 'static, Hover: 'static>(
    steps: impl AsRef<[Step]> + 'static,
    projection: Option<Partial<World, Hover>>,
    default_projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::program(Rc::new(move |context, build| {
        context.project.descend(
            context.text,
            build,
            steps.as_ref(),
            projection.clone(),
            default_projection.clone(),
        )
    }))
}

/// Descend into one child shown as Raw shows it.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn descend_raw<World: 'static, Hover: 'static>(step: Step) -> Layout<World, Hover> {
    Layout::program(Rc::new(move |context, build| {
        context
            .project
            .descend_raw(context.text, build, step.clone())
    }))
}

/// Project a supplied value at an occurrence with no document source.
pub fn at<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: impl Clone + Into<grap::RuntimeValue>,
) -> Layout<World, Hover> {
    at_with_projection(steps, value, None, None)
}

/// Draw at `steps` the value at a document path from its root, as
/// [`jump_at`] does.
pub fn jump<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    document: impl Into<Vec<Step>>,
) -> Layout<World, Hover> {
    jump_at(
        steps,
        crate::location::Location::root(document.into()),
        None,
    )
}

/// Draw at `steps` the value at `location`, by `projection` if given: the
/// occurrence path jumps there, so it and what's below it read and write
/// there.
pub fn jump_at<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    location: crate::location::Location<'static>,
    projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    let steps = steps.into();
    Layout::program(Rc::new(move |context, build| {
        context.project.jump(
            context.text,
            build,
            steps.clone(),
            location.clone(),
            projection.clone(),
            None,
        )
    }))
}

/// Draw at `steps` the value stored where `target` is, edited there along
/// with what's stored below it, as [`jump`] does; `otherwise` where nothing
/// stores it.
pub fn jump_to<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    target: crate::editing::Site,
    projection: Option<Partial<World, Hover>>,
    otherwise: Layout<World, Hover>,
) -> Layout<World, Hover> {
    let steps = steps.into();
    Layout::program(Rc::new(move |context, build| match target.location() {
        Some(document) => context.project.jump(
            context.text,
            build,
            steps.clone(),
            document.into_owned(),
            projection.clone(),
            None,
        ),
        None => otherwise.measure(context, build),
    }))
}

/// Try a specialization here, keeping the default for descendants.
pub fn descend_local<World: 'static, Hover: 'static>(
    steps: impl AsRef<[Step]> + 'static,
    projection: Partial<World, Hover>,
    default: &Partial<World, Hover>,
) -> Layout<World, Hover> {
    descend(
        steps,
        Some(compose_partials([projection, default.clone()])),
        Some(default.clone()),
    )
}

/// Try a specialization here and at every descendant.
pub fn descend_scoped<World: 'static, Hover: 'static>(
    steps: impl AsRef<[Step]> + 'static,
    projection: Partial<World, Hover>,
    default: &Partial<World, Hover>,
) -> Layout<World, Hover> {
    let projection = compose_partials([projection, default.clone()]);
    descend(steps, Some(projection.clone()), Some(projection))
}

pub fn at_with_projection<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: impl Clone + Into<grap::RuntimeValue>,
    projection: Option<Partial<World, Hover>>,
    default_projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    computed(steps, value, None, projection, default_projection)
}

/// [`at`], for the result of `evaluated`, which what's drawn there can run
/// again to ask how it got there.
pub fn at_evaluated<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: impl Clone + Into<grap::RuntimeValue>,
    evaluated: Evaluated,
) -> Layout<World, Hover> {
    computed(steps, value, Some(Rc::new(evaluated)), None, None)
}

fn computed<World: 'static, Hover: 'static>(
    steps: impl Into<Vec<Step>>,
    value: impl Clone + Into<grap::RuntimeValue>,
    evaluated: Option<Rc<Evaluated>>,
    projection: Option<Partial<World, Hover>>,
    default_projection: Option<Partial<World, Hover>>,
) -> Layout<World, Hover> {
    let steps = steps.into();
    let value = value.into();
    Layout::program(Rc::new(move |context, build| {
        context.project.at(
            context.text,
            build,
            steps.clone(),
            value.clone(),
            evaluated.clone(),
            projection.clone(),
            default_projection.clone(),
        )
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

#[cfg(test)]
mod tests;
