//! The assigned-size pane contract, reusing ordinary projection lowering.

use super::Projection;
use crate::sources::Sources;
use gid::{Path, Step, Value};
use kurbo::Size;
use progred_libraries::presentation;

pub(crate) struct Entry<'a> {
    pub value: &'a Value,
    pub path: Path,
}

pub(crate) fn entry<'a>(sources: Sources<'a>, path: &[Step]) -> Option<Entry<'a>> {
    let mut value = sources.resolve_path(path)?;
    let mut path = path.to_vec();
    let mut seen = Vec::new();
    while let Some(cell) = value.as_cell() {
        if seen.contains(&cell) {
            return None;
        }
        seen.push(cell);
        let definition = sources.resolve(cell)?;
        path.push(Step::Follow(definition.source));
        value = definition.value;
    }
    presentation::viewport(value).map(|_| Entry { value, path })
}

pub(crate) fn projection<C: 'static>(ambient: &Projection<C>, size: Size) -> Projection<C> {
    ambient
        .clone()
        .with_entry(progred_display::partial(move |input| {
            presentation::viewport_display(input, size.width, size.height)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Cells, Document, Resolution, new_cell_id};
    use progred_libraries::Libraries;

    #[test]
    fn only_root_declarations_followed_through_stable_definitions_are_viewports() {
        let alias = new_cell_id();
        let definition = new_cell_id();
        let declaration = Value::record([
            (presentation::vocabulary::VALUE, Value::record([])),
            (presentation::vocabulary::VIEWPORT, Value::record([])),
        ]);
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
}
