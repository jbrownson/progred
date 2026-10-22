//! Progred's sample GID document used by tests and render fixtures.

use crate::libraries::{
    editor::selection as selection_capability, editor::site, grap::control, layout, name,
};
use crate::test_values::label;
use gid::{Document, Value};

/// A Grap projection kept as ordinary, root-reachable sample data.
/// It is not installed into the editor's projection chain.
pub fn at_display_partial() -> Value {
    let (row, col) = (label("row"), label("col"));
    let bind = |cell| {
        Value::record([
            (::grap::isa::ISA, control::vocabulary::kind::BINDER.into()),
            (control::vocabulary::BIND, Value::from(cell)),
        ])
    };
    let spliced_text = |binder| {
        Value::record([(
            layout::vocabulary::TEXT,
            Value::record([
                (
                    layout::vocabulary::CONTENT,
                    control::unquote(Value::from(binder)),
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
                            (::grap::isa::ISA, control::vocabulary::kind::CASE.into()),
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
                            (::grap::isa::ISA, control::vocabulary::kind::CASE.into()),
                            (
                                control::vocabulary::PATTERN,
                                bind(layout::vocabulary::EVENT),
                            ),
                            (grap::vocabulary::EXPRESSION, ::grap::absent::decline()),
                        ]),
                    ]),
                ),
            ],
        ),
    );
    name::record(
        "at display",
        [
            (::grap::isa::ISA, ::grap::vocabulary::kind::LAMBDA.into()),
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
                                (::grap::isa::ISA, control::vocabulary::kind::CASE.into()),
                                (
                                    control::vocabulary::PATTERN,
                                    Value::record([(row, bind(row)), (col, bind(col))]),
                                ),
                                (
                                    grap::vocabulary::EXPRESSION,
                                    grap::call(
                                        Value::from(control::vocabulary::QUOTE),
                                        [(
                                            grap::vocabulary::EXPRESSION,
                                            layout::test_support::on(
                                                layout::test_support::row(
                                                    4.0,
                                                    [
                                                        layout::test_support::drawing(
                                                            10.0,
                                                            8.0,
                                                            2.0,
                                                            [layout::test_support::stroke(
                                                                layout::test_support::rounded_rect(
                                                                    0.5, 0.5, 9.0, 9.0, 2.0,
                                                                ),
                                                                1.0,
                                                                Value::from(
                                                                    layout::vocabulary::DIM_FACE,
                                                                ),
                                                            )],
                                                        ),
                                                        spliced_text(row),
                                                        layout::test_support::text_leaf(
                                                            "×",
                                                            layout::vocabulary::DIM_FACE,
                                                        ),
                                                        spliced_text(col),
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

/// examples/sample.gid: a small document shaped like a real one. The root
/// is an inline RECORD of roles — a document keys its parts by what they
/// are to it, and needs no identity of its own to do so. Simple names are
/// ordinary record fields ("roof", not its kind). The corner knows its roof
/// (cycle collapse on a real pattern); the style cell is unnamed and
/// referenced twice (short-id heads, secondary marks); the stroke cell
/// holds only an ordinary name record and is referenced as a label; the
/// material cell is fully bare — referenced before anything at all is
/// said about it; the swatch is a blob; each point's position is an inline
/// record, point-shaped data that wants to be a value; the favorite cell
/// holds a bare LINK to the corner — the alias pattern; and pitch flows
/// through a small Grap function to a projected computed result.
pub fn sample_document() -> Document {
    crate::gid_text::parse(include_str!("../../examples/sample.gid"))
        .expect("the sample parses")
        .0
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
            "isa": dbd7e1a38e30425fb1b408b2e446bb1b,
            "call": 8dd318828a4c42cf9f2d5d1ba9b068fe,
            "lambda": e4a0653134ca48379cab2e008bed03e7,
            "binding": 039cd524b99d47ca92d23b26de5a01aa,
            "binder": 81d57515ba3b44aab94e43d59a47fe9a,
          }},
          "cells": {{
            planet: {{name: "planet"}},
            moons: {{name: "moons"}},
            n: {{name: "n"}},
            planet_view: {{isa: lambda, name: "planet view", params: [value], body: {view_body}}},
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
  isa: call,
  function: where,
  bindings: [{isa: binding, pattern: {moons: {isa: binder, bind: n}}, subject: value}],
  expression: {isa: call, function: row, children: {isa: call, function: do, expressions: [
    {isa: call, function: text, content: "planet with"},
    {isa: call, function: descend, step: {key: moons}},
  ]}},
}"#;

#[cfg(test)]
mod tests;
