//! Session-only element identity for list values. A position is a
//! byte string read as the binary fraction `0.b₁b₂…`, so trailing
//! zero bits are value-neutral and plain lexicographic comparison —
//! the derived ordering — is the dense order. Canonical form:
//! nonempty, final byte nonzero; construction owns it. `between`
//! always exists, so relabeling is never required; identifiers grow
//! roughly a bit per adversarial same-gap insert, the immutable-label
//! side of the order-maintenance trade. Deliberately neither a
//! `Value` nor an `Atom`: positions are minted at load and insert,
//! stripped at save, and cannot occur in data.

/// A canonical binary fraction. Ordering is the sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position(Vec<u8>);

impl Position {
    fn canonical(bytes: Vec<u8>) -> Option<Self> {
        bytes
            .last()
            .is_some_and(|last| *last != 0)
            .then_some(Self(bytes))
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
    Position::canonical(between_bytes(
        low.map(|p| p.0.as_slice()).unwrap_or(&[]),
        high.map(|p| p.0.as_slice()),
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
            Position::canonical(bytes[..=end].to_vec()).expect("trimmed to a nonzero final byte")
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
mod tests {
    use super::*;

    #[test]
    fn between_is_ordered_and_dense() {
        let first = between(None, None).unwrap();
        let mid = between(Some(&first), None).unwrap();
        assert!(first < mid);
        let low = between(None, Some(&first)).unwrap();
        assert!(low < first);
        let inner = between(Some(&low), Some(&first)).unwrap();
        assert!(low < inner && inner < first);
    }

    #[test]
    fn appends_and_prepends_grow_about_a_bit_per_insert() {
        let mut last = between(None, None).unwrap();
        for _ in 0..2000 {
            let next = between(Some(&last), None).unwrap();
            assert!(last < next);
            last = next;
        }
        assert!(last.0.len() <= 2000 / 8 + 2);

        let mut first = between(None, None).unwrap();
        for _ in 0..2000 {
            let previous = between(None, Some(&first)).unwrap();
            assert!(previous < first);
            first = previous;
        }
        assert!(first.0.len() <= 2000 / 8 + 2);
    }

    #[test]
    fn same_gap_bisection_grows_about_a_bit_per_insert() {
        let low = between(None, None).unwrap();
        let mut high = between(Some(&low), None).unwrap();
        for _ in 0..2000 {
            let mid = between(Some(&low), Some(&high)).unwrap();
            assert!(low < mid && mid < high);
            high = mid;
        }
        assert!(high.0.len() <= 2000 / 8 + 3);
    }

    #[test]
    fn random_gap_insertions_keep_strict_order() {
        // Deterministic LCG so the test needs no clock or rand.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut step = move || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as usize
        };
        let mut positions = vec![between(None, None).unwrap()];
        for _ in 0..3000 {
            let gap = step() % (positions.len() + 1);
            let low = gap.checked_sub(1).map(|i| &positions[i]);
            let high = positions.get(gap);
            let fresh = between(low, high).unwrap();
            assert!(low.is_none_or(|low| *low < fresh));
            assert!(high.is_none_or(|high| fresh < *high));
            positions.insert(gap, fresh);
        }
        assert!(positions.is_sorted());
    }

    #[test]
    fn spread_mints_even_ordered_positions() {
        for n in [0, 1, 2, 3, 127, 128, 1000] {
            let positions = spread(n);
            assert_eq!(positions.len(), n);
            // Every gap admits a between — strict order included,
            // since between demands it.
            assert!(between(None, positions.first()).is_some() || n == 0);
            assert!(between(positions.last(), None).is_some() || n == 0);
            for pair in positions.windows(2) {
                assert!(between(Some(&pair[0]), Some(&pair[1])).is_some());
            }
            // Even spreading, not append-chains: payloads stay near
            // log₂₅₆ n bytes instead of growing toward n/8.
            assert!(positions.iter().all(|p| p.0.len() <= 2));
        }
    }

    #[test]
    fn between_declines_misordered_inputs() {
        let a = between(None, None).unwrap();
        assert!(between(Some(&a), Some(&a)).is_none());
        let b = between(Some(&a), None).unwrap();
        assert!(between(Some(&b), Some(&a)).is_none());
    }
}
