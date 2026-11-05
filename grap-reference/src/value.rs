//! GID values as the definition reads them, and as the evaluator holds them,
//! with the cells Grap's core reads and writes.

use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A GID value as the definition reads it. A record maps cells to values,
/// whatever order its fields were written in. A list is its elements in
/// order: positions name elements for navigation and aren't part of a value,
/// as GID's own equality says.
#[cfg(verus_only)]
pub enum Value {
    Cell(u128),
    Blob(Seq<u8>),
    List(Seq<Value>),
    Record(Map<u128, Value>),
}

/// A GID value as the evaluator holds it. A record is its fields as pairs,
/// a later pair for a cell replacing an earlier one, as GID builds records,
/// so setting a field is a push. Copies share their contents.
pub enum ExecValue {
    Cell(u128),
    Blob(Rc<Vec<u8>>),
    List(Rc<Vec<ExecValue>>),
    Record(Rc<Vec<(u128, ExecValue)>>),
}

/// Pairs read as a map, a later pair for a cell replacing an earlier one.
pub open spec fn to_map(pairs: Seq<(u128, Value)>) -> Map<u128, Value>
    decreases pairs.len(),
{
    if pairs.len() == 0 {
        Map::empty()
    } else {
        to_map(pairs.drop_last()).insert(pairs.last().0, pairs.last().1)
    }
}

/// What an evaluator's value means.
pub open spec fn view(value: ExecValue) -> Value
    decreases value,
{
    match value {
        ExecValue::Cell(cell) => Value::Cell(cell),
        ExecValue::Blob(bytes) => Value::Blob(bytes@),
        ExecValue::List(elements) => Value::List(view_elements(elements@)),
        ExecValue::Record(fields) => Value::Record(to_map(view_pairs(fields@))),
    }
}

/// Elements' meanings, in order.
pub open spec fn view_elements(elements: Seq<ExecValue>) -> Seq<Value>
    decreases elements,
{
    Seq::new(
        elements.len(),
        |i: int|
            if 0 <= i < elements.len() {
                view(elements[i])
            } else {
                arbitrary()
            },
    )
}

/// Pairs' meanings, in order.
pub open spec fn view_pairs(pairs: Seq<(u128, ExecValue)>) -> Seq<(u128, Value)>
    decreases pairs,
{
    Seq::new(
        pairs.len(),
        |i: int|
            if 0 <= i < pairs.len() {
                (pairs[i].0, view(pairs[i].1))
            } else {
                arbitrary()
            },
    )
}

/// What pairs held by the evaluator mean as a record or an environment.
pub open spec fn view_map(pairs: Seq<(u128, ExecValue)>) -> Map<u128, Value> {
    to_map(view_pairs(pairs))
}

pub proof fn lemma_view_record(fields: Rc<Vec<(u128, ExecValue)>>)
    ensures
        view(ExecValue::Record(fields)) == Value::Record(view_map(fields@)),
{
}

pub proof fn lemma_view_pairs_push(pairs: Seq<(u128, ExecValue)>, pair: (u128, ExecValue))
    ensures
        view_pairs(pairs.push(pair)) == view_pairs(pairs).push((pair.0, view(pair.1))),
        view_map(pairs.push(pair)) == view_map(pairs).insert(pair.0, view(pair.1)),
{
    assert(view_pairs(pairs.push(pair)) =~= view_pairs(pairs).push((pair.0, view(pair.1))));
    assert(view_pairs(pairs.push(pair)).drop_last() =~= view_pairs(pairs));
}

pub proof fn lemma_view_pairs_prefix(pairs: Seq<(u128, ExecValue)>, n: int)
    requires
        0 <= n <= pairs.len(),
    ensures
        view_pairs(pairs.subrange(0, n)) == view_pairs(pairs).subrange(0, n),
{
    assert(view_pairs(pairs.subrange(0, n)) =~= view_pairs(pairs).subrange(0, n));
}

/// The value itself, sharing its contents.
pub fn copy(value: &ExecValue) -> (copied: ExecValue)
    ensures
        copied == *value,
{
    match value {
        ExecValue::Cell(cell) => ExecValue::Cell(*cell),
        ExecValue::Blob(bytes) => ExecValue::Blob(Rc::clone(bytes)),
        ExecValue::List(elements) => ExecValue::List(Rc::clone(elements)),
        ExecValue::Record(fields) => ExecValue::Record(Rc::clone(fields)),
    }
}

