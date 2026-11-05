//! A faster evaluator for Grap's core and number operations, proven to
//! compute exactly what `definition` says, built the way a fast one is:
//! expressions lowered once into nodes, closures and numbers held as runtime
//! values rather than records, environments as shared frames, host cells
//! resolved when code is lowered, and a sorted set of every cell a lambda
//! binds, so looking up any other cell skips the frames.
//!
//! The proof relates each structure to what it means: a runtime value to the
//! GID value it materializes as (`means`), frames to the environment they
//! make (`environment_means`), and lowered code to the expressions it came
//! from (`node_ok`). Each step then computes what the definition's step does
//! on those meanings.

use crate::absent::caused_by_all;
#[cfg(verus_only)]
use crate::definition;
#[cfg(verus_only)]
use crate::definition::{Answer, Definition};
use crate::definition::{BinaryOperation, Number, Primitive, Reached, UnaryOperation};
use crate::evaluator::{
    ExecDefinition, ExecEvaluation, closure, cycle, first_resolving, missing_argument,
    missing_cell, named_cell, parameters, referenced_at,
};
#[cfg(verus_only)]
use crate::evaluator::{view_definition, view_host};
use crate::number::{
    FloatAnswer, binary_number, decode, encode, float_binary, float_unary, not_a_number,
    number_bytes, one_detail, tag, width,
};
#[cfg(verus_only)]
use crate::number::{float_answer, lemma_le_bytes_round_trip, lemma_le_round_trip, power256};
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// A value as the fast evaluator holds it: data, a number unboxed to its
/// bits, or a closure.
pub enum Runtime {
    Data(ExecValue),
    Number(Number, u64),
    Closure(Rc<Closure>),
}

/// A closure: its parameters and body, the code the body is in, the lambda's
/// other fields, for the record it materializes as, and what it captured.
pub struct Closure {
    pub params: Rc<Vec<u128>>,
    pub body: usize,
    pub code: Rc<Code>,
    pub fields: Rc<Vec<(u128, ExecValue)>>,
    pub environment: Environment,
}

/// Bindings as frames, innermost first; within a frame, a later binding
/// shadows an earlier one.
pub type Environment = Option<Rc<Frame>>;

pub struct Frame {
    pub parent: Environment,
    pub bindings: Vec<(u128, Runtime)>,
}

/// Lowered expressions: nodes, and the fields of every call, which a call
/// node names as a range.
pub struct Code {
    pub nodes: Vec<Node>,
    pub fields: Vec<(u128, usize)>,
}

/// An expression as lowered: the value it was lowered from, and what
/// evaluating it does, decided once.
pub struct Node {
    pub source: ExecValue,
    pub kind: Kind,
}

pub enum Kind {
    /// A cell, with its first host entry, if it has one.
    Reference { cell: u128, resolved: Option<usize> },
    /// A call: the node of its function, and the range of its fields.
    Call { function: usize, start: usize, end: usize },
    Lambda { params: Rc<Vec<u128>>, body: usize, fields: Rc<Vec<(u128, ExecValue)>> },
    /// A lambda whose parameters aren't valid: the absent it comes to.
    Malformed(ExecValue),
    /// `{value: x}`: the node of `x`.
    Wrapped(usize),
    /// A record that is exactly a number.
    Number(Number, u64),
    Data,
}

/// A host entry, lowered: the node of its value or description, and a
/// primitive's operation.
pub enum Prepared {
    Grap(usize),
    Foreign(usize),
    Primitive(usize, Primitive),
}

/// Where each of the host's cells first appears: its cells, sorted, each
/// with its first entry, and every entry's cell in order.
pub struct Index {
    pub cells: Vec<u128>,
    pub firsts: Vec<usize>,
    pub keys: Vec<u128>,
}

/// The host's definitions, lowered once.
pub struct Host {
    pub code: Rc<Code>,
    pub entries: Vec<(u128, Prepared)>,
    pub index: Index,
    /// Every cell a lambda in any lowered code binds, sorted.
    pub bound: Vec<u128>,
}

/// What evaluation changes as it goes: the cells under resolution, and
/// whether a closure written as data has been called. Such a closure can
/// bind cells no lambda names, so from then on lookups search every frame.
pub struct State {
    pub stack: Vec<(u128, Reached)>,
    pub dynamic: bool,
}

pub enum Outcome {
    Done(Runtime),
    OutOfFuel,
    Beyond,
}

// What the structures mean.

/// `{<type>: bytes}`, the record a number is.
pub open spec fn number_value(number: Number, bits: u64) -> Value {
    Value::Record(
        map![definition::tag(number) => Value::Blob(definition::le_bytes(bits as nat, definition::width(number)))],
    )
}

/// The GID value a runtime value is.
pub open spec fn means(value: Runtime) -> Value
    decreases value,
{
    match value {
        Runtime::Data(data) => view(data),
        Runtime::Number(number, bits) => number_value(number, bits),
        Runtime::Closure(closure) => Value::Record(
            map![CLOSURE => Value::Record(view_map(closure.fields@).insert(ENVIRONMENT, Value::Record(environment_means(closure.environment))))],
        ),
    }
}

/// The environment frames make: the parent's, with the bindings over it.
pub open spec fn environment_means(environment: Environment) -> Map<u128, Value>
    decreases environment,
{
    match environment {
        None => Map::empty(),
        Some(frame) => environment_means(frame.parent).union_prefer_right(
            to_map(bindings_values(frame.bindings@)),
        ),
    }
}

pub open spec fn bindings_values(bindings: Seq<(u128, Runtime)>) -> Seq<(u128, Value)>
    decreases bindings,
{
    Seq::new(
        bindings.len(),
        |i: int|
            if 0 <= i < bindings.len() {
                (bindings[i].0, means(bindings[i].1))
            } else {
                arbitrary()
            },
    )
}

/// A frame's environment, before it's made.
pub open spec fn frame_means(parent: Environment, bindings: Seq<(u128, Runtime)>) -> Map<
    u128,
    Value,
> {
    environment_means(parent).union_prefer_right(to_map(bindings_values(bindings)))
}

/// What a closure captured.
pub open spec fn captured_of(closure: Closure) -> Environment {
    closure.environment
}

pub open spec fn outcome_means(outcome: Outcome) -> definition::Outcome {
    match outcome {
        Outcome::Done(value) => definition::Outcome::Done(means(value)),
        Outcome::OutOfFuel => definition::Outcome::OutOfFuel,
        Outcome::Beyond => definition::Outcome::Beyond,
    }
}

/// The expression a node was lowered from.
pub open spec fn source(code: Code, node: int) -> Value {
    view(code.nodes@[node].source)
}

/// A call's fields, from `start` to `end`, as the record's pairs.
pub open spec fn call_pairs(code: Code, start: int, end: int) -> Seq<(u128, Value)> {
    Seq::new(
        (end - start) as nat,
        |i: int| (code.fields@[start + i].0, source(code, code.fields@[start + i].1 as int)),
    )
}

/// The record a call node was lowered from, as a map.
pub open spec fn call_map(code: Code, call: int) -> Map<u128, Value> {
    match code.nodes@[call].kind {
        Kind::Call { start, end, .. } => to_map(call_pairs(code, start as int, end as int)),
        _ => Map::empty(),
    }
}

pub open spec fn host_keys(host: Seq<(u128, Definition)>) -> Seq<u128> {
    Seq::new(host.len(), |i: int| host[i].0)
}

/// Where `cell` first appears among `keys`.
pub open spec fn first_index(keys: Seq<u128>, cell: u128) -> Option<int>
    decreases keys.len(),
{
    if keys.len() == 0 {
        None
    } else if keys[0] == cell {
        Some(0)
    } else {
        match first_index(keys.drop_first(), cell) {
            Some(at) => Some(at + 1),
            None => None,
        }
    }
}

pub open spec fn resolved_ok(keys: Seq<u128>, cell: u128, resolved: Option<usize>) -> bool {
    match (resolved, first_index(keys, cell)) {
        (Some(at), Some(first)) => at as int == first,
        (None, None) => true,
        _ => false,
    }
}

/// Whether `value` evaluates to itself.
pub open spec fn data_form(value: Value) -> bool {
    match value {
        Value::Cell(_) => false,
        Value::Record(fields) => !fields.contains_key(FUNCTION) && !(fields.contains_key(PARAMS)
            && fields.contains_key(BODY)) && !fields.contains_key(VALUE),
        _ => true,
    }
}

pub open spec fn lambda_ok(
    fields: Map<u128, Value>,
    params: Seq<u128>,
    body: Value,
    closure_fields: Map<u128, Value>,
) -> bool {
    &&& !fields.contains_key(FUNCTION)
    &&& fields.contains_key(PARAMS)
    &&& fields.contains_key(BODY)
    &&& fields[PARAMS] is List
    &&& definition::parameters(fields[PARAMS]->List_0) == Ok::<Seq<u128>, Value>(params)
    &&& body == fields[BODY]
    &&& closure_fields == fields.remove(BODY_ORIGIN)
}

pub open spec fn malformed_ok(fields: Map<u128, Value>) -> bool {
    &&& !fields.contains_key(FUNCTION)
    &&& fields.contains_key(PARAMS)
    &&& fields.contains_key(BODY)
    &&& !(fields[PARAMS] is List && definition::parameters(fields[PARAMS]->List_0) is Ok)
}

pub open spec fn wrapped_ok(fields: Map<u128, Value>) -> bool {
    &&& !fields.contains_key(FUNCTION)
    &&& !(fields.contains_key(PARAMS) && fields.contains_key(BODY))
    &&& fields.contains_key(VALUE)
}

