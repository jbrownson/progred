//! Layout a projection can return: boxes, leaves, and walk.
//! A leaf's [`Display`] is what it shows. [`on_click`] is how it
//! interacts. The editor measures boxes and turns leaves into place
//! continuations; libraries never see a UI runtime. Interaction is
//! an owned callback over the caller's `World`, not a reified action
//! interpreted by the editor.

use gid::{CellId, Step, Value};
use std::rc::Rc;

#[derive(Clone)]
pub struct LineEdit {
    pub text: String,
    pub update: fn(&Value, &str) -> Option<Value>,
    pub prefix: String,
    pub suffix: String,
}

/// Editor-mapped face a text leaf asks for. Libraries pick a role,
/// not a color.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Name,
    Dim,
    Label,
    Id,
}

/// What a layout leaf shows.
#[derive(Clone)]
pub enum Display {
    Text {
        text: String,
        face: Face,
    },
    LineEdit(LineEdit),
    /// Flat drawn delimiter; height is the glyph span.
    Delim {
        delim: Delim,
        open: bool,
    },
    /// Cell head: conventional name without string quotes, or the short id.
    Head {
        cell: CellId,
    },
    /// A record field's label: its conventional name or short id. The
    /// editor owns the spelling and the click-to-rename gesture,
    /// including landing the caret under the pointer.
    Label {
        key: CellId,
    },
    /// The engaged label query — a new field's name or a rename. The
    /// leaf is an address, not a widget: the editor reads the live
    /// pending for its text and caret. Value pendings never pass
    /// through a leaf; the editor builds them at absent locations.
    Query,
    /// Cold empty slot.
    Slot,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
}

/// A plain primary click on the subtree that owns the handler. The
/// language carries no geometry or modifiers: the editor decides what
/// holding the command key means (a pick, not a click), and leaves
/// whose interaction needs coordinates are [`Display`] variants the
/// editor renders itself.
pub type ClickHandler<World> = Rc<dyn Fn(&mut World) -> bool>;

/// Unevaluated layout: grouping, walk, and leaves. Distinct from
/// Progred's measured boxes (those have extents and place closures).
pub enum Layout<World, Hover> {
    Leaf(Display),
    OnClick {
        child: Box<Layout<World, Hover>>,
        handler: ClickHandler<World>,
    },
    /// The value a command-click here commits into an open pending —
    /// any value, not only a cell; the editor narrows where a stage
    /// demands (labels take cells). Data, not a callback.
    OnPick {
        child: Box<Layout<World, Hover>>,
        value: Value,
    },
    OnHover {
        child: Box<Layout<World, Hover>>,
        hover: Option<Hover>,
    },
    Row {
        gap: f64,
        children: Vec<Layout<World, Hover>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Layout<World, Hover>>,
    },
    Pad {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        child: Box<Layout<World, Hover>>,
    },
    /// Measure `child`, then draw tall delimiters from its extent.
    Bracket {
        delim: Delim,
        child: Box<Layout<World, Hover>>,
    },
    /// Look up this step on the value being projected.
    Descend {
        step: Step,
    },
    /// Project `value` at this path extended by `steps`.
    At {
        steps: Vec<Step>,
        value: Value,
    },
    Transient {
        value: Value,
        fuel: usize,
    },
    /// Try `flat` at unbounded width; if it is not one line or does
    /// not fit, use `broken`. If neither fits, keep the narrower.
    Group {
        flat: Box<Layout<World, Hover>>,
        broken: Box<Layout<World, Hover>>,
    },
}

impl<World, Hover: Clone> Clone for Layout<World, Hover> {
    fn clone(&self) -> Self {
        match self {
            Self::Leaf(display) => Self::Leaf(display.clone()),
            Self::OnClick { child, handler } => Self::OnClick {
                child: child.clone(),
                handler: handler.clone(),
            },
            Self::OnPick { child, value } => Self::OnPick {
                child: child.clone(),
                value: value.clone(),
            },
            Self::OnHover { child, hover } => Self::OnHover {
                child: child.clone(),
                hover: hover.clone(),
            },
            Self::Row { gap, children } => Self::Row {
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
            Self::Bracket { delim, child } => Self::Bracket {
                delim: *delim,
                child: child.clone(),
            },
            Self::Descend { step } => Self::Descend { step: step.clone() },
            Self::At { steps, value } => Self::At {
                steps: steps.clone(),
                value: value.clone(),
            },
            Self::Transient { value, fuel } => Self::Transient {
                value: value.clone(),
                fuel: *fuel,
            },
            Self::Group { flat, broken } => Self::Group {
                flat: flat.clone(),
                broken: broken.clone(),
            },
        }
    }
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    /// Remaining fuel is the evaluator budget left after this call,
    /// so a grap-shaped result can continue the same allowance.
    fn evaluate(&self, expression: &Value) -> (Value, usize);
}

/// Everything a partial projection receives for one value. The
/// default selection callback and matching hover claim may be used,
/// wrapped, ignored, or replaced; no central action vocabulary is
/// involved.
pub struct ProjectionInput<'a, World, Hover> {
    pub env: &'a dyn Env,
    pub value: &'a Value,
    pub select: ClickHandler<World>,
    pub hover: Hover,
}

