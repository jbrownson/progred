//! The definition's number operations as code that runs. u64 arithmetic is
//! proven to compute what the definition says. The float operations are the
//! one place the evaluators are trusted rather than proven: the definition
//! names them, and `float_binary` and `float_unary` run Rust's, on the
//! numbers' bits.

use crate::absent::caused_by_all;
#[cfg(verus_only)]
use crate::definition;
#[cfg(verus_only)]
use crate::definition::Answer;
use crate::definition::{BinaryOperation, Number, UnaryOperation};
use crate::origin::At;
#[cfg(verus_only)]
use crate::origin::view_at;
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

pub(crate) fn tag(number: Number) -> (cell: u128)
    ensures
        cell == definition::tag(number),
{
    match number {
        Number::F64 => F64,
        Number::F32 => F32,
        Number::U64 => U64,
    }
}

pub(crate) fn width(number: Number) -> (bytes: usize)
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

pub(crate) fn binary_number(operation: &BinaryOperation) -> (number: Number)
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
pub(crate) fn number_bytes(number: Number, value: &ExecValue) -> (bytes: Option<&[u8]>)
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
pub(crate) fn not_a_number(number: Number, value: &ExecValue) -> (made: ExecValue)
    ensures
        view(made) == definition::not_a_number(number, view(*value)),
{
    if exec_is_absent(value) {
        copy(value)
    } else {
        exec_with_detail(not_reason(number), ABSENT_VALUE, copy(value))
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
        ExecAnswer::Failure(reason) => exec_bare_absent(reason),
    }
}

/// `[(label, why)]`, the details of an operand that isn't a number.
pub(crate) fn one_detail(label: u128, why: ExecValue) -> (details: Vec<(u128, ExecValue)>)
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
    call: &At,
    first_label: u128,
    first: &ExecValue,
    second_label: u128,
    second: &ExecValue,
) -> (made: ExecValue)
    ensures
        view(made) == definition::binary(
            *operation,
            view_at(*call),
            first_label,
            view(*first),
            second_label,
            view(*second),
        ),
{
    let number = binary_number(operation);
    match (number_bytes(number, first), number_bytes(number, second)) {
        (Some(left), Some(right)) => answer_value(number, binary_answer(operation, left, right)),
        (Some(_), None) => caused_by_all(
            INVALID_ARGUMENTS,
            call,
            &one_detail(second_label, not_a_number(number, second)),
        ),
        (None, Some(_)) => caused_by_all(
            INVALID_ARGUMENTS,
            call,
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
            caused_by_all(INVALID_ARGUMENTS, call, &details)
        },
    }
}

/// A unary operation on its operand's value, as `definition::unary` says.
pub fn unary(operation: &UnaryOperation, call: &At, label: u128, value: &ExecValue) -> (made:
    ExecValue)
    ensures
        view(made) == definition::unary(*operation, view_at(*call), label, view(*value)),
{
    let number = Number::F64;
    match number_bytes(number, value) {
        Some(bytes) => answer_value(number, unary_answer(operation, bytes)),
        None => caused_by_all(
            INVALID_ARGUMENTS,
            call,
            &one_detail(label, not_a_number(number, value)),
        ),
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
                ExecAnswer::Number(encode(a + b, 8))
            } else {
                ExecAnswer::Failure(*overflow)
            }
        },
        BinaryOperation::U64Subtract(overflow) => {
            let (a, b) = (decode(left), decode(right));
            if b <= a {
                ExecAnswer::Number(encode(a - b, 8))
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
                ExecAnswer::Number(encode(product as u64, 8))
            } else {
                ExecAnswer::Failure(*overflow)
            }
        },
        BinaryOperation::U64Divide(division_by_zero) => {
            let (a, b) = (decode(left), decode(right));
            if b == 0 {
                ExecAnswer::Failure(*division_by_zero)
            } else {
                ExecAnswer::Number(encode(a / b, 8))
            }
        },
        BinaryOperation::U64Less => ExecAnswer::Truth(decode(left) < decode(right)),
        BinaryOperation::U64Equal => ExecAnswer::Truth(decode(left) == decode(right)),
        _ => {
            let (a, b) = (decode(left), decode(right));
            proof {
                lemma_le_round_trip(left@);
                lemma_le_round_trip(right@);
            }
            match float_binary(operation, a, b) {
                FloatAnswer::Bits(bits) => ExecAnswer::Number(
                    encode(bits, width(binary_number(operation))),
                ),
                FloatAnswer::Truth(truth) => ExecAnswer::Truth(truth),
            }
        },
    }
}