/// Whether a node of `kind`, lowered from `value`, does in `code` what
/// evaluating `value` does.
pub open spec fn kind_ok(kind: Kind, value: Value, code: Code, keys: Seq<u128>) -> bool {
    match kind {
        Kind::Reference { cell, resolved } => value == Value::Cell(cell) && resolved_ok(
            keys,
            cell,
            resolved,
        ),
        Kind::Call { function, start, end } => {
            &&& start <= end <= code.fields@.len()
            &&& forall|i: int|
                start <= i < end ==> (#[trigger] code.fields@[i]).1 < code.nodes@.len()
            &&& function < code.nodes@.len()
            &&& value == Value::Record(to_map(call_pairs(code, start as int, end as int)))
            &&& to_map(call_pairs(code, start as int, end as int)).contains_key(FUNCTION)
            &&& source(code, function as int) == to_map(
                call_pairs(code, start as int, end as int),
            )[FUNCTION]
        },
        Kind::Lambda { params, body, fields } => {
            &&& value is Record
            &&& body < code.nodes@.len()
            &&& lambda_ok(value->Record_0, params@, source(code, body as int), view_map(fields@))
        },
        Kind::Malformed(made) => {
            &&& value is Record
            &&& malformed_ok(value->Record_0)
            &&& view(made) == definition::lambda(value->Record_0, Map::empty())
        },
        Kind::Wrapped(inner) => {
            &&& value is Record
            &&& wrapped_ok(value->Record_0)
            &&& inner < code.nodes@.len()
            &&& source(code, inner as int) == value->Record_0[VALUE]
        },
        Kind::Number(number, bits) => value == number_value(number, bits),
        Kind::Data => data_form(value),
    }
}

/// Whether a node does what evaluating its expression does.
pub open spec fn node_ok(code: Code, node: int, keys: Seq<u128>) -> bool {
    kind_ok(code.nodes@[node].kind, source(code, node), code, keys)
}

pub open spec fn wf_code(code: Code, keys: Seq<u128>) -> bool {
    forall|node: int| 0 <= node < code.nodes@.len() ==> #[trigger] node_ok(code, node, keys)
}

pub open spec fn all_marked(cells: Seq<u128>, bound: Seq<u128>) -> bool {
    forall|i: int| 0 <= i < cells.len() ==> bound.contains(#[trigger] cells[i])
}

pub open spec fn lambda_marked(kind: Kind, bound: Seq<u128>) -> bool {
    match kind {
        Kind::Lambda { params, .. } => all_marked(params@, bound),
        _ => true,
    }
}

/// Every lambda's parameters are marked bound.
pub open spec fn code_marked(code: Code, bound: Seq<u128>) -> bool {
    forall|node: int|
        0 <= node < code.nodes@.len() ==> #[trigger] lambda_marked(code.nodes@[node].kind, bound)
}

/// A closure as a lambda node makes one.
pub open spec fn closure_ok(closure: Closure, keys: Seq<u128>) -> bool {
    let fields = view_map(closure.fields@);
    &&& wf_code(*closure.code, keys)
    &&& closure.body < closure.code.nodes@.len()
    &&& fields.contains_key(PARAMS)
    &&& fields.contains_key(BODY)
    &&& fields[PARAMS] is List
    &&& definition::parameters(fields[PARAMS]->List_0) == Ok::<Seq<u128>, Value>(closure.params@)
    &&& fields[BODY] == source(*closure.code, closure.body as int)
}

pub open spec fn shape_ok(value: Runtime, keys: Seq<u128>) -> bool
    decreases value,
{
    match value {
        Runtime::Closure(closure) => closure_ok(*closure, keys) && environment_ok(
            closure.environment,
            keys,
        ),
        _ => true,
    }
}

pub open spec fn environment_ok(environment: Environment, keys: Seq<u128>) -> bool
    decreases environment,
{
    match environment {
        None => true,
        Some(frame) => environment_ok(frame.parent, keys) && bindings_ok(frame.bindings@, keys),
    }
}

pub open spec fn bindings_ok(bindings: Seq<(u128, Runtime)>, keys: Seq<u128>) -> bool
    decreases bindings,
{
    forall|i: int| 0 <= i < bindings.len() ==> shape_ok((#[trigger] bindings[i]).1, keys)
}

pub open spec fn marked_value(value: Runtime, bound: Seq<u128>) -> bool
    decreases value,
{
    match value {
        Runtime::Closure(closure) => {
            &&& all_marked(closure.params@, bound)
            &&& code_marked(*closure.code, bound)
            &&& marked_environment(closure.environment, bound)
        },
        _ => true,
    }
}

/// Every cell bound in the frames is marked.
pub open spec fn marked_environment(environment: Environment, bound: Seq<u128>) -> bool
    decreases environment,
{
    match environment {
        None => true,
        Some(frame) => marked_environment(frame.parent, bound) && marked_bindings(
            frame.bindings@,
            bound,
        ),
    }
}

pub open spec fn marked_bindings(bindings: Seq<(u128, Runtime)>, bound: Seq<u128>) -> bool
    decreases bindings,
{
    forall|i: int|
        0 <= i < bindings.len() ==> bound.contains((#[trigger] bindings[i]).0) && marked_value(
            bindings[i].1,
            bound,
        )
}

pub open spec fn prepared_root(prepared: Prepared) -> usize {
    match prepared {
        Prepared::Grap(root) => root,
        Prepared::Foreign(root) => root,
        Prepared::Primitive(root, _) => root,
    }
}

pub open spec fn prepared_definition(code: Code, prepared: Prepared) -> Definition {
    match prepared {
        Prepared::Grap(root) => Definition::Grap(source(code, root as int)),
        Prepared::Foreign(root) => Definition::Foreign(source(code, root as int)),
        Prepared::Primitive(root, primitive) => Definition::Primitive(
            source(code, root as int),
            primitive,
        ),
    }
}

/// The definitions a host holds.
pub open spec fn host_means(host: Host) -> Seq<(u128, Definition)> {
    Seq::new(
        host.entries@.len(),
        |i: int| (host.entries@[i].0, prepared_definition(*host.code, host.entries@[i].1)),
    )
}

pub open spec fn keys_of(host: Host) -> Seq<u128> {
    Seq::new(host.entries@.len(), |i: int| host.entries@[i].0)
}

pub open spec fn sorted(cells: Seq<u128>) -> bool {
    forall|i: int, j: int| 0 <= i < j < cells.len() ==> cells[i] < cells[j]
}

/// Sorted cells, each with its first entry among `keys`, and every key's.
pub open spec fn index_ok(cells: Seq<u128>, firsts: Seq<usize>, keys: Seq<u128>) -> bool {
    &&& sorted(cells)
    &&& cells.len() == firsts.len()
    &&& forall|k: int|
        0 <= k < cells.len() ==> first_index(keys, #[trigger] cells[k]) == Some(firsts[k] as int)
    &&& forall|i: int| 0 <= i < keys.len() ==> cells.contains(#[trigger] keys[i])
}

pub open spec fn index_wf(index: Index) -> bool {
    index_ok(index.cells@, index.firsts@, index.keys@)
}

pub open spec fn wf_host(host: Host) -> bool {
    &&& wf_code(*host.code, keys_of(host))
    &&& forall|i: int|
        0 <= i < host.entries@.len() ==> prepared_root((#[trigger] host.entries@[i]).1)
            < host.code.nodes@.len()
    &&& index_wf(host.index)
    &&& host.index.keys@ == keys_of(host)
    &&& sorted(host.bound@)
    &&& code_marked(*host.code, host.bound@)
}

/// What evaluating code in an environment needs.
pub open spec fn ready(code: Code, environment: Environment, host: Host, dynamic: bool) -> bool {
    &&& wf_host(host)
    &&& wf_code(code, keys_of(host))
    &&& environment_ok(environment, keys_of(host))
    &&& dynamic || (marked_environment(environment, host.bound@) && code_marked(code, host.bound@))
}

/// What evaluation promises of its outcome.
pub open spec fn result_ok(outcome: Outcome, host: Host, dynamic: bool) -> bool {
    match outcome {
        Outcome::Done(value) => shape_ok(value, keys_of(host)) && (dynamic || marked_value(
            value,
            host.bound@,
        )),
        _ => true,
    }
}

/// `grown` extends `code`, leaving its nodes and fields as they were.
pub open spec fn extends(code: Code, grown: Code) -> bool {
    &&& code.nodes@.len() <= grown.nodes@.len()
    &&& code.fields@.len() <= grown.fields@.len()
    &&& forall|i: int| 0 <= i < code.nodes@.len() ==> grown.nodes@[i] == code.nodes@[i]
    &&& forall|i: int| 0 <= i < code.fields@.len() ==> grown.fields@[i] == code.fields@[i]
}

pub open spec fn includes(cells: Seq<u128>, more: Seq<u128>) -> bool {
    forall|cell: u128| cells.contains(cell) ==> more.contains(cell)
}

// Lemmas about the meanings.

pub proof fn lemma_first_index(keys: Seq<u128>, cell: u128)
    ensures
        match first_index(keys, cell) {
            Some(at) => 0 <= at < keys.len() && keys[at] == cell && forall|j: int|
                0 <= j < at ==> keys[j] != cell,
            None => forall|j: int| 0 <= j < keys.len() ==> keys[j] != cell,
        },
    decreases keys.len(),
{
    if keys.len() > 0 && keys[0] != cell {
        lemma_first_index(keys.drop_first(), cell);
        assert forall|j: int| 1 <= j < keys.len() implies keys[j] == keys.drop_first()[j - 1] by {}
    }
}

/// Resolving a cell finds the definition at its first key.
pub proof fn lemma_resolve(host: Seq<(u128, Definition)>, cell: u128)
    ensures
        match first_index(host_keys(host), cell) {
            Some(at) => 0 <= at < host.len() && definition::resolve(host, cell) == Some(host[at].1),
            None => definition::resolve(host, cell) is None,
        },
    decreases host.len(),
{
    lemma_first_index(host_keys(host), cell);
    if host.len() > 0 && host[0].0 != cell {
        assert(host_keys(host.drop_first()) =~= host_keys(host).drop_first());
        lemma_resolve(host.drop_first(), cell);
    }
}

pub proof fn lemma_host_keys(host: Host)
    ensures
        host_keys(host_means(host)) == keys_of(host),
{
    assert(host_keys(host_means(host)) =~= keys_of(host));
}

/// A node's kind does what it did in code that grows.
pub proof fn lemma_kind_extends(kind: Kind, value: Value, code: Code, grown: Code, keys: Seq<u128>)
    requires
        extends(code, grown),
        kind_ok(kind, value, code, keys),
    ensures
        kind_ok(kind, value, grown, keys),
{
    match kind {
        Kind::Call { function, start, end } => {
            assert(call_pairs(grown, start as int, end as int) =~= call_pairs(
                code,
                start as int,
                end as int,
            )) by {
                assert forall|i: int| 0 <= i < end - start implies #[trigger] call_pairs(
                    grown,
                    start as int,
                    end as int,
                )[i] == call_pairs(code, start as int, end as int)[i] by {
                    assert(code.fields@[start + i].1 < code.nodes@.len());
                    assert(grown.fields@[start + i] == code.fields@[start + i]);
                }
            }
            assert(grown.nodes@[function as int] == code.nodes@[function as int]);
        },
        Kind::Lambda { body, .. } => {
            assert(grown.nodes@[body as int] == code.nodes@[body as int]);
        },
        Kind::Wrapped(inner) => {
            assert(grown.nodes@[inner as int] == code.nodes@[inner as int]);
        },
        _ => {},
    }
}

/// Nodes and fields left as they were do what they did.
pub proof fn lemma_extends(code: Code, grown: Code, keys: Seq<u128>, bound: Seq<u128>)
    requires
        extends(code, grown),
    ensures
        forall|node: int|
            0 <= node < code.nodes@.len() && node_ok(code, node, keys) ==> #[trigger] node_ok(
                grown,
                node,
                keys,
            ),
        forall|node: int|
            0 <= node < code.nodes@.len() && lambda_marked(code.nodes@[node].kind, bound)
                ==> #[trigger] lambda_marked(grown.nodes@[node].kind, bound),
{
    assert forall|node: int|
        0 <= node < code.nodes@.len() && node_ok(code, node, keys) implies #[trigger] node_ok(
        grown,
        node,
        keys,
    ) by {
        assert(grown.nodes@[node] == code.nodes@[node]);
        lemma_kind_extends(code.nodes@[node].kind, source(code, node), code, grown, keys);
    }
    assert forall|node: int|
        0 <= node < code.nodes@.len() && lambda_marked(
            code.nodes@[node].kind,
            bound,
        ) implies #[trigger] lambda_marked(grown.nodes@[node].kind, bound) by {
        assert(grown.nodes@[node] == code.nodes@[node]);
    }
}

pub proof fn lemma_code_marked_grows(code: Code, bound: Seq<u128>, more: Seq<u128>)
    requires
        code_marked(code, bound),
        includes(bound, more),
    ensures
        code_marked(code, more),
{
    assert forall|node: int| 0 <= node < code.nodes@.len() implies #[trigger] lambda_marked(
        code.nodes@[node].kind,
        more,
    ) by {
        assert(lambda_marked(code.nodes@[node].kind, bound));
    }
}

pub proof fn lemma_bindings_push(bindings: Seq<(u128, Runtime)>, binding: (u128, Runtime))
    ensures
        bindings_values(bindings.push(binding)) == bindings_values(bindings).push(
            (binding.0, means(binding.1)),
        ),
{
    assert(bindings_values(bindings.push(binding)) =~= bindings_values(bindings).push(
        (binding.0, means(binding.1)),
    ));
}

/// A binding added to a frame lies over what the frame made.
pub proof fn lemma_frame_push(
    parent: Environment,
    bindings: Seq<(u128, Runtime)>,
    binding: (u128, Runtime),
)
    ensures
        frame_means(parent, bindings.push(binding)) == frame_means(parent, bindings).insert(
            binding.0,
            means(binding.1),
        ),
{
    lemma_bindings_push(bindings, binding);
    assert(bindings_values(bindings.push(binding)).drop_last() =~= bindings_values(bindings));
    assert(frame_means(parent, bindings.push(binding)) =~= frame_means(parent, bindings).insert(
        binding.0,
        means(binding.1),
    ));
}

pub proof fn lemma_frame_empty()
    ensures
        forall|parent: Environment|
            #[trigger] frame_means(parent, Seq::empty()) == environment_means(parent),
{
    assert forall|parent: Environment|
        #[trigger] frame_means(parent, Seq::empty()) == environment_means(parent) by {
        assert(bindings_values(Seq::<(u128, Runtime)>::empty()) =~= Seq::<(u128, Value)>::empty());
        assert(frame_means(parent, Seq::empty()) =~= environment_means(parent));
    }
}

/// What a closure is, read back as the definition reads closure records.
pub proof fn lemma_closure(keys: Seq<u128>)
    ensures
        forall|closure: Rc<Closure>|
            closure_ok(*closure, keys) ==> #[trigger] definition::closure(
                means(Runtime::Closure(closure)),
            ) == Some(
                (
                    closure.params@,
                    source(*closure.code, closure.body as int),
                    environment_means(closure.environment),
                ),
            ),
{
}

/// A number record isn't a closure, a named cell or an absent.
pub proof fn lemma_number_value(number: Number, bits: u64)
    ensures
        definition::closure(number_value(number, bits)) is None,
        definition::named_cell(number_value(number, bits)) is None,
        !is_absent(number_value(number, bits)),
{
}

/// Every cell a frame binds is marked, so an unmarked cell isn't bound.
pub proof fn lemma_unmarked(environment: Environment, bound: Seq<u128>, cell: u128)
    requires
        marked_environment(environment, bound),
        !bound.contains(cell),
    ensures
        !environment_means(environment).contains_key(cell),
    decreases environment,
{
    match environment {
        None => {},
        Some(frame) => {
            let ghost parent = frame.parent;
            let ghost bindings = frame.bindings@;
            assert(marked_bindings(bindings, bound));
            lemma_unmarked(parent, bound, cell);
            lemma_to_map_keys(bindings_values(bindings), cell);
            assert forall|i: int|
                0 <= i < bindings_values(bindings).len() implies #[trigger] bindings_values(
                bindings,
            )[i].0 != cell by {
                assert(bound.contains(bindings[i].0));
            }
        },
    }
}

pub proof fn lemma_le_bytes_len(n: nat, width: nat)
    ensures
        definition::le_bytes(n, width).len() == width,
    decreases width,
{
    if width > 0 {
        lemma_le_bytes_len(n / 256, (width - 1) as nat);
    }
}

// Sorted cells.

/// Where `cell` is, or would go, among sorted `cells`.
fn position(cells: &Vec<u128>, cell: u128) -> (at: usize)
    requires
        sorted(cells@),
    ensures
        at <= cells@.len(),
        forall|i: int| 0 <= i < at ==> cells@[i] < cell,
        forall|i: int| at <= i < cells@.len() ==> cell <= cells@[i],
{
    let mut low = 0;
    let mut high = cells.len();
    while low < high
        invariant
            low <= high <= cells@.len(),
            sorted(cells@),
            forall|i: int| 0 <= i < low ==> cells@[i] < cell,
            forall|i: int| high <= i < cells@.len() ==> cell <= cells@[i],
        decreases high - low,
    {
        let middle = low + (high - low) / 2;
        if cells[middle] < cell {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// Where sorted `cells` hold `cell`, or where it would go.
fn find_cell(cells: &Vec<u128>, cell: u128) -> (found: Result<usize, usize>)
    requires
        sorted(cells@),
    ensures
        match found {
            Ok(at) => at < cells@.len() && cells@[at as int] == cell,
            Err(at) => at <= cells@.len() && !cells@.contains(cell) && (forall|i: int|
                0 <= i < at ==> cells@[i] < cell) && (forall|i: int|
                at <= i < cells@.len() ==> cell < cells@[i]),
        },
{
    let at = position(cells, cell);
    if at < cells.len() && cells[at] == cell {
        Ok(at)
    } else {
        proof {
            assert forall|i: int| 0 <= i < cells@.len() implies cells@[i] != cell by {
                if i > at {
                    assert(cells@[at as int] < cells@[i]);
                }
            }
            assert forall|i: int| at <= i < cells@.len() implies cell < cells@[i] by {
                assert(cells@[i] != cell);
            }
        }
        Err(at)
    }
}

fn marked(bound: &Vec<u128>, cell: u128) -> (found: bool)
    requires
        sorted(bound@),
    ensures
        found == bound@.contains(cell),
{
    match find_cell(bound, cell) {
        Ok(_at) => {
            proof {
                assert(bound@[_at as int] == cell);
            }
            true
        },
        Err(_) => false,
    }
}

fn mark(bound: &mut Vec<u128>, cell: u128)
    requires
        sorted(old(bound)@),
    ensures
        sorted(final(bound)@),
        final(bound)@.contains(cell),
        includes(old(bound)@, final(bound)@),
{
    match find_cell(bound, cell) {
        Ok(_at) => {
            proof {
                assert(bound@[_at as int] == cell);
            }
        },
        Err(at) => {
            let ghost before = bound@;
            bound.insert(at, cell);
            proof {
                assert(bound@[at as int] == cell);
                assert forall|i: int, j: int| 0 <= i < j < bound@.len() implies bound@[i]
                    < bound@[j] by {
                    if j < at {
                        assert(bound@[i] == before[i] && bound@[j] == before[j]);
                    } else if j == at {
                        assert(bound@[i] == before[i]);
                    } else if i < at {
                        assert(bound@[i] == before[i] && bound@[j] == before[j - 1]);
                    } else if i == at {
                        assert(bound@[j] == before[j - 1]);
                    } else {
                        assert(bound@[i] == before[i - 1] && bound@[j] == before[j - 1]);
                    }
                }
                assert forall|x: u128| #[trigger] before.contains(x) implies bound@.contains(x) by {
                    let k = choose|k: int| 0 <= k < before.len() && before[k] == x;
                    if k < at {
                        assert(bound@[k] == x);
                    } else {
                        assert(bound@[k + 1] == x);
                    }
                }
            }
        },
    }
}

/// `cell`'s first host entry.
fn resolve_index(index: &Index, cell: u128) -> (resolved: Option<usize>)
    requires
        index_wf(*index),
    ensures
        resolved_ok(index.keys@, cell, resolved),
{
    let ghost keys = index.keys@;
    let cells = &index.cells;
    let firsts = &index.firsts;
    match find_cell(cells, cell) {
        Ok(at) => {
            proof {
                assert(first_index(keys, cells@[at as int]) == Some(firsts@[at as int] as int));
            }
            Some(firsts[at])
        },
        Err(_) => {
            proof {
                lemma_first_index(keys, cell);
                if first_index(keys, cell) is Some {
                    let at = first_index(keys, cell)->Some_0;
                    assert(cells@.contains(keys[at]));
                }
            }
            None
        },
    }
}

// Lowering.

/// The number a record is, when it's nothing but one.
fn exact_number(fields: &Rc<Vec<(u128, ExecValue)>>) -> (number: Option<(Number, u64)>)
    ensures
        match number {
            Some((number, bits)) => view(ExecValue::Record(*fields)) == number_value(number, bits),
            None => true,
        },
{
    if fields.len() != 1 {
        return None;
    }
    let number = if fields[0].0 == F64 {
        Number::F64
    } else if fields[0].0 == F32 {
        Number::F32
    } else if fields[0].0 == U64 {
        Number::U64
    } else {
        return None;
    };
    match &fields[0].1 {
        ExecValue::Blob(bytes) => {
            if bytes.len() == width(number) {
                let bits = decode(bytes.as_slice());
                proof {
                    lemma_view_record(*fields);
                    lemma_le_round_trip(bytes@);
                    assert(fields@ =~= Seq::<(u128, ExecValue)>::empty().push(fields@[0]));
                    lemma_view_pairs_push(Seq::empty(), fields@[0]);
                    assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<
                        (u128, Value),
                    >::empty());
                    assert(view_map(fields@) =~= map![definition::tag(number) => Value::Blob(definition::le_bytes(bits as nat, definition::width(number)))]);
                }
                Some((number, bits))
            } else {
                None
            }
        },
        _ => None,
    }
}

/// A node of `kind`, lowered from `value`, added to `code`.
#[allow(unused_variables)]
fn push_node(code: &mut Code, value: &ExecValue, kind: Kind, index: &Index, bound: &Vec<u128>) -> (node:
    usize)
    requires
        wf_code(*old(code), index.keys@),
        code_marked(*old(code), bound@),
        kind_ok(kind, view(*value), *old(code), index.keys@),
        lambda_marked(kind, bound@),
    ensures
        extends(*old(code), *final(code)),
        final(code).fields@ == old(code).fields@,
        wf_code(*final(code), index.keys@),
        code_marked(*final(code), bound@),
        node == old(code).nodes@.len(),
        node < final(code).nodes@.len(),
        source(*final(code), node as int) == view(*value),
{
    let ghost keys = index.keys@;
    let ghost bound = bound@;
    let ghost before = *code;
    let ghost spec_kind = kind;
    code.nodes.push(Node { source: copy(value), kind });
    proof {
        assert(extends(before, *code));
        lemma_extends(before, *code, keys, bound);
        lemma_kind_extends(spec_kind, view(*value), before, *code, keys);
        let node = before.nodes@.len() as int;
        assert(code.nodes@[node].kind == spec_kind);
        assert(node_ok(*code, node, keys));
        assert(lambda_marked(code.nodes@[node].kind, bound));
    }
    code.nodes.len() - 1
}

/// `value` lowered into `code`, with the expressions in it, marking the
/// cells its lambdas bind: its node.
#[verifier::loop_isolation(false)]
fn lower(value: &ExecValue, index: &Index, bound: &mut Vec<u128>, code: &mut Code) -> (node:
    usize)
    requires
        index_wf(*index),
        sorted(old(bound)@),
        wf_code(*old(code), index.keys@),
        code_marked(*old(code), old(bound)@),
    ensures
        extends(*old(code), *final(code)),
        wf_code(*final(code), index.keys@),
        code_marked(*final(code), final(bound)@),
        sorted(final(bound)@),
        includes(old(bound)@, final(bound)@),
        node < final(code).nodes@.len(),
        source(*final(code), node as int) == view(*value),
    decreases *value,
{
    let ghost keys = index.keys@;
    let ghost original = *code;
    let ghost original_bound = bound@;
    match value {
        ExecValue::Cell(cell) => {
            let kind = Kind::Reference {
                cell: *cell,
                resolved: resolve_index(index, *cell),
            };
            push_node(code, value, kind, index, bound)
        },
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            if has(fields, FUNCTION) {
                let mut children: Vec<(u128, usize)> = Vec::new();
                let mut i = 0;
                while i < fields.len()
                    invariant
                        i <= fields@.len(),
                        children@.len() == i,
                        extends(original, *code),
                        wf_code(*code, keys),
                        code_marked(*code, bound@),
                        sorted(bound@),
                        includes(original_bound, bound@),
                        forall|k: int|
                            0 <= k < i ==> (#[trigger] children@[k]).0 == fields@[k].0
                                && children@[k].1 < code.nodes@.len() && source(
                                *code,
                                children@[k].1 as int,
                            ) == view(fields@[k].1),
                    decreases fields.len() - i,
                {
                    let ghost before = *code;
                    let child = lower(&fields[i].1, index, bound, code);
                    proof {
                        assert forall|k: int| 0 <= k < i implies source(
                            *code,
                            (#[trigger] children@[k]).1 as int,
                        ) == view(fields@[k].1) by {
                            assert(code.nodes@[children@[k].1 as int]
                                == before.nodes@[children@[k].1 as int]);
                        }
                    }
                    children.push((fields[i].0, child));
                    i += 1;
                }
                let start = code.fields.len();
                let ghost lowered = *code;
                let mut j = 0;
                while j < children.len()
                    invariant
                        j <= children@.len(),
                        start == lowered.fields@.len(),
                        code.nodes@ == lowered.nodes@,
                        code.fields@.len() == start + j,
                        forall|k: int| 0 <= k < start ==> #[trigger] code.fields@[k] == lowered.fields@[k],
                        forall|k: int| 0 <= k < j ==> code.fields@[start + k] == children@[k],
                    decreases children.len() - j,
                {
                    code.fields.push(children[j]);
                    j += 1;
                }
                let end = code.fields.len();
                let function = match get_index(fields, FUNCTION) {
                    Some(at) => children[at].1,
                    None => 0,
                };
                proof {
                    assert(extends(lowered, *code));
                    lemma_extends(lowered, *code, keys, bound@);
                    assert forall|k: int| 0 <= k < fields@.len() implies #[trigger] call_pairs(
                        *code,
                        start as int,
                        end as int,
                    )[k] == view_pairs(fields@)[k] by {
                        assert(code.fields@[start + k] == children@[k]);
                        assert(children@[k].0 == fields@[k].0);
                        assert(source(lowered, children@[k].1 as int) == view(fields@[k].1));
                    }
                    assert(call_pairs(*code, start as int, end as int) =~= view_pairs(fields@));
                    assert forall|k: int| start <= k < end implies (#[trigger] code.fields@[k]).1
                        < code.nodes@.len() by {
                        assert(code.fields@[k] == children@[k - start]);
                    }
                }
                let kind = Kind::Call { function, start, end };
                push_node(code, value, kind, index, bound)
            } else if has(fields, PARAMS) && has(fields, BODY) {
                match get(fields, PARAMS) {
                    Some(ExecValue::List(elements)) => match parameters(elements.as_slice()) {
                        Ok(params) => {
                            let mut k = 0;
                            while k < params.len()
                                invariant
                                    k <= params@.len(),
                                    sorted(bound@),
                                    includes(original_bound, bound@),
                                    forall|i: int|
                                        0 <= i < k ==> bound@.contains(#[trigger] params@[i]),
                                decreases params.len() - k,
                            {
                                mark(bound, params[k]);
                                k += 1;
                            }
                            proof {
                                lemma_code_marked_grows(*code, original_bound, bound@);
                            }
                            let ghost marked_bound = bound@;
                            let body = match get_index(fields, BODY) {
                                Some(at) => lower(&fields[at].1, index, bound, code),
                                None => 0,
                            };
                            let closure_fields = if has(fields, BODY_ORIGIN) {
                                Rc::new(without(fields, BODY_ORIGIN))
                            } else {
                                proof {
                                    assert(view_map(fields@).remove(BODY_ORIGIN) =~= view_map(
                                        fields@,
                                    ));
                                }
                                Rc::clone(fields)
                            };
                            proof {
                                assert forall|i: int| 0 <= i < params@.len() implies bound@.contains(
                                    #[trigger] params@[i],
                                ) by {
                                    assert(marked_bound.contains(params@[i]));
                                }
                            }
                            let kind = Kind::Lambda { params: Rc::new(params), body, fields: closure_fields };
                            push_node(code, value, kind, index, bound)
                        },
                        Err(parameter) => {
                            let kind = Kind::Malformed(
                                exec_absent(INVALID_PARAMETER, ABSENT_VALUE, parameter),
                            );
                            push_node(code, value, kind, index, bound)
                        },
                    },
                    _ => {
                        let kind = Kind::Malformed(
                            exec_absent(MALFORMED_LAMBDA, ABSENT_VALUE, copy(value)),
                        );
                        push_node(code, value, kind, index, bound)
                    },
                }
            } else {
                match get_index(fields, VALUE) {
                    Some(at) => {
                        let inner = lower(&fields[at].1, index, bound, code);
                        push_node(code, value, Kind::Wrapped(inner), index, bound)
                    },
                    None => {
                        let kind = match exact_number(fields) {
                            Some((number, bits)) => Kind::Number(number, bits),
                            None => Kind::Data,
                        };
                        push_node(code, value, kind, index, bound)
                    },
                }
            }
        },
        _ => push_node(code, value, Kind::Data, index, bound),
    }
}

/// The host's definitions lowered, with each cell's first entry, marking
/// the cells their lambdas bind.
#[verifier::loop_isolation(false)]
fn prepare(host: &Vec<(u128, ExecDefinition)>, bound: &mut Vec<u128>) -> (prepared: (
    Code,
    Vec<(u128, Prepared)>,
    Index,
))
    requires
        sorted(old(bound)@),
    ensures
        prepared.1@.len() == host@.len(),
        forall|i: int|
            0 <= i < host@.len() ==> (#[trigger] prepared.1@[i]).0 == host@[i].0 && prepared_root(
                prepared.1@[i].1,
            ) < prepared.0.nodes@.len() && prepared_definition(prepared.0, prepared.1@[i].1)
                == view_definition(host@[i].1),
        wf_code(prepared.0, host_keys(view_host(host@))),
        code_marked(prepared.0, final(bound)@),
        index_wf(prepared.2),
        prepared.2.keys@ == host_keys(view_host(host@)),
        sorted(final(bound)@),
        includes(old(bound)@, final(bound)@),
{
    let ghost keys = host_keys(view_host(host@));
    let mut cells: Vec<u128> = Vec::new();
    let mut firsts: Vec<usize> = Vec::new();
    let mut all: Vec<u128> = Vec::new();
    let mut i = 0;
    while i < host.len()
        invariant
            i <= host@.len(),
            all@ == keys.subrange(0, i as int),
            keys.len() == host@.len(),
            forall|k: int| 0 <= k < host@.len() ==> #[trigger] keys[k] == host@[k].0,
            sorted(cells@),
            cells@.len() == firsts@.len(),
            forall|k: int|
                0 <= k < cells@.len() ==> first_index(keys, #[trigger] cells@[k]) == Some(
                    firsts@[k] as int,
                ),
            forall|k: int| 0 <= k < i ==> cells@.contains(#[trigger] keys[k]),
        decreases host.len() - i,
    {
        let cell = host[i].0;
        all.push(cell);
        proof {
            assert(all@ =~= keys.subrange(0, i + 1));
        }
        match find_cell(&cells, cell) {
            Ok(_at) => {
                proof {
                    assert(cells@[_at as int] == keys[i as int]);
                }
            },
            Err(at) => {
                proof {
                    lemma_first_index(keys, cell);
                    assert forall|j: int| 0 <= j < i implies keys[j] != cell by {
                        assert(cells@.contains(keys[j]));
                    }
                    if first_index(keys, cell) is Some {
                        let first = first_index(keys, cell)->Some_0;
                        if first > i {
                            assert(keys[i as int] == cell);
                        }
                    } else {
                        assert(keys[i as int] == cell);
                    }
                    assert(first_index(keys, cell) == Some(i as int));
                }
                let ghost before = cells@;
                let ghost before_firsts = firsts@;
                cells.insert(at, cell);
                firsts.insert(at, i);
                proof {
                    assert forall|a: int, b: int| 0 <= a < b < cells@.len() implies cells@[a]
                        < cells@[b] by {
                        if b < at {
                            assert(cells@[a] == before[a] && cells@[b] == before[b]);
                        } else if b == at {
                            assert(cells@[a] == before[a]);
                        } else if a < at {
                            assert(cells@[a] == before[a] && cells@[b] == before[b - 1]);
                        } else if a == at {
                            assert(cells@[b] == before[b - 1]);
                        } else {
                            assert(cells@[a] == before[a - 1] && cells@[b] == before[b - 1]);
                        }
                    }
                    assert forall|k: int| 0 <= k < cells@.len() implies first_index(
                        keys,
                        #[trigger] cells@[k],
                    ) == Some(firsts@[k] as int) by {
                        if k < at {
                            assert(cells@[k] == before[k] && firsts@[k] == before_firsts[k]);
                        } else if k > at {
                            assert(cells@[k] == before[k - 1] && firsts@[k] == before_firsts[k
                                - 1]);
                        }
                    }
                    assert forall|k: int| 0 <= k <= i implies cells@.contains(#[trigger] keys[k]) by {
                        if k < i {
                            let m = choose|m: int| 0 <= m < before.len() && before[m] == keys[k];
                            if m < at {
                                assert(cells@[m] == keys[k]);
                            } else {
                                assert(cells@[m + 1] == keys[k]);
                            }
                        } else {
                            assert(cells@[at as int] == keys[k]);
                        }
                    }
                }
            },
        }
        i += 1;
    }
    proof {
        assert(all@ =~= keys);
    }
    let index = Index { cells, firsts, keys: all };
    let mut code = Code { nodes: Vec::new(), fields: Vec::new() };
    let mut entries: Vec<(u128, Prepared)> = Vec::new();
    let mut j = 0;
    while j < host.len()
        invariant
            j <= host@.len(),
            index_wf(index),
            index.keys@ == keys,
            entries@.len() == j,
            wf_code(code, keys),
            code_marked(code, bound@),
            sorted(bound@),
            includes(old(bound)@, bound@),
            forall|k: int|
                0 <= k < j ==> (#[trigger] entries@[k]).0 == host@[k].0 && prepared_root(entries@[k].1)
                    < code.nodes@.len() && prepared_definition(code, entries@[k].1)
                    == view_definition(host@[k].1),
        decreases host.len() - j,
    {
        let ghost before = code;
        let entry = match &host[j].1 {
            ExecDefinition::Grap(value) => Prepared::Grap(
                lower(value, &index, bound, &mut code),
            ),
            ExecDefinition::Foreign(description) => Prepared::Foreign(
                lower(description, &index, bound, &mut code),
            ),
            ExecDefinition::Primitive(description, primitive) => Prepared::Primitive(
                lower(description, &index, bound, &mut code),
                *primitive,
            ),
        };
        proof {
            assert forall|k: int| 0 <= k < j implies prepared_definition(
                code,
                (#[trigger] entries@[k]).1,
            ) == view_definition(host@[k].1) by {
                assert(code.nodes@[prepared_root(entries@[k].1) as int] == before.nodes@[prepared_root(
                    entries@[k].1,
                ) as int]);
            }
        }
        entries.push((host[j].0, entry));
        j += 1;
    }
    (code, entries, index)
}

// Values.

fn duplicate(value: &Runtime) -> (copied: Runtime)
    ensures
        copied == *value,
{
    match value {
        Runtime::Data(data) => Runtime::Data(copy(data)),
        Runtime::Number(number, bits) => Runtime::Number(*number, *bits),
        Runtime::Closure(closure) => Runtime::Closure(Rc::clone(closure)),
    }
}

fn share(environment: &Environment) -> (shared: Environment)
    ensures
        shared == *environment,
{
    match environment {
        None => None,
        Some(frame) => Some(Rc::clone(frame)),
    }
}

/// `value` as GID data.
fn materialize(value: &Runtime) -> (made: ExecValue)
    ensures
        view(made) == means(*value),
    decreases *value,
{
    match value {
        Runtime::Data(data) => copy(data),
        Runtime::Number(number, bits) => exec_single(
            tag(*number),
            ExecValue::Blob(Rc::new(encode(*bits, width(*number)))),
        ),
        Runtime::Closure(closure) => {
            let mut inner = copy_pairs(&closure.fields);
            let pairs = Rc::new(flatten(&closure.environment));
            let environment = ExecValue::Record(pairs);
            proof {
                lemma_view_record(pairs);
                lemma_view_pairs_push(inner@, (ENVIRONMENT, environment));
            }
            inner.push((ENVIRONMENT, environment));
            let inner = Rc::new(inner);
            proof {
                lemma_view_record(inner);
            }
            exec_single(CLOSURE, ExecValue::Record(inner))
        },
    }
}

/// An environment's bindings as GID pairs, outermost first, so a later pair
/// shadows an earlier one as in a record.
#[verifier::loop_isolation(false)]
#[verifier::rlimit(40)]
fn flatten(environment: &Environment) -> (pairs: Vec<(u128, ExecValue)>)
    ensures
        view_map(pairs@) == environment_means(*environment),
    decreases *environment,
{
    match environment {
        None => {
            let pairs = Vec::new();
            proof {
                assert(view_pairs(pairs@) =~= Seq::<(u128, Value)>::empty());
            }
            pairs
        },
        Some(frame) => {
            let mut pairs = flatten(&frame.parent);
            let ghost before = pairs@;
            let mut i = 0;
            proof {
                assert(view_pairs(pairs@) =~= view_pairs(before) + bindings_values(
                    frame.bindings@,
                ).subrange(0, 0));
            }
            while i < frame.bindings.len()
                invariant
                    i <= frame.bindings@.len(),
                    view_pairs(pairs@) == view_pairs(before) + bindings_values(
                        frame.bindings@,
                    ).subrange(0, i as int),
                decreases frame.bindings.len() - i,
            {
                let made = materialize(&frame.bindings[i].1);
                proof {
                    lemma_view_pairs_push(pairs@, (frame.bindings@[i as int].0, made));
                }
                pairs.push((frame.bindings[i].0, made));
                i += 1;
                proof {
                    assert(view_pairs(pairs@) =~= view_pairs(before) + bindings_values(
                        frame.bindings@,
                    ).subrange(0, i as int));
                }
            }
            proof {
                assert(bindings_values(frame.bindings@).subrange(
                    0,
                    frame.bindings@.len() as int,
                ) =~= bindings_values(frame.bindings@));
                lemma_to_map_concat(view_pairs(before), bindings_values(frame.bindings@));
            }
            pairs
        },
    }
}

fn is_absent_value(value: &Runtime) -> (absent: bool)
    ensures
        absent == is_absent(means(*value)),
{
    match value {
        Runtime::Data(data) => exec_is_absent(data),
        _ => false,
    }
}

/// The value bound to `cell` in the frames, innermost first.
#[verifier::loop_isolation(false)]
fn find<'a>(environment: &'a Environment, cell: u128, host: &Host, dynamic: bool) -> (found:
    Option<&'a Runtime>)
    requires
        environment_ok(*environment, keys_of(*host)),
        dynamic || marked_environment(*environment, host.bound@),
    ensures
        match found {
            Some(value) => environment_means(*environment).contains_key(cell) && environment_means(
                *environment,
            )[cell] == means(*value) && shape_ok(*value, keys_of(*host)) && (dynamic
                || marked_value(*value, host.bound@)),
            None => !environment_means(*environment).contains_key(cell),
        },
    decreases *environment,
{
    let ghost keys = keys_of(*host);
    let ghost bound = host.bound@;
    match environment {
        None => None,
        Some(frame) => {
            let ghost values = bindings_values(frame.bindings@);
            proof {
                assert(bindings_ok(frame.bindings@, keys));
                assert(dynamic || marked_bindings(frame.bindings@, bound));
            }
            let mut i = frame.bindings.len();
            while i > 0
                invariant
                    i <= frame.bindings@.len(),
                    values == bindings_values(frame.bindings@),
                    forall|j: int| i <= j < values.len() ==> #[trigger] values[j].0 != cell,
                decreases i,
            {
                i -= 1;
                if frame.bindings[i].0 == cell {
                    proof {
                        lemma_to_map_last(values, i as int);
                        assert(shape_ok(frame.bindings@[i as int].1, keys));
                    }
                    return Some(&frame.bindings[i].1);
                }
            }
            proof {
                lemma_to_map_keys(values, cell);
            }
            find(&frame.parent, cell, host, dynamic)
        },
    }
}

/// The value bound to `cell`; none, without looking, for a cell nothing
/// binds.
fn lookup<'a>(environment: &'a Environment, host: &Host, dynamic: bool, cell: u128) -> (found:
    Option<&'a Runtime>)
    requires
        wf_host(*host),
        environment_ok(*environment, keys_of(*host)),
        dynamic || marked_environment(*environment, host.bound@),
    ensures
        match found {
            Some(value) => environment_means(*environment).contains_key(cell) && environment_means(
                *environment,
            )[cell] == means(*value) && shape_ok(*value, keys_of(*host)) && (dynamic
                || marked_value(*value, host.bound@)),
            None => !environment_means(*environment).contains_key(cell),
        },
{
    if dynamic || marked(&host.bound, cell) {
        find(environment, cell, host, dynamic)
    } else {
        proof {
            lemma_unmarked(*environment, host.bound@, cell);
        }
        None
    }
}

/// The node of the call's field under `label`: its last, as a record reads.
#[verifier::loop_isolation(false)]
#[allow(unused_variables)]
fn field(code: &Code, call: usize, label: u128, host: &Host) -> (found: Option<usize>)
    requires
        call < code.nodes@.len(),
        code.nodes@[call as int].kind is Call,
        node_ok(*code, call as int, keys_of(*host)),
    ensures
        match found {
            Some(node) => node < code.nodes@.len() && call_map(*code, call as int).contains_key(
                label,
            ) && call_map(*code, call as int)[label] == source(*code, node as int),
            None => !call_map(*code, call as int).contains_key(label),
        },
{
    match &code.nodes[call].kind {
        Kind::Call { start, end, .. } => {
            let ghost pairs = call_pairs(*code, *start as int, *end as int);
            let mut i = *end;
            while i > *start
                invariant
                    *start <= i <= *end,
                    *end <= code.fields@.len(),
                    pairs == call_pairs(*code, *start as int, *end as int),
                    forall|j: int| i - *start <= j < pairs.len() ==> #[trigger] pairs[j].0 != label,
                decreases i - *start,
            {
                i -= 1;
                if code.fields[i].0 == label {
                    proof {
                        lemma_to_map_last(pairs, i - *start);
                        assert(code.fields@[i as int].1 < code.nodes@.len());
                    }
                    return Some(code.fields[i].1);
                }
            }
            proof {
                lemma_to_map_keys(pairs, label);
            }
            None
        },
        _ => None,
    }
}

// Numbers.

/// The bits of the `number` that `value` is.
fn bits(number: Number, value: &Runtime) -> (found: Option<u64>)
    ensures
        match found {
            Some(bits) => definition::number_bytes(number, means(*value)) == Some(
                definition::le_bytes(bits as nat, definition::width(number)),
            ),
            None => definition::number_bytes(number, means(*value)) is None,
        },
{
    match value {
        Runtime::Number(held, bits) => {
            proof {
                lemma_le_bytes_len(*bits as nat, definition::width(*held));
            }
            match (held, number) {
                (Number::F64, Number::F64) | (Number::F32, Number::F32) | (
                    Number::U64,
                    Number::U64,
                ) => Some(*bits),
                _ => None,
            }
        },
        Runtime::Data(data) => match number_bytes(number, data) {
            Some(bytes) => {
                proof {
                    lemma_le_round_trip(bytes@);
                }
                Some(decode(bytes))
            },
            None => None,
        },
        Runtime::Closure(_) => None,
    }
}

fn truth(answer: bool) -> (made: Runtime)
    ensures
        means(made) == definition::answer_value(Number::U64, Answer::Truth(answer)),
        !(made is Closure),
{
    Runtime::Data(
        exec_single(
            BOOL,
            ExecValue::Cell(
                if answer {
                    TRUE
                } else {
                    FALSE
                },
            ),
        ),
    )
}

fn failure(reason: u128) -> (made: Runtime)
    ensures
        means(made) == definition::answer_value(Number::U64, Answer::Failure(reason)),
        !(made is Closure),
{
    Runtime::Data(exec_single(ABSENT, ExecValue::Cell(reason)))
}

/// A binary operation on numbers' bits.
fn binary_bits(operation: &BinaryOperation, a: u64, b: u64) -> (made: Runtime)
    ensures
        means(made) == definition::answer_value(
            definition::binary_number(*operation),
            definition::binary_answer(
                *operation,
                definition::le_bytes(
                    a as nat,
                    definition::width(definition::binary_number(*operation)),
                ),
                definition::le_bytes(
                    b as nat,
                    definition::width(definition::binary_number(*operation)),
                ),
            ),
        ),
        !(made is Closure),
{
    proof {
        assert(power256(8) == 0x1_0000_0000_0000_0000) by (compute);
        lemma_le_bytes_round_trip(a as nat, 8);
        lemma_le_bytes_round_trip(b as nat, 8);
    }
    match operation {
        BinaryOperation::U64Sum(overflow) => if a <= u64::MAX - b {
            Runtime::Number(Number::U64, a + b)
        } else {
            failure(*overflow)
        },
        BinaryOperation::U64Subtract(overflow) => if b <= a {
            Runtime::Number(Number::U64, a - b)
        } else {
            failure(*overflow)
        },
        BinaryOperation::U64Multiply(overflow) => {
            proof {
                assert(a as int * b as int <= 0xFFFF_FFFF_FFFF_FFFF * 0xFFFF_FFFF_FFFF_FFFF)
                    by (nonlinear_arith)
                    requires
                        a <= 0xFFFF_FFFF_FFFF_FFFFu64,
                        b <= 0xFFFF_FFFF_FFFF_FFFFu64,
                ;
            }
            let product = (a as u128) * (b as u128);
            if product <= u64::MAX as u128 {
                Runtime::Number(Number::U64, product as u64)
            } else {
                failure(*overflow)
            }
        },
        BinaryOperation::U64Divide(division_by_zero) => if b == 0 {
            failure(*division_by_zero)
        } else {
            Runtime::Number(Number::U64, a / b)
        },
        BinaryOperation::U64Less => truth(a < b),
        BinaryOperation::U64Equal => truth(a == b),
        _ => {
            let number = binary_number(operation);
            match float_binary(operation, a, b) {
                FloatAnswer::Bits(bits) => Runtime::Number(number, bits),
                FloatAnswer::Truth(answer) => truth(answer),
            }
        },
    }
}

/// Why `value` isn't a `number`.
fn not_number(number: Number, value: &Runtime) -> (made: ExecValue)
    ensures
        view(made) == definition::not_a_number(number, means(*value)),
{
    not_a_number(number, &materialize(value))
}

fn binary(
    operation: &BinaryOperation,
    left: u128,
    first: &Runtime,
    right: u128,
    second: &Runtime,
) -> (made: Runtime)
    ensures
        means(made) == definition::binary(*operation, left, means(*first), right, means(*second)),
        !(made is Closure),
{
    let number = binary_number(operation);
    match (bits(number, first), bits(number, second)) {
        (Some(a), Some(b)) => binary_bits(operation, a, b),
        (Some(_), None) => Runtime::Data(
            caused_by_all(INVALID_ARGUMENTS, &one_detail(right, not_number(number, second))),
        ),
        (None, Some(_)) => Runtime::Data(
            caused_by_all(INVALID_ARGUMENTS, &one_detail(left, not_number(number, first))),
        ),
        (None, None) => {
            let mut details = one_detail(left, not_number(number, first));
            let why = not_number(number, second);
            proof {
                lemma_view_pairs_push(details@, (right, why));
            }
            details.push((right, why));
            proof {
                assert(view_pairs(details@) =~= seq![
                    (left, definition::not_a_number(number, means(*first))),
                    (right, definition::not_a_number(number, means(*second))),
                ]);
            }
            Runtime::Data(caused_by_all(INVALID_ARGUMENTS, &details))
        },
    }
}

fn unary(operation: &UnaryOperation, label: u128, value: &Runtime) -> (made: Runtime)
    ensures
        means(made) == definition::unary(*operation, label, means(*value)),
        !(made is Closure),
{
    match bits(Number::F64, value) {
        Some(a) => match float_unary(operation, a) {
            FloatAnswer::Bits(bits) => Runtime::Number(Number::F64, bits),
            FloatAnswer::Truth(answer) => truth(answer),
        },
        None => Runtime::Data(
            caused_by_all(INVALID_ARGUMENTS, &one_detail(label, not_number(Number::F64, value))),
        ),
    }
}

// Evaluation.

fn eval(
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        ready(**code, *environment, *host, old(state).dynamic),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::eval(
            source(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 0nat, 0nat,
{
    proof {
        assert(node_ok(**code, node as int, keys_of(*host)));
        lemma_host_keys(*host);
    }
    if fuel < 2 {
        return (Outcome::OutOfFuel, fuel);
    }
    let rest = fuel - 1;
    let (outcome, spent) = match &code.nodes[node].kind {
        Kind::Reference { cell, resolved } => reference(
            *cell,
            *resolved,
            environment,
            host,
            state,
            rest,
        ),
        Kind::Call { .. } => call(code, node, environment, host, state, rest),
        Kind::Lambda { params, body, fields } => {
            let closure = Rc::new(
                Closure {
                    params: Rc::clone(params),
                    body: *body,
                    code: Rc::clone(code),
                    fields: Rc::clone(fields),
                    environment: share(environment),
                },
            );
            proof {
                assert(lambda_marked(code.nodes@[node as int].kind, host.bound@) || state.dynamic);
            }
            (Outcome::Done(Runtime::Closure(closure)), 0)
        },
        Kind::Malformed(made) => (Outcome::Done(Runtime::Data(copy(made))), 0),
        Kind::Wrapped(inner) => eval(code, *inner, environment, host, state, rest),
        Kind::Number(number, bits) => (Outcome::Done(Runtime::Number(*number, *bits)), 0),
        Kind::Data => (Outcome::Done(Runtime::Data(copy(&code.nodes[node].source))), 0),
    };
    (outcome, spent + 1)
}

/// The node a host entry's cell evaluates as when referenced.
fn entry_root(host: &Host, entry: usize) -> (node: usize)
    requires
        entry < host.entries@.len(),
    ensures
        node == prepared_root(host.entries@[entry as int].1),
{
    match &host.entries[entry].1 {
        Prepared::Grap(root) => *root,
        Prepared::Foreign(root) => *root,
        Prepared::Primitive(root, _) => *root,
    }
}

fn reference(
    cell: u128,
    resolved: Option<usize>,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        wf_host(*host),
        environment_ok(*environment, keys_of(*host)),
        old(state).dynamic || marked_environment(*environment, host.bound@),
        resolved_ok(keys_of(*host), cell, resolved),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::reference(
            cell,
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 1nat, 0nat,
{
    proof {
        lemma_host_keys(*host);
        lemma_resolve(host_means(*host), cell);
    }
    match lookup(environment, host, state.dynamic, cell) {
        Some(bound) => (Outcome::Done(duplicate(bound)), 0),
        None => match resolved {
            None => (Outcome::Done(Runtime::Data(missing_cell(cell))), 0),
            Some(entry) => match referenced_at(state.stack.as_slice(), cell) {
                Some(at) => (Outcome::Done(Runtime::Data(cycle(&state.stack, at, cell))), 0),
                None => {
                    let root = entry_root(host, entry);
                    proof {
                        assert(prepared_root(host.entries@[entry as int].1) < host.code.nodes@.len());
                    }
                    state.stack.push((cell, Reached::Reference));
                    let evaluated = eval(&host.code, root, environment, host, state, fuel);
                    state.stack.pop();
                    evaluated
                },
            },
        },
    }
}

fn call(
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::call(
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 4nat, 0nat,
{
    proof {
        assert(node_ok(**code, node as int, keys_of(*host)));
    }
    let function = match &code.nodes[node].kind {
        Kind::Call { function, .. } => *function,
        _ => 0,
    };
    proof {
        assert(node_ok(**code, function as int, keys_of(*host)));
    }
    match &code.nodes[function].kind {
        Kind::Reference { cell, resolved } => match lookup(
            environment,
            host,
            state.dynamic,
            *cell,
        ) {
            Some(bound) => {
                if fuel < 2 {
                    (Outcome::OutOfFuel, fuel)
                } else {
                    let bound = duplicate(bound);
                    let (outcome, spent) = apply(
                        &bound,
                        code,
                        node,
                        environment,
                        host,
                        state,
                        fuel - 1,
                    );
                    (outcome, spent + 1)
                }
            },
            None => through_cell(*cell, *resolved, code, node, environment, host, state, fuel),
        },
        _ => {
            let (outcome, spent) = eval(code, function, environment, host, state, fuel);
            match outcome {
                Outcome::Done(callable) => {
                    let (applied, more) = apply(
                        &callable,
                        code,
                        node,
                        environment,
                        host,
                        state,
                        fuel - spent,
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

fn through_cell(
    cell: u128,
    resolved: Option<usize>,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
        resolved_ok(keys_of(*host), cell, resolved),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::through_cell(
            cell,
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 2nat, 0nat,
{
    proof {
        lemma_host_keys(*host);
        lemma_resolve(host_means(*host), cell);
    }
    if fuel < 2 {
        return (Outcome::OutOfFuel, fuel);
    }
    let rest = fuel - 1;
    let (outcome, spent) = match resolved {
        None => (Outcome::Done(Runtime::Data(missing_cell(cell))), 0),
        Some(entry) => match &host.entries[entry].1 {
            Prepared::Foreign(_) => (Outcome::Beyond, 0),
            Prepared::Primitive(_, primitive) => primitive_call(
                primitive,
                code,
                node,
                environment,
                host,
                state,
                rest,
            ),
            Prepared::Grap(root) => {
                proof {
                    assert(prepared_root(host.entries@[entry as int].1) < host.code.nodes@.len());
                }
                call_grap(cell, *root, code, node, environment, host, state, rest)
            },
        },
    };
    (outcome, spent + 1)
}

/// What `definition::through_cell` does once `cell` resolves to Grap's
/// `definition`, with the fuel left after its unit.
pub open spec fn through_grap(
    cell: u128,
    definition: Value,
    call_fields: Map<u128, Value>,
    environment: Map<u128, Value>,
    host: Seq<(u128, Definition)>,
    stack: Seq<(u128, Reached)>,
    fuel: nat,
) -> (definition::Outcome, nat) {
    match definition::first_resolving(stack, cell) {
        Some(at) => (definition::Outcome::Done(definition::cycle(stack, at, cell)), 0),
        None => {
            let (evaluated, spent) = definition::eval(
                definition,
                environment,
                host,
                stack.push((cell, Reached::Call)),
                fuel,
            );
            match evaluated {
                definition::Outcome::Done(callable) => if is_absent(callable) {
                    (definition::Outcome::Done(callable), spent)
                } else {
                    let (applied, more) = definition::apply(
                        callable,
                        call_fields,
                        environment,
                        host,
                        stack,
                        definition::left(fuel, spent),
                    );
                    (applied, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
    }
}

/// Calling through `cell`, whose definition is Grap's, at host node `root`.
fn call_grap(
    cell: u128,
    root: usize,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        root < host.code.nodes@.len(),
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == through_grap(
            cell,
            source(*host.code, root as int),
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 5nat, 0nat,
{
    let ghost stack = state.stack@;
    let ghost pushed = stack.push((cell, Reached::Call));
    match first_resolving(state.stack.as_slice(), cell) {
        Some(at) => (Outcome::Done(Runtime::Data(cycle(&state.stack, at, cell))), 0),
        None => {
            state.stack.push((cell, Reached::Call));
            proof {
                assert(state.stack@ == pushed);
            }
            let (evaluated, spent) = eval(&host.code, root, environment, host, state, fuel);
            proof {
                assert(outcome_means(evaluated) == definition::eval(
                    source(*host.code, root as int),
                    environment_means(*environment),
                    host_means(*host),
                    pushed,
                    fuel as nat,
                ).0 && spent as nat == definition::eval(
                    source(*host.code, root as int),
                    environment_means(*environment),
                    host_means(*host),
                    pushed,
                    fuel as nat,
                ).1);
            }
            state.stack.pop();
            proof {
                assert(state.stack@ =~= stack);
            }
            match evaluated {
                Outcome::Done(callable) => {
                    if is_absent_value(&callable) {
                        (Outcome::Done(callable), spent)
                    } else {
                        let (applied, more) = apply(
                            &callable,
                            code,
                            node,
                            environment,
                            host,
                            state,
                            fuel - spent,
                        );
                        proof {
                            assert(outcome_means(applied) == definition::apply(
                                means(callable),
                                call_map(**code, node as int),
                                environment_means(*environment),
                                host_means(*host),
                                stack,
                                definition::left(fuel as nat, spent as nat),
                            ).0 && more as nat == definition::apply(
                                means(callable),
                                call_map(**code, node as int),
                                environment_means(*environment),
                                host_means(*host),
                                stack,
                                definition::left(fuel as nat, spent as nat),
                            ).1);
                        }
                        (applied, spent + more)
                    }
                },
                stopped => (stopped, spent),
            }
        },
    }
}

#[verifier::loop_isolation(false)]
fn apply(
    callable: &Runtime,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
        shape_ok(*callable, keys_of(*host)),
        old(state).dynamic || marked_value(*callable, host.bound@),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::apply(
            means(*callable),
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 3nat, 0nat,
{
    proof {
        lemma_host_keys(*host);
    }
    match callable {
        Runtime::Closure(closure) => {
            proof {
                lemma_closure(keys_of(*host));
                lemma_frame_empty();
                assert(closure.params@.subrange(0, closure.params@.len() as int)
                    =~= closure.params@);
            }
            bind(
                &closure.params,
                0,
                Vec::new(),
                closure.body,
                &closure.code,
                &closure.environment,
                code,
                node,
                environment,
                host,
                state,
                fuel,
            )
        },
        Runtime::Data(data) => match closure(data) {
            Some((params, body, captured)) => {
                state.dynamic = true;
                let mut scratch: Vec<u128> = Vec::new();
                let mut body_code = Code { nodes: Vec::new(), fields: Vec::new() };
                let root = lower(body, &host.index, &mut scratch, &mut body_code);
                let mut bindings: Vec<(u128, Runtime)> = Vec::new();
                let mut i = 0;
                proof {
                    assert(bindings_values(bindings@) =~= view_pairs(captured@).subrange(0, 0));
                }
                while i < captured.len()
                    invariant
                        i <= captured@.len(),
                        bindings_values(bindings@) == view_pairs(captured@).subrange(0, i as int),
                        bindings_ok(bindings@, keys_of(*host)),
                    decreases captured.len() - i,
                {
                    let binding = (captured[i].0, Runtime::Data(copy(&captured[i].1)));
                    proof {
                        lemma_bindings_push(bindings@, binding);
                    }
                    bindings.push(binding);
                    i += 1;
                    proof {
                        assert(bindings_values(bindings@) =~= view_pairs(captured@).subrange(
                            0,
                            i as int,
                        ));
                    }
                }
                let ghost made = bindings@;
                let frame = Some(Rc::new(Frame { parent: None, bindings }));
                proof {
                    assert(view_pairs(captured@).subrange(0, captured@.len() as int) =~= view_pairs(
                        captured@,
                    ));
                    assert(environment_means(None) == Map::<u128, Value>::empty());
                    assert(environment_ok(None, keys_of(*host)));
                    assert(frame->Some_0.bindings@ == made);
                    assert(bindings_values(made) == view_pairs(captured@));
                    assert(Map::<u128, Value>::empty().union_prefer_right(to_map(view_pairs(captured@)))
                        =~= view_map(captured@));
                    assert(environment_means(frame) =~= view_map(captured@));
                    lemma_frame_empty();
                    assert(params@.subrange(0, params@.len() as int) =~= params@);
                }
                bind(
                    &params,
                    0,
                    Vec::new(),
                    root,
                    &Rc::new(body_code),
                    &frame,
                    code,
                    node,
                    environment,
                    host,
                    state,
                    fuel,
                )
            },
            None => match named_cell(data) {
                Some(cell) => through_cell(
                    cell,
                    resolve_index(&host.index, cell),
                    code,
                    node,
                    environment,
                    host,
                    state,
                    fuel,
                ),
                None => (
                    Outcome::Done(
                        Runtime::Data(exec_absent(NOT_CALLABLE, ABSENT_VALUE, copy(data))),
                    ),
                    0,
                ),
            },
        },
        Runtime::Number(_number, _bits) => {
            proof {
                lemma_number_value(*_number, *_bits);
            }
            (
                Outcome::Done(
                    Runtime::Data(exec_absent(NOT_CALLABLE, ABSENT_VALUE, materialize(callable))),
                ),
                0,
            )
        },
    }
}

/// Binding a closure's parameters from `i` on to the call's arguments,
/// evaluated where the call is, over the bindings made so far; then
/// evaluating its body in what it captured, with them.
fn bind(
    params: &Vec<u128>,
    i: usize,
    bindings: Vec<(u128, Runtime)>,
    body: usize,
    body_code: &Rc<Code>,
    captured: &Environment,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        i <= params@.len(),
        body < body_code.nodes@.len(),
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
        wf_code(**body_code, keys_of(*host)),
        environment_ok(*captured, keys_of(*host)),
        bindings_ok(bindings@, keys_of(*host)),
        old(state).dynamic || (code_marked(**body_code, host.bound@) && marked_environment(
            *captured,
            host.bound@,
        ) && marked_bindings(bindings@, host.bound@) && all_marked(params@, host.bound@)),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::bind(
            params@.subrange(i as int, params@.len() as int),
            source(**body_code, body as int),
            frame_means(*captured, bindings@),
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 2nat, params@.len() - i,
{
    proof {
        lemma_host_keys(*host);
        assert(node_ok(**code, node as int, keys_of(*host)));
    }
    if i == params.len() {
        let frame = Some(Rc::new(Frame { parent: share(captured), bindings }));
        return eval(body_code, body, &frame, host, state, fuel);
    }
    let parameter = params[i];
    proof {
        let rest = params@.subrange(i as int, params@.len() as int);
        assert(rest[0] == parameter);
        assert(rest.drop_first() =~= params@.subrange(i + 1, params@.len() as int));
    }
    let mut bindings = bindings;
    match field(code, node, parameter, host) {
        Some(argument) => {
            let (evaluated, spent) = eval(code, argument, environment, host, state, fuel);
            match evaluated {
                Outcome::Done(value) => {
                    let binding = (parameter, value);
                    proof {
                        lemma_frame_push(*captured, bindings@, binding);
                    }
                    bindings.push(binding);
                    let (outcome, more) = bind(
                        params,
                        i + 1,
                        bindings,
                        body,
                        body_code,
                        captured,
                        code,
                        node,
                        environment,
                        host,
                        state,
                        fuel - spent,
                    );
                    (outcome, spent + more)
                },
                stopped => (stopped, spent),
            }
        },
        None => {
            let binding = (parameter, Runtime::Data(missing_argument(parameter)));
            proof {
                lemma_frame_push(*captured, bindings@, binding);
            }
            bindings.push(binding);
            bind(
                params,
                i + 1,
                bindings,
                body,
                body_code,
                captured,
                code,
                node,
                environment,
                host,
                state,
                fuel,
            )
        },
    }
}

fn operand(
    label: u128,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::operand(
            label,
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 1nat, 0nat,
{
    proof {
        assert(node_ok(**code, node as int, keys_of(*host)));
    }
    match field(code, node, label, host) {
        Some(argument) => eval(code, argument, environment, host, state, fuel),
        None => (Outcome::Done(Runtime::Data(missing_argument(label))), 0),
    }
}

fn primitive_call(
    primitive: &Primitive,
    code: &Rc<Code>,
    node: usize,
    environment: &Environment,
    host: &Host,
    state: &mut State,
    fuel: usize,
) -> (evaluated: (Outcome, usize))
    requires
        node < code.nodes@.len(),
        code.nodes@[node as int].kind is Call,
        ready(**code, *environment, *host, old(state).dynamic),
    ensures
        final(state).stack@ == old(state).stack@,
        old(state).dynamic ==> final(state).dynamic,
        evaluated.1 <= fuel,
        (outcome_means(evaluated.0), evaluated.1 as nat) == definition::primitive_call(
            *primitive,
            call_map(**code, node as int),
            environment_means(*environment),
            host_means(*host),
            old(state).stack@,
            fuel as nat,
        ),
        result_ok(evaluated.0, *host, final(state).dynamic),
    decreases fuel, 2nat, 0nat,
{
    match primitive {
        Primitive::Binary { operation, left, right } => {
            let (first, spent) = operand(*left, code, node, environment, host, state, fuel);
            match first {
                Outcome::Done(first) => {
                    let (second, more) = operand(
                        *right,
                        code,
                        node,
                        environment,
                        host,
                        state,
                        fuel - spent,
                    );
                    match second {
                        Outcome::Done(second) => (
                            Outcome::Done(binary(operation, *left, &first, *right, &second)),
                            spent + more,
                        ),
                        stopped => (stopped, spent + more),
                    }
                },
                stopped => (stopped, spent),
            }
        },
        Primitive::Unary { operation, operand: label } => {
            let (value, spent) = operand(*label, code, node, environment, host, state, fuel);
            match value {
                Outcome::Done(value) => (Outcome::Done(unary(operation, *label, &value)), spent),
                stopped => (stopped, spent),
            }
        },
    }
}

/// A program evaluated with `fuel` against `host`'s definitions, exactly as
/// `definition::evaluate` says: `None` when it calls a foreign function.
pub fn evaluate(expression: &ExecValue, host: &Vec<(u128, ExecDefinition)>, fuel: usize) -> (evaluation:
    Option<ExecEvaluation>)
    ensures
        match (evaluation, definition::evaluate(view(*expression), view_host(host@), fuel as nat)) {
            (Some(evaluation), Some((result, remaining, completed))) => view(evaluation.result)
                == result && evaluation.remaining_fuel as nat == remaining && evaluation.completed
                == completed,
            (None, None) => true,
            _ => false,
        },
{
    let mut bound: Vec<u128> = Vec::new();
    let (host_code, entries, index) = prepare(host, &mut bound);
    let ghost keys = host_keys(view_host(host@));
    let ghost host_bound = bound@;
    let mut code = Code { nodes: Vec::new(), fields: Vec::new() };
    let root = lower(expression, &index, &mut bound, &mut code);
    proof {
        lemma_code_marked_grows(host_code, host_bound, bound@);
    }
    let prepared = Host { code: Rc::new(host_code), entries, index, bound };
    proof {
        assert(keys_of(prepared) =~= keys);
        assert(host_means(prepared) =~= view_host(host@));
    }
    let code = Rc::new(code);
    let mut state = State { stack: Vec::new(), dynamic: false };
    let (outcome, spent) = eval(&code, root, &None, &prepared, &mut state, fuel);
    proof {
        assert(state.stack@ =~= Seq::<(u128, Reached)>::empty());
    }
    match outcome {
        Outcome::Done(value) => Some(
            ExecEvaluation { result: materialize(&value), remaining_fuel: fuel - spent, completed: true },
        ),
        Outcome::OutOfFuel => Some(
            ExecEvaluation { result: exec_fuel_exhausted(), remaining_fuel: 0, completed: false },
        ),
        Outcome::Beyond => None,
    }
}

} // verus!
