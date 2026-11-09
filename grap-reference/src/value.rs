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

pub proof fn lemma_view_elements_push(elements: Seq<ExecValue>, element: ExecValue)
    ensures
        view_elements(elements.push(element)) == view_elements(elements).push(view(element)),
{
    assert(view_elements(elements.push(element)) =~= view_elements(elements).push(view(element)));
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

/// Whether two values are the same, as GID compares them.
pub fn same_value(a: &ExecValue, b: &ExecValue) -> (same: bool)
    ensures
        same == (view(*a) == view(*b)),
    decreases *a,
{
    match a {
        ExecValue::Cell(x) => match b {
            ExecValue::Cell(y) => *x == *y,
            _ => false,
        },
        ExecValue::Blob(x) => match b {
            ExecValue::Blob(y) => {
                proof {
                    assert(view(*a) == Value::Blob(x@));
                    assert(view(*b) == Value::Blob(y@));
                }
                if x.len() != y.len() {
                    return false;
                }
                let mut i = 0;
                while i < x.len()
                    invariant
                        view(*a) == Value::Blob(x@),
                        view(*b) == Value::Blob(y@),
                        x@.len() == y@.len(),
                        i <= x@.len(),
                        forall|j: int| 0 <= j < i ==> x@[j] == y@[j],
                    decreases x.len() - i,
                {
                    if x[i] != y[i] {
                        proof {
                            assert(x@[i as int] != y@[i as int]);
                        }
                        return false;
                    }
                    i += 1;
                }
                proof {
                    assert(x@ =~= y@);
                }
                true
            },
            _ => false,
        },
        ExecValue::List(x) => match b {
            ExecValue::List(y) => {
                proof {
                    assert(view(*a) == Value::List(view_elements(x@)));
                    assert(view(*b) == Value::List(view_elements(y@)));
                }
                if x.len() != y.len() {
                    proof {
                        assert(view_elements(x@).len() != view_elements(y@).len());
                    }
                    return false;
                }
                let mut i = 0;
                while i < x.len()
                    invariant
                        *a == ExecValue::List(*x),
                        view(*a) == Value::List(view_elements(x@)),
                        view(*b) == Value::List(view_elements(y@)),
                        x@.len() == y@.len(),
                        i <= x@.len(),
                        forall|j: int| 0 <= j < i ==> view(#[trigger] x@[j]) == view(y@[j]),
                    decreases x.len() - i,
                {
                    if !same_value(&x[i], &y[i]) {
                        proof {
                            assert(view_elements(x@)[i as int] != view_elements(y@)[i as int]);
                        }
                        return false;
                    }
                    i += 1;
                }
                proof {
                    assert(view_elements(x@) =~= view_elements(y@));
                }
                true
            },
            _ => false,
        },
        ExecValue::Record(x) => match b {
            ExecValue::Record(y) => {
                proof {
                    lemma_view_record(*x);
                    lemma_view_record(*y);
                }
                let mut i = 0;
                while i < x.len()
                    invariant
                        *a == ExecValue::Record(*x),
                        i <= x@.len(),
                        view(*a) == Value::Record(view_map(x@)),
                        view(*b) == Value::Record(view_map(y@)),
                        forall|j: int|
                            0 <= j < i ==> view_map(y@).contains_key((#[trigger] x@[j]).0)
                                && view_map(x@)[x@[j].0] == view_map(y@)[x@[j].0],
                    decreases x.len() - i,
                {
                    let cell = x[i].0;
                    proof {
                        lemma_view_map_keys(x@, cell);
                        assert(x@[i as int].0 == cell);
                    }
                    match get_index(x, cell) {
                        Some(here) => match get_index(y, cell) {
                            Some(there) => {
                                if !same_value(&x[here].1, &y[there].1) {
                                    return false;
                                }
                            },
                            None => {
                                return false;
                            },
                        },
                        None => {
                            return false;
                        },
                    }
                    i += 1;
                }
                let mut j = 0;
                while j < y.len()
                    invariant
                        j <= y@.len(),
                        view(*a) == Value::Record(view_map(x@)),
                        view(*b) == Value::Record(view_map(y@)),
                        forall|k: int| 0 <= k < j ==> view_map(x@).contains_key((#[trigger] y@[k]).0),
                    decreases y.len() - j,
                {
                    let present = has(x, y[j].0);
                    if !present {
                        proof {
                            lemma_view_map_keys(y@, y@[j as int].0);
                        }
                        return false;
                    }
                    j += 1;
                }
                proof {
                    assert forall|cell: u128| #[trigger]
                        view_map(x@).contains_key(cell) implies view_map(y@).contains_key(cell)
                        && view_map(x@)[cell] == view_map(y@)[cell] by {
                        lemma_view_map_keys(x@, cell);
                        let k = choose|k: int| 0 <= k < x@.len() && #[trigger] x@[k].0 == cell;
                        assert(view_map(y@).contains_key(x@[k].0));
                    }
                    assert forall|cell: u128| #[trigger]
                        view_map(y@).contains_key(cell) implies view_map(x@).contains_key(cell) by {
                        lemma_view_map_keys(y@, cell);
                        let k = choose|k: int| 0 <= k < y@.len() && #[trigger] y@[k].0 == cell;
                        assert(view_map(x@).contains_key(y@[k].0));
                    }
                    assert(view_map(x@) =~= view_map(y@));
                }
                true
            },
            _ => false,
        },
    }
}

/// `{field: value}`.
pub fn exec_single(field: u128, value: ExecValue) -> (made: ExecValue)
    ensures
        view(made) == Value::Record(map![field => view(value)]),
{
    let mut fields = Vec::new();
    fields.push((field, value));
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
        lemma_view_pairs_push(seq![], (field, value));
        assert(fields@ =~= Seq::<(u128, ExecValue)>::empty().push((field, value)));
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(fields@) =~= map![field => view(value)]);
    }
    ExecValue::Record(fields)
}

/// `{first: a, second: b}`.
pub fn exec_pair(first: u128, a: ExecValue, second: u128, b: ExecValue) -> (made: ExecValue)
    ensures
        view(made) == Value::Record(map![first => view(a), second => view(b)]),
{
    let mut fields = Vec::new();
    proof {
        lemma_view_pairs_push(fields@, (first, a));
    }
    fields.push((first, a));
    proof {
        lemma_view_pairs_push(fields@, (second, b));
    }
    fields.push((second, b));
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(fields@) =~= map![first => view(a), second => view(b)]);
    }
    ExecValue::Record(fields)
}

/// `{}`.
pub fn exec_empty() -> (made: ExecValue)
    ensures
        view(made) == Value::Record(Map::empty()),
{
    let fields: Rc<Vec<(u128, ExecValue)>> = Rc::new(Vec::new());
    proof {
        lemma_view_record(fields);
        assert(view_pairs(fields@) =~= Seq::<(u128, Value)>::empty());
        assert(view_map(fields@) =~= Map::<u128, Value>::empty());
    }
    ExecValue::Record(fields)
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

/// Where the value under `cell` is, when there is one: the last pair for it.
pub fn get_index(pairs: &Vec<(u128, ExecValue)>, cell: u128) -> (found: Option<usize>)
    ensures
        match found {
            Some(at) => at < pairs.len() && pairs@[at as int].0 == cell && view_map(
                pairs@,
            ).contains_key(cell) && view_map(pairs@)[cell] == view(pairs@[at as int].1),
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
            return Some(i - 1);
        }
        i -= 1;
    }
    proof {
        assert(view_pairs(pairs@.subrange(0, 0)) =~= Seq::<(u128, Value)>::empty());
    }
    None
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
    match get_index(pairs, cell) {
        Some(at) => Some(&pairs[at].1),
        None => None,
    }
}

/// A record's fields are the cells its pairs name.
pub proof fn lemma_view_map_keys(pairs: Seq<(u128, ExecValue)>, cell: u128)
    ensures
        view_map(pairs).contains_key(cell) <==> exists|j: int|
            0 <= j < pairs.len() && #[trigger] pairs[j].0 == cell,
    decreases pairs.len(),
{
    if pairs.len() == 0 {
        assert(view_pairs(pairs) =~= Seq::<(u128, Value)>::empty());
    } else {
        let init = pairs.drop_last();
        assert(pairs =~= init.push(pairs.last()));
        lemma_view_pairs_push(init, pairs.last());
        lemma_view_map_keys(init, cell);
        if view_map(pairs).contains_key(cell) {
            if pairs.last().0 == cell {
                assert(pairs[pairs.len() - 1].0 == cell);
            } else {
                let j = choose|j: int| 0 <= j < init.len() && #[trigger] init[j].0 == cell;
                assert(pairs[j] == init[j]);
            }
        }
        if exists|j: int| 0 <= j < pairs.len() && #[trigger] pairs[j].0 == cell {
            let j = choose|j: int| 0 <= j < pairs.len() && #[trigger] pairs[j].0 == cell;
            if j < pairs.len() - 1 {
                assert(init[j] == pairs[j]);
            }
        }
    }
}

