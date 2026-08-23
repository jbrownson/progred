//! Layout a projection can return: boxes, leaves, walk, and event
//! attachment. A leaf is a paint-parametric Puri program. The editor
//! measures boxes and turns leaves into place continuations;
//! libraries never see a UI runtime. Host intents are owned callbacks
//! over the caller's `World`; Grap handlers are data carried by
//! [`Layout::OnEvent`], not a central enum of editor actions.

use gid::{CellId, Step, Value};
use peniko::Brush;
use puri::{Affine, Command, Drawing, Leaf, RoundedRect, Shape, Stroke};
use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

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

/// The paint a Progred projection supplies to Puri. Faces defer to
/// the editor theme; a literal brush belongs to the projected
/// content itself.
#[derive(Clone)]
pub enum Paint {
    Face(Face),
    Brush(Brush),
}

/// A Rust library's description of the stock host line editor. This
/// is a layout/control request, not a drawing primitive: Progred
/// lowers it through Puri into text, canvas ink, and event handlers.
#[derive(Clone)]
pub struct LineEdit {
    pub text: String,
    /// The Grap write-back rule applied to CURRENT and typed INPUT.
    pub update: Value,
    pub prefix: String,
    pub suffix: String,
}

/// A layout-owned decoration whose geometry depends on the box it
/// surrounds. It is deliberately not a display leaf.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    Delim { delim: Delim, side: Side },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Open,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowAlignment {
    Baseline,
    Center,
}

/// A coordinate-free action on the subtree that owns the handler.
/// The language carries no pointer geometry or modifiers; those stay
/// in [`Layout::OnEvent`] and the editor chooses the later semantic
/// action itself.
pub type ActionHandler<World> = Rc<dyn Fn(&mut World) -> bool>;

/// Selection and hover behavior for a location relative to the value
/// currently being projected. The host resolves the relative steps;
/// libraries never receive its absolute document path.
pub struct ProjectionTarget<World, Hover> {
    pub select: ActionHandler<World>,
    pub hover: Hover,
}

/// A borrowed resolver for the current projection target and relative
/// targets beneath it. Resolving owns the returned interaction data;
/// merely trying a partial projection allocates nothing.
#[derive(Clone, Copy)]
pub struct ProjectionTargets<'a, World, Hover> {
    at: &'a dyn Fn(Vec<Step>) -> ProjectionTarget<World, Hover>,
}

impl<'a, World, Hover> ProjectionTargets<'a, World, Hover> {
    pub fn new(
        at: &'a dyn Fn(Vec<Step>) -> ProjectionTarget<World, Hover>,
    ) -> Self {
        Self { at }
    }

    pub fn at(&self, steps: impl Into<Vec<Step>>) -> ProjectionTarget<World, Hover> {
        (self.at)(steps.into())
    }

    pub fn current(&self) -> ProjectionTarget<World, Hover> {
        (self.at)(Vec::new())
    }
}

