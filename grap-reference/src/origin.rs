//! Where code was written, as the evaluators hold it, proven to read and
//! write the GID data `definition` says: an origin is where its path starts,
//! with the path, or a field of code written somewhere else, which shares
//! that origin, so finding where code is costs nothing until it's written out.

#[cfg(verus_only)]
use crate::definition;
use crate::definition::Start;
#[cfg(verus_only)]
use crate::definition::{Origin, Step};
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A step, as the evaluators hold it.
pub enum ExecStep {
    Key(u128),
    Element(Rc<Vec<u8>>),
    Follow,
}

/// Where code was written, as the evaluators hold it.
pub enum ExecOrigin {
    /// Where a path starts, and the path from there.
    Start(Start, Rc<Vec<ExecStep>>),
    /// The code under a field of code written at another origin.
    Field(Rc<ExecOrigin>, u128),
}

/// Code's origin, which code a program builds as it runs doesn't have.
pub type At = Option<Rc<ExecOrigin>>;

pub open spec fn view_step(step: ExecStep) -> Step {
    match step {
        ExecStep::Key(cell) => Step::Key(cell),
        ExecStep::Element(bytes) => Step::Element(bytes@),
        ExecStep::Follow => Step::Follow,
    }
}

pub open spec fn view_steps(steps: Seq<ExecStep>) -> Seq<Step> {
    Seq::new(steps.len(), |i: int| view_step(steps[i]))
}

/// What an origin the evaluators hold means.
pub open spec fn view_origin(origin: ExecOrigin) -> Origin
    decreases origin,
{
    match origin {
        ExecOrigin::Start(start, steps) => Origin { start, path: view_steps(steps@) },
        ExecOrigin::Field(parent, field) => {
            let at = view_origin(*parent);
            Origin { start: at.start, path: at.path.push(Step::Key(field)) }
        },
    }
}

pub open spec fn view_at(at: At) -> Option<Origin> {
    match at {
        Some(origin) => Some(view_origin(*origin)),
        None => None,
    }
}

/// The origin itself, shared.
pub fn share(at: &At) -> (shared: At)
    ensures
        shared == *at,
{
    match at {
        Some(origin) => Some(Rc::clone(origin)),
        None => None,
    }
}

/// The code under `field` of code written at `at`.
pub fn child(at: &At, field: u128) -> (found: At)
    ensures
        view_at(found) == definition::child(view_at(*at), field),
{
    match at {
        Some(origin) => Some(Rc::new(ExecOrigin::Field(Rc::clone(origin), field))),
        None => None,
    }
}

fn starting(start: Start) -> (made: At)
    ensures
        view_at(made) == Some(Origin { start, path: Seq::empty() }),
{
    let steps: Rc<Vec<ExecStep>> = Rc::new(Vec::new());
    proof {
        assert(view_steps(steps@) =~= Seq::<Step>::empty());
    }
    Some(Rc::new(ExecOrigin::Start(start, steps)))
}

/// Where `cell`'s definition starts.
pub fn defined_at(cell: u128) -> (made: At)
    ensures
        view_at(made) == Some(definition::defined_at(cell)),
{
    starting(Start::Cell(cell))
}

/// Where an evaluation's input starts.
pub fn input() -> (made: At)
    ensures
        view_at(made) == Some(definition::input()),
{
    starting(Start::Input)
}

// Writing origins out.

fn step_value(step: &ExecStep) -> (made: ExecValue)
    ensures
        view(made) == definition::step_value(view_step(*step)),
{
    match step {
        ExecStep::Key(cell) => exec_single(STEP_KEY, ExecValue::Cell(*cell)),
        ExecStep::Element(bytes) => exec_single(
            STEP_ELEMENT,
            exec_single(STEP_INDEXABLE, ExecValue::Blob(Rc::clone(bytes))),
        ),
        ExecStep::Follow => exec_single(STEP_FOLLOW, exec_empty()),
    }
}