/// The pairs themselves, sharing their values' contents.
pub fn copy_pairs(pairs: &Vec<(u128, ExecValue)>) -> (copied: Vec<(u128, ExecValue)>)
    ensures
        copied@ == pairs@,
{
    let mut copied = Vec::new();
    let mut i = 0;
    while i < pairs.len()
        invariant
            i <= pairs.len(),
            copied@ == pairs@.subrange(0, i as int),
        decreases pairs.len() - i,
    {
        copied.push((pairs[i].0, copy(&pairs[i].1)));
        i += 1;
        assert(copied@ =~= pairs@.subrange(0, i as int));
    }
    assert(pairs@.subrange(0, pairs.len() as int) =~= pairs@);
    copied
}

/// The value under `cell`, when there is one: the last pair for it.
pub fn get(pairs: &Vec<(u128, ExecValue)>, cell: u128) -> (found: Option<&ExecValue>)
    ensures
        match found {
            Some(value) => view_map(pairs@).contains_key(cell) && view_map(pairs@)[cell] == view(
                *value,
            ),
            None => !view_map(pairs@).contains_key(cell),
        },
{
    let mut i = pairs.len();
    proof {
        assert(pairs@.subrange(0, pairs.len() as int) =~= pairs@);
    }
    while i > 0
        invariant
            i <= pairs.len(),
            forall|j: int| i <= j < pairs.len() ==> #[trigger] pairs@[j].0 != cell,
            view_map(pairs@).contains_key(cell) == view_map(pairs@.subrange(0, i as int)).contains_key(
                cell,
            ),
            view_map(pairs@.subrange(0, i as int)).contains_key(cell) ==> view_map(pairs@)[cell]
                == view_map(pairs@.subrange(0, i as int))[cell],
        decreases i,
    {
        proof {
            assert(pairs@.subrange(0, i as int) =~= pairs@.subrange(0, i - 1).push(pairs@[i - 1]));
            lemma_view_pairs_push(pairs@.subrange(0, i - 1), pairs@[i - 1]);
        }
        if pairs[i - 1].0 == cell {
            return Some(&pairs[i - 1].1);
        }
        i -= 1;
    }
    proof {
        assert(view_pairs(pairs@.subrange(0, 0)) =~= Seq::<(u128, Value)>::empty());
    }
    None
}

/// Whether there is a value under `cell`.
pub fn has(pairs: &Vec<(u128, ExecValue)>, cell: u128) -> (found: bool)
    ensures
        found == view_map(pairs@).contains_key(cell),
{
    get(pairs, cell).is_some()
}

/// The pairs without any for `cell`.
pub fn without(pairs: &Vec<(u128, ExecValue)>, cell: u128) -> (rest: Vec<(u128, ExecValue)>)
    ensures
        view_map(rest@) == view_map(pairs@).remove(cell),
{
    let mut rest: Vec<(u128, ExecValue)> = Vec::new();
    let mut i = 0;
    proof {
        assert(view_pairs(pairs@.subrange(0, 0)) =~= Seq::<(u128, Value)>::empty());
        assert(view_pairs(rest@) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(rest@) =~= view_map(pairs@.subrange(0, 0)).remove(cell));
    }
    while i < pairs.len()
        invariant
            i <= pairs.len(),
            view_map(rest@) == view_map(pairs@.subrange(0, i as int)).remove(cell),
        decreases pairs.len() - i,
    {
        let ghost before = rest@;
        proof {
            assert(pairs@.subrange(0, i + 1) =~= pairs@.subrange(0, i as int).push(pairs@[i as int]));
            lemma_view_pairs_push(pairs@.subrange(0, i as int), pairs@[i as int]);
        }
        if pairs[i].0 != cell {
            rest.push((pairs[i].0, copy(&pairs[i].1)));
            proof {
                lemma_view_pairs_push(before, rest@.last());
                assert(rest@ =~= before.push(rest@.last()));
                assert(view_map(rest@) =~= view_map(pairs@.subrange(0, i + 1)).remove(cell));
            }
        } else {
            proof {
                assert(view_map(rest@) =~= view_map(pairs@.subrange(0, i + 1)).remove(cell));
            }
        }
        i += 1;
    }
    proof {
        assert(pairs@.subrange(0, pairs.len() as int) =~= pairs@);
    }
    rest
}

/// `{absent: reason, field: detail}`.
pub open spec fn absent(reason: u128, field: u128, detail: Value) -> Value {
    Value::Record(map![ABSENT => Value::Cell(reason), field => detail])
}

/// `{absent: fuel exhausted}`, what a program that runs out of fuel comes to.
pub open spec fn fuel_exhausted() -> Value {
    Value::Record(map![ABSENT => Value::Cell(FUEL_EXHAUSTED)])
}

