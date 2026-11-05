//! The definition's number operations as code that runs. u64 arithmetic is
//! proven to compute what the definition says. The float operations are the
//! one place the evaluator is trusted rather than proven: the definition
//! names them, and `float_binary` and `float_unary` run Rust's.

use crate::absent::caused_by_all;
#[cfg(verus_only)]
use crate::definition;
#[cfg(verus_only)]
use crate::definition::Answer;
use crate::definition::{BinaryOperation, Number, UnaryOperation};
use crate::value::*;
use std::rc::Rc;
use verus_builtin_macros::verus;
#[cfg(verus_only)]
use vstd::prelude::*;

verus! {

/// What an operation comes to, as the evaluator holds it.
pub enum ExecAnswer {
    Number(Vec<u8>),
    Truth(bool),
    Failure(u128),
}

pub open spec fn view_answer(answer: ExecAnswer) -> Answer {
    match answer {
        ExecAnswer::Number(bytes) => Answer::Number(bytes@),
        ExecAnswer::Truth(truth) => Answer::Truth(truth),
        ExecAnswer::Failure(reason) => Answer::Failure(reason),
    }
}

fn tag(number: Number) -> (cell: u128)
    ensures
        cell == definition::tag(number),
{
    match number {
        Number::F64 => F64,
        Number::F32 => F32,
        Number::U64 => U64,
    }
}

fn width(number: Number) -> (bytes: usize)
    ensures
        bytes as nat == definition::width(number),
{
    match number {
        Number::F32 => 4,
        _ => 8,
    }
}

fn not_reason(number: Number) -> (cell: u128)
    ensures
        cell == definition::not_reason(number),
{
    match number {
        Number::F64 => NOT_F64,
        Number::F32 => NOT_F32,
        Number::U64 => NOT_U64,
    }
}

fn binary_number(operation: &BinaryOperation) -> (number: Number)
    ensures
        number == definition::binary_number(*operation),
{
    match operation {
        BinaryOperation::F64Sum
        | BinaryOperation::F64Multiply
        | BinaryOperation::F64Subtract
        | BinaryOperation::F64Divide
        | BinaryOperation::F64Min
        | BinaryOperation::F64Max
        | BinaryOperation::F64Hypot
        | BinaryOperation::F64Less
        | BinaryOperation::F64Equal => Number::F64,
        BinaryOperation::F32Sum
        | BinaryOperation::F32Subtract
        | BinaryOperation::F32Multiply
        | BinaryOperation::F32Divide
        | BinaryOperation::F32Less
        | BinaryOperation::F32Equal => Number::F32,
        _ => Number::U64,
    }
}

/// The bytes of the `number` that `value` is.
fn number_bytes(number: Number, value: &ExecValue) -> (bytes: Option<&[u8]>)
    ensures
        match bytes {
            Some(bytes) => definition::number_bytes(number, view(*value)) == Some(bytes@),
            None => definition::number_bytes(number, view(*value)) is None,
        },
{
    match value {
        ExecValue::Record(fields) => {
            proof {
                lemma_view_record(*fields);
            }
            match get(fields, tag(number)) {
                Some(ExecValue::Blob(bytes)) => if bytes.len() == width(number) {
                    Some(bytes.as_slice())
                } else {
                    None
                },
                _ => None,
            }
        },
        _ => None,
    }
}

/// Why `value` isn't a `number`.
fn not_a_number(number: Number, value: &ExecValue) -> (made: ExecValue)
    ensures
        view(made) == definition::not_a_number(number, view(*value)),
{
    if exec_is_absent(value) {
        copy(value)
    } else {
        exec_absent(not_reason(number), ABSENT_VALUE, copy(value))
    }
}

/// An answer as a value.
fn answer_value(number: Number, answer: ExecAnswer) -> (made: ExecValue)
    ensures
        view(made) == definition::answer_value(number, view_answer(answer)),
{
    match answer {
        ExecAnswer::Number(bytes) => exec_single(tag(number), ExecValue::Blob(Rc::new(bytes))),
        ExecAnswer::Truth(truth) => exec_single(
            BOOL,
            ExecValue::Cell(
                if truth {
                    TRUE
                } else {
                    FALSE
                },
            ),
        ),
        ExecAnswer::Failure(reason) => exec_single(ABSENT, ExecValue::Cell(reason)),
    }
}

/// `[(label, why)]`, the details of an operand that isn't a number.
fn one_detail(label: u128, why: ExecValue) -> (details: Vec<(u128, ExecValue)>)
    ensures
        view_pairs(details@) == seq![(label, view(why))],
{
    let mut details = Vec::new();
    proof {
        lemma_view_pairs_push(details@, (label, why));
        assert(view_pairs(Seq::<(u128, ExecValue)>::empty()) =~= Seq::<(u128, Value)>::empty());
    }
    details.push((label, why));
    proof {
        assert(view_pairs(details@) =~= seq![(label, view(why))]);
    }
    details
}

/// A binary operation on its operands' values, as `definition::binary` says.
pub fn binary(
    operation: &BinaryOperation,
    first_label: u128,
    first: &ExecValue,
    second_label: u128,
    second: &ExecValue,
) -> (made: ExecValue)
    ensures
        view(made) == definition::binary(*operation, first_label, view(*first), second_label, view(*second)),
{
    let number = binary_number(operation);
    match (number_bytes(number, first), number_bytes(number, second)) {
        (Some(left), Some(right)) => answer_value(number, binary_answer(operation, left, right)),
        (Some(_), None) => caused_by_all(
            INVALID_ARGUMENTS,
            &one_detail(second_label, not_a_number(number, second)),
        ),
        (None, Some(_)) => caused_by_all(
            INVALID_ARGUMENTS,
            &one_detail(first_label, not_a_number(number, first)),
        ),
        (None, None) => {
            let mut details = one_detail(first_label, not_a_number(number, first));
            let why = not_a_number(number, second);
            proof {
                lemma_view_pairs_push(details@, (second_label, why));
            }
            details.push((second_label, why));
            proof {
                assert(view_pairs(details@) =~= seq![
                    (first_label, definition::not_a_number(number, view(*first))),
                    (second_label, definition::not_a_number(number, view(*second))),
                ]);
            }
            caused_by_all(INVALID_ARGUMENTS, &details)
        },
    }
}

/// A unary operation on its operand's value, as `definition::unary` says.
pub fn unary(operation: &UnaryOperation, label: u128, value: &ExecValue) -> (made: ExecValue)
    ensures
        view(made) == definition::unary(*operation, label, view(*value)),
{
    let number = Number::F64;
    match number_bytes(number, value) {
        Some(bytes) => answer_value(number, unary_answer(operation, bytes)),
        None => caused_by_all(INVALID_ARGUMENTS, &one_detail(label, not_a_number(number, value))),
    }
}

/// What a binary operation comes to on its operands' bytes: u64 arithmetic
/// here, floats through `float_binary`.
fn binary_answer(operation: &BinaryOperation, left: &[u8], right: &[u8]) -> (answer: ExecAnswer)
    requires
        left@.len() == definition::width(definition::binary_number(*operation)),
        right@.len() == definition::width(definition::binary_number(*operation)),
    ensures
        view_answer(answer) == definition::binary_answer(*operation, left@, right@),
{
    match operation {
        BinaryOperation::U64Sum(overflow) => {
            let (a, b) = (decode(left), decode(right));
            if a <= u64::MAX - b {
                ExecAnswer::Number(encode(a + b))
            } else {
                ExecAnswer::Failure(*overflow)
            }
        },
        BinaryOperation::U64Subtract(overflow) => {
            let (a, b) = (decode(left), decode(right));
            if b <= a {
                ExecAnswer::Number(encode(a - b))
            } else {
                ExecAnswer::Failure(*overflow)
            }
        },
        BinaryOperation::U64Multiply(overflow) => {
            let (a, b) = (decode(left), decode(right));
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
                ExecAnswer::Number(encode(product as u64))
            } else {
                ExecAnswer::Failure(*overflow)
            }
        },
        BinaryOperation::U64Divide(division_by_zero) => {
            let (a, b) = (decode(left), decode(right));
            if b == 0 {
                ExecAnswer::Failure(*division_by_zero)
            } else {
                ExecAnswer::Number(encode(a / b))
            }
        },
        BinaryOperation::U64Less => ExecAnswer::Truth(decode(left) < decode(right)),
        BinaryOperation::U64Equal => ExecAnswer::Truth(decode(left) == decode(right)),
        _ => float_binary(operation, left, right),
    }
}