/// Where `origin` starts, and its path's steps as GID data.
fn path_values(origin: &ExecOrigin) -> (made: (Start, Vec<ExecValue>))
    ensures
        made.0 == view_origin(*origin).start,
        view_elements(made.1@) == definition::step_values(view_origin(*origin).path),
    decreases *origin,
{
    match origin {
        ExecOrigin::Start(start, steps) => {
            let ghost path = view_steps(steps@);
            let mut values: Vec<ExecValue> = Vec::new();
            let mut i = 0;
            proof {
                assert(view_elements(values@) =~= definition::step_values(path).subrange(0, 0));
            }
            while i < steps.len()
                invariant
                    i <= steps@.len(),
                    path == view_steps(steps@),
                    view_elements(values@) == definition::step_values(path).subrange(0, i as int),
                decreases steps.len() - i,
            {
                let made = step_value(&steps[i]);
                let ghost before = values@;
                values.push(made);
                proof {
                    lemma_view_elements_push(before, made);
                    assert(view(made) == definition::step_values(path)[i as int]);
                }
                i += 1;
                proof {
                    assert(view_elements(values@) =~= definition::step_values(path).subrange(
                        0,
                        i as int,
                    ));
                }
            }
            proof {
                assert(definition::step_values(path).subrange(0, steps@.len() as int)
                    =~= definition::step_values(path));
            }
            (*start, values)
        },
        ExecOrigin::Field(parent, field) => {
            let (start, mut values) = path_values(parent);
            let ghost before = values@;
            let key = exec_single(STEP_KEY, ExecValue::Cell(*field));
            values.push(key);
            proof {
                lemma_view_elements_push(before, key);
                lemma_step_values_push();
                assert(view(key) == definition::step_value(Step::Key(*field)));
            }
            (start, values)
        },
    }
}

proof fn lemma_step_values_push()
    ensures
        forall|path: Seq<Step>, step: Step|
            #[trigger] definition::step_values(path.push(step)) == definition::step_values(
                path,
            ).push(definition::step_value(step)),
{
    assert forall|path: Seq<Step>, step: Step|
        #[trigger] definition::step_values(path.push(step)) == definition::step_values(path).push(
            definition::step_value(step),
        ) by {
        assert(definition::step_values(path.push(step)) =~= definition::step_values(path).push(
            definition::step_value(step),
        ));
    }
}

/// `origin` as GID data, as `definition::origin_value` says.
pub fn origin_value(origin: &ExecOrigin) -> (made: ExecValue)
    ensures
        view(made) == definition::origin_value(view_origin(*origin)),
{
    let (start, values) = path_values(origin);
    let path = ExecValue::List(Rc::new(values));
    proof {
        assert(view(path) == definition::path_value(view_origin(*origin).path));
    }
    match start {
        Start::Input => exec_single(ORIGIN_INPUT, path),
        Start::Root => exec_single(ORIGIN_ROOT, path),
        Start::Cell(cell) => exec_pair(ORIGIN_CELL, ExecValue::Cell(cell), ORIGIN_PATH, path),
    }
}

/// `{call: origin}`, as `definition::called` says, after `details`.
pub fn push_called(details: &mut Vec<(u128, ExecValue)>, at: &At)
    ensures
        view_pairs(final(details)@) == view_pairs(old(details)@) + definition::called(view_at(*at)),
{
    match at {
        Some(origin) => {
            let made = origin_value(origin);
            proof {
                lemma_view_pairs_push(details@, (ABSENT_CALL, made));
            }
            details.push((ABSENT_CALL, made));
            proof {
                assert(view_pairs(details@) =~= view_pairs(old(details)@) + definition::called(
                    view_at(*at),
                ));
            }
        },
        None => {
            proof {
                assert(view_pairs(details@) =~= view_pairs(details@) + Seq::<(u128, Value)>::empty());
            }
        },
    }
}

// Reading origins.