/// What a unary operation comes to on its operand's bytes.
fn unary_answer(operation: &UnaryOperation, operand: &[u8]) -> (answer: ExecAnswer)
    requires
        operand@.len() == 8,
    ensures
        view_answer(answer) == definition::unary_answer(*operation, operand@),
{
    let a = decode(operand);
    proof {
        lemma_le_round_trip(operand@);
    }
    match float_unary(operation, a) {
        FloatAnswer::Bits(bits) => ExecAnswer::Number(encode(bits, 8)),
        FloatAnswer::Truth(truth) => ExecAnswer::Truth(truth),
    }
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

/// The number up to eight little-endian bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> (n: u64)
    requires
        bytes@.len() <= 8,
    ensures
        n as nat == definition::le(bytes@),
{
    let length = bytes.len();
    let mut n: u64 = 0;
    let mut i: usize = length;
    proof {
        assert(bytes@.subrange(length as int, length as int) =~= Seq::<u8>::empty());
    }
    while i > 0
        invariant
            i <= length,
            length == bytes@.len(),
            length <= 8,
            n as nat == definition::le(bytes@.subrange(i as int, length as int)),
            (n as nat) < power256((length - i) as nat),
        decreases i,
    {
        i -= 1;
        proof {
            let rest = bytes@.subrange(i as int, length as int);
            assert(rest.drop_first() =~= bytes@.subrange(i + 1, length as int));
            assert(rest[0] == bytes@[i as int]);
            let below = power256((length - (i + 1)) as nat);
            assert(power256((length - i) as nat) == 256 * below);
            assert(n as int * 256 + (bytes@[i as int] as int) < 256 * below) by (nonlinear_arith)
                requires
                    (n as int) < below,
                    bytes@[i as int] < 256,
            ;
            assert(power256(8) == 0x1_0000_0000_0000_0000) by (compute);
            lemma_power256_grows((length - i) as nat, 8);
        }
        n = n * 256 + bytes[i] as u64;
    }
    proof {
        assert(bytes@.subrange(0, length as int) =~= bytes@);
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

/// Bytes read as a number and written back are the same bytes.
pub proof fn lemma_le_round_trip(bytes: Seq<u8>)
    ensures
        definition::le_bytes(definition::le(bytes), bytes.len()) == bytes,
        definition::le(bytes) < power256(bytes.len()),
    decreases bytes.len(),
{
    if bytes.len() > 0 {
        let rest = bytes.drop_first();
        lemma_le_round_trip(rest);
        let first = bytes[0] as int;
        let higher = definition::le(rest) as int;
        assert((first + 256 * higher) % 256 == first && (first + 256 * higher) / 256 == higher)
            by (nonlinear_arith)
            requires
                0 <= first < 256,
                0 <= higher,
        ;
        assert(definition::le_bytes(definition::le(bytes), bytes.len()) =~= bytes);
        assert(first + 256 * higher < 256 * power256(rest.len())) by (nonlinear_arith)
            requires
                0 <= first < 256,
                0 <= higher < power256(rest.len()),
        ;
    }
}

/// A number written as bytes and read back is the same number, when it fits.
pub proof fn lemma_le_bytes_round_trip(n: nat, width: nat)
    requires
        n < power256(width),
    ensures
        definition::le(definition::le_bytes(n, width)) == n,
        definition::le_bytes(n, width).len() == width,
    decreases width,
{
    if width > 0 {
        assert(n / 256 < power256((width - 1) as nat)) by (nonlinear_arith)
            requires
                n < 256 * power256((width - 1) as nat),
        ;
        lemma_le_bytes_round_trip(n / 256, (width - 1) as nat);
        let bytes = definition::le_bytes(n, width);
        assert(bytes.drop_first() =~= definition::le_bytes(n / 256, (width - 1) as nat));
        assert(((n % 256) as u8) as nat == n % 256);
        assert(n == n % 256 + 256 * (n / 256)) by (nonlinear_arith);
    }
}

/// `n` as `width` little-endian bytes.
pub(crate) fn encode(n: u64, width: usize) -> (bytes: Vec<u8>)
    requires
        width <= 8,
    ensures
        bytes@ == definition::le_bytes(n as nat, width as nat),
{
    let mut bytes = Vec::new();
    let mut rest = n;
    let mut i: usize = 0;
    proof {
        assert(bytes@ + definition::le_bytes(rest as nat, width as nat) =~= definition::le_bytes(
            n as nat,
            width as nat,
        ));
    }
    while i < width
        invariant
            i <= width,
            bytes@ + definition::le_bytes(rest as nat, (width - i) as nat) == definition::le_bytes(
                n as nat,
                width as nat,
            ),
        decreases width - i,
    {
        let ghost before = bytes@;
        let ghost remaining = (width - i) as nat;
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

/// What a float operation comes to, by its result's bits.
pub enum FloatAnswer {
    Bits(u64),
    Truth(bool),
}

/// A float answer as the definition's, for numbers `width` bytes wide.
pub open spec fn float_answer(answer: FloatAnswer, width: nat) -> Answer {
    match answer {
        FloatAnswer::Bits(bits) => Answer::Number(definition::le_bytes(bits as nat, width)),
        FloatAnswer::Truth(truth) => Answer::Truth(truth),
    }
}

/// A float binary operation, run as Rust runs it, by `run_float_binary`,
/// on the numbers whose bits these are: the definition's `float_arithmetic`
/// and `float_comparison`, which it trusts this to compute.
#[verifier::external_body]
pub(crate) fn float_binary(operation: &BinaryOperation, left: u64, right: u64) -> (answer:
    FloatAnswer)
    requires
        !(definition::binary_number(*operation) is U64),
    ensures
        float_answer(answer, definition::width(definition::binary_number(*operation)))
            == definition::binary_answer(
            *operation,
            definition::le_bytes(left as nat, definition::width(definition::binary_number(*operation))),
            definition::le_bytes(right as nat, definition::width(definition::binary_number(*operation))),
        ),
{
    run_float_binary(operation, left, right)
}

/// A float unary operation, run as Rust runs it, by `run_float_unary`, on the
/// f64 whose bits these are: the definition's `float_unary` and
/// `float_predicate`, which it trusts this to compute.
#[verifier::external_body]
pub(crate) fn float_unary(operation: &UnaryOperation, operand: u64) -> (answer: FloatAnswer)
    ensures
        float_answer(answer, 8) == definition::unary_answer(
            *operation,
            definition::le_bytes(operand as nat, 8),
        ),
{
    run_float_unary(operation, operand)
}

} // verus!

// The float operations themselves, which Verus doesn't read: Rust's, on the
// numbers whose bits the operands are. The trusted functions above promise
// these compute what the definition names: `from_bits` reads the same number
// `from_le_bytes` reads from those bits' little-endian bytes.

fn run_float_binary(operation: &BinaryOperation, left: u64, right: u64) -> FloatAnswer {
    let f64s = |run: fn(f64, f64) -> FloatAnswer| run(f64::from_bits(left), f64::from_bits(right));
    let f32s = |run: fn(f32, f32) -> FloatAnswer| {
        run(f32::from_bits(left as u32), f32::from_bits(right as u32))
    };
    match operation {
        BinaryOperation::F64Sum => f64s(|a, b| f64_answer(a + b)),
        BinaryOperation::F64Multiply => f64s(|a, b| f64_answer(a * b)),
        BinaryOperation::F64Subtract => f64s(|a, b| f64_answer(a - b)),
        BinaryOperation::F64Divide => f64s(|a, b| f64_answer(a / b)),
        BinaryOperation::F64Min => f64s(|a, b| f64_answer(a.min(b))),
        BinaryOperation::F64Max => f64s(|a, b| f64_answer(a.max(b))),
        BinaryOperation::F64Hypot => f64s(|a, b| f64_answer(a.hypot(b))),
        BinaryOperation::F64Less => f64s(|a, b| FloatAnswer::Truth(a < b)),
        BinaryOperation::F64Equal => f64s(|a, b| FloatAnswer::Truth(a == b)),
        BinaryOperation::F32Sum => f32s(|a, b| f32_answer(a + b)),
        BinaryOperation::F32Subtract => f32s(|a, b| f32_answer(a - b)),
        BinaryOperation::F32Multiply => f32s(|a, b| f32_answer(a * b)),
        BinaryOperation::F32Divide => f32s(|a, b| f32_answer(a / b)),
        BinaryOperation::F32Less => f32s(|a, b| FloatAnswer::Truth(a < b)),
        BinaryOperation::F32Equal => f32s(|a, b| FloatAnswer::Truth(a == b)),
        _ => unreachable!("u64 operations are proven, not trusted"),
    }
}

fn run_float_unary(operation: &UnaryOperation, operand: u64) -> FloatAnswer {
    let a = f64::from_bits(operand);
    match operation {
        UnaryOperation::F64Sin => f64_answer(a.sin()),
        UnaryOperation::F64Cos => f64_answer(a.cos()),
        UnaryOperation::F64Floor => f64_answer(a.floor()),
        UnaryOperation::F64Ceil => f64_answer(a.ceil()),
        UnaryOperation::F64IsFinite => FloatAnswer::Truth(a.is_finite()),
    }
}

fn f64_answer(number: f64) -> FloatAnswer {
    FloatAnswer::Bits(number.to_bits())
}

fn f32_answer(number: f32) -> FloatAnswer {
    FloatAnswer::Bits(number.to_bits() as u64)
}
