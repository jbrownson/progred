//! Grap-facing layout: scoped emitting programs, and stored descriptions.
//! A `layout program` returns a layout program, its ordinary closure run by
//! [`scope`] with Rust-owned output. In the stored description form, each
//! node is a record of its kind, each drawing command and shape a record
//! under its tag, strings ride the text convention and numbers the f64
//! convention. Interaction either attaches
//! host-provided editor intents or a Grap handler to generic event
//! data. Event dispatch supplies a capability overlay closed over the
//! projection site. Traversal uses the path step convention.
//! Decoding is resilient the projection way: any junk node decodes to
//! `None`, and the whole layout falls through to the next partial.
//!
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, color, f64 as f64_convention, name};
use gid::{CellId, Step, Value};

pub const ID: CellId = CellId::from_u128(0xfb2a4dac87512d69448650bc0e29dc80);
use crate::display::{
    Delim, Face, Paint, RowAlignment, alternatives, block_hover, border, bracket, descend, leaf,
    overlay as layout_overlay, pickable, selectable, slot,
};
use ::grap::{Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use kurbo::{Affine, BezPath, Circle, Line, PathEl, Point, Rect, RoundedRect, Stroke};
use peniko::{Brush, ColorStop, Gradient};
use puri::draw::{Command, Drawing, Leaf, Shape};

mod drawing;
mod events;
pub use events::on_event;
pub mod scope;
#[cfg(test)]
#[path = "tests/layout/test_support.rs"]
pub(crate) mod test_support;

pub mod vocabulary {
    use gid::CellId;

    /// The editor's event records, which `on event` hands a drawing's
    /// handler. Drawings share their point and text fields.
    pub use crate::site::event::vocabulary::*;

    // Boxes and walk.
    pub const ROW: CellId = CellId::from_u128(0x1af52c96e380b7d40c9e1f6a2d5b83e7);
    pub const COL: CellId = CellId::from_u128(0x9d04b6e1783f2ca5f17d09c4e6a2358b);
    pub const PAD: CellId = CellId::from_u128(0x4e8a17d0952cb6f3a30c5e92b7d1f648);
    /// A child in the middle of the width its container offers.
    pub const CENTER: CellId = CellId::from_u128(0x6837fa29d597fede353287ffd98394f5);
    pub const BORDER: CellId = CellId::from_u128(0xa4edf70410d16734683d9f2de0bb0080);
    pub const OVERLAY: CellId = CellId::from_u128(0x95de46726d377f70f4f8b6f88893aa52);
    pub const BRACKET: CellId = CellId::from_u128(0xc71e0f4b2d8a6395e6b34a08d15c97f2);
    pub const ALTERNATIVES: CellId = CellId::from_u128(0x62d9b3f0a47e158c37b60d2c81f5e94a);
    pub const DESCEND: CellId = CellId::from_u128(0x35c7a8e2f10d49b6d2f8016c4b9ea375);
    pub const DESCEND_PATH: CellId = CellId::from_u128(0xfa9520a04e0c79e7ff78adc6e3f6e754);
    pub const JUMP: CellId = CellId::from_u128(0x3cce33504c57677528fe92071248b6ff);
    pub const DOCUMENT_PATH: CellId = CellId::from_u128(0xad54f4f4c0e5c4a41878247bc1363489);
    pub const AT: CellId = CellId::from_u128(0xe90d25c8b64a37f1084b92d7f3a65c1e);

    // Leaves.
    pub const TEXT: CellId = CellId::from_u128(0x08e64d1f3a92c5b7b7f0d38a165e29c4);
    pub const SLOT: CellId = CellId::from_u128(0x96e07d2a58c4b1f3f3b18e57d0c2946a);
    /// A line across the width its container offers.
    pub const RULE: CellId = CellId::from_u128(0xf8a0697c10a7a18e750e3fb32a57bcf2);
    pub const LAYOUT_PROGRAM: CellId = CellId::from_u128(0x120660d976130c8ba722d26b1a28c975);
    pub const INVALID_PROGRAM: CellId = CellId::from_u128(0xd0f97606bf2fe368cb9645e4bad8b977);
    pub const INVALID_DRAWING: CellId = CellId::from_u128(0x3ff5ffa7414309b0ed4ed3f17ba5d5cd);

    // Generic display and event nodes.
    pub const DRAWING: CellId = CellId::from_u128(0x6889fa235b002be4c8b106d5f31dafbf);
    pub const PROGRAM: CellId = CellId::from_u128(0xbdf607810b274ad5b02bd409aef34b6a);
    pub const ON_EVENT: CellId = CellId::from_u128(0x1b87f7de18e7c46c5fbfadea1f18aea4);

    // Interaction attach-points.
    pub const SELECTABLE: CellId = CellId::from_u128(0x40b93f6e17d5a28c6a2df1905e83b7c4);
    pub const PICKABLE: CellId = CellId::from_u128(0xf8261c05d94eb7a3072c48e6b3f19d58);
    pub const HOVER_BLOCK: CellId = CellId::from_u128(0x83f0d5b7264a19ce4c07f3921ea6b85d);
    pub const HANDLER: CellId = CellId::from_u128(0x3e5d38e4658b1895e38307ad12862061);

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

    /// The kinds of stored layout nodes and of a layout program, each a cell
    /// of its own, apart from the scoped function of the same name.
    pub mod kind {
        use gid::CellId;

        /// Children side by side: `{isa: row, gap, children, baseline?}`.
        pub const ROW: CellId = CellId::from_u128(0x4843d61b088a4e569fafe00f7a617c6a);
        /// Children one under another: `{isa: col, baseline, gap, children}`.
        pub const COL: CellId = CellId::from_u128(0x167741552b1a4d7cbffecb45d1326970);
        /// A child with room around it: `{isa: pad, left, top, right, bottom, child}`.
        pub const PAD: CellId = CellId::from_u128(0x426095b35fa14ed0a452d15a18d40f53);
        /// A child in the middle of the width its container offers: `{isa: center, child}`.
        pub const CENTER: CellId = CellId::from_u128(0xeae2886e51e142708d37a537a61ad05b);
        /// A child framed: `{isa: border, child}`.
        pub const BORDER: CellId = CellId::from_u128(0x876d14ee7d5a4b70aa9709cd55135649);
        /// Children drawn over one another: `{isa: overlay, children}`.
        pub const OVERLAY: CellId = CellId::from_u128(0xad61b87d4b3e4a3a800c5cd805b58c28);
        /// A child between delimiters: `{isa: bracket, delim, child}`.
        pub const BRACKET: CellId = CellId::from_u128(0x1a35405498e843ca9bd091aa1e0983eb);
        /// The first of its children that fits: `{isa: alternatives, children}`.
        pub const ALTERNATIVES: CellId = CellId::from_u128(0x117dc122744f4b0aa69014e4cbe7b76a);
        /// The value one step down, as the projection draws it: `{isa: descend, step}`.
        pub const DESCEND: CellId = CellId::from_u128(0x5d75597b17544d90b66f74bac4201423);
        /// The value some steps down: `{isa: descend path, steps}`.
        pub const DESCEND_PATH: CellId = CellId::from_u128(0x9a2529c646d648b3a1e4e50a9515d660);
        /// A stored value elsewhere, drawn here: `{isa: jump, steps, document path}`.
        pub const JUMP: CellId = CellId::from_u128(0xa0ddfad49b4c4e1e9a9923dcaaae7188);
        /// A computed value, drawn here: `{isa: at, steps, value}`.
        pub const AT: CellId = CellId::from_u128(0xa67e18ce34834e5aa470045a50640b4f);
        /// Text: `{isa: text, content, paint}`.
        pub const TEXT: CellId = CellId::from_u128(0xfd00dc28f7f24065bc76a528fc941c66);
        /// Drawn commands or a drawing program: `{isa: drawing, width, ascent, descent, commands | program, fuel}`.
        pub const DRAWING: CellId = CellId::from_u128(0x0f42d484547d4e61b0f5dae119911117);
        /// An empty slot: `{isa: slot}`.
        pub const SLOT: CellId = CellId::from_u128(0xd03a133d0b6e4fbc9f2d108701348a16);
        /// A horizontal rule: `{isa: rule}`.
        pub const RULE: CellId = CellId::from_u128(0x25647a81e7104009b228bf59411722c2);
        /// A child that selects the value drawn: `{isa: selectable, child}`.
        pub const SELECTABLE: CellId = CellId::from_u128(0x5ef1270bffc74529bf66fc7dbe667bbd);
        /// A child that picks a value: `{isa: pickable, child, value}`.
        pub const PICKABLE: CellId = CellId::from_u128(0xd57345940fae43a0b1fec1f4302b66fc);
        /// A child whose hover stops at it: `{isa: hover block, child}`.
        pub const HOVER_BLOCK: CellId = CellId::from_u128(0xa7dd5e84882c47f5ba86c3b27444727b);
        /// A child whose events a Grap handler gets: `{isa: on event, child, handler}`.
        pub const ON_EVENT: CellId = CellId::from_u128(0xb8eafe383f924449a980d74faea1996b);
        /// A layout program, run to emit its layout: `{isa: layout program, program}`.
        pub const LAYOUT_PROGRAM: CellId = CellId::from_u128(0xe8d550f504354ddba74b2960bcda2d84);
    }
}

/// Which child's baseline a column, or a row aligning tops, keeps. A
/// container with no children has nothing to choose, so any index passes.
fn baseline_index(baseline: f64, children: usize) -> Option<usize> {
    ::grap::f64::to_usize(baseline).filter(|index| children == 0 || *index < children)
}

/// The kinds of the stored form's nodes, each drawn as [`decode_with`]
/// reads it.
const NODES: [CellId; 20] = [
    vocabulary::kind::ROW,
    vocabulary::kind::COL,
    vocabulary::kind::OVERLAY,
    vocabulary::kind::PAD,
    vocabulary::kind::CENTER,
    vocabulary::kind::BORDER,
    vocabulary::kind::BRACKET,
    vocabulary::kind::ALTERNATIVES,
    vocabulary::kind::DESCEND,
    vocabulary::kind::DESCEND_PATH,
    vocabulary::kind::JUMP,
    vocabulary::kind::AT,
    vocabulary::kind::TEXT,
    vocabulary::kind::DRAWING,
    vocabulary::kind::SLOT,
    vocabulary::kind::RULE,
    vocabulary::kind::SELECTABLE,
    vocabulary::kind::PICKABLE,
    vocabulary::kind::HOVER_BLOCK,
    vocabulary::kind::ON_EVENT,
];

/// Where a stored node sits in the value being decoded, kept as decoding
/// descends and spelled out only for a drawing's program.
enum Place<'a> {
    Root,
    Field(&'a Place<'a>, CellId),
    /// Item `index` of `list`.
    Item(&'a Place<'a>, &'a ::grap::RuntimeValue, usize),
}

impl Place<'_> {
    /// The steps from the decoded value to here, each pushed onto its
    /// parent's.
    fn steps(&self) -> Option<Vec<Step>> {
        let below = |parent: &Place<'_>, step| {
            let mut steps = parent.steps()?;
            steps.push(step);
            Some(steps)
        };
        match *self {
            Place::Root => Some(Vec::new()),
            Place::Field(parent, key) => below(parent, Step::Key(key)),
            Place::Item(parent, list, index) => below(
                parent,
                Step::Element(list.list_positions()?.get(index)?.clone()),
            ),
        }
    }
}

