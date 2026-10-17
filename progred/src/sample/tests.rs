use super::*;
use crate::test_support::src;
use crate::test_values::key;
use gid::Step;

#[test]
fn the_sample_document_shows_the_constructs() {
    let doc = sample_document();
    let lib = crate::stack::load().libraries;
    let sources = src(&doc, &lib);
    // The root is an inline record of roles.
    assert!(doc.root.as_ref().unwrap().as_record().is_some());
    let roof = sources
        .resolve_path(&[key("shape")])
        .unwrap()
        .as_cell()
        .unwrap();
    assert_eq!(
        sources
            .value(roof, &gid::Resolution::Document)
            .and_then(name::read),
        Some("roof")
    );
    // The material cell is referenced and fully bare.
    let material = sources
        .resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("material"),
        ])
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(
        sources
            .value(material, &gid::Resolution::Document)
            .is_none()
    );
    // The stroke cell is a name-only ordinary record, referenced
    // as a label.
    let stroke = sources
        .value(roof, &gid::Resolution::Document)
        .unwrap()
        .as_record()
        .unwrap()
        .keys()
        .copied()
        .find(|cell| {
            sources
                .value(*cell, &gid::Resolution::Document)
                .and_then(name::read)
                == Some("stroke")
        })
        .unwrap();
    assert_eq!(
        sources
            .value(stroke, &gid::Resolution::Document)
            .and_then(name::read),
        Some("stroke")
    );
    // The style cell is shared by the root and the roof.
    assert_eq!(
        sources.resolve_path(&[key("style")]),
        sources.resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("style")
        ])
    );
    // Points hold inline records; the swatch is a blob.
    let points = sources
        .resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("points"),
        ])
        .unwrap();
    let origin = points
        .as_list()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(matches!(
        sources
            .value(origin, &gid::Resolution::Document)
            .and_then(|value| value.as_record())
            .and_then(|fields| fields.get(&crate::test_values::label("at"))),
        Some(Value::Record(_))
    ));
    assert!(
        sources
            .resolve_path(&[
                key("style"),
                Step::Follow(gid::Resolution::Document),
                key("swatch")
            ])
            .unwrap()
            .as_blob()
            .is_some()
    );
    // Documents round trip with convention data unchanged.
    let json = serde_json::to_string(&doc).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.root, doc.root);
    assert_eq!(loaded.cells.value(roof).and_then(name::read), Some("roof"));
    assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
}
