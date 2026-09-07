//! A simple name on a record. The field is ordinary GID data. This
//! editor assumes the library; other hosts need not.

use crate::{Library, text};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x3209ad5d23a0c8513f6bd76324a5cf60);

pub mod vocabulary {
    use gid::CellId;

    pub const NAME: CellId = CellId::from_u128(0x02e562654d6d0828d3a7559e6f75fffe);
}

pub fn field(name: impl Into<String>) -> (CellId, Value) {
    (vocabulary::NAME, text::value(name))
}

pub fn record(name: impl Into<String>, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(std::iter::once(field(name)).chain(fields))
}

pub fn read(value: &Value) -> Option<&str> {
    value
        .as_record()?
        .get(&vocabulary::NAME)
        .and_then(text::read)
}

pub(crate) fn short_id(cell: CellId) -> String {
    let hex = cell.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::NAME, record("name", []));
    Library::named(
        ID,
        "name",
        crate::Definitions::from_parts(cells, Default::default()),
        progred_display::partial(|_| None),
    )
    .with_completions(completions)
}

fn completions(
    request: &progred_display::CompletionRequest<'_>,
) -> Option<Vec<progred_display::Completion>> {
    use progred_display::{CompletionKind, CompletionScope};
    match (request.scope, request.kind, request.path.last()) {
        (
            CompletionScope::Suggested,
            CompletionKind::Value,
            Some(gid::Step::Key(vocabulary::NAME)),
        ) => Some(vec![text::completion(text::query_spelling(request.query))]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    #[test]
    fn name_values_offer_text_but_leave_other_completion_contexts_alone() {
        use progred_display::{CompletionKind, CompletionRequest, CompletionScope};
        let path = [
            gid::Step::Follow(gid::Resolution::Document),
            gid::Step::Key(vocabulary::NAME),
        ];
        let request = CompletionRequest {
            query: "",
            kind: CompletionKind::Value,
            scope: CompletionScope::Suggested,
            path: &path,
            value_at: &|_| None,
            resolve: &|_| None,
        };
        for (query, expected) in [
            ("", ""),
            ("tree", "tree"),
            ("123", "123"),
            ("\"tree", "tree"),
            ("\"tree\"", "tree"),
        ] {
            let offers = completions(&CompletionRequest { query, ..request }).unwrap();
            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].value.instantiate(), text::value(expected));
            assert!(offers[0].on_commit.is_some());
        }
        assert!(
            completions(&CompletionRequest {
                kind: CompletionKind::Field,
                ..request
            })
            .is_none()
        );
        assert!(
            completions(&CompletionRequest {
                scope: CompletionScope::Everything,
                ..request
            })
            .is_none()
        );
        assert!(
            completions(&CompletionRequest {
                path: &[],
                ..request
            })
            .is_none()
        );
        assert!(
            completions(&CompletionRequest {
                path: &[gid::Step::Key(new_cell_id())],
                ..request
            })
            .is_none()
        );
    }

    #[test]
    fn names_are_extensible_ordinary_record_data() {
        let mut fields = record("roof", []).as_record().unwrap().clone();
        fields.insert(new_cell_id(), text::value("anything"));
        assert_eq!(read(&Value::Record(fields)), Some("roof"));
        assert_eq!(read(&text::value("roof")), None);

        let empty = record("", []);
        assert_eq!(read(&empty), Some(""));
        assert_eq!(
            empty
                .as_record()
                .unwrap()
                .get(&vocabulary::NAME)
                .and_then(text::read),
            Some("")
        );
    }

    #[test]
    fn the_name_relation_describes_itself_without_core_support() {
        let library = library::<(), ()>();
        assert_eq!(library.value(vocabulary::NAME).and_then(read), Some("name"));
    }
}