fn decode_with<W: Host>(
    value: &::grap::RuntimeValue,
    place: &Place<'_>,
    site: &impl Fn() -> crate::editing::Site,
) -> Option<Layout<W>> {
    use crate::libraries::editor::path;
    use vocabulary::*;
    let number = |field| read_number(value.field(field)?.as_value());
    let nonnegative = |field| read_nonnegative(value.field(field)?.as_value());
    let child = || decode_with(&value.field(CHILD)?, &Place::Field(place, CHILD), site);
    let children_field = || {
        children(
            &value.field(CHILDREN)?,
            &Place::Field(place, CHILDREN),
            site,
        )
    };
    match value.kind()? {
        kind::ROW => {
            let gap = number(GAP)?;
            let children = children_field()?;
            Some(match value.field(BASELINE) {
                None => crate::display::row(gap, children),
                Some(baseline) => crate::display::aligned_row(
                    RowAlignment::Top {
                        baseline: baseline_index(
                            read_number(baseline.as_value())?,
                            children.len(),
                        )?,
                    },
                    gap,
                    children,
                ),
            })
        }
        kind::COL => {
            let baseline = number(BASELINE)?;
            let children = children_field()?;
            Some(crate::display::col(
                baseline_index(baseline, children.len())?,
                number(GAP)?,
                children,
            ))
        }
        kind::OVERLAY => Some(layout_overlay(children_field()?)),
        kind::PAD => Some(crate::display::padding(
            (number(LEFT)?, number(TOP)?, number(RIGHT)?, number(BOTTOM)?).into(),
            child()?,
        )),
        kind::CENTER => Some(crate::display::center(child()?)),
        kind::BORDER => Some(border(child()?)),
        kind::BRACKET => Some(bracket(read_delim(&value.field(DELIM)?)?, child()?)),
        kind::ALTERNATIVES => Some(alternatives(children_field()?)),
        kind::DESCEND => Some(descend(
            [path::read_step(value.field(STEP)?.as_value())?],
            None,
            None,
        )),
        kind::DESCEND_PATH => Some(descend(
            path::read(value.field(STEPS)?.as_value())?,
            None,
            None,
        )),
        kind::JUMP => Some(crate::display::jump(
            path::read(value.field(STEPS)?.as_value())?,
            crate::location::Location::root(path::read(value.field(DOCUMENT_PATH)?.as_value())?),
            None,
            None,
        )),
        kind::AT => Some(crate::display::at(
            path::read(value.field(STEPS)?.as_value())?,
            value.field(VALUE)?,
            None,
            None,
        )),
        kind::TEXT => Some(leaf(Leaf::Text {
            text: value.field(CONTENT)?.as_text()?.to_string(),
            paint: Paint::Face(read_face(value.field(PAINT)?.as_value())?),
            script: puri::text::Script::Normal,
        })),
        kind::DRAWING => {
            let extent = crate::display::widget::Extent {
                width: nonnegative(WIDTH)?,
                ascent: nonnegative(ASCENT)?,
                descent: nonnegative(DESCENT)?,
            };
            Some(match value.field(PROGRAM) {
                Some(program) => drawing::program(
                    extent,
                    ::grap::f64::to_usize(number(FUEL)?)?,
                    Place::Field(place, PROGRAM).steps(),
                    program,
                ),
                None => leaf(Leaf::Drawing(Drawing {
                    width: extent.width,
                    ascent: extent.ascent,
                    descent: extent.descent,
                    commands: value
                        .field(COMMANDS)?
                        .list_values()?
                        .map(|command| read_command(command.as_value()))
                        .collect::<Option<Vec<_>>>()?,
                })),
            })
        }
        kind::SLOT => Some(slot()),
        kind::RULE => Some(crate::display::rule()),
        kind::SELECTABLE => Some(selectable(child()?, &site())),
        kind::PICKABLE => Some(pickable(
            child()?,
            W::Hover::from(site().hover()),
            value.field(VALUE)?,
        )),
        kind::HOVER_BLOCK => Some(block_hover(child()?)),
        kind::ON_EVENT => Some(on_event(child()?, value.field(HANDLER)?)),
        // Any other kind isn't a layout node, so it declines, as a
        // malformed node does.
        _ => None,
    }
}

