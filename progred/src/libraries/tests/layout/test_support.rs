//! Stored layout descriptions built for tests: each node a record of its
//! kind, each drawing command and shape a record under its tag, and each
//! number in the f64 convention, as the layout library decodes them.

use super::{Place, decode_with, vocabulary};
use crate::display::host::{Host, Layout};
use crate::libraries::f64 as f64_convention;
use gid::{CellId, Step, Value};

/// A stored layout node of `kind`.
pub fn layout_node(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::isa::record(kind, fields)
}

/// A drawing command, shape or path element under its tag.
pub fn node(key: CellId, content: Value) -> Value {
    Value::record([(key, content)])
}

pub fn number(value: f64) -> Value {
    f64_convention::value(value)
}

pub fn row(gap: f64, children: impl IntoIterator<Item = Value>) -> Value {
    layout_node(
        vocabulary::kind::ROW,
        [
            (vocabulary::GAP, number(gap)),
            (vocabulary::CHILDREN, Value::list(children)),
        ],
    )
}

pub fn col(baseline: usize, gap: f64, children: impl IntoIterator<Item = Value>) -> Value {
    layout_node(
        vocabulary::kind::COL,
        [
            (vocabulary::BASELINE, number(baseline as f64)),
            (vocabulary::GAP, number(gap)),
            (vocabulary::CHILDREN, Value::list(children)),
        ],
    )
}

pub fn bracketed(delim: CellId, child: Value) -> Value {
    layout_node(
        vocabulary::kind::BRACKET,
        [
            (vocabulary::DELIM, Value::Cell(delim)),
            (vocabulary::CHILD, child),
        ],
    )
}

pub fn options(options: impl IntoIterator<Item = Value>) -> Value {
    layout_node(
        vocabulary::kind::ALTERNATIVES,
        [(vocabulary::CHILDREN, Value::list(options))],
    )
}

pub fn descend_key(key: CellId) -> Value {
    descend_step(Step::Key(key))
}

pub fn descend_follow() -> Value {
    descend_step(Step::Follow)
}

pub fn descend_step(step: Step) -> Value {
    layout_node(
        vocabulary::kind::DESCEND,
        [(vocabulary::STEP, ::grap::path::step_value(&step))],
    )
}

pub fn text_leaf(content: &str, face: CellId) -> Value {
    layout_node(
        vocabulary::kind::TEXT,
        [
            (vocabulary::CONTENT, crate::libraries::text::value(content)),
            (vocabulary::PAINT, Value::Cell(face)),
        ],
    )
}

pub fn drawing(
    width: f64,
    ascent: f64,
    descent: f64,
    commands: impl IntoIterator<Item = Value>,
) -> Value {
    layout_node(
        vocabulary::kind::DRAWING,
        [
            (vocabulary::WIDTH, number(width)),
            (vocabulary::ASCENT, number(ascent)),
            (vocabulary::DESCENT, number(descent)),
            (vocabulary::COMMANDS, Value::list(commands)),
        ],
    )
}

/// A drawing whose Grap `program` draws it, with `fuel` to run.
pub fn program_drawing(
    width: f64,
    ascent: f64,
    descent: f64,
    fuel: usize,
    program: Value,
) -> Value {
    layout_node(
        vocabulary::kind::DRAWING,
        [
            (vocabulary::WIDTH, number(width)),
            (vocabulary::ASCENT, number(ascent)),
            (vocabulary::DESCENT, number(descent)),
            (vocabulary::FUEL, number(fuel as f64)),
            (vocabulary::PROGRAM, program),
        ],
    )
}

pub fn fill(shape: Value, paint: Value) -> Value {
    node(
        vocabulary::FILL,
        Value::record([(vocabulary::SHAPE, shape), (vocabulary::PAINT, paint)]),
    )
}

pub fn stroke(shape: Value, line_width: f64, paint: Value) -> Value {
    node(
        vocabulary::STROKE,
        Value::record([
            (vocabulary::SHAPE, shape),
            (vocabulary::LINE_WIDTH, number(line_width)),
            (vocabulary::PAINT, paint),
        ]),
    )
}