/// What a unary operation comes to on its operand's bytes.
fn unary_answer(operation: &UnaryOperation, operand: &[u8]) -> (answer: ExecAnswer)
    requires
        operand@.len() == 8,
    ensures
        view_answer(answer) == definition::unary_answer(*operation, operand@),
{
    float_unary(operation, operand)
}

/// 256 to the `k`.
pub open spec fn power256(k: nat) -> nat
    decreases k,
{
    if k == 0 {
        1
    } else {
        256 * power256((k - 1) as nat)
    }
}

/// The u64 eight little-endian bytes hold.
fn decode(bytes: &[u8]) -> (n: u64)
    requires
        bytes@.len() == 8,
    ensures
        n as nat == definition::le(bytes@),
{
    let mut n: u64 = 0;
    let mut i: usize = 8;
    proof {
        assert(bytes@.subrange(8, 8) =~= Seq::<u8>::empty());
    }
    while i > 0
        invariant
            i <= 8,
            bytes@.len() == 8,
            n as nat == definition::le(bytes@.subrange(i as int, 8)),
            (n as nat) < power256((8 - i) as nat),
        decreases i,
    {
        i -= 1;
        proof {
            let rest = bytes@.subrange(i as int, 8);
            assert(rest.drop_first() =~= bytes@.subrange(i + 1, 8));
            assert(rest[0] == bytes@[i as int]);
            let below = power256((8 - (i + 1)) as nat);
            assert(power256((8 - i) as nat) == 256 * below);
            assert(n as int * 256 + (bytes@[i as int] as int) < 256 * below) by (nonlinear_arith)
                requires
                    (n as int) < below,
                    bytes@[i as int] < 256,
            ;
            assert(power256(8) == 0x1_0000_0000_0000_0000) by (compute);
            if i > 0 {
                lemma_power256_grows((8 - i) as nat, 8);
            }
        }
        n = n * 256 + bytes[i] as u64;
    }
    proof {
        assert(bytes@.subrange(0, 8) =~= bytes@);
    }
    n
}

