//! Grap's core, defined: what it means, as Verus specification functions in
//! `definition`, and two evaluators proven to compute exactly that: a simple
//! one, `evaluate`, and one built for speed, `fast::evaluate`. Grap's own
//! tests check its real evaluator against them.
//!
//! The core is what Grap's evaluator does itself: cells; the calls, lambdas
//! and value wrappers a record's `isa` makes it; closures; the absents those
//! come to; where code was written, which closures and some absents carry;
//! fuel; and how deeply evaluations nest. With it come the number operations
//! it runs natively, which the numbers library registers as primitives.
//! Calls to other foreign functions, and so the rest of the libraries'
//! functions, aren't part of it yet. `make verify` proves the evaluators,
//! trusting only the float operations they run, which the definition names
//! rather than defines; see docs/verification.md.

mod absent;
mod definition;
mod evaluator;
pub mod fast;
mod number;
mod origin;
mod value;

pub use definition::{
    BinaryOperation, CHAIN_LIMIT, Halt, Implementation, MAX_DEPTH, Number, Primitive, Start,
    UnaryOperation,
};
pub use evaluator::{ExecDefinition, ExecEvaluation, evaluate};
pub use value::{
    ABSENT_CALL, ABSENT_CELL, ABSENT_VALUE, BODY, BODY_ORIGIN, BOOL, DEPTH_EXHAUSTED, ENVIRONMENT,
    ExecValue, F32, F64, FALSE, FUEL_EXHAUSTED, FUNCTION, IMPLEMENTATION, INVALID_ARGUMENTS,
    INVALID_PARAMETER, ISA, KIND_ABSENT, KIND_CALL, KIND_CLOSURE, KIND_FOREIGN_FUNCTION,
    KIND_LAMBDA, KIND_VALUE_WRAPPER, LAMBDA, MALFORMED_LAMBDA, MISSING_ARGUMENT, MISSING_CELL,
    MISSING_FIELD, NOT_CALLABLE, NOT_F32, NOT_F64, NOT_U64, ORIGIN_CELL, ORIGIN_INPUT, ORIGIN_PATH,
    ORIGIN_ROOT, PARAMS, REASON, STEP_ELEMENT, STEP_FOLLOW, STEP_INDEXABLE, STEP_KEY, TRUE, U64,
    VALUE,
};
