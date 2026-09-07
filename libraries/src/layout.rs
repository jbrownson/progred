//! The layout language and Puri leaf language as GID values, so a Grap
//! projection can return them. Each node is a
//! record under a single marker key, strings ride the text convention
//! and numbers the f64 convention. Interaction either attaches
//! host-provided editor intents or a Grap handler to generic event
//! data. Event dispatch supplies a capability overlay closed over the
//! projection site. Traversal uses the path library's step convention.
//! Decoding is resilient the projection way: any junk node decodes to
//! `None`, and the whole layout falls through to the next partial.
//!
use crate::{Library, color, f64 as f64_convention, name, presentation, text};
use gid::{CellId, Step, Value};

pub const ID: CellId = CellId::from_u128(0xfb2a4dac87512d69448650bc0e29dc80);
use grap_runtime::{Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{
    ActionHandler, Delim, Face, Layout, Paint, ProjectionInput, ProjectionTarget, RowAlignment,
    alternatives, block_hover, border, bracket, descend, leaf, on_activate, on_hover,
    overlay as layout_overlay, pickable, slot,
};
use puri::{
    Affine, BezPath, Brush, Circle, ColorStop, Command, Drawing, Gradient, Leaf, Line, Point, Rect,
    RoundedRect, Shape, Stroke,
};

const APPLY_BORDER_PROJECTION: CellId = CellId::from_u128(0x9803fe7e085a661271b4339db22db136);

mod events;
pub use events::on_event;

pub mod vocabulary {
    use gid::CellId;

    // Boxes and walk.
    pub const ROW: CellId = CellId::from_u128(0x1af52c96e380b7d40c9e1f6a2d5b83e7);
    pub const COL: CellId = CellId::from_u128(0x9d04b6e1783f2ca5f17d09c4e6a2358b);
    pub const PAD: CellId = CellId::from_u128(0x4e8a17d0952cb6f3a30c5e92b7d1f648);
    pub const BORDER: CellId = CellId::from_u128(0xa4edf70410d16734683d9f2de0bb0080);
    pub const OVERLAY: CellId = CellId::from_u128(0x95de46726d377f70f4f8b6f88893aa52);
    pub const BRACKET: CellId = CellId::from_u128(0xc71e0f4b2d8a6395e6b34a08d15c97f2);
    pub const ALTERNATIVES: CellId = CellId::from_u128(0x62d9b3f0a47e158c37b60d2c81f5e94a);
    pub const DESCEND: CellId = CellId::from_u128(0x35c7a8e2f10d49b6d2f8016c4b9ea375);
    pub const AT: CellId = CellId::from_u128(0xe90d25c8b64a37f1084b92d7f3a65c1e);
    pub const TRANSIENT: CellId = CellId::from_u128(0x7b3f9a05d1e284c6952e07b1c8d643fa);

    // Leaves.
    pub const TEXT: CellId = CellId::from_u128(0x08e64d1f3a92c5b7b7f0d38a165e29c4);
    pub const SLOT: CellId = CellId::from_u128(0x96e07d2a58c4b1f3f3b18e57d0c2946a);

    // Generic display and event nodes.
    pub const DRAWING: CellId = CellId::from_u128(0x6889fa235b002be4c8b106d5f31dafbf);
    pub const PROGRAM: CellId = CellId::from_u128(0xbdf607810b274ad5b02bd409aef34b6a);
    pub const ON_EVENT: CellId = CellId::from_u128(0x1b87f7de18e7c46c5fbfadea1f18aea4);

    // Interaction attach-points.
    pub const SELECTABLE: CellId = CellId::from_u128(0x40b93f6e17d5a28c6a2df1905e83b7c4);
    pub const PICKABLE: CellId = CellId::from_u128(0xf8261c05d94eb7a3072c48e6b3f19d58);
    pub const HOVERABLE: CellId = CellId::from_u128(0x1d7c40a396f58e2b95e1a2c7048d63bf);
    pub const HOVER_BLOCK: CellId = CellId::from_u128(0x83f0d5b7264a19ce4c07f3921ea6b85d);
    pub const HANDLER: CellId = CellId::from_u128(0x3e5d38e4658b1895e38307ad12862061);

    // Event kinds and the handler call's argument.
    pub const EVENT: CellId = CellId::from_u128(0xbd232b5fb4450b52e088470776ed2a01);
    pub const EVENT_KIND: CellId = CellId::from_u128(0x9cacae824e9661b167f3ed2b8e187ee0);
    pub const POINTER_DOWN: CellId = CellId::from_u128(0x67a9a626caff2f3568224eafcb338428);
    pub const POINTER_MOVE: CellId = CellId::from_u128(0xbf54bd5b9cd29a7c32fc496d6d59e4ca);
    pub const POINTER_UP: CellId = CellId::from_u128(0x5c2173cf26dfc305ebd89e3cf1d62890);
    pub const POINTER_CANCEL: CellId = CellId::from_u128(0x594eeff3b32941fea937818d89ca2002);
    pub const TOUCH_START: CellId = CellId::from_u128(0x87a2ba9cac6a4805b4e3baf9e9218dd5);
    pub const TOUCH_MOVE: CellId = CellId::from_u128(0x5359b767cb744fb881c8a92b95664c20);
    pub const TOUCH_END: CellId = CellId::from_u128(0x32b27cf0994b45208502d133d0da4eb2);
    pub const TOUCH_CANCEL: CellId = CellId::from_u128(0xf96ee39a16f748cc85b6ea8c99b46394);
    pub const SCROLL: CellId = CellId::from_u128(0x7b0de6ea9b052da4b40e5f97438d537c);
    pub const KEY: CellId = CellId::from_u128(0xbabfda8d94c4a003ae22faf4a4a2fd01);
    pub const IME: CellId = CellId::from_u128(0x33b7ee93c08863b54d3106802a28d110);

    // Fields.
    pub const GAP: CellId = CellId::from_u128(0xa4917e2c60d3f8b5310b6d8f2c74ae95);
    pub const BASELINE: CellId = CellId::from_u128(0x6cd28a4f91e057b3c2941e6a07f5d3b8);
    pub const CHILDREN: CellId = CellId::from_u128(0xe10b73d5482f96ca7d3852c0f16b49ea);
    pub const CHILD: CellId = CellId::from_u128(0x59a6f0c2e8b1d4738f6e04a9d21c57b3);
    pub const LEFT: CellId = CellId::from_u128(0x0d34c8a1f7625e9ba81f37d2c50e964b);
    pub const TOP: CellId = CellId::from_u128(0xb7e2569d0c48a3f1543a90e6b8d1f2c7);
    pub const RIGHT: CellId = CellId::from_u128(0x28f4a0b3d17c95e6e7c62b04a9f358d1);
    pub const BOTTOM: CellId = CellId::from_u128(0x94d1e75b3f28c6a02b85d4c7e6013f9a);
    pub const DELIM: CellId = CellId::from_u128(0x71c35a9e04b8d2f6f9401c7b3e685da2);
    pub const CONTENT: CellId = CellId::from_u128(0x3e6b91d4a25f70c8815d29f6c4a30e7b);
    pub const PAINT: CellId = CellId::from_u128(0xcb04728f5e6a1d93a6790238b5f1ce4d);
    pub const STEP: CellId = CellId::from_u128(0x67a2d5e0b93c48f14e28b671d0a5c39f);
    pub const STEPS: CellId = CellId::from_u128(0x1298c6f4a7053edb09b64d2e8371fa5c);
    pub const VALUE: CellId = CellId::from_u128(0x85e3b0d729c4165ffa1e0c5d49b3872e);
    pub const FUEL: CellId = CellId::from_u128(0x4a0f68c1d3952b7ec5d7f2a1806e3b49);
    pub const WIDTH: CellId = CellId::from_u128(0xf33c672fef9d0d4561102410fd64129f);
    pub const HEIGHT: CellId = CellId::from_u128(0xcc32dd050a9351804e7a704b4d59e7ac);
    pub const ASCENT: CellId = CellId::from_u128(0x7e1d0997f09e8231cabdd246ec1888f9);
    pub const DESCENT: CellId = CellId::from_u128(0x2ed0b9294782093c08695dd5137df21a);
    pub const COMMANDS: CellId = CellId::from_u128(0xb3604c47bef05aa4c7ef0b2d5794da14);
    pub const X: CellId = CellId::from_u128(0x415def0fa0a9ac40dfba5fca4d0f8876);
    pub const Y: CellId = CellId::from_u128(0x4e2dcde5b1ab1480a2f126176dd148c7);
    pub const BUTTON: CellId = CellId::from_u128(0x0ec32df180178e9fbc958d3d061481d7);
    pub const PRIMARY: CellId = CellId::from_u128(0x09f73c1a02464b762ae34adc9ec17baa);
    pub const SHIFT: CellId = CellId::from_u128(0x3766462666e33f13096d6afd581de3be);
    pub const COMMAND: CellId = CellId::from_u128(0xfaf6451fbf89c75cf7b6a906f27590e2);
    pub const MODIFIERS: CellId = CellId::from_u128(0x36dc2e12ccce7f3cd48713123170714a);
    pub const COUNT: CellId = CellId::from_u128(0xc6b90219f3e63bbbc14b7ebefd1bd443);
    pub const SCALE: CellId = CellId::from_u128(0xa370331d2ba2325f05fe6d621b885bef);
    pub const DELTA_X: CellId = CellId::from_u128(0x798ade16a1f9f7beeb7009c1e183b3c7);
    pub const DELTA_Y: CellId = CellId::from_u128(0x1336f899599217d2535819a51a4c981c);
    pub const COALESCED: CellId = CellId::from_u128(0x5ac14439cfe0120df29ae87604adf300);
    pub const EVENT_STATE: CellId = CellId::from_u128(0x15ab408f66b4286b33f35d95a651b20a);
    pub const DOWN: CellId = CellId::from_u128(0x96a441425ba7048c7fbb1722922e5ffb);
    pub const UP: CellId = CellId::from_u128(0xe47d794f06d1f6fc6166c401aeb17c82);
    pub const REPEAT: CellId = CellId::from_u128(0x5636109fec98530f178de15f65b07372);
    pub const IME_ENABLED: CellId = CellId::from_u128(0xb773d2a984a80a30dec3d0877ff05895);
    pub const IME_DISABLED: CellId = CellId::from_u128(0xa6122f0fea5b0bda9f9a20b44a3e12db);
    pub const IME_PREEDIT: CellId = CellId::from_u128(0xa8dd99527060e9d38b0637fbe842abe4);
    pub const IME_COMMIT: CellId = CellId::from_u128(0xff48152d0c492add92f179b2d719f745);
    pub const START: CellId = CellId::from_u128(0xc68a36ea81944cdbc8e7897ffda5a36c);
    pub const END: CellId = CellId::from_u128(0xabbfd273a978cb520a145eb65568f224);
    pub const RADIUS: CellId = CellId::from_u128(0x6423c35e07d7a4ff536127d1f1d8eb53);
    pub const LINE_WIDTH: CellId = CellId::from_u128(0xa8e4d1cb2cd9f070eb428e013fecc5ef);
    pub const FILL: CellId = CellId::from_u128(0x1624dc973ec7790203eb8fd22c9d6d05);
    pub const STROKE: CellId = CellId::from_u128(0xc2025f7714e1f74c8ad75f28ea00f616);
    pub const CLIP: CellId = CellId::from_u128(0x30b1e31f7eda7e84d9309e5e2d6ef48f);
    pub const SHAPE: CellId = CellId::from_u128(0xfcaaadef14980397cce95363fc5a54f9);
    pub const RECT: CellId = CellId::from_u128(0xbb5e62c1b300b1e8c2484a0f4879a1fe);
    pub const ROUNDED_RECT: CellId = CellId::from_u128(0xb07684708c0d04b7dfd66b9ec7ecf35b);
    pub const CIRCLE: CellId = CellId::from_u128(0xe06a6d094c4f75cd6c1d59ed6ed64e05);
    pub const LINE: CellId = CellId::from_u128(0x5fb0417f31da1dea06802eb484fa0b17);
    pub const PATH: CellId = CellId::from_u128(0xfbcf1931c6e050d62c4caf5cccba2ce8);
    pub const X1: CellId = CellId::from_u128(0x97da731fd85c22bbfc5f9dde4b74fedd);
    pub const Y1: CellId = CellId::from_u128(0xc2ff9b77b6ea033bc3b6f02dd2860282);
    pub const X2: CellId = CellId::from_u128(0xfbca1692325e21e057204bbee4296f87);
    pub const Y2: CellId = CellId::from_u128(0x7bc90a68d48548a46ecf99a1d105d5ef);
    pub const MOVE_TO: CellId = CellId::from_u128(0x1f6ae3a0b82fb63f233f4713cd0c8e30);
    pub const LINE_TO: CellId = CellId::from_u128(0x2b1cebde44cbdadcd6de44facda4ee94);
    pub const QUAD_TO: CellId = CellId::from_u128(0xda1c37c330243e538702cb0e19a711ce);
    pub const CURVE_TO: CellId = CellId::from_u128(0x44bfb8ebda540e9c037328f706c0c142);
    pub const CLOSE: CellId = CellId::from_u128(0xadf2742dbde1b09b03e1fc649f5e1f58);
    pub const LINEAR_GRADIENT: CellId = CellId::from_u128(0x7595f5eee03cd47be1f3fee10806d6a9);
    pub const STOPS: CellId = CellId::from_u128(0x5d4211d5e7184a41fe1ffaa2cd5865c9);
    pub const OFFSET: CellId = CellId::from_u128(0xf348e826a277875d63e7a0197730f036);
    pub const TRANSFORM: CellId = CellId::from_u128(0x0c69b749e5d076609a6642a20b736fb4);
    pub const TRANSLATE: CellId = CellId::from_u128(0xb9c300206cc1c193cc801f4f058f2647);
    pub const ROTATE: CellId = CellId::from_u128(0x2500213d415930c6b40f15d633cc04a4);

    // Faces.
    pub const NAME_FACE: CellId = CellId::from_u128(0x520e9b3c7ad6f18409cf25a7d8631be0);
    pub const STRING_FACE: CellId = CellId::from_u128(0x81c5bf325b9be5e1923e4317371b0eab);
    pub const DIM_FACE: CellId = CellId::from_u128(0xf14b6a08d29c53e7bd0561f8a3c2497e);
    pub const LABEL_FACE: CellId = CellId::from_u128(0x7d90c4e5f1382ab6270d94c1e5a8f36b);
    pub const ID_FACE: CellId = CellId::from_u128(0xb38a1d67e02f49c5c9e8073a6b5d21f4);
    pub const ACCENT_WASH_FACE: CellId = CellId::from_u128(0x1ec921b1240171ceb6dcae8d15889ef4);
    pub const INK_FACE: CellId = CellId::from_u128(0xe553afe01621dbdfe528b1f5fcd69e21);

    // Delimiters.
    pub const PAREN: CellId = CellId::from_u128(0x0af59c27b1e4d68318f4a06c9d7325eb);
    pub const SQUARE: CellId = CellId::from_u128(0x9e61d40b7f3ca258745c1e9b02d8f6a3);
    pub const CURLY: CellId = CellId::from_u128(0x63b8f5a2c90e17d4e12489d5b6a0c73f);
}

fn node(key: CellId, content: Value) -> Value {
    Value::record([(key, content)])
}

/// `drawing` is both the display-language marker and its ordinary
/// unary constructor/projection function. Calling it with `value`
/// produces the same data form that can also be written literally.
fn drawing_projection(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(value) = context.field(call, presentation::vocabulary::VALUE) else {
        return Ok(context.missing_argument(presentation::vocabulary::VALUE));
    };
    Ok(node(vocabulary::DRAWING, context.eval(value, environment)?))
}

/// Turn an ordinary projection into one whose projected result is
/// redispatched inside the standard border layout.
fn border_projection(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<grap_runtime::RuntimeValue, Halt> {
    let Some(projection) = context.field(call, presentation::vocabulary::PROJECTION) else {
        return Ok(context.missing_runtime_argument(presentation::vocabulary::PROJECTION));
    };
    let projection = context.eval(projection, environment)?;
    let body = grap_runtime::call(
        Value::from(APPLY_BORDER_PROJECTION),
        [
            (presentation::vocabulary::PROJECTION, projection),
            (
                presentation::vocabulary::VALUE,
                Value::from(presentation::vocabulary::VALUE),
            ),
        ],
    );
    Ok(context.closure([presentation::vocabulary::VALUE], body, environment))
}

fn apply_border_projection(
    context: &mut grap_runtime::Context,
    call: Expression,
    environment: &Environment,
) -> Result<grap_runtime::RuntimeValue, Halt> {
    let Some(projection) = context.field(call, presentation::vocabulary::PROJECTION) else {
        return Ok(context.missing_runtime_argument(presentation::vocabulary::PROJECTION));
    };
    let projection = context.eval(projection, environment)?;
    let Some(value) = context.field(call, presentation::vocabulary::VALUE) else {
        return Ok(context.missing_runtime_argument(presentation::vocabulary::VALUE));
    };
    let value = context.eval(value, environment)?;
    let projected = context.apply(&projection, [(presentation::vocabulary::VALUE, value)])?;
    Ok(bordered(transient(projected, context.remaining_fuel())).into())
}

fn number(value: f64) -> Value {
    f64_convention::value(value)
}

// Builders: the data form's construction helpers, mirroring the Rust
// language's, for tests and quoted Grap projections.

pub fn row(gap: f64, children: impl IntoIterator<Item = Value>) -> Value {
    node(
        vocabulary::ROW,
        Value::record([
            (vocabulary::GAP, number(gap)),
            (vocabulary::CHILDREN, Value::list(children)),
        ]),
    )
}

pub fn col(baseline: usize, gap: f64, children: impl IntoIterator<Item = Value>) -> Value {
    node(
        vocabulary::COL,
        Value::record([
            (vocabulary::BASELINE, number(baseline as f64)),
            (vocabulary::GAP, number(gap)),
            (vocabulary::CHILDREN, Value::list(children)),
        ]),
    )
}

pub fn overlay(children: impl IntoIterator<Item = Value>) -> Value {
    node(vocabulary::OVERLAY, Value::list(children))
}

pub fn pad(left: f64, top: f64, right: f64, bottom: f64, child: Value) -> Value {
    node(
        vocabulary::PAD,
        Value::record([
            (vocabulary::LEFT, number(left)),
            (vocabulary::TOP, number(top)),
            (vocabulary::RIGHT, number(right)),
            (vocabulary::BOTTOM, number(bottom)),
            (vocabulary::CHILD, child),
        ]),
    )
}

pub fn bracketed(delim: CellId, child: Value) -> Value {
    node(
        vocabulary::BRACKET,
        Value::record([
            (vocabulary::DELIM, Value::Cell(delim)),
            (vocabulary::CHILD, child),
        ]),
    )
}

pub fn options(options: impl IntoIterator<Item = Value>) -> Value {
    node(vocabulary::ALTERNATIVES, Value::list(options))
}

pub fn descend_key(key: CellId) -> Value {
    descend_step(Step::Key(key))
}

pub fn descend_follow(resolution: gid::Resolution) -> Value {
    descend_step(Step::Follow(resolution))
}

pub fn descend_step(step: Step) -> Value {
    node(
        vocabulary::DESCEND,
        Value::record([(vocabulary::STEP, crate::path::step_value(&step))]),
    )
}

pub fn transient(value: Value, fuel: usize) -> Value {
    node(
        vocabulary::TRANSIENT,
        Value::record([
            (vocabulary::VALUE, value),
            (vocabulary::FUEL, number(fuel as f64)),
        ]),
    )
}

pub fn text_leaf(content: &str, face: CellId) -> Value {
    node(
        vocabulary::TEXT,
        Value::record([
            (vocabulary::CONTENT, text::value(content)),
            (vocabulary::PAINT, Value::Cell(face)),
        ]),
    )
}

pub fn drawing(
    width: f64,
    ascent: f64,
    descent: f64,
    commands: impl IntoIterator<Item = Value>,
) -> Value {
    node(
        vocabulary::DRAWING,
        Value::record([
            (vocabulary::WIDTH, number(width)),
            (vocabulary::ASCENT, number(ascent)),
            (vocabulary::DESCENT, number(descent)),
            (vocabulary::COMMANDS, Value::list(commands)),
        ]),
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

pub fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Value {
    node(
        vocabulary::LINE,
        Value::record([
            (vocabulary::X1, number(x1)),
            (vocabulary::Y1, number(y1)),
            (vocabulary::X2, number(x2)),
            (vocabulary::Y2, number(y2)),
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

#[allow(clippy::too_many_arguments)]
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

pub fn linear_gradient(
    start: (f64, f64),
    end: (f64, f64),
    stops: impl IntoIterator<Item = (f64, Value)>,
) -> Value {
    node(
        vocabulary::LINEAR_GRADIENT,
        Value::record([
            (vocabulary::START, point(start.0, start.1)),
            (vocabulary::END, point(end.0, end.1)),
            (
                vocabulary::STOPS,
                Value::list(stops.into_iter().map(|(offset, paint)| {
                    Value::record([
                        (vocabulary::OFFSET, number(offset)),
                        (vocabulary::PAINT, paint),
                    ])
                })),
            ),
        ]),
    )
}

fn point(x: f64, y: f64) -> Value {
    Value::record([(vocabulary::X, number(x)), (vocabulary::Y, number(y))])
}

pub fn selectable(child: Value) -> Value {
    node(vocabulary::SELECTABLE, child)
}

pub fn pick_target(child: Value, value: Value) -> Value {
    node(
        vocabulary::PICKABLE,
        Value::record([(vocabulary::CHILD, child), (vocabulary::VALUE, value)]),
    )
}

pub fn hoverable(child: Value) -> Value {
    node(vocabulary::HOVERABLE, child)
}

pub fn hover_block(child: Value) -> Value {
    node(vocabulary::HOVER_BLOCK, child)
}

pub fn on(child: Value, handler: Value) -> Value {
    node(
        vocabulary::ON_EVENT,
        Value::record([(vocabulary::CHILD, child), (vocabulary::HANDLER, handler)]),
    )
}

pub fn bordered(child: Value) -> Value {
    node(vocabulary::BORDER, child)
}

/// Decode a layout value into the display language, attaching the
/// PROVIDED intents where the data marks their spots. `None` on any
/// junk, so a malformed layout falls through whole.
pub fn decode<World: 'static, Hover: Clone + 'static>(
    value: &Value,
    select: &ActionHandler<World>,
    hover: &Hover,
) -> Option<Layout<World, Hover>> {
    decode_with(value, &|| ProjectionTarget {
        select: select.clone(),
        select_with: {
            let select = select.clone();
            std::rc::Rc::new(move |world, _| select(world))
        },
        hover: hover.clone(),
    })
}

fn decode_with<World: 'static, Hover: Clone + 'static>(
    value: &Value,
    target: &impl Fn() -> ProjectionTarget<World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::ROW) {
        let content = content.as_record()?;
        return Some(Layout::Row {
            alignment: RowAlignment::Baseline,
            gap: read_number(content.get(&vocabulary::GAP)?)?,
            children: children(content.get(&vocabulary::CHILDREN)?, target)?,
        });
    }
    if let Some(content) = fields.get(&vocabulary::COL) {
        let content = content.as_record()?;
        let baseline = read_number(content.get(&vocabulary::BASELINE)?)?;
        (baseline >= 0.0 && baseline.fract() == 0.0).then_some(())?;
        return Some(Layout::Col {
            baseline: baseline as usize,
            gap: read_number(content.get(&vocabulary::GAP)?)?,
            children: children(content.get(&vocabulary::CHILDREN)?, target)?,
        });
    }
    if let Some(content) = fields.get(&vocabulary::OVERLAY) {
        return Some(layout_overlay(children(content, target)?));
    }
    if let Some(content) = fields.get(&vocabulary::PAD) {
        let content = content.as_record()?;
        return Some(Layout::Pad {
            left: read_number(content.get(&vocabulary::LEFT)?)?,
            top: read_number(content.get(&vocabulary::TOP)?)?,
            right: read_number(content.get(&vocabulary::RIGHT)?)?,
            bottom: read_number(content.get(&vocabulary::BOTTOM)?)?,
            child: Box::new(decode_with(content.get(&vocabulary::CHILD)?, target)?),
        });
    }
    if let Some(content) = fields.get(&vocabulary::BORDER) {
        return Some(border(decode_with(content, target)?));
    }
    if let Some(content) = fields.get(&vocabulary::BRACKET) {
        let content = content.as_record()?;
        let delim = match content.get(&vocabulary::DELIM)?.as_cell()? {
            cell if cell == vocabulary::PAREN => Delim::Paren,
            cell if cell == vocabulary::SQUARE => Delim::Bracket,
            cell if cell == vocabulary::CURLY => Delim::Brace,
            _ => return None,
        };
        return Some(bracket(
            delim,
            decode_with(content.get(&vocabulary::CHILD)?, target)?,
        ));
    }
    if let Some(content) = fields.get(&vocabulary::ALTERNATIVES) {
        return Some(alternatives(children(content, target)?));
    }
    if let Some(content) = fields.get(&vocabulary::DESCEND) {
        let step = crate::path::read_step(content.as_record()?.get(&vocabulary::STEP)?)?;
        return Some(descend(step, None, None));
    }
    if let Some(content) = fields.get(&vocabulary::AT) {
        let content = content.as_record()?;
        let steps = crate::path::read(content.get(&vocabulary::STEPS)?)?;
        return Some(Layout::At {
            steps,
            value: content.get(&vocabulary::VALUE)?.clone(),
            projection: None,
            default_projection: None,
        });
    }
    if let Some(content) = fields.get(&vocabulary::TRANSIENT) {
        let content = content.as_record()?;
        let fuel = read_number(content.get(&vocabulary::FUEL)?)?;
        (fuel >= 0.0 && fuel.fract() == 0.0).then_some(())?;
        return Some(Layout::Transient {
            value: content.get(&vocabulary::VALUE)?.clone(),
            fuel: fuel as usize,
        });
    }
    if let Some(content) = fields.get(&vocabulary::TEXT) {
        let content = content.as_record()?;
        let face = match content.get(&vocabulary::PAINT)?.as_cell()? {
            cell if cell == vocabulary::NAME_FACE => Face::Name,
            cell if cell == vocabulary::STRING_FACE => Face::String,
            cell if cell == vocabulary::DIM_FACE => Face::Dim,
            cell if cell == vocabulary::LABEL_FACE => Face::Label,
            cell if cell == vocabulary::ID_FACE => Face::Id,
            cell if cell == vocabulary::ACCENT_WASH_FACE => Face::AccentWash,
            cell if cell == vocabulary::INK_FACE => Face::Ink,
            _ => return None,
        };
        return Some(leaf(Leaf::Text {
            text: text::read(content.get(&vocabulary::CONTENT)?)?.to_string(),
            paint: Paint::Face(face),
            script: puri::text::Script::Normal,
        }));
    }
    if let Some(content) = fields.get(&vocabulary::DRAWING) {
        let content = content.as_record()?;
        let width = read_nonnegative(content.get(&vocabulary::WIDTH)?)?;
        let ascent = read_nonnegative(content.get(&vocabulary::ASCENT)?)?;
        let descent = read_nonnegative(content.get(&vocabulary::DESCENT)?)?;
        if let Some(program) = content.get(&vocabulary::PROGRAM) {
            let fuel = read_nonnegative(content.get(&vocabulary::FUEL)?)?;
            (fuel.fract() == 0.0).then_some(())?;
            return Some(Layout::DrawingProgram {
                width,
                ascent,
                descent,
                fuel: fuel as usize,
                program: program.clone(),
            });
        }
        let commands = content
            .get(&vocabulary::COMMANDS)?
            .as_list()?
            .values()
            .map(read_command)
            .collect::<Option<Vec<_>>>()?;
        return Some(leaf(Leaf::Drawing(Drawing {
            width,
            ascent,
            descent,
            commands,
        })));
    }
    if fields.get(&vocabulary::SLOT).is_some() {
        return Some(slot());
    }
    if let Some(content) = fields.get(&vocabulary::SELECTABLE) {
        let child = decode_with(content, target)?;
        let interaction = target();
        return Some(on_activate(child, interaction.hover, interaction.select));
    }
    if let Some(content) = fields.get(&vocabulary::PICKABLE) {
        let content = content.as_record()?;
        let child = decode_with(content.get(&vocabulary::CHILD)?, target)?;
        let interaction = target();
        return Some(pickable(
            child,
            interaction.hover,
            content.get(&vocabulary::VALUE)?.clone(),
        ));
    }
    if let Some(content) = fields.get(&vocabulary::HOVERABLE) {
        return Some(on_hover(decode_with(content, target)?, target().hover));
    }
    if let Some(content) = fields.get(&vocabulary::HOVER_BLOCK) {
        return Some(block_hover(decode_with(content, target)?));
    }
    if let Some(content) = fields.get(&vocabulary::ON_EVENT) {
        let content = content.as_record()?;
        return Some(on_event(
            decode_with(content.get(&vocabulary::CHILD)?, target)?,
            content.get(&vocabulary::HANDLER)?.clone(),
        ));
    }
    None
}

fn children<World: 'static, Hover: Clone + 'static>(
    list: &Value,
    target: &impl Fn() -> ProjectionTarget<World, Hover>,
) -> Option<Vec<Layout<World, Hover>>> {
    list.as_list()?
        .values()
        .map(|child| decode_with(child, target))
        .collect()
}

fn read_number(value: &Value) -> Option<f64> {
    f64_convention::read(value).filter(|number| number.is_finite())
}

fn read_nonnegative(value: &Value) -> Option<f64> {
    read_number(value).filter(|number| *number >= 0.0)
}

fn read_face(value: &Value) -> Option<Face> {
    match value.as_cell()? {
        cell if cell == vocabulary::NAME_FACE => Some(Face::Name),
        cell if cell == vocabulary::STRING_FACE => Some(Face::String),
        cell if cell == vocabulary::DIM_FACE => Some(Face::Dim),
        cell if cell == vocabulary::LABEL_FACE => Some(Face::Label),
        cell if cell == vocabulary::ID_FACE => Some(Face::Id),
        cell if cell == vocabulary::ACCENT_WASH_FACE => Some(Face::AccentWash),
        cell if cell == vocabulary::INK_FACE => Some(Face::Ink),
        _ => None,
    }
}

pub fn read_paint(value: &Value) -> Option<Paint> {
    read_face(value)
        .map(Paint::Face)
        .or_else(|| color::read(value).map(|color| Paint::Brush(Brush::from(color))))
        .or_else(|| read_linear_gradient(value).map(|gradient| Paint::Brush(Brush::from(gradient))))
}

fn read_linear_gradient(value: &Value) -> Option<Gradient> {
    let fields = value
        .as_record()?
        .get(&vocabulary::LINEAR_GRADIENT)?
        .as_record()?;
    let start = read_point(fields.get(&vocabulary::START)?)?;
    let end = read_point(fields.get(&vocabulary::END)?)?;
    let stops = fields
        .get(&vocabulary::STOPS)?
        .as_list()?
        .values()
        .map(|stop| {
            let stop = stop.as_record()?;
            let offset = read_number(stop.get(&vocabulary::OFFSET)?)?;
            (0.0..=1.0).contains(&offset).then_some(ColorStop {
                offset: offset as f32,
                color: color::read(stop.get(&vocabulary::PAINT)?)?.into(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Gradient::new_linear(start, end).with_stops(stops.as_slice()))
}

fn read_command(value: &Value) -> Option<Command<Paint>> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::FILL) {
        let content = content.as_record()?;
        Some(Command::Fill {
            shape: read_shape(content.get(&vocabulary::SHAPE)?)?,
            paint: read_paint(content.get(&vocabulary::PAINT)?)?,
            transform: read_optional_transform(content)?,
        })
    } else if let Some(content) = fields.get(&vocabulary::STROKE) {
        let content = content.as_record()?;
        Some(Command::Stroke {
            shape: read_shape(content.get(&vocabulary::SHAPE)?)?,
            style: Stroke::new(read_nonnegative(content.get(&vocabulary::LINE_WIDTH)?)?),
            paint: read_paint(content.get(&vocabulary::PAINT)?)?,
            transform: read_optional_transform(content)?,
        })
    } else if let Some(content) = fields.get(&vocabulary::CLIP) {
        let content = content.as_record()?;
        Some(Command::Clip {
            shape: read_shape(content.get(&vocabulary::SHAPE)?)?,
            transform: read_optional_transform(content)?,
            children: content
                .get(&vocabulary::COMMANDS)?
                .as_list()?
                .values()
                .map(read_command)
                .collect::<Option<Vec<_>>>()?,
        })
    } else {
        None
    }
}

fn read_optional_transform(fields: &gid::Record) -> Option<Affine> {
    match fields.get(&vocabulary::TRANSFORM) {
        Some(transform) => read_transform(transform),
        None => Some(Affine::IDENTITY),
    }
}

pub fn read_transform(value: &Value) -> Option<Affine> {
    value
        .as_list()?
        .values()
        .try_fold(Affine::IDENTITY, |transform, operation| {
            let fields = operation.as_record()?;
            if let Some(point) = fields.get(&vocabulary::TRANSLATE) {
                let point = read_point(point)?;
                Some(transform * Affine::translate((point.x, point.y)))
            } else if let Some(angle) = fields.get(&vocabulary::ROTATE) {
                Some(transform * Affine::rotate(read_number(angle)?))
            } else {
                None
            }
        })
}

pub fn read_shape(value: &Value) -> Option<Shape> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::RECT) {
        let (x, y, width, height) = read_box(content)?;
        Some(Shape::Rect(Rect::new(x, y, x + width, y + height)))
    } else if let Some(content) = fields.get(&vocabulary::ROUNDED_RECT) {
        let (x, y, width, height) = read_box(content)?;
        let radius = read_nonnegative(content.as_record()?.get(&vocabulary::RADIUS)?)?;
        Some(Shape::RoundedRect(RoundedRect::from_rect(
            Rect::new(x, y, x + width, y + height),
            radius,
        )))
    } else if let Some(content) = fields.get(&vocabulary::CIRCLE) {
        let content = content.as_record()?;
        Some(Shape::Circle(Circle::new(
            Point::new(
                read_number(content.get(&vocabulary::X)?)?,
                read_number(content.get(&vocabulary::Y)?)?,
            ),
            read_nonnegative(content.get(&vocabulary::RADIUS)?)?,
        )))
    } else if let Some(content) = fields.get(&vocabulary::LINE) {
        let content = content.as_record()?;
        Some(Shape::Line(Line::new(
            Point::new(
                read_number(content.get(&vocabulary::X1)?)?,
                read_number(content.get(&vocabulary::Y1)?)?,
            ),
            Point::new(
                read_number(content.get(&vocabulary::X2)?)?,
                read_number(content.get(&vocabulary::Y2)?)?,
            ),
        )))
    } else if let Some(content) = fields.get(&vocabulary::PATH) {
        let mut path = BezPath::new();
        for element in content.as_list()?.values() {
            read_path_element(&mut path, element)?;
        }
        Some(Shape::Path(path))
    } else {
        None
    }
}

fn read_box(value: &Value) -> Option<(f64, f64, f64, f64)> {
    let fields = value.as_record()?;
    Some((
        read_number(fields.get(&vocabulary::X)?)?,
        read_number(fields.get(&vocabulary::Y)?)?,
        read_nonnegative(fields.get(&vocabulary::WIDTH)?)?,
        read_nonnegative(fields.get(&vocabulary::HEIGHT)?)?,
    ))
}

fn read_path_element(path: &mut BezPath, value: &Value) -> Option<()> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::MOVE_TO) {
        path.move_to(read_point(content)?);
    } else if let Some(content) = fields.get(&vocabulary::LINE_TO) {
        path.line_to(read_point(content)?);
    } else if let Some(content) = fields.get(&vocabulary::QUAD_TO) {
        let content = content.as_record()?;
        path.quad_to(
            Point::new(
                read_number(content.get(&vocabulary::X1)?)?,
                read_number(content.get(&vocabulary::Y1)?)?,
            ),
            Point::new(
                read_number(content.get(&vocabulary::X)?)?,
                read_number(content.get(&vocabulary::Y)?)?,
            ),
        );
    } else if let Some(content) = fields.get(&vocabulary::CURVE_TO) {
        let content = content.as_record()?;
        path.curve_to(
            Point::new(
                read_number(content.get(&vocabulary::X1)?)?,
                read_number(content.get(&vocabulary::Y1)?)?,
            ),
            Point::new(
                read_number(content.get(&vocabulary::X2)?)?,
                read_number(content.get(&vocabulary::Y2)?)?,
            ),
            Point::new(
                read_number(content.get(&vocabulary::X)?)?,
                read_number(content.get(&vocabulary::Y)?)?,
            ),
        );
    } else if fields.get(&vocabulary::CLOSE).is_some() {
        path.close_path();
    } else {
        return None;
    }
    Some(())
}

fn read_point(value: &Value) -> Option<Point> {
    let fields = value.as_record()?;
    Some(Point::new(
        read_number(fields.get(&vocabulary::X)?)?,
        read_number(fields.get(&vocabulary::Y)?)?,
    ))
}

pub fn display<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    decode_with(input.value?, &|| input.targets.current())
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::ROW, "row"),
        (vocabulary::COL, "col"),
        (vocabulary::PAD, "pad"),
        (vocabulary::BORDER, "border"),
        (vocabulary::OVERLAY, "overlay"),
        (vocabulary::BRACKET, "bracket"),
        (vocabulary::ALTERNATIVES, "alternatives"),
        (vocabulary::DESCEND, "descend"),
        (vocabulary::AT, "at"),
        (vocabulary::TRANSIENT, "transient"),
        (vocabulary::TEXT, "text"),
        (vocabulary::DRAWING, "drawing"),
        (vocabulary::PROGRAM, "program"),
        (vocabulary::SLOT, "slot"),
        (vocabulary::SELECTABLE, "selectable"),
        (vocabulary::PICKABLE, "pickable"),
        (vocabulary::HOVERABLE, "hoverable"),
        (vocabulary::HOVER_BLOCK, "hover block"),
        (vocabulary::ON_EVENT, "on event"),
        (vocabulary::HANDLER, "handler"),
        (vocabulary::EVENT, "event"),
        (vocabulary::EVENT_KIND, "event kind"),
        (vocabulary::POINTER_DOWN, "pointer down"),
        (vocabulary::POINTER_MOVE, "pointer move"),
        (vocabulary::POINTER_UP, "pointer up"),
        (vocabulary::POINTER_CANCEL, "pointer cancel"),
        (vocabulary::TOUCH_START, "touch start"),
        (vocabulary::TOUCH_MOVE, "touch move"),
        (vocabulary::TOUCH_END, "touch end"),
        (vocabulary::TOUCH_CANCEL, "touch cancel"),
        (vocabulary::SCROLL, "scroll"),
        (vocabulary::KEY, "key"),
        (vocabulary::IME, "ime"),
        (vocabulary::GAP, "gap"),
        (vocabulary::BASELINE, "baseline"),
        (vocabulary::CHILDREN, "children"),
        (vocabulary::CHILD, "child"),
        (vocabulary::LEFT, "left"),
        (vocabulary::TOP, "top"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::BOTTOM, "bottom"),
        (vocabulary::DELIM, "delim"),
        (vocabulary::CONTENT, "content"),
        (vocabulary::PAINT, "paint"),
        (vocabulary::STEP, "step"),
        (vocabulary::STEPS, "steps"),
        (vocabulary::VALUE, "value"),
        (vocabulary::FUEL, "fuel"),
        (vocabulary::WIDTH, "width"),
        (vocabulary::HEIGHT, "height"),
        (vocabulary::ASCENT, "ascent"),
        (vocabulary::DESCENT, "descent"),
        (vocabulary::COMMANDS, "commands"),
        (vocabulary::X, "x"),
        (vocabulary::Y, "y"),
        (vocabulary::BUTTON, "button"),
        (vocabulary::PRIMARY, "primary"),
        (vocabulary::SHIFT, "shift"),
        (vocabulary::COMMAND, "command"),
        (vocabulary::MODIFIERS, "modifiers"),
        (vocabulary::COUNT, "count"),
        (vocabulary::SCALE, "scale"),
        (vocabulary::DELTA_X, "delta x"),
        (vocabulary::DELTA_Y, "delta y"),
        (vocabulary::COALESCED, "coalesced"),
        (vocabulary::EVENT_STATE, "event state"),
        (vocabulary::DOWN, "down"),
        (vocabulary::UP, "up"),
        (vocabulary::REPEAT, "repeat"),
        (vocabulary::IME_ENABLED, "ime enabled"),
        (vocabulary::IME_DISABLED, "ime disabled"),
        (vocabulary::IME_PREEDIT, "ime preedit"),
        (vocabulary::IME_COMMIT, "ime commit"),
        (vocabulary::START, "start"),
        (vocabulary::END, "end"),
        (vocabulary::RADIUS, "radius"),
        (vocabulary::LINE_WIDTH, "line width"),
        (vocabulary::FILL, "fill"),
        (vocabulary::STROKE, "stroke"),
        (vocabulary::CLIP, "clip"),
        (vocabulary::SHAPE, "shape"),
        (vocabulary::RECT, "rect"),
        (vocabulary::ROUNDED_RECT, "rounded rect"),
        (vocabulary::CIRCLE, "circle"),
        (vocabulary::LINE, "line"),
        (vocabulary::PATH, "path"),
        (vocabulary::X1, "x1"),
        (vocabulary::Y1, "y1"),
        (vocabulary::X2, "x2"),
        (vocabulary::Y2, "y2"),
        (vocabulary::MOVE_TO, "move to"),
        (vocabulary::LINE_TO, "line to"),
        (vocabulary::QUAD_TO, "quad to"),
        (vocabulary::CURVE_TO, "curve to"),
        (vocabulary::CLOSE, "close"),
        (vocabulary::LINEAR_GRADIENT, "linear gradient"),
        (vocabulary::STOPS, "stops"),
        (vocabulary::OFFSET, "offset"),
        (vocabulary::TRANSFORM, "transform"),
        (vocabulary::TRANSLATE, "translate"),
        (vocabulary::ROTATE, "rotate"),
        (vocabulary::NAME_FACE, "name face"),
        (vocabulary::STRING_FACE, "string face"),
        (vocabulary::DIM_FACE, "dim face"),
        (vocabulary::LABEL_FACE, "label face"),
        (vocabulary::ID_FACE, "id face"),
        (vocabulary::ACCENT_WASH_FACE, "accent wash face"),
        (vocabulary::INK_FACE, "ink face"),
        (vocabulary::PAREN, "paren"),
        (vocabulary::SQUARE, "square"),
        (vocabulary::CURLY, "curly"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "layout",
        crate::Definitions::from_parts(
            cells,
            ForeignFunctions::default()
                .register(
                    vocabulary::DRAWING,
                    ForeignFunction::new(drawing_projection),
                )
                .register(
                    vocabulary::BORDER,
                    ForeignFunction::runtime(border_projection),
                )
                .register(
                    APPLY_BORDER_PROJECTION,
                    ForeignFunction::runtime(apply_border_projection),
                ),
        ),
        progred_display::partial(display::<World, Hover>),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn decoded(value: &Value) -> Option<Layout<(), ()>> {
        let select: ActionHandler<()> = Rc::new(|_| false);
        decode(value, &select, &())
    }

    #[test]
    fn traversal_uses_the_path_library_for_every_step() {
        let steps = vec![
            Step::Key(gid::new_cell_id()),
            Step::Element(gid::position::between(None, None).unwrap()),
            Step::Follow(gid::Resolution::Document),
            Step::Follow(gid::Resolution::Library(gid::new_cell_id())),
        ];
        for step in &steps {
            assert!(matches!(
                decoded(&descend_step(step.clone())),
                Some(Layout::Descend { step: decoded, .. }) if decoded == *step
            ));
        }
        let child = text::value("child");
        let value = node(
            vocabulary::AT,
            Value::record([
                (vocabulary::STEPS, crate::path::value(&steps)),
                (vocabulary::VALUE, child.clone()),
            ]),
        );
        assert!(matches!(
            decoded(&value),
            Some(Layout::At { steps: decoded, value, .. }) if decoded == steps && value == child
        ));
    }

    #[test]
    fn drawing_is_an_ordinary_projection_function() {
        let library = library::<(), ()>();
        let configuration = Value::record([(vocabulary::WIDTH, number(12.0))]);
        let evaluation = crate::test_apply(
            &Value::from(vocabulary::DRAWING),
            [(presentation::vocabulary::VALUE, configuration.clone())],
            |cell| library.value(cell).cloned(),
            &library.functions(),
            20,
        );
        assert_eq!(
            evaluation.result,
            Value::record([(vocabulary::DRAWING, configuration)])
        );
    }

    #[test]
    fn border_composes_with_an_ordinary_projection() {
        let library = library::<(), ()>();
        let configuration = Value::record([(vocabulary::WIDTH, number(12.0))]);
        let composition = crate::test_evaluate(
            &grap_runtime::call(
                Value::from(vocabulary::BORDER),
                [(
                    presentation::vocabulary::PROJECTION,
                    grap_runtime::ffi(vocabulary::DRAWING),
                )],
            ),
            |cell| library.value(cell).cloned(),
            &library.functions(),
            20,
        );
        assert!(
            composition
                .result
                .as_record()
                .is_some_and(|fields| fields.contains_key(&grap_runtime::vocabulary::CLOSURE))
        );
        let evaluation = crate::test_apply(
            &composition.result,
            [(presentation::vocabulary::VALUE, configuration.clone())],
            |cell| library.value(cell).cloned(),
            &library.functions(),
            50,
        );
        let Some(Layout::After { child, .. }) = decoded(&evaluation.result) else {
            panic!(
                "the composed projection returns a border: {:?}",
                evaluation.result
            );
        };
        assert!(matches!(
            child.as_ref(),
            Layout::Transient { value, .. }
                if value == &Value::record([(vocabulary::DRAWING, configuration)])
        ));
    }

    #[test]
    fn border_wraps_any_decoded_layout() {
        let value = bordered(text_leaf("inside", vocabulary::NAME_FACE));
        let Some(Layout::After { child, .. }) = decoded(&value) else {
            panic!("border decodes");
        };
        assert!(matches!(
            child.as_ref(),
            Layout::Leaf(Leaf::Text { text, .. }) if text == "inside"
        ));
    }

    #[test]
    fn a_layout_round_trips_from_data() {
        let value = options([
            selectable(row(
                4.0,
                [
                    text_leaf("shape", vocabulary::NAME_FACE),
                    descend_key(vocabulary::GAP),
                ],
            )),
            bracketed(
                vocabulary::CURLY,
                col(
                    0,
                    2.0,
                    [
                        descend_follow(gid::Resolution::Document),
                        text_leaf("…", vocabulary::DIM_FACE),
                    ],
                ),
            ),
        ]);
        let Some(Layout::Alternatives(forms)) = decoded(&value) else {
            panic!("alternatives decode");
        };
        assert_eq!(forms.len(), 2);
        let Layout::Before { child, .. } = &forms[0] else {
            panic!("selectable attaches the provided select");
        };
        let Layout::Row { gap, children, .. } = child.as_ref() else {
            panic!("row inside");
        };
        assert_eq!(*gap, 4.0);
        assert!(matches!(
            &children[0],
            Layout::Leaf(Leaf::Text {
                text,
                paint: Paint::Face(Face::Name),
                ..
            }) if text == "shape"
        ));
        assert!(matches!(
            &children[1],
            Layout::Descend { step: Step::Key(key), .. } if *key == vocabulary::GAP
        ));
        let Layout::Surround { left, child, right } = &forms[1] else {
            panic!("bracket decodes to side widgets around its child");
        };
        crate::test_widgets::assert_delimiter(left, Delim::Brace, progred_display::Side::Open);
        crate::test_widgets::assert_delimiter(right, Delim::Brace, progred_display::Side::Close);
        let Layout::Col { children, .. } = child.as_ref() else {
            panic!("col inside");
        };
        assert!(matches!(
            &children[0],
            Layout::Descend {
                step: Step::Follow(gid::Resolution::Document),
                ..
            }
        ));
    }

    #[test]
    fn intents_and_leaves_decode() {
        assert_eq!(
            crate::test_widgets::picked(
                &decoded(&pick_target(
                    text_leaf("k", vocabulary::ID_FACE),
                    Value::Cell(vocabulary::GAP),
                ))
                .unwrap()
            ),
            Some(Value::Cell(vocabulary::GAP))
        );
        assert_eq!(
            crate::test_widgets::claim(
                &decoded(&hoverable(text_leaf("h", vocabulary::LABEL_FACE))).unwrap()
            ),
            Some(puri::hover::Claim::Direct(()))
        );
        assert_eq!(
            crate::test_widgets::claim(
                &decoded(&hover_block(node(vocabulary::SLOT, Value::record([])))).unwrap()
            ),
            Some(puri::hover::Claim::Occludes)
        );
        let handler = Value::Cell(vocabulary::HANDLER);
        assert_eq!(
            crate::test_widgets::event_handler(
                &decoded(&on(text_leaf("go", vocabulary::NAME_FACE), handler.clone())).unwrap()
            ),
            Some(handler)
        );
    }

    #[test]
    fn puri_commands_are_ordinary_display_data() {
        let display = drawing(
            20.0,
            8.0,
            2.0,
            [
                fill(
                    rounded_rect(0.0, 0.0, 20.0, 10.0, 2.0),
                    Value::from(vocabulary::DIM_FACE),
                ),
                stroke(
                    circle(10.0, 5.0, 4.0),
                    1.0,
                    Value::from(vocabulary::NAME_FACE),
                ),
                clip(
                    rect(0.0, 0.0, 20.0, 10.0),
                    [stroke(
                        path([
                            move_to(0.0, 0.0),
                            line_to(5.0, 5.0),
                            quad_to(7.0, 3.0, 10.0, 5.0),
                            curve_to(11.0, 6.0, 12.0, 4.0, 15.0, 5.0),
                            close(),
                        ]),
                        2.0,
                        Value::from(vocabulary::INK_FACE),
                    )],
                ),
            ],
        );
        let Some(Layout::Leaf(Leaf::Drawing(drawing))) = decoded(&display) else {
            panic!("Puri drawing leaf");
        };
        assert_eq!(
            (drawing.width, drawing.ascent, drawing.descent),
            (20.0, 8.0, 2.0)
        );
        assert_eq!(drawing.commands.len(), 3);
        assert!(matches!(
            drawing.commands[1],
            Command::Stroke { shape: Shape::Circle(_), ref style, .. } if style.width == 1.0
        ));
        assert!(matches!(
            drawing.commands[2],
            Command::Clip { ref children, .. }
                if matches!(children.as_slice(), [Command::Stroke { shape: Shape::Path(_), .. }])
        ));
    }

    #[test]
    fn junk_falls_through_whole() {
        assert!(decoded(&Value::record([])).is_none());
        assert!(decoded(&text::value("plain text is not a layout")).is_none());
        let broken = row(
            1.0,
            [text_leaf("ok", vocabulary::NAME_FACE), Value::record([])],
        );
        assert!(decoded(&broken).is_none());
        let bad_face = node(
            vocabulary::TEXT,
            Value::record([
                (vocabulary::CONTENT, text::value("x")),
                (vocabulary::PAINT, Value::Cell(vocabulary::GAP)),
            ]),
        );
        assert!(decoded(&bad_face).is_none());
    }
}