/// Unevaluated layout: grouping, walk, and leaves. Distinct from
/// Progred's measured boxes (those have extents and place closures).
pub enum Layout<World, Hover> {
    Leaf(Leaf<Paint>),
    /// A Grap program interpreted against the host's live Puri canvas
    /// during rendering. Production streams through canvas FFIs;
    /// tests may install a recording canvas instead.
    DrawingProgram {
        width: f64,
        ascent: f64,
        descent: f64,
        fuel: usize,
        program: Value,
    },
    /// Progred's still-host-owned completion query. This is explicit
    /// layout composition debt, not a drawing primitive disguised as
    /// one.
    Query,
    LineEdit(LineEdit),
    OnClick {
        child: Box<Layout<World, Hover>>,
        handler: ActionHandler<World>,
    },
    /// A coordinate-free editor action addressed by the same target
    /// used for hover. Raw pointer events get first refusal in the
    /// host; activation is the semantic fallback.
    OnActivate {
        child: Box<Layout<World, Hover>>,
        target: Hover,
        handler: ActionHandler<World>,
    },
    /// The value a Pick action here commits into an open pending — any
    /// value, not only a cell; the editor narrows where a stage demands
    /// (labels take cells). Data, not a callback.
    OnPick {
        child: Box<Layout<World, Hover>>,
        target: Hover,
        value: Value,
    },
    /// Apply a Grap callable when an event reaches the subtree.
    /// The editor supplies the event value and a capability overlay
    /// closed over the current projection site.
    OnEvent {
        child: Box<Layout<World, Hover>>,
        handler: Value,
    },
    OnHover {
        child: Box<Layout<World, Hover>>,
        hover: Option<Hover>,
    },
    Row {
        alignment: RowAlignment,
        gap: f64,
        children: Vec<Layout<World, Hover>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Layout<World, Hover>>,
    },
    /// Children share a top-left and baseline, in back-to-front
    /// order. Its extent is the component-wise maximum.
    Overlay {
        children: Vec<Layout<World, Hover>>,
    },
    Pad {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        child: Box<Layout<World, Hover>>,
    },
    /// Measure `child`, then give `left` and `right` the side
    /// columns: flat advance by the child's height. Layout does not
    /// paint them. Growth is typographic overhang.
    Surround {
        left: Ink,
        child: Box<Layout<World, Hover>>,
        right: Ink,
    },
    /// Look up this step on the value being projected.
    Descend {
        step: Step,
    },
    /// Project `value` at this path extended by `steps`.
    At {
        steps: Vec<Step>,
        value: Value,
        /// Prepend contextual partial projections for this subtree.
        /// They compose in front of the current projection; the total
        /// structural projection remains the root.
        projection: Option<Vec<Partial<World, Hover>>>,
    },
    Transient {
        value: Value,
        fuel: usize,
    },
    /// One projected child used by mutually exclusive layout forms.
    /// The editor measures the shared child once and the selected
    /// form consumes it once. This is local layout sharing, not value
    /// identity and not a cross-frame cache.
    Shared {
        id: usize,
        child: Rc<Layout<World, Hover>>,
    },
    /// Ordered forms of the same content. Non-final forms use their
    /// natural preferred widths; the first that fits wins. Otherwise
    /// the final accommodating form receives the real allocation.
    /// When nothing fits, the narrowest form wins; earlier forms win
    /// ties.
    Alternatives(Vec<Layout<World, Hover>>),
}

impl<World, Hover: Clone> Clone for Layout<World, Hover> {
    fn clone(&self) -> Self {
        match self {
            Self::Leaf(display) => Self::Leaf(display.clone()),
            Self::DrawingProgram {
                width,
                ascent,
                descent,
                fuel,
                program,
            } => Self::DrawingProgram {
                width: *width,
                ascent: *ascent,
                descent: *descent,
                fuel: *fuel,
                program: program.clone(),
            },
            Self::Query => Self::Query,
            Self::LineEdit(line) => Self::LineEdit(line.clone()),
            Self::OnClick { child, handler } => Self::OnClick {
                child: child.clone(),
                handler: handler.clone(),
            },
            Self::OnActivate {
                child,
                target,
                handler,
            } => Self::OnActivate {
                child: child.clone(),
                target: target.clone(),
                handler: handler.clone(),
            },
            Self::OnPick {
                child,
                target,
                value,
            } => Self::OnPick {
                child: child.clone(),
                target: target.clone(),
                value: value.clone(),
            },
            Self::OnEvent {
                child,
                handler,
            } => Self::OnEvent {
                child: child.clone(),
                handler: handler.clone(),
            },
            Self::OnHover { child, hover } => Self::OnHover {
                child: child.clone(),
                hover: hover.clone(),
            },
            Self::Row {
                alignment,
                gap,
                children,
            } => Self::Row {
                alignment: *alignment,
                gap: *gap,
                children: children.clone(),
            },
            Self::Col {
                baseline,
                gap,
                children,
            } => Self::Col {
                baseline: *baseline,
                gap: *gap,
                children: children.clone(),
            },
            Self::Overlay { children } => Self::Overlay {
                children: children.clone(),
            },
            Self::Pad {
                left,
                top,
                right,
                bottom,
                child,
            } => Self::Pad {
                left: *left,
                top: *top,
                right: *right,
                bottom: *bottom,
                child: child.clone(),
            },
            Self::Surround {
                left,
                child,
                right,
            } => Self::Surround {
                left: left.clone(),
                child: child.clone(),
                right: right.clone(),
            },
            Self::Descend { step } => Self::Descend { step: step.clone() },
            Self::At {
                steps,
                value,
                projection,
            } => Self::At {
                steps: steps.clone(),
                value: value.clone(),
                projection: projection.clone(),
            },
            Self::Transient { value, fuel } => Self::Transient {
                value: value.clone(),
                fuel: *fuel,
            },
            Self::Shared { id, child } => Self::Shared {
                id: *id,
                child: child.clone(),
            },
            Self::Alternatives(options) => Self::Alternatives(options.clone()),
        }
    }
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    /// Remaining fuel is the evaluator budget left after this call,
    /// so a grap-shaped result can continue the same allowance.
    fn evaluate(&self, expression: &Value) -> (Value, usize);

