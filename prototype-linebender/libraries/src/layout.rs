//! The display language's data form: layouts as GID values, so a
//! projection defined in a document can RETURN one. Each node is a
//! record under a single marker key, strings ride the text convention
//! and numbers the f64 convention. Interaction either attaches
//! host-provided editor intents or a Grap handler to a generic event
//! kind. Event dispatch supplies a capability overlay closed over the
//! projection site, so document paths never enter layout data.
//! Decoding is resilient the projection way: any junk node decodes to
//! `None`, and the whole layout falls through to the next partial.
//!
use crate::{Library, f64 as f64_convention, name, text};
use gid::{CellId, Step, Value};
use progred_display::{
    ClickHandler, Delim, Display, EventKind, Face, Layout, Vector, VectorCommand, alternatives,
    block_hover, bracket, leaf, on_click, on_event, on_hover, overlay as layout_overlay, pickable,
    slot,
};

pub mod vocabulary {
    use gid::CellId;

    // Boxes and walk.
    pub const ROW: CellId = CellId::from_u128(0x1af52c96e380b7d40c9e1f6a2d5b83e7);
    pub const COL: CellId = CellId::from_u128(0x9d04b6e1783f2ca5f17d09c4e6a2358b);
    pub const PAD: CellId = CellId::from_u128(0x4e8a17d0952cb6f3a30c5e92b7d1f648);
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
    pub const VECTOR: CellId = CellId::from_u128(0x1c2b3ed21cc83b14e73f02aa5067423e);
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
    pub const FACE: CellId = CellId::from_u128(0xcb04728f5e6a1d93a6790238b5f1ce4d);
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
    pub const FILL_ROUNDED_RECT: CellId =
        CellId::from_u128(0xd63668dc14d562633833bb7677c97df4);
    pub const STROKE_ROUNDED_RECT: CellId =
        CellId::from_u128(0x9e3e40a6c5b11699b89c182266f53e0a);

    // Faces.
    pub const NAME_FACE: CellId = CellId::from_u128(0x520e9b3c7ad6f18409cf25a7d8631be0);
    pub const STRING_FACE: CellId = CellId::from_u128(0x81c5bf325b9be5e1923e4317371b0eab);
    pub const DIM_FACE: CellId = CellId::from_u128(0xf14b6a08d29c53e7bd0561f8a3c2497e);
    pub const LABEL_FACE: CellId = CellId::from_u128(0x7d90c4e5f1382ab6270d94c1e5a8f36b);
    pub const ID_FACE: CellId = CellId::from_u128(0xb38a1d67e02f49c5c9e8073a6b5d21f4);
    pub const ACCENT_WASH_FACE: CellId =
        CellId::from_u128(0x1ec921b1240171ceb6dcae8d15889ef4);
    pub const INK_FACE: CellId = CellId::from_u128(0xe553afe01621dbdfe528b1f5fcd69e21);

    // Delimiters.
    pub const PAREN: CellId = CellId::from_u128(0x0af59c27b1e4d68318f4a06c9d7325eb);
    pub const SQUARE: CellId = CellId::from_u128(0x9e61d40b7f3ca258745c1e9b02d8f6a3);
    pub const CURLY: CellId = CellId::from_u128(0x63b8f5a2c90e17d4e12489d5b6a0c73f);

    /// A walk step following the value's link — the one step that is
    /// not a field key.
    pub const FOLLOW: CellId = CellId::from_u128(0xdc27a94e6b105f83b0562f8ea19d34c7);

    // The document's partial registry and the partial call contract.
    /// The document sets this cell's value to a list of Grap
    /// callables; the editor tries each per value, before its own
    /// partials, and any decline falls through whole.
    pub const PROJECTIONS: CellId = CellId::from_u128(0xcdaff65dbbd2e37b0a7deafe862d8695);
    /// Argument: the selection payload, absent-classified when this
    /// value's path is not the selected one.
    pub const SELECTION: CellId = CellId::from_u128(0x0fd3adb8df8a5b05905df9a88db337fd);
    /// Argument: this path's annotation record, absent-classified
    /// when there is none. The projected value arrives as [`VALUE`].
    pub const STATE: CellId = CellId::from_u128(0x69760bdc4814129a9d31510f0f1c3123);
}

