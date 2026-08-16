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

/// What a layout leaf shows.
#[derive(Clone)]
pub enum Display {
    Text(String),
    Dim(String),
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

/// Unevaluated layout: grouping, walk, and leaves. Distinct from
/// Progred's measured boxes (those have extents and place closures).
#[derive(Clone)]
pub enum Layout {
    Leaf(Display),
    OnClick {
        child: Box<Layout>,
        click: Click,
    },
    Row { gap: f64, children: Vec<Layout> },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Layout>,
    },
    Nest { step: Step, value: Value },
    Project(Value),
    Transient(Value),
    Arrow {
        expression: Box<Layout>,
        result: Box<Layout>,
    },
}

/// Host services a projection may need while building a [`Layout`].
pub trait Env {
    fn evaluate(&self, expression: &Value) -> Value;
    fn transient(&self) -> bool;
}

pub type Partial = fn(&dyn Env, &Value) -> Option<Layout>;

pub fn text(text: impl Into<String>) -> Layout {
    Layout::Leaf(Display::Text(text.into()))
}

pub fn dim(text: impl Into<String>) -> Layout {
    Layout::Leaf(Display::Dim(text.into()))
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

pub fn transient(value: &Value) -> Layout {
    Layout::Transient(value.clone())
}

pub fn arrow(expression: Layout, result: Layout) -> Layout {
    Layout::Arrow {
        expression: Box::new(expression),
        result: Box::new(result),
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
