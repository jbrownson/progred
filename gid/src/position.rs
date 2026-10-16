//! Session-only element identity for GID list values. A position is a
//! byte string read as the binary fraction `0.b₁b₂…`, so trailing
//! zero bits are value-neutral and plain lexicographic comparison —
//! the derived ordering — is the dense order. Canonical form:
//! nonempty, final byte nonzero; construction owns it. `between`
//! always exists, so relabeling is never required; identifiers grow
//! roughly a bit per adversarial same-gap insert, the immutable-label
//! side of the order-maintenance trade. Positions are minted at load
//! and insert and stripped from list storage at save. Libraries may
//! encode their bytes in ordinary values to describe session paths.

use std::sync::Arc;

/// A canonical binary fraction. Ordering is the sequence; clones share its immutable bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position(Arc<[u8]>);

impl Position {
    pub fn from_bytes(bytes: Vec<u8>) -> Option<Self> {
        bytes
            .last()
            .is_some_and(|last| *last != 0)
            .then(|| Self(bytes.into()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A fresh position strictly between `low` and `high`, where `None`
/// is the open end (before the first element, after the last, or both
/// for the first position in an empty list). `None` when the pair is
/// not strictly ordered.
pub fn between(low: Option<&Position>, high: Option<&Position>) -> Option<Position> {
    if let (Some(low), Some(high)) = (low, high)
        && low >= high
    {
        return None;
    }
    Position::from_bytes(between_bytes(
        low.map(Position::as_bytes).unwrap_or(&[]),
        high.map(Position::as_bytes),
    ))
}

/// `n` canonical positions in increasing order, spread evenly across
/// the unit interval — minted for a loaded sequence, whose file keeps
/// only the order. Payloads stay near log₂₅₆ n bytes with balanced
/// gaps for the session's `between`s.
pub fn spread(n: usize) -> Vec<Position> {
    // k bytes give 256^k slots; twice the need keeps every gap ≥ 2,
    // so consecutive floors below stay strictly ordered.
    let k = (1..)
        .find(|k| 256u128.pow(*k) >= 2 * (n as u128 + 1))
        .expect("a document-sized list");
    (1..=n as u128)
        .map(|i| {
            let m = i * 256u128.pow(k) / (n as u128 + 1);
            // m as exactly k big-endian digits — the fraction m/256^k
            // — then trailing zeros trimmed for the canonical form
            // (leading zeros are load-bearing; trailing are not).
            let bytes: Vec<u8> = (1..=k).map(|j| (m >> (8 * (k - j))) as u8).collect();
            let end = bytes
                .iter()
                .rposition(|byte| *byte != 0)
                .expect("m ≥ 2, so some digit is nonzero");
            Position::from_bytes(bytes[..=end].to_vec()).expect("trimmed to a nonzero final byte")
        })
        .collect()
}

/// Digits base 256 under the fraction reading: `low` extends with
/// zeros, an absent `high` is the open supremum 1.0. Bit-aware where
/// it matters: adjacent digits descend into `low`'s branch and pick
/// the midpoint of what remains, so growth stays near a bit per
/// forced split rather than a byte.
fn between_bytes(low: &[u8], high: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        let a = u16::from(low.get(i).copied().unwrap_or(0));
        let b = match high {
            None => 0x100,
            Some(high) => u16::from(high.get(i).copied().unwrap_or(0)),
        };
        if a == b {
            out.push(a as u8);
            i += 1;
        } else if b - a >= 2 {
            out.push(((a + b) / 2) as u8);
            return out;
        } else {
            // Adjacent digits: anything strictly above `low` inside
            // its own branch is below `high`.
            out.push(a as u8);
            i += 1;
            loop {
                let a = u16::from(low.get(i).copied().unwrap_or(0));
                if a < 0xFF {
                    out.push(((a + 0x100) / 2) as u8);
                    return out;
                }
                out.push(0xFF);
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;