    /// Evaluate with an explicit allowance. Hosts that do not expose
    /// fuel may keep their ordinary behavior.
    fn evaluate_with_fuel(&self, expression: &Value, _fuel: usize) -> (Value, usize) {
        self.evaluate(expression)
    }

    /// Conventional human name for a cell, when this host has one.
    /// A projection remains responsible for its unnamed fallback.
    fn name(&self, _cell: CellId) -> Option<String> {
        None
    }

    /// The stored value of a cell, without evaluating it. Contextual
    /// projections may inspect definitions to choose a presentation;
    /// absent and computed values remain opaque.
    fn cell_value(&self, _cell: CellId) -> Option<&Value> {
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
    pub value: &'a Value,
    /// The selection payload — stage, query, choice — iff this
    /// value's path is the selected one.
    pub selection: Option<&'a Value>,
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
pub type Partial<World, Hover> =
    for<'a, 'input> fn(
        &'input ProjectionInput<'a, World, Hover>,
    ) -> Option<Layout<World, Hover>>;

pub fn text<World, Hover>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Name)
}

pub fn dim<World, Hover>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Dim)
}

pub fn label<World, Hover>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Label)
}

pub fn id<World, Hover>(text: impl Into<String>) -> Layout<World, Hover> {
    faced(text, Face::Id)
}

pub fn faced<World, Hover>(text: impl Into<String>, face: Face) -> Layout<World, Hover> {
    leaf(Leaf::Text {
        text: text.into(),
        paint: Paint::Face(face),
    })
}

pub fn query<World, Hover>() -> Layout<World, Hover> {
    Layout::Query
}

pub fn line_edit<World, Hover>(line: LineEdit) -> Layout<World, Hover> {
    Layout::LineEdit(line)
}

pub fn slot<World, Hover>() -> Layout<World, Hover> {
    leaf(Leaf::Drawing(Drawing {
        width: 21.0,
        ascent: 11.0,
        descent: 4.0,
        commands: vec![Command::Stroke {
            shape: Shape::RoundedRect(RoundedRect::from_rect(
                puri::Rect::new(0.5, 0.5, 20.5, 14.5),
                3.0,
            )),
            style: Stroke::new(1.0),
            paint: Paint::Face(Face::Dim),
            transform: Affine::IDENTITY,
        }],
    }))
}

pub fn leaf<World, Hover>(leaf: Leaf<Paint>) -> Layout<World, Hover> {
    Layout::Leaf(leaf)
}

pub fn on_click<World, Hover>(
    child: Layout<World, Hover>,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    Layout::OnClick {
        child: Box::new(child),
        handler,
    }
}

pub fn on_activate<World, Hover>(
    child: Layout<World, Hover>,
    target: Hover,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    Layout::OnActivate {
        child: Box::new(child),
        target,
        handler,
    }
}

