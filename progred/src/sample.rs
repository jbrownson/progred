//! Progred's sample GID document used by tests and render fixtures.

use crate::libraries::{
    control, f64, geometry, layout, name, selection as selection_capability, site, text,
};
use gid::{Cells, Document, Value, new_cell_id};

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
                    Value::list([
                        Value::record([
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
                                    [
                                        (
                                            selection_capability::vocabulary::PATH,
                                            grap::call(site::vocabulary::PATH.into(), []),
                                        ),
                                        (
                                            site::vocabulary::VALUE,
                                            crate::selection::payload::edge(),
                                        ),
                                    ],
                                ),
                            ),
                        ]),
                        Value::record([
                            (
                                control::vocabulary::PATTERN,
                                bind(layout::vocabulary::EVENT),
                            ),
                            (
                                grap::vocabulary::EXPRESSION,
                                crate::libraries::absent::decline(),
                            ),
                        ]),
                    ]),
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
                                                                Value::from(
                                                                    layout::vocabulary::DIM_FACE,
                                                                ),
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

#[test]
fn checked_in_projection_uses_the_current_selection_capabilities() {
    let (document, _) = crate::gid_text::parse(include_str!("../../examples/sample.gid")).unwrap();
    let projection = document
        .cells
        .iter()
        .find_map(|(_, value)| (name::read(value) == Some("at display")).then_some(value));
    assert_eq!(projection, Some(&at_display_partial()));
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
                (sample_vocabulary::DOUBLE_PITCH, evaluation(double_pitch())),
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

/// A document with a planet library: its view says "planet with" and descends
/// into the moons, and declines anything without them.
pub fn planets(view_body: &str) -> (crate::Editor, crate::gid_text::Binders) {
    let source = format!(
        r#"{{
          "binders": {{
            "name": 02e562654d6d0828d3a7559e6f75fffe,
            "f64": ed11fde03b7c2c1ba2fccc3cdba5d561,
            "function": 751fca4373debdd0b7e6eb73e08d684b,
            "params": 195b378d0d31d90ab0d7366c15346b70,
            "body": 986143866eda2e2fbf9ab8484357a0c9,
            "expression": ccc55b0eb63b9f564ea74436094d4014,
            "where": 2a3432e9c5ce4c62b8261a6b248f13c2,
            "bindings": f6fb0062e8a14f808e416a9edece2a9d,
            "pattern": b9dc97198709bc6f7bae4c7afc7d424f,
            "bind": 5e46d12705690e8a377eb0f16ad9dba6,
            "subject": 00dafdc01c7e014edd857e174c6c8b6f,
            "do": b1fc4cb45c58b1a662c431feef5bd140,
            "expressions": 5fab151c006ae1487c28837f2003f43c,
            "row": 1af52c96e380b7d40c9e1f6a2d5b83e7,
            "children": e10b73d5482f96ca7d3852c0f16b49ea,
            "text": 08e64d1f3a92c5b7b7f0d38a165e29c4,
            "content": 3e6b91d4a25f70c8815d29f6c4a30e7b,
            "descend": 35c7a8e2f10d49b6d2f8016c4b9ea375,
            "descend_path": fa9520a04e0c79e7ff78adc6e3f6e754,
            "step": 67a2d5e0b93c48f14e28b671d0a5c39f,
            "steps": 1298c6f4a7053edb09b64d2e8371fa5c,
            "key": f12dea12c741fe36312750a264f3a235,
            "value": 84d3ba81fd2a52ea37478f4a868106f4,
            "projection": 873503e2e37a1722a0dd21399be9ee7f,
            "libraries": bc197ad66fea59a9bd97c4191c7845b2,
            "keys": bb30d215605f5599e31f6274641ca89f,
            "planet": 6655e4ba9e8ae76706056e0b8edf94f0,
            "moons": 6122302fdbe3db3624731717321b94a1,
            "planet_view": 0b570cad527f02e4ed4b06110a62bf67,
          }},
          "cells": {{
            planet: {{name: "planet"}},
            moons: {{name: "moons"}},
            n: {{name: "n"}},
            planet_view: {{name: "planet view", params: [value], body: {view_body}}},
            planets: {{name: "planets", keys: [planet], projection: planet_view}},
          }},
          "root": {{
            libraries: [planets],
            mars: {{planet: "Mars", moons: {{f64: 0x0000000000000040}}}},
            note: {{planet: "Pluto"}},
          }},
        }}"#
    );
    let (doc, names) = crate::gid_text::parse(&source).unwrap();
    (crate::test_editor(doc), names)
}

pub const PLANET_VIEW: &str = r#"{
  function: where,
  bindings: [{pattern: {moons: {bind: n}}, subject: value}],
  expression: {function: row, children: {function: do, expressions: [
    {function: text, content: "planet with"},
    {function: descend, step: {key: moons}},
  ]}},
}"#;

#[cfg(test)]
mod tests;
