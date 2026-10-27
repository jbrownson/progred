use crate::libraries::{Library, f32, f64, grap::control, name, png, text, u64};
use gid::CellId;

#[test]
fn each_library_names_its_vocabulary() {
    let libraries: [(Library<crate::Editor>, &[(CellId, &str)]); 8] = [
        (f32::library(), &[(f32::vocabulary::F32, "f32")]),
        (u64::library(), &[(u64::vocabulary::U64, "u64")]),
        (f64::library(), &[(f64::vocabulary::F64, "f64")]),
        (
            crate::libraries::grap::library(),
            &[
                (crate::libraries::grap::numbers::f32::NOT_F32, "not f32"),
                (
                    crate::libraries::grap::numbers::u64::OVERFLOW,
                    "u64 overflow",
                ),
                (crate::libraries::grap::numbers::f64::NOT_F64, "not f64"),
            ],
        ),
        (
            png::library(),
            &[
                (png::vocabulary::WIDTH, "png width"),
                (png::vocabulary::INVALID_PNG, "invalid png"),
            ],
        ),
        (name::library(), &[(name::vocabulary::NAME, "name")]),
        (text::library(), &[(text::vocabulary::UTF8, "utf8")]),
        (
            crate::libraries::grap::library(),
            &[
                (control::vocabulary::QUOTE, "quote"),
                (control::vocabulary::UNQUOTE, "unquote"),
                (control::vocabulary::DO, "do"),
                (control::vocabulary::INVALID_CASES, "invalid cases"),
                (control::vocabulary::INVALID_CASE, "invalid case"),
                (control::vocabulary::INVALID_BINDER, "invalid binder"),
                (control::vocabulary::INVALID_BINDINGS, "invalid bindings"),
                (control::vocabulary::INVALID_BINDING, "invalid binding"),
                (
                    control::vocabulary::INVALID_EXPRESSIONS,
                    "invalid expressions",
                ),
            ],
        ),
    ];
    for (library, names) in libraries {
        for &(cell, spelling) in names {
            assert_eq!(
                library.definitions.value(cell).and_then(name::read),
                Some(spelling),
                "{cell}"
            );
        }
    }
}

#[test]
fn a_number_operator_names_its_representation_as_a_subscript() {
    use crate::display::Name;
    use crate::libraries::grap::numbers;
    use puri::text::Script::{Normal, Subscript};
    let document = gid::Document::default();
    let libraries = crate::test_support::core_libraries();
    let sources = crate::test_support::src(&document, &libraries);
    for (operator, spelling, representation) in [
        (numbers::f64::SUM, "+", "f64"),
        (numbers::f32::EQUAL, "==", "f32"),
        (numbers::u64::SUM, "+", "u64"),
        (numbers::f64::LERP, "lerp", "f64"),
    ] {
        assert_eq!(
            name::of(&sources, operator),
            Some(Name::Formatted(
                [
                    (spelling.to_owned(), Normal),
                    (representation.to_owned(), Subscript)
                ]
                .into()
            ))
        );
    }
}

#[test]
fn a_typed_edit_keeps_the_fields_of_the_record_it_replaces() {
    let named = name::record("tilt", f64::value(45.0).as_record().unwrap().clone());
    assert_eq!(
        crate::libraries::keeping_fields(Some(&named), f64::value(90.0)),
        name::record("tilt", f64::value(90.0).as_record().unwrap().clone())
    );
    let cell = gid::Value::from(gid::new_cell_id());
    assert_eq!(
        crate::libraries::keeping_fields(Some(&cell), f64::value(90.0)),
        f64::value(90.0)
    );
    assert_eq!(
        crate::libraries::keeping_fields(Some(&named), cell.clone()),
        cell
    );
    assert_eq!(
        crate::libraries::keeping_fields(None, text::value("x")),
        text::value("x")
    );
}