pub fn activatable<World, Hover: Clone>(
    child: Layout<World, Hover>,
    target: Hover,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    on_hover(on_activate(child, target.clone(), handler), target)
}

pub fn pickable<World, Hover>(
    child: Layout<World, Hover>,
    target: Hover,
    value: Value,
) -> Layout<World, Hover> {
    Layout::OnPick {
        child: Box::new(child),
        target,
        value,
    }
}

pub fn on_event<World, Hover>(
    child: Layout<World, Hover>,
    handler: Value,
) -> Layout<World, Hover> {
    Layout::OnEvent {
        child: Box::new(child),
        handler,
    }
}

pub fn on_hover<World, Hover>(child: Layout<World, Hover>, hover: Hover) -> Layout<World, Hover> {
    Layout::OnHover {
        child: Box::new(child),
        hover: Some(hover),
    }
}

pub fn block_hover<World, Hover>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    Layout::OnHover {
        child: Box::new(child),
        hover: None,
    }
}

pub fn row<World, Hover>(
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Row {
        alignment: RowAlignment::Baseline,
        gap,
        children: children.into_iter().collect(),
    }
}

pub fn centered_row<World, Hover>(
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Row {
        alignment: RowAlignment::Center,
        gap,
        children: children.into_iter().collect(),
    }
}

pub fn col<World, Hover>(
    baseline: usize,
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Col {
        baseline,
        gap,
        children: children.into_iter().collect(),
    }
}

pub fn overlay<World, Hover>(
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Overlay {
        children: children.into_iter().collect(),
    }
}

pub fn pad<World, Hover>(left: f64, child: Layout<World, Hover>) -> Layout<World, Hover> {
    Layout::Pad {
        left,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        child: Box::new(child),
    }
}

pub fn surround<World, Hover>(
    left: Ink,
    child: Layout<World, Hover>,
    right: Ink,
) -> Layout<World, Hover> {
    Layout::Surround {
        left,
        child: Box::new(child),
        right,
    }
}

pub fn bracket<World, Hover>(delim: Delim, child: Layout<World, Hover>) -> Layout<World, Hover> {
    surround(
        Ink::Delim {
            delim,
            side: Side::Open,
        },
        child,
        Ink::Delim {
            delim,
            side: Side::Close,
        },
    )
}

/// Project record-shaped fields in a caller-supplied order. The
/// caller owns each field's meaning — label behavior, child
/// projection, and edit policy — while this combinator owns the
/// braced flat and column forms.
pub struct RecordField<World, Hover> {
    pub label: Layout<World, Hover>,
    pub value: Layout<World, Hover>,
}

pub fn record<'a, World, Hover: Clone>(
    fields: impl IntoIterator<Item = (CellId, &'a Value)>,
    mut order: impl FnMut(&CellId, &CellId) -> Ordering,
    mut field: impl FnMut(CellId, &'a Value) -> RecordField<World, Hover>,
) -> Layout<World, Hover> {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.sort_by(|(left, _), (right, _)| order(left, right));
    let fields = fields
        .into_iter()
        .map(|(key, value)| {
            let field = field(key, value);
            RecordField {
                label: shared(field.label),
                value: shared(field.value),
            }
        })
        .collect::<Vec<_>>();
    let mut flat = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        flat.push(row(
            0.0,
            [field.label.clone(), dim(": "), field.value.clone()],
        ));
    }
    let rows = fields.into_iter().map(|field| {
        hug(
            row(0.0, [field.label, dim(":")]),
            field.value,
            6.0,
            20.0,
        )
    });
    bracket(
        Delim::Brace,
        alternatives([row(0.0, flat), col(0, 2.0, rows)]),
    )
}

