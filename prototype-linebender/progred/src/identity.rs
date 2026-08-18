//! Presentation helpers for GID identities.

use gid::CellId;

/// An ellipsis and the final five hex digits. This is display only;
/// the full cell identity remains the underlying value.
pub fn short_id(id: CellId) -> String {
    let hex = id.simple().to_string();
    format!("…{}", &hex[hex.len() - 5..])
}
