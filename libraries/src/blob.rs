//! Hex spelling and editing for GID blobs.

use crate::{Library, absent, line_edit, name, text};
use gid::{Cells, Value};
use grap_runtime::{ForeignFunction, ForeignFunctions};
use progred_display::{Layout, ProjectionInput, TextFamily};

pub const ID: gid::CellId = gid::CellId::from_u128(0x4ab5da466a7c5f1202f5ef862f5ff915);

pub mod vocabulary {
    pub const UPDATE: gid::CellId = gid::CellId::from_u128(0xb691acb5f895285755e9fc74d6da09e4);
}

pub fn parse(text: &str) -> Option<Vec<u8>> {
    parse_hex(text.strip_prefix("0x")?)
}

fn parse_hex(hex: &str) -> Option<Vec<u8>> {
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    hex.as_bytes()
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => Some(digit(*high)? << 4 | digit(*low)?),
            _ => None,
        })
        .collect()
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::UPDATE,
        ForeignFunction::new(|context, call, environment| {
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let input = context.eval(input, environment)?;
            Ok(text::read(&input)
                .and_then(|text| parse_hex(text.trim()))
                .map(Value::from)
                .unwrap_or_else(absent::value))
        }),
    )
}

pub fn display<World, Hover>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(line_edit::layout_with_family(
        gid::hex_string(input.value.as_blob()?),
        grap_runtime::ffi(vocabulary::UPDATE),
        "0x",
        "",
        TextFamily::Monospace,
    ))
}

pub fn library<World: 'static, Hover: 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::UPDATE, name::record("blob update", []));
    Library::named(
        "blob",
        crate::Definitions::from_parts(cells, functions()),
        vec![progred_display::partial(display::<World, Hover>)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_spelling_round_trips_every_byte_and_requires_complete_bytes() {
        let bytes: Vec<u8> = (0..=255).collect();
        for hex in [
            gid::hex_string(&bytes),
            gid::hex_string(&bytes).to_uppercase(),
        ] {
            assert_eq!(parse(&format!("0x{hex}")), Some(bytes.clone()));
        }
        assert_eq!(parse("0x"), Some(vec![]));
        for malformed in ["ff", "0xf", "0xfg", "0xé", "0xff 00", "0x🦀"] {
            assert_eq!(parse(malformed), None);
        }
    }

    #[test]
    fn line_updates_require_complete_hex() {
        for (input, expected) in [
            ("DEad", Some(vec![0xde, 0xad])),
            ("", Some(vec![])),
            (" 00ff ", Some(vec![0x00, 0xff])),
            ("f", None),
            ("xyz", None),
        ] {
            let evaluation = crate::test_evaluate(
                &grap_runtime::call(
                    grap_runtime::ffi(vocabulary::UPDATE),
                    [(line_edit::vocabulary::INPUT, text::value(input))],
                ),
                |_| None,
                &functions(),
                100,
            );
            assert!(evaluation.completed);
            match expected {
                Some(bytes) => assert_eq!(evaluation.result, Value::from(bytes)),
                None => assert!(absent::is_absent(&evaluation.result)),
            }
        }
    }
}
