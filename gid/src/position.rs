//! Element identity for GID list values, saved with a document, so a
//! path through a list survives a reload. A position is a byte string
//! read as the binary fraction `0.b₁b₂…`, so trailing zero bits are
//! value-neutral and plain lexicographic comparison — the derived
//! ordering — is the dense order. Canonical form: nonempty, final byte
//! nonzero; construction owns it. `between` always exists, so
//! relabeling is never required: appends and prepends step by one at
//! the first digit with room, growing a byte per 255, and inserts
//! between two elements bisect, growing about a bit per insert into the
//! same gap, the immutable-label side of the order-maintenance trade.
//! A list that has none, such as one written by hand, gets evenly
//! spread ones. Libraries may encode their bytes in ordinary values to
//! describe paths.
//!
//! `make verify` proves the digit arithmetic below `between` with Verus:
//! see docs/verification.md.

use std::sync::Arc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

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
    digits_between(low.map(Position::as_bytes), high.map(Position::as_bytes))
        .map(|digits| Position(digits.into()))
}

/// `n` canonical positions in increasing order, spread evenly across
/// the unit interval — minted for a sequence that has none yet, such
/// as a list written without them. Payloads stay near log₂₅₆ n bytes with balanced
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

verus! {

/// Nonempty with a nonzero final digit: the one spelling of its fraction.
pub open spec fn canonical(digits: Seq<u8>) -> bool {
    digits.len() > 0 && digits.last() != 0
}

/// `a` sorts before `b`, decided at `i`: they agree before it, and there
/// `a` ends while `b` goes on, or `a`'s digit is the smaller.
pub open spec fn precedes_at(a: Seq<u8>, b: Seq<u8>, i: int) -> bool {
    &&& 0 <= i <= a.len()
    &&& i <= b.len()
    &&& forall|j: int| 0 <= j < i ==> a[j] == b[j]
    &&& (i == a.len() && i < b.len()) || (i < a.len() && i < b.len() && a[i] < b[i])
}

/// Lexicographic order, which is how `Position`'s derived `Ord` compares.
pub open spec fn precedes(a: Seq<u8>, b: Seq<u8>) -> bool {
    exists|i: int| #[trigger] precedes_at(a, b, i)
}

/// The fraction's digit at `i`, reading zeros past the end.
pub open spec fn digit(digits: Seq<u8>, i: int) -> int {
    if 0 <= i < digits.len() {
        digits[i] as int
    } else {
        0
    }
}

/// The digits of a position strictly between `low` and `high`, `None`
/// being an open end, or `None` when both are given out of order.
fn digits_between(low: Option<&[u8]>, high: Option<&[u8]>) -> (out: Option<Vec<u8>>)
    requires
        low matches Some(low) ==> canonical(low@),
        high matches Some(high) ==> canonical(high@),
    ensures
        out is None <==> (low matches Some(low) && high matches Some(high) && !precedes(low@, high@)),
        out matches Some(out) ==> {
            &&& canonical(out@)
            &&& low matches Some(low) ==> precedes(low@, out@)
            &&& high matches Some(high) ==> precedes(out@, high@)
        },
{
    match (low, high) {
        (None, None) => Some(vec![0x80]),
        (Some(low), None) => Some(after(low)),
        (None, Some(high)) => Some(before(high)),
        (Some(low), Some(high)) => if in_order(low, high) {
            Some(bisect(low, high))
        } else {
            None
        },
    }
}

/// Whether `low` sorts strictly before `high`.
fn in_order(low: &[u8], high: &[u8]) -> (ordered: bool)
    ensures
        ordered == precedes(low@, high@),
{
    let mut i = 0;
    while i < low.len() && i < high.len() && low[i] == high[i]
        invariant
            i <= low.len(),
            i <= high.len(),
            forall|j: int| 0 <= j < i ==> low@[j] == high@[j],
        decreases low.len() - i,
    {
        i += 1;
    }
    let ordered = if i < low.len() && i < high.len() {
        low[i] < high[i]
    } else {
        i == low.len() && i < high.len()
    };
    proof {
        if ordered {
            assert(precedes_at(low@, high@, i as int));
        } else {
            assert forall|k: int| !precedes_at(low@, high@, k) by {
                if precedes_at(low@, high@, k) && k != i {
                    if k < i {
                        assert(low@[k] == high@[k]);
                    } else {
                        assert(low@[i as int] == high@[i as int]);
                    }
                }
            }
        }
    }
    ordered
}

/// A position after `low` with nothing beyond it: `low` stepped up by one
/// at its first digit with room, so appends take 255 steps at a depth before
/// growing a byte.
fn after(low: &[u8]) -> (out: Vec<u8>)
    requires
        canonical(low@),
    ensures
        canonical(out@),
        precedes(low@, out@),
{
    let mut room = 0;
    while room < low.len() && low[room] == 0xFF
        invariant
            room <= low.len(),
        decreases low.len() - room,
    {
        room += 1;
    }
    let mut out = Vec::new();
    if room < low.len() {
        out.extend_from_slice(&low[..room]);
        out.push(low[room] + 1);
    } else {
        out.extend_from_slice(low);
        out.push(1);
    }
    assert(precedes_at(low@, out@, room as int));
    out
}

/// A position before `high` with nothing below it: `high` stepped down by
/// one at its first digit with room, so prepends take 255 steps at a depth
/// before growing a byte. A `high` of only zero and one digits has no room
/// above zero; just below its first one is the canonical `…00 FF`.
fn before(high: &[u8]) -> (out: Vec<u8>)
    requires
        canonical(high@),
    ensures
        canonical(out@),
        precedes(out@, high@),
{
    let mut room = 0;
    while room < high.len() && high[room] <= 1
        invariant
            room <= high.len(),
            forall|j: int| 0 <= j < room ==> high@[j] <= 1,
        decreases high.len() - room,
    {
        room += 1;
    }
    let mut out = Vec::new();
    if room < high.len() {
        out.extend_from_slice(&high[..room]);
        out.push(high[room] - 1);
        assert(precedes_at(out@, high@, room as int));
    } else {
        let mut one = 0;
        while high[one] == 0
            invariant
                one < high.len(),
                forall|j: int| 0 <= j < high.len() ==> high@[j] <= 1,
                high@[high.len() - 1] != 0,
            decreases high.len() - one,
        {
            one += 1;
        }
        out.extend_from_slice(&high[..one]);
        out.push(0x00);
        out.push(0xFF);
        assert(precedes_at(out@, high@, one as int));
    }
    out
}

/// `digits`' digit at `i`, reading zeros past the end. It also tells its
/// caller that the length fits a `usize`, which bounds its indices.
fn digit_at(digits: &[u8], i: usize) -> (d: u8)
    ensures
        d == digit(digits@, i as int),
        digits@.len() <= usize::MAX,
{
    if i < digits.len() {
        digits[i]
    } else {
        0
    }
}

/// Digits strictly between `low` and `high`: their common digits, then
/// the midpoint of the first pair that differ by two or more. Adjacent
/// digits descend into `low`'s branch and pick the midpoint of what
/// remains above it, so growth stays near a bit per forced split rather
/// than a byte.
fn bisect(low: &[u8], high: &[u8]) -> (out: Vec<u8>)
    requires
        canonical(low@),
        canonical(high@),
        precedes(low@, high@),
    ensures
        canonical(out@),
        precedes(low@, out@),
        precedes(out@, high@),
{
    // The digits differ by `limit`: where the order is decided or, when
    // `low` is a prefix of `high`, at `high`'s nonzero final digit.
    let ghost decided = choose|i: int| precedes_at(low@, high@, i);
    let ghost limit = if decided < low@.len() { decided } else { high@.len() - 1 };
    assert(digit(low@, limit) != digit(high@, limit));
    let mut split = 0;
    let mut out = Vec::new();
    while digit_at(low, split) == digit_at(high, split)
        invariant
            split <= limit < high@.len(),
            digit(low@, limit) != digit(high@, limit),
            out@.len() == split,
            forall|j: int| 0 <= j < split ==> out@[j] == digit(low@, j) && out@[j] == digit(high@, j),
        decreases limit - split,
    {
        out.push(digit_at(low, split));
        split += 1;
    }
    let a = digit_at(low, split);
    let b = digit_at(high, split);
    assert(a < b) by {
        if split < decided {
            assert(low@[split as int] == high@[split as int]);
        }
    }
    if b - a >= 2 {
        out.push(((a as u16 + b as u16) / 2) as u8);
        if split < low.len() {
            assert(precedes_at(low@, out@, split as int));
        } else {
            assert(precedes_at(low@, out@, low@.len() as int));
        }
    } else {
        out.push(a);
        let mut room = split + 1;
        while digit_at(low, room) == 0xFF
            invariant
                split < room,
                out@.len() == room,
                forall|j: int| 0 <= j < room ==> out@[j] == digit(low@, j),
                forall|j: int| 0 <= j < split ==> out@[j] == digit(high@, j),
                out@[split as int] < digit(high@, split as int),
            decreases low.len() - room,
        {
            out.push(0xFF);
            room += 1;
        }
        out.push(((digit_at(low, room) as u16 + 0x100) / 2) as u8);
        if room < low.len() {
            assert(precedes_at(low@, out@, room as int));
        } else {
            assert(precedes_at(low@, out@, low@.len() as int));
        }
    }
    assert(precedes_at(out@, high@, split as int));
    out
}

} // verus!

#[cfg(test)]
#[path = "tests/position.rs"]
mod tests;
