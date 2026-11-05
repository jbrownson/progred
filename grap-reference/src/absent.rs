//! The definition's rule for absents caused by several details, as code that
//! runs, proven to compute what `definition::caused_by_all` says.
//!
//! Comparing two chains of causes doesn't count them: `as_long` walks both
//! together, so no count can overflow, however deep an absent a program
//! writes.

#[cfg(verus_only)]
use crate::definition;
use crate::definition::CHAIN_LIMIT;
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A chain at least `m` long is at least any shorter length too.
pub proof fn lemma_chain_shorter(value: Value, n: nat, m: nat)
    requires
        n <= m,
        definition::chain_at_least(value, m),
    ensures
        definition::chain_at_least(value, n),
    decreases n,
{
    if n >= 2 {
        let fields = value->Record_0;
        let field = choose|field: u128|
            field != ABSENT && #[trigger] fields.contains_key(field)
                && definition::chain_at_least(fields[field], (m - 1) as nat);
        lemma_chain_shorter(fields[field], (n - 1) as nat, (m - 1) as nat);
    }
}

/// Whether `value`'s causes hold a chain of at least `n` absents.
pub fn chain_at_least(value: &ExecValue, n: usize) -> (holds: bool)
    ensures
        holds == definition::chain_at_least(view(*value), n as nat),
    decreases n,
{
    if n == 0 {
        return true;
    }
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            if !exec_is_absent(value) {
                return false;
            }
            if n == 1 {
                return true;
            }
            let mut i = 0;
            while i < fields.len()
                invariant
                    2 <= n,
                    i <= fields@.len(),
                    view(*value) == Value::Record(view_map(fields@)),
                    is_absent(view(*value)),
                    forall|k: int|
                        0 <= k < i && (#[trigger] fields@[k]).0 != ABSENT
                            ==> !definition::chain_at_least(
                            view_map(fields@)[fields@[k].0],
                            (n - 1) as nat,
                        ),
                decreases fields.len() - i,
            {
                let cell = fields[i].0;
                if cell != ABSENT {
                    proof {
                        lemma_view_map_keys(fields@, cell);
                        assert(fields@[i as int].0 == cell);
                    }
                    match get_index(fields, cell) {
                        Some(at) => {
                            if chain_at_least(&fields[at].1, n - 1) {
                                return true;
                            }
                        },
                        None => {},
                    }
                }
                i += 1;
            }
            proof {
                assert forall|cell: u128|
                    cell != ABSENT && #[trigger] view_map(fields@).contains_key(
                        cell,
                    ) implies !definition::chain_at_least(
                    view_map(fields@)[cell],
                    (n - 1) as nat,
                ) by {
                    lemma_view_map_keys(fields@, cell);
                    let k = choose|k: int| 0 <= k < fields@.len() && #[trigger] fields@[k].0 == cell;
                    assert(fields@[k].0 != ABSENT);
                }
            }
            false
        },
        _ => false,
    }
}

/// Whether `value`'s causes include one at least as long as `other`'s
/// detail under `cell`, when that detail is a cause.
pub open spec fn matched(value: Map<u128, Value>, other: Map<u128, Value>, cell: u128) -> bool {
    cell != ABSENT && is_absent(other[cell]) ==> exists|field: u128|
        field != ABSENT && #[trigger] value.contains_key(field) && definition::as_long(
            value[field],
            other[cell],
        )
}