fn read_step(value: &ExecValue) -> (found: Option<ExecStep>)
    ensures
        match found {
            Some(step) => definition::read_step(view(*value)) == Some(view_step(step)),
            None => definition::read_step(view(*value)) is None,
        },
{
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            let key = has(fields, STEP_KEY);
            let element = has(fields, STEP_ELEMENT);
            let follow = has(fields, STEP_FOLLOW);
            if key && !element && !follow {
                match get(fields, STEP_KEY) {
                    Some(ExecValue::Cell(cell)) => Some(ExecStep::Key(*cell)),
                    _ => None,
                }
            } else if !key && element && !follow {
                match get(fields, STEP_ELEMENT) {
                    Some(ExecValue::Record(inner)) => {
                        proof {
                            lemma_view_record(*inner);
                        }
                        match get(inner, STEP_INDEXABLE) {
                            Some(ExecValue::Blob(bytes)) => {
                                if bytes.len() > 0 && bytes[bytes.len() - 1] != 0 {
                                    Some(ExecStep::Element(Rc::clone(bytes)))
                                } else {
                                    None
                                }
                            },
                            _ => None,
                        }
                    },
                    _ => None,
                }
            } else if !key && !element && follow {
                Some(ExecStep::Follow)
            } else {
                None
            }
        },
        _ => None,
    }
}

/// Values that don't read as steps from some point on don't read as steps.
proof fn lemma_read_steps_prefix(values: Seq<Value>, k: int)
    requires
        0 <= k <= values.len(),
        definition::read_steps(values.subrange(0, k)) is None,
    ensures
        definition::read_steps(values) is None,
    decreases values.len(),
{
    if k < values.len() {
        assert(values.drop_last().subrange(0, k) =~= values.subrange(0, k));
        lemma_read_steps_prefix(values.drop_last(), k);
    } else {
        assert(values.subrange(0, k) =~= values);
    }
}

fn read_path(value: &ExecValue) -> (found: Option<Vec<ExecStep>>)
    ensures
        match found {
            Some(steps) => definition::read_path(view(*value)) == Some(view_steps(steps@)),
            None => definition::read_path(view(*value)) is None,
        },
{
    match value {
        ExecValue::List(values) => {
            let ghost elements = view_elements(values@);
            let mut steps: Vec<ExecStep> = Vec::new();
            let mut i = 0;
            proof {
                assert(elements.subrange(0, 0) =~= Seq::<Value>::empty());
                assert(view_steps(steps@) =~= Seq::<Step>::empty());
            }
            while i < values.len()
                invariant
                    i <= values@.len(),
                    elements == view_elements(values@),
                    view(*value) == Value::List(elements),
                    definition::read_steps(elements.subrange(0, i as int)) == Some(
                        view_steps(steps@),
                    ),
                decreases values.len() - i,
            {
                let ghost prefix = elements.subrange(0, i + 1);
                proof {
                    assert(prefix.drop_last() =~= elements.subrange(0, i as int));
                    assert(prefix.last() == view(values@[i as int]));
                }
                match read_step(&values[i]) {
                    Some(step) => {
                        let ghost before = steps@;
                        steps.push(step);
                        i += 1;
                        proof {
                            assert(view_steps(steps@) =~= view_steps(before).push(view_step(step)));
                        }
                    },
                    None => {
                        proof {
                            lemma_read_steps_prefix(elements, i + 1);
                        }
                        return None;
                    },
                }
            }
            proof {
                assert(elements.subrange(0, values@.len() as int) =~= elements);
            }
            Some(steps)
        },
        _ => None,
    }
}

/// The origin GID data is, as `definition::read_origin` says.
pub fn read_origin(value: &ExecValue) -> (found: At)
    ensures
        view_at(found) == definition::read_origin(view(*value)),
{
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            let input = has(fields, ORIGIN_INPUT);
            let root = has(fields, ORIGIN_ROOT);
            let cell = has(fields, ORIGIN_CELL);
            let start = if input && !root && !cell {
                Start::Input
            } else if !input && root && !cell {
                Start::Root
            } else if !input && !root && cell && has(fields, ORIGIN_PATH) {
                match get(fields, ORIGIN_CELL) {
                    Some(ExecValue::Cell(at)) => Start::Cell(*at),
                    _ => {
                        return None;
                    },
                }
            } else {
                return None;
            };
            let field = match start {
                Start::Input => ORIGIN_INPUT,
                Start::Root => ORIGIN_ROOT,
                Start::Cell(_) => ORIGIN_PATH,
            };
            match get(fields, field) {
                Some(path) => match read_path(path) {
                    Some(steps) => Some(Rc::new(ExecOrigin::Start(start, Rc::new(steps)))),
                    None => None,
                },
                None => None,
            }
        },
        _ => None,
    }
}