pub fn clip(shape: Value, commands: impl IntoIterator<Item = Value>) -> Value {
    node(
        vocabulary::CLIP,
        Value::record([
            (vocabulary::SHAPE, shape),
            (vocabulary::COMMANDS, Value::list(commands)),
        ]),
    )
}

pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Value {
    shape_box(vocabulary::RECT, x, y, width, height, None)
}

pub fn rounded_rect(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Value {
    shape_box(vocabulary::ROUNDED_RECT, x, y, width, height, Some(radius))
}

fn shape_box(kind: CellId, x: f64, y: f64, width: f64, height: f64, radius: Option<f64>) -> Value {
    let fields = [
        Some((vocabulary::X, number(x))),
        Some((vocabulary::Y, number(y))),
        Some((vocabulary::WIDTH, number(width))),
        Some((vocabulary::HEIGHT, number(height))),
        radius.map(|radius| (vocabulary::RADIUS, number(radius))),
    ];
    node(kind, Value::record(fields.into_iter().flatten()))
}

pub fn circle(x: f64, y: f64, radius: f64) -> Value {
    node(
        vocabulary::CIRCLE,
        Value::record([
            (vocabulary::X, number(x)),
            (vocabulary::Y, number(y)),
            (vocabulary::RADIUS, number(radius)),
        ]),
    )
}

pub fn path(elements: impl IntoIterator<Item = Value>) -> Value {
    node(vocabulary::PATH, Value::list(elements))
}

pub fn move_to(x: f64, y: f64) -> Value {
    point_element(vocabulary::MOVE_TO, x, y)
}

pub fn line_to(x: f64, y: f64) -> Value {
    point_element(vocabulary::LINE_TO, x, y)
}

fn point_element(kind: CellId, x: f64, y: f64) -> Value {
    node(
        kind,
        Value::record([(vocabulary::X, number(x)), (vocabulary::Y, number(y))]),
    )
}

pub fn quad_to(x1: f64, y1: f64, x: f64, y: f64) -> Value {
    node(
        vocabulary::QUAD_TO,
        Value::record([
            (vocabulary::X1, number(x1)),
            (vocabulary::Y1, number(y1)),
            (vocabulary::X, number(x)),
            (vocabulary::Y, number(y)),
        ]),
    )
}

pub fn curve_to(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> Value {
    node(
        vocabulary::CURVE_TO,
        Value::record([
            (vocabulary::X1, number(x1)),
            (vocabulary::Y1, number(y1)),
            (vocabulary::X2, number(x2)),
            (vocabulary::Y2, number(y2)),
            (vocabulary::X, number(x)),
            (vocabulary::Y, number(y)),
        ]),
    )
}

pub fn close() -> Value {
    node(vocabulary::CLOSE, Value::record([]))
}

pub fn selectable_node(child: Value) -> Value {
    layout_node(vocabulary::kind::SELECTABLE, [(vocabulary::CHILD, child)])
}

pub fn pick_target(child: Value, value: Value) -> Value {
    layout_node(
        vocabulary::kind::PICKABLE,
        [(vocabulary::CHILD, child), (vocabulary::VALUE, value)],
    )
}

pub fn hover_block(child: Value) -> Value {
    layout_node(vocabulary::kind::HOVER_BLOCK, [(vocabulary::CHILD, child)])
}

pub fn on(child: Value, handler: Value) -> Value {
    layout_node(
        vocabulary::kind::ON_EVENT,
        [(vocabulary::CHILD, child), (vocabulary::HANDLER, handler)],
    )
}

pub fn bordered(child: Value) -> Value {
    layout_node(vocabulary::kind::BORDER, [(vocabulary::CHILD, child)])
}

/// Decode a layout value into the display language, attaching the
/// PROVIDED intents where the data marks their spots. `None` on any
/// junk, so a malformed layout falls through whole.
pub fn decode<W: Host>(value: &Value, site: &crate::editing::Site) -> Option<Layout<W>> {
    decode_with(&value.into(), &Place::Root, &|| site.clone())
}