/// A map built from pairs has a cell when some pair names it.
pub proof fn lemma_to_map_keys(pairs: Seq<(u128, Value)>, cell: u128)
    ensures
        to_map(pairs).contains_key(cell) <==> exists|i: int|
            0 <= i < pairs.len() && #[trigger] pairs[i].0 == cell,
    decreases pairs.len(),
{
    if pairs.len() > 0 {
        let init = pairs.drop_last();
        lemma_to_map_keys(init, cell);
        if to_map(pairs).contains_key(cell) && pairs.last().0 != cell {
            let i = choose|i: int| 0 <= i < init.len() && #[trigger] init[i].0 == cell;
            assert(pairs[i] == init[i]);
        }
        if exists|i: int| 0 <= i < pairs.len() && #[trigger] pairs[i].0 == cell {
            let i = choose|i: int| 0 <= i < pairs.len() && #[trigger] pairs[i].0 == cell;
            if i < pairs.len() - 1 {
                assert(init[i] == pairs[i]);
            }
        }
    }
}

/// What a map built from pairs holds under a cell is its last pair's.
pub proof fn lemma_to_map_last(pairs: Seq<(u128, Value)>, i: int)
    requires
        0 <= i < pairs.len(),
        forall|j: int| i < j < pairs.len() ==> #[trigger] pairs[j].0 != pairs[i].0,
    ensures
        to_map(pairs).contains_key(pairs[i].0),
        to_map(pairs)[pairs[i].0] == pairs[i].1,
    decreases pairs.len(),
{
    if i < pairs.len() - 1 {
        let init = pairs.drop_last();
        assert(init[i] == pairs[i]);
        assert forall|j: int| i < j < init.len() implies #[trigger] init[j].0 != init[i].0 by {
            assert(init[j] == pairs[j]);
        }
        lemma_to_map_last(init, i);
    }
}

