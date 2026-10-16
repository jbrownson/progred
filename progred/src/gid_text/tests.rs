use super::*;

fn parse_ok(text: &str) -> (Document, Binders) {
    parse(text).expect("parses")
}

#[test]
fn a_file_round_trips_canonically() {
    let text = r#"{
  "binders": {
    "color": 267770b27d00e45a18f730652ff90dcf,
    "name": 02e562654d6d0828d3a7559e6f75fffe,
    "payload": 6675eed11743101e3af8ec08c468da50,
    "shape": f7c17a58b92f3f1b6090142612b16595,
    "stroke": 0a6fba22bfd658dedb62920b6ea06699,
    "style": 0f950b2aa9ed44554b87ab8f88d82cea,
    "swatch": 0f3ae682742540de963d02d5f4b1a5a5,
    "tags": 98a45386e99286144421e94d8c444d9b,
  },
  "cells": {
    roof1: {name: "roof", stroke: "hairline", tags: ["draft", "gabled"]},
    9d2c1e10ab3440de963d02d5f4b1a5a5: {payload: 0x663399},
  },
  "root": {shape: roof1, style: 9d2c1e10ab3440de963d02d5f4b1a5a5, color: swatch},
}
"#;
    let (doc, binders) = parse_ok(text);
    assert_eq!(binders.len(), 9);
    let roof = binders["roof1"];
    assert_eq!(doc.cells.value(roof).and_then(name::read), Some("roof"));
    // The canonical print is a fixed point.
    let printed = print(&doc, &binders);
    let (again, binders_again) = parse_ok(&printed);
    assert_eq!(print(&again, &binders_again), printed);
    // The loaded binder survives the round trip.
    assert!(printed.contains("\"roof1\": "));
    assert!(printed.contains("\"swatch\": 0f3ae682742540de963d02d5f4b1a5a5"));
}

