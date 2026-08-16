//! Layout a projection can return: boxes, leaves, and walk.
//! A leaf's [`Display`] is what it shows. [`on_click`] is how it
//! interacts. The editor measures boxes and turns leaves into place
//! continuations; libraries never see a UI runtime.

use progred_graph::{Step, Value};

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
}

/// What a layout leaf shows.
#[derive(Clone)]
pub enum Display {
    Text { text: String, face: Face },
    LineEdit(LineEdit),
}

/// What a primary click on a subtree should do.
#[derive(Clone)]
pub enum Click {
    /// Select the value this layout is projecting.
    Select,
    /// Select that value and mount this line editor; the click
    /// point places the caret.
    Line(LineEdit),
}

/// A key a subtree can claim.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Backspace and Delete.
    Delete,
}

/// Unevaluated layout: grouping, walk, and leaves. Distinct from
/// Progred's measured boxes (those have extents and place closures).
#[derive(Clone)]
pub enum Layout {
    Leaf(Display),
    OnClick {
        child: Box<Layout>,
        click: Click,
    },
    OnKey {
        child: Box<Layout>,
        key: Key,
    },
    Row { gap: f64, children: Vec<Layout> },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Layout>,
    },
    Nest { step: Step, value: Value },
    Project(Value),
    Transient { value: Value, fuel: usize },
    /// Try `flat` at unbounded width; if it is not one line or does
    /// not fit, use `broken` at the available width.
    Group {
        flat: Box<Layout>,
        broken: Box<Layout>,
    },
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    /// Remaining fuel is the evaluator budget left after this call,
    /// so a grap-shaped result can continue the same allowance.
    fn evaluate(&self, expression: &Value) -> (Value, usize);
}

pub type Partial = fn(&dyn Env, &Value) -> Option<Layout>;

pub fn text(text: impl Into<String>) -> Layout {
    leaf(Display::Text {
        text: text.into(),
        face: Face::Name,
    })
}

pub fn dim(text: impl Into<String>) -> Layout {
    leaf(Display::Text {
        text: text.into(),
        face: Face::Dim,
    })
}

pub fn leaf(display: Display) -> Layout {
    Layout::Leaf(display)
}

pub fn on_click(child: Layout, click: Click) -> Layout {
    Layout::OnClick {
        child: Box::new(child),
        click,
    }
}

pub fn on_key(child: Layout, key: Key) -> Layout {
    Layout::OnKey {
        child: Box::new(child),
        key,
    }
}

/// The `LineEdit` a layout mounts, if the whole thing is an
/// [`editable_line`] (or a key wrapper around one).
pub fn line_edit_of(layout: &Layout) -> Option<&LineEdit> {
    match layout {
        Layout::OnClick {
            click: Click::Line(line),
            ..
        } => Some(line),
        Layout::OnClick { child, .. } | Layout::OnKey { child, .. } => line_edit_of(child),
        Layout::Group { flat, broken } => line_edit_of(flat).or_else(|| line_edit_of(broken)),
        Layout::Leaf(Display::LineEdit(line)) => Some(line),
        _ => None,
    }
}

/// A line editor: the leaf plus a click that selects it and places
/// the caret. Shared by text, f64, and later line projections.
pub fn editable_line(line: LineEdit) -> Layout {
    on_click(
        leaf(Display::LineEdit(line.clone())),
        Click::Line(line),
    )
}

pub fn row(gap: f64, children: impl IntoIterator<Item = Layout>) -> Layout {
    Layout::Row {
        gap,
        children: children.into_iter().collect(),
    }
}

pub fn col(
    baseline: usize,
    gap: f64,
    children: impl IntoIterator<Item = Layout>,
) -> Layout {
    Layout::Col {
        baseline,
        gap,
        children: children.into_iter().collect(),
    }
}

pub fn nest(step: Step, value: &Value) -> Layout {
    Layout::Nest {
        step,
        value: value.clone(),
    }
}

pub fn project(value: &Value) -> Layout {
    Layout::Project(value.clone())
}

pub fn transient(value: &Value, fuel: usize) -> Layout {
    Layout::Transient {
        value: value.clone(),
        fuel,
    }
}

pub fn group(flat: Layout, broken: Layout) -> Layout {
    Layout::Group {
        flat: Box::new(flat),
        broken: Box::new(broken),
    }
}

/// If both values are records, `patch` fields win on shared keys.
/// Otherwise `patch`.
pub fn overlay(current: &Value, patch: Value) -> Value {
    match (current.as_record(), patch.as_record()) {
        (Some(current), Some(patch)) => {
            Value::record(current.clone().union_with(patch.clone(), |_, incoming| incoming))
        }
        _ => patch,
    }
}
