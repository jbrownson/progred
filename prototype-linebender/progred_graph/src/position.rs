//! The position space: ordered identities for list labels. A payload
//! is a byte string read as the binary fraction `0.b₁b₂…`, so
//! trailing zero bits are value-neutral and plain lexicographic
//! payload comparison — the derived `Id` ordering — is the dense
//! order. Canonical form: nonempty, final byte nonzero (one spelling
//! per position; strict reads reject the rest). `between` always
//! exists, so relabeling is never required; identifiers grow roughly
//! a bit per adversarial same-gap insert, the immutable-label side of
//! the order-maintenance trade. Space minted via uuidgen (CSPRNG) on
//! 2026-07-06.

use crate::id::Id;
use uuid::Uuid;

pub const POSITION_SPACE: Uuid = Uuid::from_u128(0x99dc_ae09_6dd4_45ab_9eec_f12b_4b6a_0fed);

/// The canonical payload of a position id, or `None` for other spaces
/// and non-canonical spellings.
pub fn as_position(id: &Id) -> Option<&[u8]> {
    (id.space() == POSITION_SPACE)
        .then(|| id.payload())
        .filter(|payload| payload.last().is_some_and(|last| *last != 0))
}

/// A fresh position strictly between `low` and `high`, where `None`
/// is the open end (before the first element, after the last, or both
/// for the first position in an empty list). `None` when an input is
/// not a canonical position or the pair is not strictly ordered.
pub fn between(low: Option<&Id>, high: Option<&Id>) -> Option<Id> {
    let low = match low {
        Some(id) => Some(as_position(id)?),
        None => None,
    };
    let high = match high {
        Some(id) => Some(as_position(id)?),
        None => None,
    };
    if let (Some(low), Some(high)) = (low, high)
        && low >= high
    {
        return None;
    }
    Some(Id::in_space(
        POSITION_SPACE,
        between_bytes(low.unwrap_or(&[]), high),
    ))
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
mod tests {
    use super::*;

    fn canonical(id: &Id) -> bool {
        as_position(id).is_some()
    }

    #[test]
    fn between_is_ordered_dense_and_canonical() {
        let first = between(None, None).unwrap();
        assert!(canonical(&first));

        let mid = between(Some(&first), None).unwrap();
        assert!(first < mid && canonical(&mid));

        let low = between(None, Some(&first)).unwrap();
        assert!(low < first && canonical(&low));

        let inner = between(Some(&low), Some(&first)).unwrap();
        assert!(low < inner && inner < first && canonical(&inner));
    }

    #[test]
    fn appends_and_prepends_grow_about_a_bit_per_insert() {
        let mut last = between(None, None).unwrap();
        for _ in 0..2000 {
            let next = between(Some(&last), None).unwrap();
            assert!(last < next && canonical(&next));
            last = next;
        }
        assert!(last.payload().len() <= 2000 / 8 + 2);

        let mut first = between(None, None).unwrap();
        for _ in 0..2000 {
            let previous = between(None, Some(&first)).unwrap();
            assert!(previous < first && canonical(&previous));
            first = previous;
        }
        assert!(first.payload().len() <= 2000 / 8 + 2);
    }

    #[test]
    fn same_gap_bisection_grows_about_a_bit_per_insert() {
        let low = between(None, None).unwrap();
        let mut high = between(Some(&low), None).unwrap();
        for _ in 0..2000 {
            let mid = between(Some(&low), Some(&high)).unwrap();
            assert!(low < mid && mid < high && canonical(&mid));
            high = mid;
        }
        assert!(high.payload().len() <= 2000 / 8 + 3);
    }

    #[test]
    fn random_gap_insertions_keep_strict_order() {
        // Deterministic LCG so the test needs no clock or rand.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut step = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) as usize
        };
        let mut positions = vec![between(None, None).unwrap()];
        for _ in 0..3000 {
            let gap = step() % (positions.len() + 1);
            let low = gap.checked_sub(1).map(|i| &positions[i]);
            let high = positions.get(gap);
            let fresh = between(low, high).unwrap();
            assert!(canonical(&fresh));
            assert!(low.is_none_or(|low| *low < fresh));
            assert!(high.is_none_or(|high| fresh < *high));
            positions.insert(gap, fresh);
        }
        assert!(positions.is_sorted());
    }

    #[test]
    fn between_declines_bad_inputs() {
        let a = between(None, None).unwrap();
        assert!(between(Some(&a), Some(&a)).is_none());
        let b = between(Some(&a), None).unwrap();
        assert!(between(Some(&b), Some(&a)).is_none());
        // Non-canonical spellings and other spaces are not positions.
        let trailing_zero = Id::in_space(POSITION_SPACE, vec![0x80, 0x00]);
        assert!(as_position(&trailing_zero).is_none());
        assert!(between(Some(&trailing_zero), None).is_none());
        assert!(as_position(&Id::in_space(POSITION_SPACE, vec![])).is_none());
        assert!(as_position(&Id::from(0.5)).is_none());
    }
}