pub proof fn lemma_power256_grows(k: nat, m: nat)
    requires
        k <= m,
    ensures
        power256(k) <= power256(m),
    decreases m,
{
    if k < m {
        lemma_power256_grows(k, (m - 1) as nat);
    }
}

/// `n` as eight little-endian bytes.
fn encode(n: u64) -> (bytes: Vec<u8>)
    ensures
        bytes@ == definition::le_bytes(n as nat, 8),
{
    let mut bytes = Vec::new();
    let mut rest = n;
    let mut i: usize = 0;
    proof {
        assert(bytes@ + definition::le_bytes(rest as nat, 8) =~= definition::le_bytes(n as nat, 8));
    }
    while i < 8
        invariant
            i <= 8,
            bytes@ + definition::le_bytes(rest as nat, (8 - i) as nat) == definition::le_bytes(
                n as nat,
                8,
            ),
        decreases 8 - i,
    {
        let ghost before = bytes@;
        let ghost remaining = (8 - i) as nat;
        bytes.push((rest % 256) as u8);
        proof {
            assert(definition::le_bytes(rest as nat, remaining) == seq![((rest as nat) % 256) as u8]
                + definition::le_bytes((rest as nat) / 256, (remaining - 1) as nat));
            assert(bytes@ =~= before.push(((rest as nat) % 256) as u8));
            assert(bytes@ + definition::le_bytes((rest as nat) / 256, (remaining - 1) as nat)
                =~= before + definition::le_bytes(rest as nat, remaining));
        }
        rest = rest / 256;
        i += 1;
    }
    proof {
        assert(bytes@ + Seq::<u8>::empty() =~= bytes@);
    }
    bytes
}

