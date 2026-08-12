//! A document: its root value plus the cell table. Paths name
//! locations in that structure. The sample fixture lives here because
//! it is document data, not a projection.

use progred_graph::{CellId, Cells, Step, Value, new_cell_id};

/// A document: its `root` value plus the cell table holding every
/// identity's current value. Every projection path starts at `root` —
/// typically a link, or an inline record keying the document's parts
/// by role. The root is a location like any other — the empty path —
/// so edits there commit to this field, and deleting it empties the
/// document. Clones are O(1): the table and its values share
/// structure, which is what makes snapshot undo free.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Value>,
    pub cells: Cells,
}

/// A location in the projected spanning tree: Key steps into record
/// fields, Element steps into list values, Follow steps through a
/// link to its cell's current value. The same value can be projected
/// at several paths, so the path — not the value — is the identity a
/// selection names; every reference site unfolds through its own
/// Follow, and no site is the value's home. List elements sit at
/// positions sibling edits never move; wraps and unwraps will adjust
/// path-keyed state through one general rewrite — see
/// `docs/model.md`.
pub type Path = Vec<Step>;

#[cfg_attr(not(test), allow(dead_code))]
pub mod sample_vocabulary {
    use progred_graph::CellId;

    pub const AT: CellId = CellId::from_u128(0x4c2cb3268f1911bd26a0eb74622ba097);
    pub const ROW: CellId = CellId::from_u128(0xa791e4873aa95e21bc925dacbbbf6ea5);
    pub const COL: CellId = CellId::from_u128(0x64bad273f94f32f9957b99a6e4d14d39);
    pub const OF: CellId = CellId::from_u128(0x4544b0db160b6330f20a69dd3ce34e2d);
    pub const COLOR: CellId = CellId::from_u128(0x897fc1c794c08a0506590276aa72a3d7);
    pub const SWATCH: CellId = CellId::from_u128(0xf2aadbb9e548ea30aceb7fed5773ea8a);
    pub const POINTS: CellId = CellId::from_u128(0xe92356b75104edae387062fcf8a859e0);
    pub const TAGS: CellId = CellId::from_u128(0x41f5587d6560bfbebc9fb72fb0728e27);
    pub const MATERIAL: CellId = CellId::from_u128(0xc7c1197574183c44d7e038cb52d78760);
    pub const STYLE: CellId = CellId::from_u128(0x2b9652d2cb8cb5c9d633b34d827048b1);
    pub const PITCH: CellId = CellId::from_u128(0x563079b77defe2a26abcdccfa47655ed);
    pub const DOUBLE_PITCH: CellId = CellId::from_u128(0xe69c085ed00f5270f24a895cca9cd7d6);
    pub const PROFILE: CellId = CellId::from_u128(0x3624cc3724556440847e7da953d398fc);
    pub const SHAPE: CellId = CellId::from_u128(0xb5db29c46198e28df0c26ac4aa5a411a);
    pub const FAVORITE: CellId = CellId::from_u128(0xa83b16a0d85afeb98d46c3459f2e7e16);
}

