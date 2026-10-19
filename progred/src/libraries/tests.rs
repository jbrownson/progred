use crate::libraries::{Library, control, f32, f64, name, png, text, u64};
use gid::CellId;

#[test]
fn each_library_names_its_vocabulary() {
    let libraries: [(Library<crate::Editor>, &[(CellId, &str)]); 7] = [
        (
            f32::library(),
            &[
                (f32::vocabulary::SUM, "+"),
                (f32::vocabulary::EQUAL, "=="),
                (f32::vocabulary::LEFT_NOT_F32, "left is not f32"),
            ],
        ),
        (
            u64::library(),
            &[
                (u64::vocabulary::SUM, "+"),
                (u64::vocabulary::OVERFLOW, "u64 overflow"),
            ],
        ),
        (
            f64::library(),
            &[
                (f64::vocabulary::F64, "f64"),
                (f64::vocabulary::SUM, "+"),
                (f64::vocabulary::LEFT_NOT_F64, "left is not f64"),
                (f64::vocabulary::RIGHT_NOT_F64, "right is not f64"),
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
            control::library(),
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
