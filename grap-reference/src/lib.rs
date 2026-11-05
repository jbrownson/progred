//! Grap's core, defined: what it means, as Verus specification functions in
//! `definition`, and an evaluator, `evaluate`, proven to compute exactly
//! that. Grap's own tests check its real evaluator against it.
//!
//! The core is what Grap's evaluator does itself: cells, lambdas, calls,
//! value wrappers, closures, the absents those come to, and fuel. Calls to
//! foreign functions, and so the libraries' functions, aren't part of it
//! yet. `make verify` proves the evaluator; see docs/verification.md.

mod definition;
mod evaluator;
mod value;

pub use definition::Reached;
pub use evaluator::{ExecDefinition, ExecEvaluation, evaluate};
pub use value::{
    ABSENT, ABSENT_CELL, ABSENT_VALUE, BODY, BODY_ORIGIN, CELL_CYCLE, CLOSURE, CYCLE, ENVIRONMENT,
    ExecValue, FFI, FUEL_EXHAUSTED, FUNCTION, INVALID_PARAMETER, MALFORMED_LAMBDA,
    MISSING_ARGUMENT, MISSING_CELL, NOT_CALLABLE, PARAMS, VALUE,
};