/// Whether `value`'s longest chain of causes is at least as long as
/// `other`'s: an absent's is, over anything else's; otherwise each cause of
/// `other`'s needs a cause of `value`'s at least as long.
#[verifier::loop_isolation(false)]
pub fn as_long(value: &ExecValue, other: &ExecValue) -> (holds: bool)
    ensures
        holds == definition::as_long(view(*value), view(*other)),
    decreases *other,
{
    if !exec_is_absent(other) {
        proof {
            assert forall|n: nat| #[trigger]
                definition::chain_at_least(view(*other), n) implies definition::chain_at_least(
                view(*value),
                n,
            ) by {
                assert(n == 0);
            }
        }
        return true;
    }
    if !exec_is_absent(value) {
        proof {
            assert(definition::chain_at_least(view(*other), 1));
            assert(!definition::chain_at_least(view(*value), 1));
        }
        return false;
    }
    match other {
        ExecValue::Record(others) => match value {
            ExecValue::Record(fields) => {
                proof {
                    lemma_view_record(*fields);
                    lemma_view_record(*others);
                }
                let ghost mine = view_map(fields@);
                let ghost theirs = view_map(others@);
                let mut j = 0;
                while j < others.len()
                    invariant
                        *other == ExecValue::Record(*others),
                        j <= others@.len(),
                        mine == view_map(fields@),
                        theirs == view_map(others@),
                        view(*value) == Value::Record(mine),
                        view(*other) == Value::Record(theirs),
                        is_absent(view(*value)),
                        is_absent(view(*other)),
                        forall|k: int| 0 <= k < j ==> matched(mine, theirs, (#[trigger] others@[k]).0),
                    decreases others.len() - j,
                {
                    let cell = others[j].0;
                    proof {
                        lemma_view_map_keys(others@, cell);
                        assert(others@[j as int].0 == cell);
                    }
                    let at = match get_index(others, cell) {
                        Some(at) => at,
                        None => {
                            proof {
                                assert(false);
                            }
                            return false;
                        },
                    };
                    if cell != ABSENT && exec_is_absent(&others[at].1) {
                        let ghost cause = theirs[cell];
                        let mut found = false;
                        let ghost mut bound: nat = 1;
                        let mut i = 0;
                        while i < fields.len() && !found
                            invariant
                                *other == ExecValue::Record(*others),
                                at < others@.len(),
                                i <= fields@.len(),
                                mine == view_map(fields@),
                                cause == view(others@[at as int].1),
                                is_absent(cause),
                                found ==> exists|field: u128|
                                    field != ABSENT && #[trigger] mine.contains_key(field)
                                        && definition::as_long(mine[field], cause),
                                !found ==> bound >= 1 && definition::chain_at_least(cause, bound),
                                !found ==> forall|k: int|
                                    0 <= k < i && (#[trigger] fields@[k]).0 != ABSENT
                                        ==> !definition::chain_at_least(mine[fields@[k].0], bound),
                            decreases fields.len() - i,
                        {
                            let field = fields[i].0;
                            if field != ABSENT {
                                proof {
                                    lemma_view_map_keys(fields@, field);
                                    assert(fields@[i as int].0 == field);
                                }
                                match get_index(fields, field) {
                                    Some(here) => {
                                        if as_long(&fields[here].1, &others[at].1) {
                                            found = true;
                                        } else {
                                            proof {
                                                let detail = mine[field];
                                                let n = choose|n: nat|
                                                    #[trigger] definition::chain_at_least(cause, n)
                                                        && !definition::chain_at_least(detail, n);
                                                let previous = bound;
                                                if n > bound {
                                                    bound = n;
                                                }
                                                assert forall|k: int|
                                                    0 <= k <= i && (#[trigger] fields@[k]).0
                                                        != ABSENT implies !definition::chain_at_least(
                                                    mine[fields@[k].0],
                                                    bound,
                                                ) by {
                                                    if k < i {
                                                        if definition::chain_at_least(
                                                            mine[fields@[k].0],
                                                            bound,
                                                        ) {
                                                            lemma_chain_shorter(
                                                                mine[fields@[k].0],
                                                                previous,
                                                                bound,
                                                            );
                                                        }
                                                    } else {
                                                        if definition::chain_at_least(detail, bound) {
                                                            lemma_chain_shorter(detail, n, bound);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    None => {},
                                }
                            }
                            i += 1;
                        }
                        if !found {
                            proof {
                                let n = bound + 1;
                                assert(definition::chain_at_least(view(*other), n)) by {
                                    assert(theirs.contains_key(cell));
                                }
                                assert forall|field: u128|
                                    field != ABSENT && #[trigger] mine.contains_key(
                                        field,
                                    ) implies !definition::chain_at_least(mine[field], bound) by {
                                    lemma_view_map_keys(fields@, field);
                                    let k = choose|k: int|
                                        0 <= k < fields@.len() && #[trigger] fields@[k].0 == field;
                                    assert(fields@[k].0 != ABSENT);
                                }
                                assert(!definition::chain_at_least(view(*value), n));
                            }
                            return false;
                        }
                    }
                    j += 1;
                }
                proof {
                    assert forall|n: nat| #[trigger]
                        definition::chain_at_least(view(*other), n) implies definition::chain_at_least(
                        view(*value),
                        n,
                    ) by {
                        if n >= 2 {
                            let cell = choose|cell: u128|
                                cell != ABSENT && #[trigger] theirs.contains_key(cell)
                                    && definition::chain_at_least(theirs[cell], (n - 1) as nat);
                            lemma_view_map_keys(others@, cell);
                            let k = choose|k: int|
                                0 <= k < others@.len() && #[trigger] others@[k].0 == cell;
                            assert(matched(mine, theirs, others@[k].0));
                            let field = choose|field: u128|
                                field != ABSENT && #[trigger] mine.contains_key(field)
                                    && definition::as_long(mine[field], theirs[cell]);
                            assert(definition::chain_at_least(mine[field], (n - 1) as nat));
                        }
                    }
                }
                true
            },
            _ => {
                proof {
                    assert(false);
                }
                false
            },
        },
        _ => {
            proof {
                assert(false);
            }
            false
        },
    }
}

/// The details without an absent the same as one kept before it.
fn distinct(details: &Vec<(u128, ExecValue)>) -> (kept: Vec<(u128, ExecValue)>)
    ensures
        view_pairs(kept@) == definition::distinct(view_pairs(details@)),
{
    let mut kept: Vec<(u128, ExecValue)> = Vec::new();
    let mut i = 0;
    proof {
        assert(view_pairs(details@.subrange(0, 0)) =~= Seq::<(u128, Value)>::empty());
        assert(view_pairs(kept@) =~= Seq::<(u128, Value)>::empty());
    }
    while i < details.len()
        invariant
            i <= details@.len(),
            view_pairs(kept@) == definition::distinct(view_pairs(details@.subrange(0, i as int))),
        decreases details.len() - i,
    {
        let ghost before = details@.subrange(0, i as int);
        let ghost spec_kept = view_pairs(kept@);
        proof {
            assert(details@.subrange(0, i + 1) =~= before.push(details@[i as int]));
            lemma_view_pairs_push(before, details@[i as int]);
            assert(view_pairs(details@.subrange(0, i + 1)).drop_last() =~= view_pairs(before));
        }
        let detail = &details[i].1;
        let mut seen = false;
        if exec_is_absent(detail) {
            let mut j = 0;
            while j < kept.len()
                invariant
                    j <= kept@.len(),
                    spec_kept == view_pairs(kept@),
                    seen == exists|k: int|
                        0 <= k < j && (#[trigger] spec_kept[k]).1 == view(*detail),
                decreases kept.len() - j,
            {
                if !seen {
                    if same_value(&kept[j].1, detail) {
                        seen = true;
                        proof {
                            assert(spec_kept[j as int].1 == view(*detail));
                        }
                    } else {
                        proof {
                            assert(spec_kept[j as int].1 != view(*detail));
                            assert forall|k: int| 0 <= k < j + 1 implies (#[trigger] spec_kept[k]).1
                                != view(*detail) by {
                                if k == j {
                                } else {
                                }
                            }
                        }
                    }
                } else {
                    proof {
                        let k = choose|k: int|
                            0 <= k < j && (#[trigger] spec_kept[k]).1 == view(*detail);
                        assert(0 <= k < j + 1);
                    }
                }
                j += 1;
            }
        }
        if !seen {
            let pair = (details[i].0, copy(detail));
            proof {
                lemma_view_pairs_push(kept@, pair);
            }
            kept.push(pair);
        }
        i += 1;
    }
    proof {
        assert(details@.subrange(0, details@.len() as int) =~= details@);
    }
    kept
}

/// Where among `details` the longest chain of causes is.
fn longest(details: &Vec<(u128, ExecValue)>) -> (at: usize)
    requires
        details.len() > 0,
    ensures
        at < details.len(),
        at as int == definition::longest(view_pairs(details@)),
{
    let mut at = 0;
    let mut i = 1;
    proof {
        assert(view_pairs(details@.subrange(0, 1)).len() == 1);
    }
    while i < details.len()
        invariant
            1 <= i <= details@.len(),
            at < i,
            at as int == definition::longest(view_pairs(details@.subrange(0, i as int))),
        decreases details.len() - i,
    {
        let ghost before = details@.subrange(0, i as int);
        proof {
            assert(details@.subrange(0, i + 1) =~= before.push(details@[i as int]));
            lemma_view_pairs_push(before, details@[i as int]);
            assert(view_pairs(details@.subrange(0, i + 1)).drop_last() =~= view_pairs(before));
        }
        if as_long(&details[i].1, &details[at].1) {
            at = i;
        }
        i += 1;
    }
    proof {
        assert(details@.subrange(0, details@.len() as int) =~= details@);
    }
    at
}

/// An absent for `reason`, carrying each detail under its field, as
/// `definition::caused_by_all` says.
pub fn caused_by_all(reason: u128, details: &Vec<(u128, ExecValue)>) -> (made: ExecValue)
    ensures
        view(made) == definition::caused_by_all(reason, view_pairs(details@)),
{
    let kept = distinct(details);
    if kept.len() > 0 {
        let at = longest(&kept);
        if chain_at_least(&kept[at].1, CHAIN_LIMIT) {
            return copy(&kept[at].1);
        }
    }
    let mut fields: Vec<(u128, ExecValue)> = Vec::new();
    let first = (ABSENT, ExecValue::Cell(reason));
    proof {
        lemma_view_pairs_push(fields@, first);
    }
    fields.push(first);
    let ghost head = seq![(ABSENT, Value::Cell(reason))];
    proof {
        reveal_with_fuel(view, 2);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_pairs(fields@) =~= head + view_pairs(kept@).subrange(0, 0));
    }
    let mut i = 0;
    while i < kept.len()
        invariant
            i <= kept@.len(),
            view_pairs(fields@) == head + view_pairs(kept@).subrange(0, i as int),
        decreases kept.len() - i,
    {
        let pair = (kept[i].0, copy(&kept[i].1));
        proof {
            lemma_view_pairs_push(fields@, pair);
        }
        fields.push(pair);
        i += 1;
        proof {
            assert(view_pairs(fields@) =~= head + view_pairs(kept@).subrange(0, i as int));
        }
    }
    proof {
        assert(view_pairs(kept@).subrange(0, kept@.len() as int) =~= view_pairs(kept@));
    }
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
    }
    ExecValue::Record(fields)
}

} // verus!