#[test]
fn leniencies_normalize_and_minting_persists() {
    // Uppercase cell id, no trailing commas, a binder never declared:
    // all defined leniencies; saving canonicalizes.
    let text = r#"{"cells": {florp: "x"},
                       "root": {a: florp, b: 9D2C1E10AB3440DE963D02D5F4B1A5A5}}"#;
    let (doc, binders) = parse_ok(text);
    let florp = binders["florp"];
    assert_eq!(doc.cells.value(florp), Some(&crate::test_values::text("x")));
    let printed = print(&doc, &binders);
    assert!(printed.contains("\"florp\": "));
    assert!(printed.contains("9d2c1e10ab3440de963d02d5f4b1a5a5"));
    // A hyphenated id is neither a cell id nor a binder.
    assert!(parse(r#"{"root": e6c0b205-8b84-6842-8d64-576d953e2d62}"#).is_err());
    // A reference to a never-defined binder is a bare cell —
    // create-on-reference at the file layer.
    let (doc, binders) = parse_ok(r#"{"root": [ghost]}"#);
    let ghost = binders["ghost"];
    assert!(doc.cells.value(ghost).is_none());
    assert_eq!(doc.root, Some(Value::list([Value::from(ghost)])));
}

#[test]
fn a_cell_stated_twice_fails_and_duplicate_names_coexist() {
    // The same cell twice, by the same spelling or different
    // ones, refuses rather than clobbering.
    assert!(parse(r#"{"cells": {x: "a", x: "b"}}"#).is_err());
    let aliased = r#"{
            "binders": {"x": 9d2c1e10ab3440de963d02d5f4b1a5a5},
            "cells": {
                x: "a",
                9d2c1e10ab3440de963d02d5f4b1a5a5: "b",
            },
        }"#;
    assert!(parse(aliased).is_err());
    // Duplicate simple-name facts are ordinary GID data.
    let (doc, _) = parse_ok(
        r#"{"binders": {
                "name": 02e562654d6d0828d3a7559e6f75fffe,
                "payload": 0a6fba22bfd658dedb62920b6ea06699
            }, "cells": {
                a: {name: "twin", payload: "a"},
                b: {name: "twin", payload: "b"},
            }}"#,
    );
    let twins = doc
        .cells
        .iter()
        .filter(|(_, value)| name::read(value) == Some("twin"))
        .count();
    assert_eq!(twins, 2);
}

#[test]
fn derived_binders_come_from_simple_name_facts() {
    let mut cells = Cells::new();
    let cell = new_cell_id();
    cells.set_value(
        cell,
        name::record(
            "grap program",
            [(
                crate::test_values::label("body"),
                crate::test_values::text("body"),
            )],
        ),
    );
    let doc = Document {
        root: Some(Value::from(cell)),
        cells,
    };
    let printed = print(&doc, &Binders::new());
    assert!(printed.contains("\"grap_program\": "));
    assert!(printed.contains("    grap_program: {"));
    let (again, binders) = parse_ok(&printed);
    assert_eq!(
        again
            .cells
            .value(binders["grap_program"])
            .and_then(name::read),
        Some("grap program")
    );
    assert_eq!(print(&again, &binders), printed);
}

#[test]
fn generated_binders_cover_every_identity_and_disambiguate() {
    let loaded = CellId::from_u128(0x0a6fba22bfd658dedb62920b6ea06699);
    let named_a = CellId::from_u128(0x6675eed11743101e3af8ec08c468da50);
    let named_b = CellId::from_u128(0x98a45386e99286144421e94d8c444d9b);
    let unnamed_a = CellId::from_u128(0x111111111111111111111111111abcde);
    let unnamed_b = CellId::from_u128(0x222222222222222222222222222abcde);
    let mut cells = Cells::new();
    cells.set_value(loaded, Value::from(vec![0]));
    cells.set_value(named_a, name::record("same", []));
    cells.set_value(named_b, name::record("same", []));
    cells.set_value(unnamed_a, Value::from(vec![1]));
    cells.set_value(unnamed_b, Value::from(vec![2]));
    let doc = Document { root: None, cells };
    let printed = print(&doc, &Binders::from([("same".to_string(), loaded)]));
    let (_, binders) = parse_ok(&printed);
    assert_eq!(binders["same"], loaded);
    assert_eq!(binders["same_2"], named_a);
    assert_eq!(binders["same_3"], named_b);
    assert_eq!(binders["_abcde"], unnamed_a);
    assert_eq!(binders["_abcde_2"], unnamed_b);
}

#[test]
fn structural_damage_refuses() {
    assert!(parse("{").is_err());
    assert!(parse(r#"{"root": "unterminated}"#).is_err());
    assert!(parse(r#"{"root": {"label": "value"}}"#).is_err());
    assert!(parse(r#"{"root": {a: "x", a: "y"}}"#).is_err());
    assert!(parse(r#"{"root": 0x123}"#).is_err());
    assert!(parse(r#"{"cells": {x: }}"#).is_err());
    assert!(parse(r#"{"bogus": {}}"#).is_err());
    assert!(parse(r#"{"binders": {"x": notagid}}"#).is_err());
    assert!(parse(r#"{"root": 9-not-a-binder}"#).is_err());
}

#[test]
fn text_escapes_and_blobs_round_trip() {
    let doc = Document {
        root: Some(Value::record([
            (
                crate::test_values::label("s"),
                crate::test_values::text("say \"hi\"\n\tdone\\"),
            ),
            (
                crate::test_values::label("b"),
                Value::from(vec![0x00_u8, 0xff]),
            ),
            (
                crate::test_values::label("empty"),
                Value::from(Vec::<u8>::new()),
            ),
        ])),
        cells: Cells::new(),
    };
    let printed = print(&doc, &Binders::new());
    let (again, _) = parse_ok(&printed);
    assert_eq!(again.root, doc.root);
}

#[test]
fn the_empty_document_is_the_empty_record() {
    let (doc, binders) = parse_ok("{}");
    assert!(doc.root.is_none());
    assert!(doc.cells.cells().next().is_none());
    assert_eq!(print(&doc, &binders), "{\n}\n");
}
