use super::*;
use crate::library::Libraries;
use gid::{Cells, Document, Resolution, new_cell_id};

#[test]
fn only_root_declarations_followed_through_stable_definitions_are_viewports() {
    let alias = new_cell_id();
    let definition = new_cell_id();
    let declaration = presentation::sized(Value::record([]), Value::record([]));
    let mut cells = Cells::new();
    cells.set_value(alias, definition.into());
    cells.set_value(definition, declaration.clone());
    let mut doc = Document {
        root: Some(alias.into()),
        cells,
    };
    let libraries = Libraries::default();
    let resolved = entry(
        Sources {
            doc: &doc,
            libraries: &libraries,
        },
        &[],
    )
    .unwrap();
    assert_eq!(resolved.value, &declaration);
    assert_eq!(resolved.path, vec![Step::Follow(Resolution::Document); 2]);

    for root in [
        Value::list([declaration]),
        Value::record([(presentation::vocabulary::VIEWPORT, definition.into())]),
        new_cell_id().into(),
    ] {
        doc.root = Some(root);
        assert!(
            entry(
                Sources {
                    doc: &doc,
                    libraries: &libraries
                },
                &[]
            )
            .is_none()
        );
    }
    doc.root = Some(alias.into());
    doc.cells.set_value(definition, alias.into());
    assert!(
        entry(
            Sources {
                doc: &doc,
                libraries: &libraries
            },
            &[]
        )
        .is_none()
    );
}