fn node(key: CellId, content: Value) -> Value {
    Value::record([(key, content)])
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
    node(
        vocabulary::DESCEND,
        Value::record([(vocabulary::STEP, Value::Cell(key))]),
    )
}

pub fn descend_follow() -> Value {
    node(
        vocabulary::DESCEND,
        Value::record([(vocabulary::STEP, Value::Cell(vocabulary::FOLLOW))]),
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
            (vocabulary::FACE, Value::Cell(face)),
        ]),
    )
}

pub fn vector(
    width: f64,
    ascent: f64,
    descent: f64,
    commands: impl IntoIterator<Item = Value>,
) -> Value {
    node(
        vocabulary::VECTOR,
        Value::record([
            (vocabulary::WIDTH, number(width)),
            (vocabulary::ASCENT, number(ascent)),
            (vocabulary::DESCENT, number(descent)),
            (vocabulary::COMMANDS, Value::list(commands)),
        ]),
    )
}

pub fn fill_rounded_rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    face: CellId,
) -> Value {
    vector_command(
        vocabulary::FILL_ROUNDED_RECT,
        x,
        y,
        width,
        height,
        radius,
        None,
        face,
    )
}

pub fn stroke_rounded_rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    line_width: f64,
    face: CellId,
) -> Value {
    vector_command(
        vocabulary::STROKE_ROUNDED_RECT,
        x,
        y,
        width,
        height,
        radius,
        Some(line_width),
        face,
    )
}

#[allow(clippy::too_many_arguments)]
fn vector_command(
    kind: CellId,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    line_width: Option<f64>,
    face: CellId,
) -> Value {
    let fields = [
        Some((vocabulary::X, number(x))),
        Some((vocabulary::Y, number(y))),
        Some((vocabulary::WIDTH, number(width))),
        Some((vocabulary::HEIGHT, number(height))),
        Some((vocabulary::RADIUS, number(radius))),
        line_width.map(|width| (vocabulary::LINE_WIDTH, number(width))),
        Some((vocabulary::FACE, Value::Cell(face))),
    ];
    node(kind, Value::record(fields.into_iter().flatten()))
}

pub fn selectable(child: Value) -> Value {
    node(vocabulary::SELECTABLE, child)
}

pub fn pick_target(child: Value, value: Value) -> Value {
    node(
        vocabulary::PICKABLE,
        Value::record([
            (vocabulary::CHILD, child),
            (vocabulary::VALUE, value),
        ]),
    )
}

pub fn hoverable(child: Value) -> Value {
    node(vocabulary::HOVERABLE, child)
}

pub fn hover_block(child: Value) -> Value {
    node(vocabulary::HOVER_BLOCK, child)
}

pub fn on(child: Value, kind: CellId, handler: Value) -> Value {
    node(
        vocabulary::ON_EVENT,
        Value::record([
            (vocabulary::CHILD, child),
            (vocabulary::EVENT_KIND, Value::Cell(kind)),
            (vocabulary::HANDLER, handler),
        ]),
    )
}

pub fn click(child: Value, handler: Value) -> Value {
    on(child, vocabulary::POINTER_DOWN, handler)
}