/// Later pairs replace what they name.
pub proof fn lemma_to_map_concat(first: Seq<(u128, Value)>, second: Seq<(u128, Value)>)
    ensures
        to_map(first + second) == to_map(first).union_prefer_right(to_map(second)),
    decreases second.len(),
{
    if second.len() == 0 {
        assert(first + second =~= first);
        assert(to_map(first).union_prefer_right(to_map(second)) =~= to_map(first));
    } else {
        lemma_to_map_concat(first, second.drop_last());
        assert((first + second).drop_last() =~= first + second.drop_last());
        assert((first + second).last() == second.last());
        assert(to_map(first + second) =~= to_map(first).union_prefer_right(to_map(second)));
    }
}

/// Whether there is a value under `cell`.
pub fn has(pairs: &Vec<(u128, ExecValue)>, cell: u128) -> (found: bool)
    ensures
        found == view_map(pairs@).contains_key(cell),
{
    get(pairs, cell).is_some()
}

/// The fields an absent for `reason` starts with: `{isa: absent, reason}`.
pub open spec fn absent_header(reason: u128) -> Seq<(u128, Value)> {
    seq![(ISA, Value::Cell(KIND_ABSENT)), (REASON, Value::Cell(reason))]
}

/// An absent for `reason` carrying `details`: `{isa: absent, reason,
/// ...details}`, a later detail under a field replacing an earlier one, as
/// GID builds records.
pub open spec fn absent(reason: u128, details: Seq<(u128, Value)>) -> Value {
    Value::Record(to_map(absent_header(reason) + details))
}