pub fn hug<World, Hover: Clone>(
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
pub fn shared<World, Hover>(child: Layout<World, Hover>) -> Layout<World, Hover> {
    Layout::Shared {
        id: NEXT_SHARED_LAYOUT.fetch_add(1, AtomicOrdering::Relaxed),
        child: Rc::new(child),
    }
}

pub fn nest<World, Hover>(step: Step, value: &Value) -> Layout<World, Hover> {
    at([step], value)
}

pub fn descend<World, Hover>(step: Step) -> Layout<World, Hover> {
    Layout::Descend { step }
}

pub fn at<World, Hover>(steps: impl Into<Vec<Step>>, value: &Value) -> Layout<World, Hover> {
    Layout::At {
        steps: steps.into(),
        value: value.clone(),
        projection: None,
    }
}

pub fn at_with_projection<World, Hover>(
    steps: impl Into<Vec<Step>>,
    value: &Value,
    projection: impl IntoIterator<Item = Partial<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::At {
        steps: steps.into(),
        value: value.clone(),
        projection: Some(projection.into_iter().collect()),
    }
}

pub fn project<World, Hover>(value: &Value) -> Layout<World, Hover> {
    at([], value)
}

pub fn transient<World, Hover>(value: &Value, fuel: usize) -> Layout<World, Hover> {
    Layout::Transient {
        value: value.clone(),
        fuel,
    }
}

pub fn alternatives<World, Hover>(
    options: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Alternatives(options.into_iter().collect())
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

    fn unshared<World, Hover>(mut layout: &Layout<World, Hover>) -> &Layout<World, Hover> {
        while let Layout::Shared { child, .. } = layout {
            layout = child.as_ref();
        }
        layout
    }

    #[test]
    fn click_handlers_receive_the_live_world() {
        #[derive(Default)]
        struct World {
            clicks: usize,
        }

        let layout: Layout<World, ()> = on_click(
            text("click me"),
            Rc::new(|world| {
                world.clicks += 1;
                true
            }),
        );
        let Layout::OnClick { handler, .. } = layout else {
            panic!("on_click builds an interaction node");
        };
        let mut world = World::default();
        assert!(handler(&mut world));
        assert_eq!(world.clicks, 1);
    }

    #[test]
    fn record_uses_the_supplied_order_in_both_responsive_forms() {
        const FIRST: CellId = CellId::from_u128(1);
        const SECOND: CellId = CellId::from_u128(2);
        let first = Value::from(vec![1]);
        let second = Value::from(vec![2]);
        let layout: Layout<(), ()> = record(
            [(FIRST, &first), (SECOND, &second)],
            |left, right| right.cmp(left),
            |key, value| RecordField {
                label: dim("field"),
                value: at([Step::Key(key)], value),
            },
        );
        let Layout::Surround { child, .. } = layout else {
            panic!("a record is delimited");
        };
        let Layout::Alternatives(forms) = child.as_ref() else {
            panic!("a record has responsive forms");
        };
        let Layout::Row { children, .. } = &forms[0] else {
            panic!("the first form is flat");
        };
        let Layout::Row { children: first, .. } = &children[0] else {
            panic!("a flat field keeps its label and value together");
        };
        assert!(matches!(
            unshared(&first[2]),
            Layout::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
        let Layout::Row { children: second, .. } = &children[2] else {
            panic!("a flat field keeps its label and value together");
        };
        assert!(matches!(
            unshared(&second[2]),
            Layout::At { steps, .. } if *steps == [Step::Key(FIRST)]
        ));
        let Layout::Col { children, .. } = &forms[1] else {
            panic!("the second form is a column");
        };
        let Layout::Alternatives(first) = &children[0] else {
            panic!("a column field may break after its label");
        };
        let Layout::Row { children: inline, .. } = &first[0] else {
            panic!("a field first stays inline");
        };
        assert!(matches!(
            unshared(&inline[1]),
            Layout::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
        let Layout::Col { children: broken, .. } = &first[1] else {
            panic!("a field may put its value below its label");
        };
        let Layout::Pad { child, .. } = &broken[1] else {
            panic!("a broken value is indented");
        };
        assert!(matches!(
            unshared(child),
            Layout::At { steps, .. } if *steps == [Step::Key(SECOND)]
        ));
    }
}
