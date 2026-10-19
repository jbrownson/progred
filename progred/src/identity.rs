//! Presentation helpers for GID identities.

use gid::CellId;

/// An ellipsis and the final five hex digits. This is display only;
/// the full cell identity remains the underlying value.
pub fn short_id(id: CellId) -> String {
    let hex = id.to_string();
    format!("…{}", &hex[hex.len() - 5..])
}

/// A cell's `name`, or its short id where it has none.
pub fn name_or_short_id(name: Option<&str>, id: CellId) -> String {
    name.map_or_else(|| short_id(id), str::to_owned)
}