/// An absent for `reason` carrying one detail.
pub open spec fn with_detail(reason: u128, field: u128, detail: Value) -> Value {
    absent(reason, seq![(field, detail)])
}

/// Whether `value` is an absent: a record whose kind is absent, naming its
/// reason with a cell.
pub open spec fn is_absent(value: Value) -> bool {
    value matches Value::Record(fields) && fields.contains_key(ISA) && fields[ISA] == Value::Cell(
        KIND_ABSENT,
    ) && fields.contains_key(REASON) && fields[REASON] is Cell
}

/// An absent for `reason` carrying `details`, as `absent` says.
pub fn exec_absent(reason: u128, details: &Vec<(u128, ExecValue)>) -> (made: ExecValue)
    ensures
        view(made) == absent(reason, view_pairs(details@)),
{
    let mut fields: Vec<(u128, ExecValue)> = Vec::new();
    let kind = (ISA, ExecValue::Cell(KIND_ABSENT));
    proof {
        lemma_view_pairs_push(fields@, kind);
    }
    fields.push(kind);
    let named = (REASON, ExecValue::Cell(reason));
    proof {
        lemma_view_pairs_push(fields@, named);
    }
    fields.push(named);
    let ghost head = absent_header(reason);
    proof {
        reveal_with_fuel(view, 2);
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(view_pairs(fields@) =~= head + view_pairs(details@).subrange(0, 0));
    }
    let mut i = 0;
    while i < details.len()
        invariant
            i <= details@.len(),
            view_pairs(fields@) == head + view_pairs(details@).subrange(0, i as int),
        decreases details.len() - i,
    {
        let pair = (details[i].0, copy(&details[i].1));
        proof {
            lemma_view_pairs_push(fields@, pair);
        }
        fields.push(pair);
        i += 1;
        proof {
            assert(view_pairs(fields@) =~= head + view_pairs(details@).subrange(0, i as int));
        }
    }
    proof {
        assert(view_pairs(details@).subrange(0, details@.len() as int) =~= view_pairs(details@));
    }
    let fields = Rc::new(fields);
    proof {
        lemma_view_record(fields);
    }
    ExecValue::Record(fields)
}

/// An absent for `reason` carrying one detail, as `with_detail` says.
pub fn exec_with_detail(reason: u128, field: u128, detail: ExecValue) -> (made: ExecValue)
    ensures
        view(made) == with_detail(reason, field, view(detail)),
{
    let mut details = Vec::new();
    proof {
        lemma_view_pairs_push(details@, (field, detail));
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
    }
    details.push((field, detail));
    proof {
        assert(view_pairs(details@) =~= seq![(field, view(detail))]);
    }
    exec_absent(reason, &details)
}

/// `{isa: absent, reason}`, an absent carrying nothing more.
pub fn exec_bare_absent(reason: u128) -> (made: ExecValue)
    ensures
        view(made) == absent(reason, Seq::empty()),
{
    let details: Vec<(u128, ExecValue)> = Vec::new();
    proof {
        assert(view_pairs(details@) =~= Seq::<(u128, Value)>::empty());
    }
    exec_absent(reason, &details)
}

/// The kind `fields` say they are: the cell under `isa`.
pub open spec fn kind(fields: Map<u128, Value>) -> Option<u128> {
    if fields.contains_key(ISA) {
        match fields[ISA] {
            Value::Cell(kind) => Some(kind),
            _ => None,
        }
    } else {
        None
    }
}