pub type Partial<World, Hover> =
    for<'a> fn(ProjectionInput<'a, World, Hover>) -> Option<Layout<World, Hover>>;

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
    leaf(Display::Text {
        text: text.into(),
        face,
    })
}

pub fn delim<World, Hover>(delim: Delim, open: bool) -> Layout<World, Hover> {
    leaf(Display::Delim { delim, open })
}

pub fn head<World, Hover>(cell: CellId) -> Layout<World, Hover> {
    leaf(Display::Head { cell })
}

pub fn field_label<World, Hover>(key: CellId) -> Layout<World, Hover> {
    leaf(Display::Label { key })
}

pub fn query<World, Hover>() -> Layout<World, Hover> {
    leaf(Display::Query)
}

pub fn slot<World, Hover>() -> Layout<World, Hover> {
    leaf(Display::Slot)
}

pub fn leaf<World, Hover>(display: Display) -> Layout<World, Hover> {
    Layout::Leaf(display)
}

pub fn on_click<World, Hover>(
    child: Layout<World, Hover>,
    handler: ClickHandler<World>,
) -> Layout<World, Hover> {
    Layout::OnClick {
        child: Box::new(child),
        handler,
    }
}

pub fn pickable<World, Hover>(child: Layout<World, Hover>, value: Value) -> Layout<World, Hover> {
    Layout::OnPick {
        child: Box::new(child),
        value,
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

/// The `LineEdit` a layout mounts, if the whole thing is an
/// [`editable_line`] (or a key wrapper around one).
pub fn line_edit_of<World, Hover>(layout: &Layout<World, Hover>) -> Option<&LineEdit> {
    match layout {
        Layout::OnClick { child, .. }
        | Layout::OnPick { child, .. }
        | Layout::OnHover { child, .. } => line_edit_of(child),
        Layout::Group { flat, broken } => line_edit_of(flat).or_else(|| line_edit_of(broken)),
        Layout::Pad { child, .. } | Layout::Bracket { child, .. } => line_edit_of(child),
        Layout::Leaf(Display::LineEdit(line)) => Some(line),
        _ => None,
    }
}

/// A line-editing leaf. Its value update is library-supplied; the
/// editor runtime supplies focus, selection, and caret interaction.
/// Shared by text, f64, and later line projections.
pub fn editable_line<World, Hover>(line: LineEdit) -> Layout<World, Hover> {
    leaf(Display::LineEdit(line))
}

pub fn row<World, Hover>(
    gap: f64,
    children: impl IntoIterator<Item = Layout<World, Hover>>,
) -> Layout<World, Hover> {
    Layout::Row {
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

pub fn pad<World, Hover>(left: f64, child: Layout<World, Hover>) -> Layout<World, Hover> {
    Layout::Pad {
        left,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        child: Box::new(child),
    }
}

pub fn bracket<World, Hover>(delim: Delim, child: Layout<World, Hover>) -> Layout<World, Hover> {
    Layout::Bracket {
        delim,
        child: Box::new(child),
    }
}

pub fn hug<World, Hover: Clone>(
    head: Layout<World, Hover>,
    child: Layout<World, Hover>,
    gap: f64,
    tab: f64,
) -> Layout<World, Hover> {
    group(
        row(gap, [head.clone(), child.clone()]),
        col(0, 2.0, [head, pad(tab, child)]),
    )
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

pub fn group<World, Hover>(
    flat: Layout<World, Hover>,
    broken: Layout<World, Hover>,
) -> Layout<World, Hover> {
    Layout::Group {
        flat: Box::new(flat),
        broken: Box::new(broken),
    }
}

/// If both values are records, `patch` fields win on shared keys.
/// Otherwise `patch`.
pub fn overlay(current: &Value, patch: Value) -> Value {
    match (current.as_record(), patch.as_record()) {
        (Some(current), Some(patch)) => Value::record(
            current
                .clone()
                .union_with(patch.clone(), |_, incoming| incoming),
        ),
        _ => patch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
