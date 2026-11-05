//! Grap's core, defined: what it means, as Verus specification functions in
//! `definition`, and an evaluator, `evaluate`, proven to compute exactly
//! that. Grap's own tests check its real evaluator against it.
//!
//! The core is what Grap's evaluator does itself: cells, lambdas, calls,
//! value wrappers, closures, the absents those come to, and fuel; and the
//! number operations it runs natively, which the numbers library registers as
//! primitives. Calls to other foreign functions, and so the rest of the
//! libraries' functions, aren't part of it yet. `make verify` proves the
//! evaluator, trusting only the float operations it runs, which the
//! definition names rather than defines; see docs/verification.md.

mod absent;
mod definition;
mod evaluator;
mod number;
mod value;

pub use definition::{BinaryOperation, CHAIN_LIMIT, Number, Primitive, Reached, UnaryOperation};
pub use evaluator::{ExecDefinition, ExecEvaluation, evaluate};
pub use value::{
    ABSENT, ABSENT_CELL, ABSENT_VALUE, BODY, BODY_ORIGIN, BOOL, CELL_CYCLE, CLOSURE, CYCLE,
    ENVIRONMENT, ExecValue, F32, F64, FALSE, FFI, FUEL_EXHAUSTED, FUNCTION, INVALID_ARGUMENTS,
    INVALID_PARAMETER, MALFORMED_LAMBDA, MISSING_ARGUMENT, MISSING_CELL, NOT_CALLABLE, NOT_F32,
    NOT_F64, NOT_U64, PARAMS, TRUE, U64, VALUE,
};
