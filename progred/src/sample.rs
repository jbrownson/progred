//! Progred's sample GID document used by tests and render fixtures.

use gid::{Cells, Document, Value, new_cell_id};
use progred_libraries::{
    control, f64, geometry, layout, name, selection as selection_capability, site, text,
};

#[cfg_attr(not(test), allow(dead_code))]
pub mod sample_vocabulary {
    use gid::CellId;

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

/// A Grap projection kept as ordinary, root-reachable sample data.
/// It is not installed into the editor's projection chain.
#[cfg_attr(not(test), allow(dead_code))]
pub fn at_display_partial() -> Value {
    let bind = |cell| Value::record([(control::vocabulary::BIND, Value::from(cell))]);
    let spliced_text = |binder| {
        Value::record([(
            layout::vocabulary::TEXT,
            Value::record([
                (
                    layout::vocabulary::CONTENT,
                    Value::record([(control::vocabulary::UNQUOTE, Value::from(binder))]),
                ),
                (
                    layout::vocabulary::PAINT,
                    Value::from(layout::vocabulary::NAME_FACE),
                ),
            ]),
        )])
    };
    let select_here = grap::lambda(
        [layout::vocabulary::EVENT],
        grap::call(
            Value::from(control::vocabulary::MATCH),
            [
                (
                    control::vocabulary::VALUE,
                    Value::from(layout::vocabulary::EVENT),
                ),
                (
                    control::vocabulary::CASES,
                    Value::list([Value::record([
                        (
                            control::vocabulary::PATTERN,
                            Value::record([
                                (
                                    layout::vocabulary::EVENT_KIND,
                                    Value::from(layout::vocabulary::POINTER_DOWN),
                                ),
                                (
                                    layout::vocabulary::BUTTON,
                                    Value::from(layout::vocabulary::PRIMARY),
                                ),
                                (layout::vocabulary::MODIFIERS, Value::list([])),
                            ]),
                        ),
                        (
                            grap::vocabulary::EXPRESSION,
                            grap::call(
                                Value::from(selection_capability::vocabulary::SET),
                                [(
                                    site::vocabulary::VALUE,
                                    crate::selection::payload::edge(),
                                )],
                            ),
                        ),
                    ])]),
                ),
            ],
        ),
    );
    name::record(
        "at display",
        [
            (
                grap::vocabulary::PARAMS,
                Value::list([Value::from(layout::vocabulary::VALUE)]),
            ),
            (
                grap::vocabulary::BODY,
                grap::call(
                    Value::from(control::vocabulary::MATCH),
                    [
                        (
                            control::vocabulary::VALUE,
                            Value::from(layout::vocabulary::VALUE),
                        ),
                        (
                            control::vocabulary::CASES,
                            Value::list([Value::record([
                                (
                                    control::vocabulary::PATTERN,
                                    Value::record([
                                        (sample_vocabulary::ROW, bind(sample_vocabulary::ROW)),
                                        (sample_vocabulary::COL, bind(sample_vocabulary::COL)),
                                    ]),
                                ),
                                (
                                    grap::vocabulary::EXPRESSION,
                                    grap::call(
                                        Value::from(control::vocabulary::QUOTE),
                                        [(
                                            grap::vocabulary::EXPRESSION,
                                            layout::on(
                                                layout::row(
                                                    4.0,
                                                    [
                                                        layout::drawing(
                                                            10.0,
                                                            8.0,
                                                            2.0,
                                                            [layout::stroke(
                                                                layout::rounded_rect(
                                                                    0.5, 0.5, 9.0, 9.0, 2.0,
                                                                ),
                                                                1.0,
                                                                Value::from(layout::vocabulary::DIM_FACE),
                                                            )],
                                                        ),
                                                        spliced_text(sample_vocabulary::ROW),
                                                        layout::text_leaf(
                                                            "×",
                                                            layout::vocabulary::DIM_FACE,
                                                        ),
                                                        spliced_text(sample_vocabulary::COL),
                                                    ],
                                                ),
                                                select_here.clone(),
                                            ),
                                        )],
                                    ),
                                ),
                            ])]),
                        ),
                    ],
                ),
            ),
        ],
    )
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
/// text-bridge form is checked in as examples/sample.gid.
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
        cells.set_value(cell, name::record(name, []));
    }
    let roof = new_cell_id();

    let origin = new_cell_id();
    cells.set_value(
        origin,
        name::record(
            "origin",
            [(
                sample_vocabulary::AT,
                Value::record([
                    (sample_vocabulary::ROW, text::value("top")),
                    (sample_vocabulary::COL, text::value("left")),
                ]),
            )],
        ),
    );

    let corner = new_cell_id();
    cells.set_value(
        corner,
        name::record(
            "corner",
            [
                (
                    sample_vocabulary::AT,
                    Value::record([
                        (sample_vocabulary::ROW, text::value("bottom")),
                        (sample_vocabulary::COL, text::value("right")),
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
    cells.set_value(stroke, name::record("stroke", []));

    let style = new_cell_id();
    cells.set_value(
        style,
        Value::record([
            (sample_vocabulary::COLOR, text::value("rebeccapurple")),
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
    cells.set_value(amount, name::record("amount", []));

    let double = new_cell_id();
    cells.set_value(
        double,
        name::record(
            "double",
            [
                (grap::vocabulary::PARAMS, Value::list([Value::from(amount)])),
                (
                    grap::vocabulary::BODY,
                    grap::call(
                        Value::from(f64::vocabulary::MULTIPLY),
                        [
                            (f64::vocabulary::LEFT, Value::from(amount)),
                            (f64::vocabulary::RIGHT, f64::value(2.0)),
                        ],
                    ),
                ),
            ],
        ),
    );

    let pitch = new_cell_id();
    cells.set_value(pitch, f64::value(2.5));

    let double_pitch = || grap::call(Value::from(double), [(amount, Value::from(pitch))]);
    let evaluation = |expression| Value::record([(grap::vocabulary::EVALUATE, expression)]);

    cells.set_value(
        roof,
        name::record(
            "roof",
            [
                (
                    sample_vocabulary::POINTS,
                    Value::list([Value::from(origin), Value::from(corner)]),
                ),
                (stroke, text::value("hairline")),
                (
                    sample_vocabulary::TAGS,
                    Value::list([text::value("draft"), text::value("gabled")]),
                ),
                (sample_vocabulary::MATERIAL, Value::from(material)),
                (sample_vocabulary::STYLE, Value::from(style)),
                (sample_vocabulary::PITCH, Value::from(pitch)),
                (
                    sample_vocabulary::DOUBLE_PITCH,
                    evaluation(double_pitch()),
                ),
                (
                    sample_vocabulary::PROFILE,
                    evaluation(grap::call(
                        Value::from(geometry::vocabulary::CIRCLE),
                        [(
                            geometry::vocabulary::RADIUS,
                            grap::call(
                                Value::from(f64::vocabulary::MULTIPLY),
                                [
                                    (f64::vocabulary::LEFT, double_pitch()),
                                    (f64::vocabulary::RIGHT, f64::value(8.0)),
                                ],
                            ),
                        )],
                    )),
                ),
            ],
        ),
    );

    let at_display = new_cell_id();
    cells.set_value(at_display, at_display_partial());

    Document {
        root: Some(Value::record([
            (sample_vocabulary::SHAPE, Value::from(roof)),
            (sample_vocabulary::STYLE, Value::from(style)),
            (sample_vocabulary::FAVORITE, Value::from(favorite)),
            (at_display, Value::from(at_display)),
        ])),
        cells,
    }
}