/// A small document shaped like a real one. The root is an inline
/// RECORD of roles — a document keys its parts by what they are to
/// it, and needs no identity of its own to do so. Simple names are
/// ordinary record fields ("roof", not its kind). The corner knows its roof (cycle
/// collapse on a real pattern); the style cell is unnamed and
/// referenced twice (short-id heads, secondary marks); the stroke
/// cell holds only an ordinary name record and is referenced as a
/// label; the
/// material cell is fully bare — referenced before anything at all
/// is said about it; the swatch is a blob; each point's position is
/// an inline record, point-shaped data that wants to be a value; the
/// favorite cell holds a bare LINK to the corner — the alias pattern;
/// and pitch flows through a small Grap function to a projected
/// computed result.
/// The app starts EMPTY now; this is the test fixture, and its
/// printed form is checked in as sample.gid.
#[cfg_attr(not(test), allow(dead_code))]
pub fn sample_document() -> Document {
    let mut cells = Cells::new();
    for (cell, name) in [
        (sample_vocabulary::AT, "at"),
        (sample_vocabulary::ROW, "row"),
        (sample_vocabulary::COL, "col"),
        (sample_vocabulary::OF, "of"),
        (sample_vocabulary::COLOR, "color"),
        (sample_vocabulary::SWATCH, "swatch"),
        (sample_vocabulary::POINTS, "points"),
        (sample_vocabulary::TAGS, "tags"),
        (sample_vocabulary::MATERIAL, "material"),
        (sample_vocabulary::STYLE, "style"),
        (sample_vocabulary::PITCH, "pitch"),
        (sample_vocabulary::DOUBLE_PITCH, "double pitch"),
        (sample_vocabulary::PROFILE, "profile"),
        (sample_vocabulary::SHAPE, "shape"),
        (sample_vocabulary::FAVORITE, "favorite"),
    ] {
        cells.set_value(cell, progred_name::record(name, []));
    }
    let roof = new_cell_id();

    let origin = new_cell_id();
    cells.set_value(
        origin,
        progred_name::record(
            "origin",
            [(
                sample_vocabulary::AT,
                Value::record([
                    (
                        sample_vocabulary::ROW,
                        progred_text::value("top"),
                    ),
                    (
                        sample_vocabulary::COL,
                        progred_text::value("left"),
                    ),
                ]),
            )],
        ),
    );

    let corner = new_cell_id();
    cells.set_value(
        corner,
        progred_name::record(
            "corner",
            [
                (
                    sample_vocabulary::AT,
                    Value::record([
                        (
                            sample_vocabulary::ROW,
                            progred_text::value("bottom"),
                        ),
                        (
                            sample_vocabulary::COL,
                            progred_text::value("right"),
                        ),
                    ]),
                ),
                // A part that knows its whole: the cycle a real document
                // has, rendered as a collapsed head rather than recursing
                // forever.
                (sample_vocabulary::OF, Value::from(roof)),
            ],
        ),
    );

    let stroke = new_cell_id();
    cells.set_value(stroke, progred_name::record("stroke", []));

    let style = new_cell_id();
    cells.set_value(
        style,
        Value::record([
            (
                sample_vocabulary::COLOR,
                progred_text::value("rebeccapurple"),
            ),
            // #663399, as bytes.
            (
                sample_vocabulary::SWATCH,
                Value::from(vec![0x66, 0x33, 0x99]),
            ),
        ]),
    );

    let material = new_cell_id();

    let favorite = new_cell_id();
    cells.set_value(favorite, Value::from(corner));

    let amount = new_cell_id();
    cells.set_value(amount, progred_name::record("amount", []));

    let double = new_cell_id();
    cells.set_value(
        double,
        progred_name::record(
            "double",
            [
                (
                    grap::vocabulary::PARAMS,
                    Value::list([Value::from(amount)]),
                ),
                (
                    grap::vocabulary::BODY,
                    grap::call(
                        Value::from(grap_f64::vocabulary::MULTIPLY),
                        [
                            (grap_f64::vocabulary::LEFT, Value::from(amount)),
                            (
                                grap_f64::vocabulary::RIGHT,
                                grap_f64::value(2.0),
                            ),
                        ],
                    ),
                ),
            ],
        ),
    );

    let pitch = new_cell_id();
    cells.set_value(pitch, grap_f64::value(2.5));

    let double_pitch = || grap::call(Value::from(double), [(amount, Value::from(pitch))]);
    let grap_projection = |expression| {
        Value::record([(crate::conventions::vocabulary::GRAP, expression)])
    };

    cells.set_value(
        roof,
        progred_name::record(
            "roof",
            [
                (
                    sample_vocabulary::POINTS,
                    Value::list([Value::from(origin), Value::from(corner)]),
                ),
                (stroke, progred_text::value("hairline")),
                (
                    sample_vocabulary::TAGS,
                    Value::list([progred_text::value("draft"), progred_text::value("gabled")]),
                ),
                (
                    sample_vocabulary::MATERIAL,
                    Value::from(material),
                ),
                (sample_vocabulary::STYLE, Value::from(style)),
                (sample_vocabulary::PITCH, Value::from(pitch)),
                (
                    sample_vocabulary::DOUBLE_PITCH,
                    grap_projection(double_pitch()),
                ),
                (
                    sample_vocabulary::PROFILE,
                    grap_projection(grap::call(
                        Value::from(grap_geometry::vocabulary::CIRCLE),
                        [(
                            grap_geometry::vocabulary::RADIUS,
                            grap::call(
                                Value::from(grap_f64::vocabulary::MULTIPLY),
                                [
                                    (grap_f64::vocabulary::LEFT, double_pitch()),
                                    (
                                        grap_f64::vocabulary::RIGHT,
                                        grap_f64::value(8.0),
                                    ),
                                ],
                            ),
                        )],
                    )),
                ),
            ],
        ),
    );

    Document {
        root: Some(Value::record([
            (sample_vocabulary::SHAPE, Value::from(roof)),
            (sample_vocabulary::STYLE, Value::from(style)),
            (
                sample_vocabulary::FAVORITE,
                Value::from(favorite),
            ),
        ])),
        cells,
    }
}