/// Whether `value` is an absent: a record naming its reason with a cell.
pub open spec fn is_absent(value: Value) -> bool {
    value matches Value::Record(fields) && fields.contains_key(ABSENT) && fields[ABSENT] is Cell
}

pub fn exec_absent(reason: u128, field: u128, detail: ExecValue) -> (made: ExecValue)
    requires
        field != ABSENT,
    ensures
        view(made) == absent(reason, field, view(detail)),
{
    let mut fields = Vec::new();
    fields.push((ABSENT, ExecValue::Cell(reason)));
    fields.push((field, detail));
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
        lemma_view_pairs_push(seq![], (ABSENT, ExecValue::Cell(reason)));
        lemma_view_pairs_push(seq![(ABSENT, ExecValue::Cell(reason))], (field, detail));
        assert(seq![(ABSENT, ExecValue::Cell(reason))] =~= Seq::<(u128, ExecValue)>::empty().push(
            (ABSENT, ExecValue::Cell(reason)),
        ));
        assert(fields@ =~= seq![(ABSENT, ExecValue::Cell(reason))].push((field, detail)));
        reveal_with_fuel(view, 2);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(fields@) =~= map![ABSENT => Value::Cell(reason), field => view(detail)]);
    }
    ExecValue::Record(fields)
}

pub fn exec_fuel_exhausted() -> (made: ExecValue)
    ensures
        view(made) == fuel_exhausted(),
{
    let mut fields = Vec::new();
    fields.push((ABSENT, ExecValue::Cell(FUEL_EXHAUSTED)));
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
        lemma_view_pairs_push(seq![], (ABSENT, ExecValue::Cell(FUEL_EXHAUSTED)));
        assert(fields@ =~= Seq::<(u128, ExecValue)>::empty().push(
            (ABSENT, ExecValue::Cell(FUEL_EXHAUSTED)),
        ));
        reveal_with_fuel(view, 2);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(fields@) =~= map![ABSENT => Value::Cell(FUEL_EXHAUSTED)]);
    }
    ExecValue::Record(fields)
}

pub fn exec_is_absent(value: &ExecValue) -> (absent: bool)
    ensures
        absent == is_absent(view(*value)),
{
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            match get(fields, ABSENT) {
                Some(ExecValue::Cell(_)) => true,
                _ => false,
            }
        },
        _ => false,
    }
}

// The cells Grap's evaluator reads and writes, the same identities as in
// grap's vocabulary, absent and source modules.

pub const FUNCTION: u128 = 0x751fca4373debdd0b7e6eb73e08d684b;

pub const PARAMS: u128 = 0x195b378d0d31d90ab0d7366c15346b70;

pub const BODY: u128 = 0x986143866eda2e2fbf9ab8484357a0c9;

pub const VALUE: u128 = 0x5adde9ececa6ad57c81a3b2907e75e08;

pub const CLOSURE: u128 = 0xdb39600f3ed07398c77ac120deb108a8;

pub const ENVIRONMENT: u128 = 0xe910025c710c25c43d0a5b296378f374;

pub const FFI: u128 = 0x912adb7252d689659b6de9eeeb827658;

pub const BODY_ORIGIN: u128 = 0x215831fe58bde294e79cccbc74212318;

pub const ABSENT: u128 = 0xd9c0a7145a38859a245640d3469cbcd4;

/// The field an absent names a cell under.
pub const ABSENT_CELL: u128 = 0x7ba69298ef92cf954456784b66e86032;

/// The field an absent carries a value under.
pub const ABSENT_VALUE: u128 = 0xcfeaf191b71e06afef86c27ee43eb936;

pub const CYCLE: u128 = 0x2776d28585d66ef8a747a8bc6dad7381;

pub const FUEL_EXHAUSTED: u128 = 0x513628d759c04b3e7088b575e555a80e;

pub const MISSING_CELL: u128 = 0xa5a1b4e3d0df96bd11af00f0780136ff;

pub const CELL_CYCLE: u128 = 0x150e0fc7e38d1670f41283c3d23a9b8d;

pub const MALFORMED_LAMBDA: u128 = 0xfbf5894d7f62b6d0048d17d26851b415;

pub const INVALID_PARAMETER: u128 = 0x93ca0e9199372c46ee24bd4c508e178c;

pub const NOT_CALLABLE: u128 = 0x8624488c2d10d2a4b84560dfa99a38e6;

pub const MISSING_ARGUMENT: u128 = 0x8b2f0db36e5c3d35595eb5666cc89c78;

} // verus!