/// A float binary operation, run as Rust runs it, by `run_float_binary`:
/// the definition's `float_arithmetic` and `float_comparison`, which it
/// trusts this to compute.
#[verifier::external_body]
fn float_binary(operation: &BinaryOperation, left: &[u8], right: &[u8]) -> (answer: ExecAnswer)
    requires
        !(definition::binary_number(*operation) is U64),
        left@.len() == definition::width(definition::binary_number(*operation)),
        right@.len() == definition::width(definition::binary_number(*operation)),
    ensures
        view_answer(answer) == definition::binary_answer(*operation, left@, right@),
{
    run_float_binary(operation, left, right)
}

/// A float unary operation, run as Rust runs it, by `run_float_unary`: the
/// definition's `float_unary` and `float_predicate`, which it trusts this to
/// compute.
#[verifier::external_body]
fn float_unary(operation: &UnaryOperation, operand: &[u8]) -> (answer: ExecAnswer)
    requires
        operand@.len() == 8,
    ensures
        view_answer(answer) == definition::unary_answer(*operation, operand@),
{
    run_float_unary(operation, operand)
}

} // verus!

// The float operations themselves, which Verus doesn't read: Rust's, on the
// numbers the operands' bytes hold. The trusted functions above promise these
// compute what the definition names.

fn run_float_binary(operation: &BinaryOperation, left: &[u8], right: &[u8]) -> ExecAnswer {
    let f64s = |run: fn(f64, f64) -> ExecAnswer| run(f64_of(left), f64_of(right));
    let f32s = |run: fn(f32, f32) -> ExecAnswer| run(f32_of(left), f32_of(right));
    match operation {
        BinaryOperation::F64Sum => f64s(|a, b| f64_answer(a + b)),
        BinaryOperation::F64Multiply => f64s(|a, b| f64_answer(a * b)),
        BinaryOperation::F64Subtract => f64s(|a, b| f64_answer(a - b)),
        BinaryOperation::F64Divide => f64s(|a, b| f64_answer(a / b)),
        BinaryOperation::F64Min => f64s(|a, b| f64_answer(a.min(b))),
        BinaryOperation::F64Max => f64s(|a, b| f64_answer(a.max(b))),
        BinaryOperation::F64Hypot => f64s(|a, b| f64_answer(a.hypot(b))),
        BinaryOperation::F64Less => f64s(|a, b| ExecAnswer::Truth(a < b)),
        BinaryOperation::F64Equal => f64s(|a, b| ExecAnswer::Truth(a == b)),
        BinaryOperation::F32Sum => f32s(|a, b| f32_answer(a + b)),
        BinaryOperation::F32Subtract => f32s(|a, b| f32_answer(a - b)),
        BinaryOperation::F32Multiply => f32s(|a, b| f32_answer(a * b)),
        BinaryOperation::F32Divide => f32s(|a, b| f32_answer(a / b)),
        BinaryOperation::F32Less => f32s(|a, b| ExecAnswer::Truth(a < b)),
        BinaryOperation::F32Equal => f32s(|a, b| ExecAnswer::Truth(a == b)),
        _ => unreachable!("u64 operations are proven, not trusted"),
    }
}

fn run_float_unary(operation: &UnaryOperation, operand: &[u8]) -> ExecAnswer {
    let a = f64_of(operand);
    match operation {
        UnaryOperation::F64Sin => f64_answer(a.sin()),
        UnaryOperation::F64Cos => f64_answer(a.cos()),
        UnaryOperation::F64Floor => f64_answer(a.floor()),
        UnaryOperation::F64Ceil => f64_answer(a.ceil()),
        UnaryOperation::F64IsFinite => ExecAnswer::Truth(a.is_finite()),
    }
}

fn f64_of(bytes: &[u8]) -> f64 {
    f64::from_le_bytes(bytes.try_into().expect("an f64's eight bytes"))
}

fn f32_of(bytes: &[u8]) -> f32 {
    f32::from_le_bytes(bytes.try_into().expect("an f32's four bytes"))
}

fn f64_answer(number: f64) -> ExecAnswer {
    ExecAnswer::Number(number.to_le_bytes().to_vec())
}

fn f32_answer(number: f32) -> ExecAnswer {
    ExecAnswer::Number(number.to_le_bytes().to_vec())
}
