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
                (crate::libraries::grap::numbers::f32::SUM, "+"),
                (crate::libraries::grap::numbers::f32::EQUAL, "=="),
                (
                    crate::libraries::grap::numbers::f32::LEFT_NOT_F32,
                    "left is not f32",
                ),
                (crate::libraries::grap::numbers::u64::SUM, "+"),
                (
                    crate::libraries::grap::numbers::u64::OVERFLOW,
                    "u64 overflow",
                ),
                (crate::libraries::grap::numbers::f64::SUM, "+"),
                (
                    crate::libraries::grap::numbers::f64::LEFT_NOT_F64,
                    "left is not f64",
                ),
                (
                    crate::libraries::grap::numbers::f64::RIGHT_NOT_F64,
                    "right is not f64",
                ),
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