pub fn exec_kind(fields: &Vec<(u128, ExecValue)>) -> (found: Option<u128>)
    ensures
        found == kind(view_map(fields@)),
{
    match get(fields, ISA) {
        Some(ExecValue::Cell(kind)) => Some(*kind),
        _ => None,
    }
}

/// Whether `fields` say they're a `kind`.
pub fn exec_is_kind(fields: &Vec<(u128, ExecValue)>, kind_cell: u128) -> (is: bool)
    ensures
        is == (kind(view_map(fields@)) == Some(kind_cell)),
{
    match exec_kind(fields) {
        Some(found) => found == kind_cell,
        None => false,
    }
}

/// An absent whose details don't name its kind field is an absent, of its
/// kind.
pub proof fn lemma_absent_kind(reason: u128, details: Seq<(u128, Value)>)
    requires
        forall|i: int| 0 <= i < details.len() ==> (#[trigger] details[i]).0 != ISA && details[i].0
            != REASON,
    ensures
        kind(absent(reason, details)->Record_0) == Some(KIND_ABSENT),
        is_absent(absent(reason, details)),
{
    let pairs = absent_header(reason) + details;
    assert forall|j: int| 0 < j < pairs.len() implies #[trigger] pairs[j].0 != pairs[0].0 by {
        if j >= 2 {
            assert(pairs[j] == details[j - 2]);
        }
    }
    lemma_to_map_last(pairs, 0);
    assert forall|j: int| 1 < j < pairs.len() implies #[trigger] pairs[j].0 != pairs[1].0 by {
        assert(pairs[j] == details[j - 2]);
    }
    lemma_to_map_last(pairs, 1);
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
            match exec_kind(fields) {
                Some(kind) => kind == KIND_ABSENT && match get(fields, REASON) {
                    Some(ExecValue::Cell(_)) => true,
                    _ => false,
                },
                None => false,
            }
        },
        _ => false,
    }
}

// The cells Grap's evaluator reads and writes, the same identities as in
// grap's isa, vocabulary, absent, source and path modules.

/// The field a record names the kind of thing it is under.
pub const ISA: u128 = 0xdbd7e1a38e30425fb1b408b2e446bb1b;

/// The kind of a call, `{isa: call, function, ...arguments}`.
pub const KIND_CALL: u128 = 0x8dd318828a4c42cf9f2d5d1ba9b068fe;

/// The kind of a lambda, `{isa: lambda, params, body}`.
pub const KIND_LAMBDA: u128 = 0xe4a0653134ca48379cab2e008bed03e7;

/// The kind of a closure, `{isa: closure, lambda, environment, body origin}`.
pub const KIND_CLOSURE: u128 = 0x0e8180c5b2f0492291ce5150f2292697;

/// The kind of a value wrapper, `{isa: value wrapper, value}`.
pub const KIND_VALUE_WRAPPER: u128 = 0xf615f8d777294ffd9e5296faf34edd9d;

/// The kind of a function the host implements, `{isa: foreign function,
/// implementation}`.
pub const KIND_FOREIGN_FUNCTION: u128 = 0xa394a3373e334363ab5a2965ec3204fa;

/// The kind of a value that isn't there, `{isa: absent, reason}`.
pub const KIND_ABSENT: u128 = 0x6509cd9921ab4cf28657ac75a9976d71;

pub const FUNCTION: u128 = 0x751fca4373debdd0b7e6eb73e08d684b;

pub const PARAMS: u128 = 0x195b378d0d31d90ab0d7366c15346b70;

pub const BODY: u128 = 0x986143866eda2e2fbf9ab8484357a0c9;

pub const VALUE: u128 = 0x5adde9ececa6ad57c81a3b2907e75e08;

/// A closure's lambda, as written.
pub const LAMBDA: u128 = 0xe070bde324bb9d90e1d457a2fdba6bf7;

pub const ENVIRONMENT: u128 = 0xe910025c710c25c43d0a5b296378f374;