/// Decode a layout value into the display language, attaching the
/// PROVIDED intents where the data marks their spots. `None` on any
/// junk, so a malformed layout falls through whole.
pub fn decode<World, Hover: Clone>(
    value: &Value,
    select: &ClickHandler<World>,
    hover: &Hover,
) -> Option<Layout<World, Hover>> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::ROW) {
        let content = content.as_record()?;
        return Some(Layout::Row {
            gap: read_number(content.get(&vocabulary::GAP)?)?,
            children: children(content.get(&vocabulary::CHILDREN)?, select, hover)?,
        });
    }
    if let Some(content) = fields.get(&vocabulary::COL) {
        let content = content.as_record()?;
        let baseline = read_number(content.get(&vocabulary::BASELINE)?)?;
        (baseline >= 0.0 && baseline.fract() == 0.0).then_some(())?;
        return Some(Layout::Col {
            baseline: baseline as usize,
            gap: read_number(content.get(&vocabulary::GAP)?)?,
            children: children(content.get(&vocabulary::CHILDREN)?, select, hover)?,
        });
    }
    if let Some(content) = fields.get(&vocabulary::OVERLAY) {
        return Some(layout_overlay(children(content, select, hover)?));
    }
    if let Some(content) = fields.get(&vocabulary::PAD) {
        let content = content.as_record()?;
        return Some(Layout::Pad {
            left: read_number(content.get(&vocabulary::LEFT)?)?,
            top: read_number(content.get(&vocabulary::TOP)?)?,
            right: read_number(content.get(&vocabulary::RIGHT)?)?,
            bottom: read_number(content.get(&vocabulary::BOTTOM)?)?,
            child: Box::new(decode(content.get(&vocabulary::CHILD)?, select, hover)?),
        });
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
            decode(content.get(&vocabulary::CHILD)?, select, hover)?,
        ));
    }
    if let Some(content) = fields.get(&vocabulary::ALTERNATIVES) {
        return Some(alternatives(children(content, select, hover)?));
    }
    if let Some(content) = fields.get(&vocabulary::DESCEND) {
        let step = read_step(content.as_record()?.get(&vocabulary::STEP)?)?;
        return Some(Layout::Descend { step });
    }
    if let Some(content) = fields.get(&vocabulary::AT) {
        let content = content.as_record()?;
        let steps = content
            .get(&vocabulary::STEPS)?
            .as_list()?
            .values()
            .map(read_step)
            .collect::<Option<Vec<Step>>>()?;
        return Some(Layout::At {
            steps,
            value: content.get(&vocabulary::VALUE)?.clone(),
            projection: None,
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
        let face = match content.get(&vocabulary::FACE)?.as_cell()? {
            cell if cell == vocabulary::NAME_FACE => Face::Name,
            cell if cell == vocabulary::STRING_FACE => Face::String,
            cell if cell == vocabulary::DIM_FACE => Face::Dim,
            cell if cell == vocabulary::LABEL_FACE => Face::Label,
            cell if cell == vocabulary::ID_FACE => Face::Id,
            cell if cell == vocabulary::ACCENT_WASH_FACE => Face::AccentWash,
            cell if cell == vocabulary::INK_FACE => Face::Ink,
            _ => return None,
        };
        return Some(leaf(Display::Text {
            text: text::read(content.get(&vocabulary::CONTENT)?)?.to_string(),
            face,
        }));
    }
    if let Some(content) = fields.get(&vocabulary::VECTOR) {
        let content = content.as_record()?;
        let width = read_nonnegative(content.get(&vocabulary::WIDTH)?)?;
        let ascent = read_nonnegative(content.get(&vocabulary::ASCENT)?)?;
        let descent = read_nonnegative(content.get(&vocabulary::DESCENT)?)?;
        let commands = content
            .get(&vocabulary::COMMANDS)?
            .as_list()?
            .values()
            .map(read_vector_command)
            .collect::<Option<Vec<_>>>()?;
        return Some(leaf(Display::Vector(Vector {
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
        return Some(on_click(decode(content, select, hover)?, select.clone()));
    }
    if let Some(content) = fields.get(&vocabulary::PICKABLE) {
        let content = content.as_record()?;
        return Some(pickable(
            decode(content.get(&vocabulary::CHILD)?, select, hover)?,
            content.get(&vocabulary::VALUE)?.clone(),
        ));
    }
    if let Some(content) = fields.get(&vocabulary::HOVERABLE) {
        return Some(on_hover(decode(content, select, hover)?, hover.clone()));
    }
    if let Some(content) = fields.get(&vocabulary::HOVER_BLOCK) {
        return Some(block_hover(decode(content, select, hover)?));
    }
    if let Some(content) = fields.get(&vocabulary::ON_EVENT) {
        let content = content.as_record()?;
        return Some(on_event(
            decode(content.get(&vocabulary::CHILD)?, select, hover)?,
            read_event_kind(content.get(&vocabulary::EVENT_KIND)?)?,
            content.get(&vocabulary::HANDLER)?.clone(),
        ));
    }
    None
}

fn children<World, Hover: Clone>(
    list: &Value,
    select: &ClickHandler<World>,
    hover: &Hover,
) -> Option<Vec<Layout<World, Hover>>> {
    list.as_list()?
        .values()
        .map(|child| decode(child, select, hover))
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

fn read_vector_command(value: &Value) -> Option<VectorCommand> {
    let fields = value.as_record()?;
    let command = |content: &Value| {
        let content = content.as_record()?;
        Some((
            read_number(content.get(&vocabulary::X)?)?,
            read_number(content.get(&vocabulary::Y)?)?,
            read_nonnegative(content.get(&vocabulary::WIDTH)?)?,
            read_nonnegative(content.get(&vocabulary::HEIGHT)?)?,
            read_nonnegative(content.get(&vocabulary::RADIUS)?)?,
            read_face(content.get(&vocabulary::FACE)?)?,
        ))
    };
    if let Some(content) = fields.get(&vocabulary::FILL_ROUNDED_RECT) {
        let (x, y, width, height, radius, face) = command(content)?;
        Some(VectorCommand::FillRoundedRect {
            x,
            y,
            width,
            height,
            radius,
            face,
        })
    } else if let Some(content) = fields.get(&vocabulary::STROKE_ROUNDED_RECT) {
        let (x, y, width, height, radius, face) = command(content)?;
        Some(VectorCommand::StrokeRoundedRect {
            x,
            y,
            width,
            height,
            radius,
            line_width: read_nonnegative(
                content.as_record()?.get(&vocabulary::LINE_WIDTH)?,
            )?,
            face,
        })
    } else {
        None
    }
}

fn read_event_kind(value: &Value) -> Option<EventKind> {
    match value.as_cell()? {
        cell if cell == vocabulary::POINTER_DOWN => Some(EventKind::PointerDown),
        cell if cell == vocabulary::POINTER_MOVE => Some(EventKind::PointerMove),
        cell if cell == vocabulary::POINTER_UP => Some(EventKind::PointerUp),
        cell if cell == vocabulary::SCROLL => Some(EventKind::Scroll),
        cell if cell == vocabulary::KEY => Some(EventKind::Key),
        cell if cell == vocabulary::IME => Some(EventKind::Ime),
        _ => None,
    }
}

/// A walk step: the FOLLOW marker, or a field key's cell. List
/// positions have no data form yet; element walks stay Rust.
fn read_step(value: &Value) -> Option<Step> {
    let cell = value.as_cell()?;
    Some(if cell == vocabulary::FOLLOW {
        Step::Follow
    } else {
        Step::Key(cell)
    })
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::ROW, "row"),
        (vocabulary::COL, "col"),
        (vocabulary::PAD, "pad"),
        (vocabulary::OVERLAY, "overlay"),
        (vocabulary::BRACKET, "bracket"),
        (vocabulary::ALTERNATIVES, "alternatives"),
        (vocabulary::DESCEND, "descend"),
        (vocabulary::AT, "at"),
        (vocabulary::TRANSIENT, "transient"),
        (vocabulary::TEXT, "text"),
        (vocabulary::VECTOR, "vector"),
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
        (vocabulary::FACE, "face"),
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
        (vocabulary::FILL_ROUNDED_RECT, "fill rounded rect"),
        (vocabulary::STROKE_ROUNDED_RECT, "stroke rounded rect"),
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
        (vocabulary::FOLLOW, "follow"),
        (vocabulary::PROJECTIONS, "projections"),
        (vocabulary::SELECTION, "selection"),
        (vocabulary::STATE, "state"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library {
        cells,
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn decoded(value: &Value) -> Option<Layout<(), ()>> {
        let select: ClickHandler<()> = Rc::new(|_| false);
        decode(value, &select, &())
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
                col(0, 2.0, [descend_follow(), text_leaf("…", vocabulary::DIM_FACE)]),
            ),
        ]);
        let Some(Layout::Alternatives(forms)) = decoded(&value) else {
            panic!("alternatives decode");
        };
        assert_eq!(forms.len(), 2);
        let Layout::OnClick { child, .. } = &forms[0] else {
            panic!("selectable attaches the provided select");
        };
        let Layout::Row { gap, children } = child.as_ref() else {
            panic!("row inside");
        };
        assert_eq!(*gap, 4.0);
        assert!(matches!(
            &children[0],
            Layout::Leaf(Display::Text { text, face: Face::Name, .. }) if text == "shape"
        ));
        assert!(matches!(
            &children[1],
            Layout::Descend { step: Step::Key(key) } if *key == vocabulary::GAP
        ));
        let Layout::Surround {
            left: progred_display::Ink::Delim {
                delim: Delim::Brace,
                side: progred_display::Side::Open,
            },
            child,
            right: progred_display::Ink::Delim {
                delim: Delim::Brace,
                side: progred_display::Side::Close,
            },
        } = &forms[1]
        else {
            panic!("bracket decodes to a surround of delim ink");
        };
        let Layout::Col { children, .. } = child.as_ref() else {
            panic!("col inside");
        };
        assert!(matches!(&children[0], Layout::Descend { step: Step::Follow }));
    }

    #[test]
    fn intents_and_leaves_decode() {
        assert!(matches!(
            decoded(&pick_target(
                text_leaf("k", vocabulary::ID_FACE),
                Value::Cell(vocabulary::GAP),
            )),
            Some(Layout::OnPick { value, .. }) if value == Value::Cell(vocabulary::GAP)
        ));
        assert!(matches!(
            decoded(&hoverable(text_leaf("h", vocabulary::LABEL_FACE))),
            Some(Layout::OnHover { hover: Some(()), .. })
        ));
        assert!(matches!(
            decoded(&hover_block(node(vocabulary::SLOT, Value::record([])))),
            Some(Layout::OnHover { hover: None, .. })
        ));
        let handler = Value::Cell(vocabulary::HANDLER);
        assert!(matches!(
            decoded(&click(text_leaf("go", vocabulary::NAME_FACE), handler.clone())),
            Some(Layout::OnEvent {
                kind: EventKind::PointerDown,
                handler: decoded,
                ..
            }) if decoded == handler
        ));
    }

    #[test]
    fn vector_commands_are_ordinary_display_data() {
        let display = vector(
            20.0,
            8.0,
            2.0,
            [
                fill_rounded_rect(0.0, 0.0, 20.0, 10.0, 2.0, vocabulary::DIM_FACE),
                stroke_rounded_rect(
                    0.5,
                    0.5,
                    19.0,
                    9.0,
                    2.0,
                    1.0,
                    vocabulary::NAME_FACE,
                ),
            ],
        );
        let Some(Layout::Leaf(Display::Vector(vector))) = decoded(&display) else {
            panic!("vector leaf");
        };
        assert_eq!((vector.width, vector.ascent, vector.descent), (20.0, 8.0, 2.0));
        assert_eq!(vector.commands.len(), 2);
        assert!(matches!(
            vector.commands[1],
            VectorCommand::StrokeRoundedRect { line_width: 1.0, .. }
        ));
    }

    #[test]
    fn junk_falls_through_whole() {
        assert!(decoded(&Value::record([])).is_none());
        assert!(decoded(&text::value("plain text is not a layout")).is_none());
        let broken = row(1.0, [text_leaf("ok", vocabulary::NAME_FACE), Value::record([])]);
        assert!(decoded(&broken).is_none());
        let bad_face = node(
            vocabulary::TEXT,
            Value::record([
                (vocabulary::CONTENT, text::value("x")),
                (vocabulary::FACE, Value::Cell(vocabulary::GAP)),
            ]),
        );
        assert!(decoded(&bad_face).is_none());
    }
}