fn children<W: Host>(
    list: &::grap::RuntimeValue,
    place: &Place<'_>,
    site: &impl Fn() -> crate::editing::Site,
) -> Option<Vec<Layout<W>>> {
    list.list_values()?
        .enumerate()
        .map(|(index, child)| decode_with(&child, &Place::Item(place, list, index), site))
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
        vocabulary::NAME_FACE => Some(Face::Name),
        vocabulary::STRING_FACE => Some(Face::String),
        vocabulary::DIM_FACE => Some(Face::Dim),
        vocabulary::LABEL_FACE => Some(Face::Label),
        vocabulary::ID_FACE => Some(Face::Id),
        vocabulary::ACCENT_WASH_FACE => Some(Face::AccentWash),
        vocabulary::INK_FACE => Some(Face::Ink),
        _ => None,
    }
}

fn read_delim(value: &::grap::RuntimeValue) -> Option<Delim> {
    match value.as_cell()? {
        vocabulary::PAREN => Some(Delim::Paren),
        vocabulary::SQUARE => Some(Delim::Bracket),
        vocabulary::CURLY => Some(Delim::Brace),
        _ => None,
    }
}

fn read_paint(value: &Value) -> Option<Paint> {
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

fn read_transform(value: &Value) -> Option<Affine> {
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

fn read_shape(value: &Value) -> Option<Shape> {
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
        content
            .as_list()?
            .values()
            .map(read_path_element)
            .collect::<Option<Vec<_>>>()
            // A path starts by moving to a point.
            .filter(|elements| matches!(elements.first(), None | Some(PathEl::MoveTo(_))))
            .map(|elements| Shape::Path(BezPath::from_vec(elements)))
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

fn read_path_element(value: &Value) -> Option<PathEl> {
    let fields = value.as_record()?;
    if let Some(content) = fields.get(&vocabulary::MOVE_TO) {
        Some(PathEl::MoveTo(read_point(content)?))
    } else if let Some(content) = fields.get(&vocabulary::LINE_TO) {
        Some(PathEl::LineTo(read_point(content)?))
    } else if let Some(content) = fields.get(&vocabulary::QUAD_TO) {
        let content = content.as_record()?;
        Some(PathEl::QuadTo(
            Point::new(
                read_number(content.get(&vocabulary::X1)?)?,
                read_number(content.get(&vocabulary::Y1)?)?,
            ),
            Point::new(
                read_number(content.get(&vocabulary::X)?)?,
                read_number(content.get(&vocabulary::Y)?)?,
            ),
        ))
    } else if let Some(content) = fields.get(&vocabulary::CURVE_TO) {
        let content = content.as_record()?;
        Some(PathEl::CurveTo(
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
        ))
    } else {
        fields.get(&vocabulary::CLOSE).map(|_| PathEl::ClosePath)
    }
}

fn read_point(value: &Value) -> Option<Point> {
    let fields = value.as_record()?;
    Some(Point::new(
        read_number(fields.get(&vocabulary::X)?)?,
        read_number(fields.get(&vocabulary::Y)?)?,
    ))
}

/// A stored node, drawn as [`decode_with`] reads it.
fn node<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    decode_with(input.value?, &Place::Root, &|| input.site())
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "layout",
        [
            (vocabulary::ROW, "row"),
            (vocabulary::COL, "col"),
            (vocabulary::PAD, "pad"),
            (vocabulary::CENTER, "center"),
            (vocabulary::RULE, "rule"),
            (vocabulary::BORDER, "border"),
            (vocabulary::OVERLAY, "overlay"),
            (vocabulary::BRACKET, "bracket"),
            (vocabulary::ALTERNATIVES, "alternatives"),
            (vocabulary::DESCEND, "descend"),
            (vocabulary::DESCEND_PATH, "descend path"),
            (vocabulary::JUMP, "jump"),
            (vocabulary::DOCUMENT_PATH, "document path"),
            (vocabulary::AT, "at"),
            (vocabulary::TEXT, "text"),
            (vocabulary::DRAWING, "drawing"),
            (vocabulary::PROGRAM, "program"),
            (vocabulary::SLOT, "slot"),
            (vocabulary::LAYOUT_PROGRAM, "layout program"),
            (vocabulary::kind::LAYOUT_PROGRAM, "layout program"),
            (vocabulary::kind::ROW, "row"),
            (vocabulary::kind::COL, "col"),
            (vocabulary::kind::PAD, "pad"),
            (vocabulary::kind::CENTER, "center"),
            (vocabulary::kind::BORDER, "border"),
            (vocabulary::kind::OVERLAY, "overlay"),
            (vocabulary::kind::BRACKET, "bracket"),
            (vocabulary::kind::ALTERNATIVES, "alternatives"),
            (vocabulary::kind::DESCEND, "descend"),
            (vocabulary::kind::DESCEND_PATH, "descend path"),
            (vocabulary::kind::JUMP, "jump"),
            (vocabulary::kind::AT, "at"),
            (vocabulary::kind::TEXT, "text"),
            (vocabulary::kind::DRAWING, "drawing"),
            (vocabulary::kind::SLOT, "slot"),
            (vocabulary::kind::RULE, "rule"),
            (vocabulary::kind::SELECTABLE, "selectable"),
            (vocabulary::kind::PICKABLE, "pickable"),
            (vocabulary::kind::HOVER_BLOCK, "hover block"),
            (vocabulary::kind::ON_EVENT, "on event"),
            (vocabulary::INVALID_PROGRAM, "invalid layout program"),
            (vocabulary::INVALID_DRAWING, "invalid drawing arguments"),
            (vocabulary::SELECTABLE, "selectable"),
            (vocabulary::PICKABLE, "pickable"),
            (vocabulary::HOVER_BLOCK, "hover block"),
            (vocabulary::ON_EVENT, "on event"),
            (vocabulary::HANDLER, "handler"),
            (vocabulary::EVENT, "event"),
            (vocabulary::EVENT_KIND, "event kind"),
            (vocabulary::POINTER_DOWN, "pointer down"),
            (vocabulary::HOVER_CHANGED, "hover changed"),
            (vocabulary::NAVIGATE, "navigate"),
            (vocabulary::MODIFIERS_CHANGED, "modifiers changed"),
            (vocabulary::POINTER_MOVE, "pointer move"),
            (vocabulary::POINTER_UP, "pointer up"),
            (vocabulary::POINTER_CANCEL, "pointer cancel"),
            (vocabulary::TOUCH_START, "touch start"),
            (vocabulary::TOUCH_MOVE, "touch move"),
            (vocabulary::TOUCH_END, "touch end"),
            (vocabulary::TOUCH_CANCEL, "touch cancel"),
            (vocabulary::SCROLL, "scroll"),
            (vocabulary::GESTURE, "gesture"),
            (vocabulary::PINCH, "pinch"),
            (vocabulary::ROTATION, "rotation"),
            (vocabulary::DELTA, "delta"),
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
        ],
        ForeignFunctions::default().register(
            vocabulary::LAYOUT_PROGRAM,
            ForeignFunction::new(scope::program)
                .parameters([::grap::vocabulary::EXPRESSION])
                .tracked(),
        ),
    )
    .with_projections_for(
        NODES
            .into_iter()
            .map(|kind| (kind, crate::display::runtime_partial(node)))
            .chain([(
                vocabulary::kind::LAYOUT_PROGRAM,
                crate::display::runtime_partial(scope::display),
            )]),
    )
}

#[cfg(test)]
#[path = "tests/layout/mod.rs"]
mod tests;