/// The cell the host implements a foreign function under.
pub const IMPLEMENTATION: u128 = 0x24e1165744a040d7a4a4ff7d46f3b03c;

pub const BODY_ORIGIN: u128 = 0x215831fe58bde294e79cccbc74212318;

/// The field an absent names its reason under.
pub const REASON: u128 = 0xc2a01958bce64151beb2b3992017de4c;

/// The field an absent names a cell under.
pub const ABSENT_CELL: u128 = 0x7ba69298ef92cf954456784b66e86032;

/// The field an absent carries a value under.
pub const ABSENT_VALUE: u128 = 0xcfeaf191b71e06afef86c27ee43eb936;

/// The field an absent names where the call it's about is under.
pub const ABSENT_CALL: u128 = 0x31232e62ec458b0e6e201f3a7c1c10fe;

pub const FUEL_EXHAUSTED: u128 = 0x513628d759c04b3e7088b575e555a80e;

pub const DEPTH_EXHAUSTED: u128 = 0x4f9f758d18767f18c098ca72090ede64;

pub const MISSING_CELL: u128 = 0xa5a1b4e3d0df96bd11af00f0780136ff;

pub const MALFORMED_LAMBDA: u128 = 0xfbf5894d7f62b6d0048d17d26851b415;

pub const INVALID_PARAMETER: u128 = 0x93ca0e9199372c46ee24bd4c508e178c;

pub const NOT_CALLABLE: u128 = 0x8624488c2d10d2a4b84560dfa99a38e6;

pub const MISSING_ARGUMENT: u128 = 0x8b2f0db36e5c3d35595eb5666cc89c78;

/// Why a call, a lambda or a value wrapper can't be used yet: it lacks a
/// field its kind requires, named under `ABSENT_CELL`.
pub const MISSING_FIELD: u128 = 0xafc95de9df924ebd8de30f7b0243c6b5;

/// Why a function couldn't use its arguments, carrying why each it couldn't
/// use under its label.
pub const INVALID_ARGUMENTS: u128 = 0xaeb37f359ed1437cbc1adee0a05ac60f;

// Where code was written, as grap's source and path modules record it.

pub const ORIGIN_INPUT: u128 = 0x9a87743644af3453a0415dab056a0c5d;

pub const ORIGIN_ROOT: u128 = 0x7548280579760b782d1f1ce75b5394e9;

pub const ORIGIN_CELL: u128 = 0xad9b456873ec85947b96bab375b3e28e;

pub const ORIGIN_PATH: u128 = 0x7da5a4ab1d994cc1f9df7af5668b6c85;

pub const STEP_KEY: u128 = 0xf12dea12c741fe36312750a264f3a235;

pub const STEP_ELEMENT: u128 = 0x2eb44bbe78bbb0e96af4a9cbf6e949e1;

pub const STEP_FOLLOW: u128 = 0x33c6fb363ddc6f13fd054c68f7a37a98;

pub const STEP_INDEXABLE: u128 = 0xbd69ea5a7f839fe98dab761c01c22c13;

// The number and boolean conventions Grap holds natively, the same
// identities as in grap's f64, f32, u64 and logic modules.

pub const F64: u128 = 0xed11fde03b7c2c1ba2fccc3cdba5d561;

pub const NOT_F64: u128 = 0x4aabc44d46a2402dbb9ef64e377632db;

pub const F32: u128 = 0x64810cfeb0631ca8875e282d1ad4af79;

pub const NOT_F32: u128 = 0x6a124272d8df415ab944020cd1fd352b;

pub const U64: u128 = 0xfc77282178c2fa31b80aed2b8b6f7888;

pub const NOT_U64: u128 = 0xae77a805a7ce41dbac03b292d2685918;

pub const BOOL: u128 = 0xb2d13b64336a9e808133d0c7d5e4981e;

pub const TRUE: u128 = 0x04831f9b704935059231eb111770e62e;

pub const FALSE: u128 = 0x8fcc0a2e5f26c72efd9e917a28919801;

} // verus!