// Origins read back.

/// Whether a step reads back from GID data: an element's at a position.
pub open spec fn valid_step(step: Step) -> bool {
    match step {
        Step::Element(bytes) => definition::position(bytes),
        _ => true,
    }
}

/// Whether an origin reads back from the GID data it's written as, as every
/// origin evaluation finds does.
pub open spec fn valid(origin: Origin) -> bool {
    forall|i: int| 0 <= i < origin.path.len() ==> valid_step(#[trigger] origin.path[i])
}

pub open spec fn valid_at(at: Option<Origin>) -> bool {
    match at {
        Some(origin) => valid(origin),
        None => true,
    }
}

proof fn lemma_read_step_value(step: Step)
    requires
        valid_step(step),
    ensures
        definition::read_step(definition::step_value(step)) == Some(step),
{
}

proof fn lemma_read_steps_values(path: Seq<Step>)
    requires
        forall|i: int| 0 <= i < path.len() ==> valid_step(#[trigger] path[i]),
    ensures
        definition::read_steps(definition::step_values(path)) == Some(path),
    decreases path.len(),
{
    if path.len() > 0 {
        let init = path.drop_last();
        assert forall|i: int| 0 <= i < init.len() implies valid_step(#[trigger] init[i]) by {
            assert(init[i] == path[i]);
        }
        lemma_read_steps_values(init);
        assert(definition::step_values(path).drop_last() =~= definition::step_values(init));
        assert(valid_step(path[path.len() - 1]));
        lemma_read_step_value(path.last());
        assert(init.push(path.last()) =~= path);
    }
}

/// A valid origin, written out and read back, is itself.
pub proof fn lemma_read_origin_value(origin: Origin)
    requires
        valid(origin),
    ensures
        definition::read_origin(definition::origin_value(origin)) == Some(origin),
{
    lemma_read_steps_values(origin.path);
}

proof fn lemma_read_steps_valid(values: Seq<Value>)
    ensures
        match definition::read_steps(values) {
            Some(path) => forall|i: int| 0 <= i < path.len() ==> valid_step(#[trigger] path[i]),
            None => true,
        },
    decreases values.len(),
{
    if values.len() > 0 {
        lemma_read_steps_valid(values.drop_last());
        match definition::read_steps(values) {
            Some(path) => {
                let init = definition::read_steps(values.drop_last())->Some_0;
                assert(path == init.push(path.last()));
                assert forall|i: int| 0 <= i < path.len() implies valid_step(#[trigger] path[i]) by {
                    if i < init.len() {
                        assert(path[i] == init[i]);
                    }
                }
            },
            None => {},
        }
    }
}

/// What an origin reads as is valid.
pub proof fn lemma_read_origin_valid(value: Value)
    ensures
        valid_at(definition::read_origin(value)),
{
    match value {
        Value::Record(fields) => {
            let lists = seq![fields[ORIGIN_INPUT], fields[ORIGIN_ROOT], fields[ORIGIN_PATH]];
            assert forall|i: int| 0 <= i < 3 implies match #[trigger] lists[i] {
                Value::List(values) => match definition::read_steps(values) {
                    Some(path) => forall|j: int|
                        0 <= j < path.len() ==> valid_step(#[trigger] path[j]),
                    None => true,
                },
                _ => true,
            } by {
                match lists[i] {
                    Value::List(values) => lemma_read_steps_valid(values),
                    _ => {},
                }
            }
            assert(lists[0] == fields[ORIGIN_INPUT]);
            assert(lists[1] == fields[ORIGIN_ROOT]);
            assert(lists[2] == fields[ORIGIN_PATH]);
        },
        _ => {},
    }
}

/// The code under a field of valid code is valid.
pub proof fn lemma_child_valid(at: Option<Origin>, field: u128)
    requires
        valid_at(at),
    ensures
        valid_at(definition::child(at, field)),
{
    if let Some(origin) = at {
        let path = origin.path.push(Step::Key(field));
        assert forall|i: int| 0 <= i < path.len() implies valid_step(#[trigger] path[i]) by {
            if i < origin.path.len() {
                assert(path[i] == origin.path[i]);
            }
        }
    }
}

} // verus!
