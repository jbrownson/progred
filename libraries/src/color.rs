//! An sRGB color value for projected content. The spelling stays
//! ordinary UTF-8 so the initial editor can expose it directly; a
//! color control can project the same value later.

use crate::{Library, name, text};
use gid::{Cells, Value};
use puri::Color;

pub mod vocabulary {
    use gid::CellId;

    pub const COLOR: CellId = CellId::from_u128(0x6c8a17cbe463186cc8b07e536ccffa6b);
}

pub fn value(spelling: impl Into<String>) -> Value {
    Value::record([(vocabulary::COLOR, text::value(spelling))])
}

pub fn read(value: &Value) -> Option<Color> {
    let spelling = value
        .as_record()?
        .get(&vocabulary::COLOR)
        .and_then(text::read)?;
    let bytes = spelling.strip_prefix('#')?;
    let byte = |at| u8::from_str_radix(bytes.get(at..at + 2)?, 16).ok();
    match bytes.len() {
        6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 255)),
        8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::COLOR, name::record("color", []));
    Library {
        cells,
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rgb_and_rgba_hex() {
        assert_eq!(
            read(&value("#b4e0fe")),
            Some(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff))
        );
        assert_eq!(
            read(&value("#ebb4cc99")),
            Some(Color::from_rgba8(0xeb, 0xb4, 0xcc, 0x99))
        );
        assert_eq!(read(&value("rebeccapurple")), None);
    }
}
