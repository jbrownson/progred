use super::*;
use gid::new_cell_id;

#[test]
fn names_are_extensible_ordinary_record_data() {
    let mut fields = record("roof", []).as_record().unwrap().clone();
    fields.set(new_cell_id(), text::value("anything"));
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
fn a_new_label_is_the_typed_text_as_a_cell_name_and_nothing_else() {
    use crate::display::{CompletionKind, CompletionScope};
    let ask =
        |kind, scope, query| completions(&crate::test_support::bare_request(kind, scope, query));
    let [label] = ask(CompletionKind::Field, CompletionScope::Literal, "\"width\"")
        .unwrap()
        .try_into()
        .ok()
        .unwrap();
    assert_eq!(label.display, "width".into());
    assert!(label.detail.is_none());
    assert!(matches!(
        label.offered,
        crate::display::Offered::New(Some(ref what)) if *what == "label".into()
    ));
    assert!(ask(CompletionKind::Field, CompletionScope::Literal, " ").is_none());
    assert!(ask(CompletionKind::Value, CompletionScope::Literal, "width").is_none());
    assert!(ask(CompletionKind::Field, CompletionScope::Everything, "width").is_none());
}

#[test]
fn a_formatted_name_spells_its_cells_and_subscripts() {
    use crate::display::Name;
    use Script::{Normal, Subscript};
    let [tag, unnamed, looping, plus, nested, broken] = std::array::from_fn(|_| new_cell_id());
    let cells = [
        (tag, record("f64", [])),
        (
            looping,
            Value::record([(
                vocabulary::NAME,
                formatted([text::value("loop"), looping.into()]),
            )]),
        ),
        (
            plus,
            Value::record([(
                vocabulary::NAME,
                formatted([text::value("+"), subscript(tag.into())]),
            )]),
        ),
        (
            nested,
            Value::record([(
                vocabulary::NAME,
                formatted([
                    plus.into(),
                    text::value(" again"),
                    subscript(formatted([unnamed.into()])),
                ]),
            )]),
        ),
        (
            broken,
            Value::record([(
                vocabulary::NAME,
                formatted([Value::list([]), Value::record([(tag, text::value("x"))])]),
            )]),
        ),
    ];
    let value_of = |cell| {
        cells
            .iter()
            .find(|(candidate, _)| *candidate == cell)
            .map(|(_, value)| value)
    };
    let runs = |cell| match of_cell(&value_of, cell) {
        Some(Name::Formatted(runs)) => runs.to_vec(),
        other => panic!("{other:?}"),
    };
    let run = |text: &str, script| (text.to_owned(), script);
    assert_eq!(of_cell(&value_of, tag), Some(Name::Plain("f64")));
    // A cell part is spelled as that cell's name, keeping its subscripts.
    assert_eq!(runs(plus), [run("+", Normal), run("f64", Subscript)]);
    let unnamed_id = crate::identity::short_id(unnamed);
    assert_eq!(
        runs(nested),
        [
            run("+", Normal),
            run("f64", Subscript),
            run(" again", Normal),
            run(&unnamed_id, Subscript),
        ]
    );
    // A name that names itself still reads, stopping at a short id.
    assert!(runs(looping)[0].0.starts_with("looploop"));
    // A part that is neither text, a cell, a list nor a subscript isn't a
    // name.
    assert_eq!(of_cell(&value_of, broken), None);
    let name = of_cell(&value_of, nested).unwrap();
    assert_eq!(name.spelling(), format!("+f64 again{unnamed_id}"));
    assert_eq!(name.subscripts(), [1..4, 10..10 + unnamed_id.len()]);
}

#[test]
fn a_library_lists_its_data_its_absents_and_its_loose_fields() {
    use crate::libraries::def::vocabulary::{ABSENTS, DATA, FIELDS};
    let libraries = crate::stack::load().libraries;
    let list = |library: gid::CellId, list| -> Vec<gid::CellId> {
        libraries
            .first_definition(library)
            .and_then(|(_, value)| value.as_record())
            .and_then(|record| record.get(&list))
            .and_then(Value::as_list)
            .map(|cells| cells.values().filter_map(Value::as_cell).collect())
            .unwrap_or_default()
    };
    use crate::libraries::logic::vocabulary as logic;
    assert_eq!(
        list(crate::libraries::logic::ID, ABSENTS),
        [logic::CONDITION_NOT_MET, logic::NOT_BOOL]
    );
    assert_eq!(
        list(crate::libraries::logic::ID, DATA),
        [logic::FALSE, logic::TRUE]
    );
    // Its fields are the loose ones: `require`'s parameter, `condition`,
    // is the function's, and png's `image` its functions'.
    let fields = list(crate::libraries::logic::ID, FIELDS);
    assert!(!fields.contains(&logic::CONDITION));
    assert!(!fields.contains(&logic::TRUE) && !fields.contains(&logic::NOT_BOOL));
    assert!(
        !list(crate::libraries::png::ID, FIELDS)
            .contains(&crate::libraries::png::vocabulary::IMAGE)
    );
    // What a library defines with a value is its data, as a named color is.
    assert!(list(crate::libraries::color::ID, DATA).len() > 100);
    assert!(list(crate::libraries::f64::ID, DATA).contains(&crate::libraries::f64::vocabulary::PI));
    // An absent's reason is its kind's field, not the library's; the
    // reasons it gives are its absents, and fields no kind declares, such
    // as an absent's causes, stay loose.
    use crate::libraries::absent::vocabulary as absent;
    let fields = list(crate::libraries::absent::ID, FIELDS);
    assert!(!fields.contains(&absent::REASON));
    assert!(fields.contains(&absent::CAUSES));
    assert!(list(crate::libraries::absent::ID, ABSENTS).contains(&absent::DECLINED));
}
