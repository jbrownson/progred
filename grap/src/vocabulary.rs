use gid::CellId;

pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
pub const VALUE: CellId = CellId::from_u128(0x5adde9ececa6ad57c81a3b2907e75e08);
/// A closure's lambda, as written: beside it are the environment it closes
/// over and where its body came from.
pub const LAMBDA: CellId = CellId::from_u128(0xe070bde324bb9d90e1d457a2fdba6bf7);
pub const ENVIRONMENT: CellId = CellId::from_u128(0xe910025c710c25c43d0a5b296378f374);
pub const EVALUATE: CellId = CellId::from_u128(0xacfc5e50881292518dab3cec77cf43ee);
pub const EXPRESSION: CellId = CellId::from_u128(0xccc55b0eb63b9f564ea74436094d4014);
/// The cell the host implements a foreign function under.
pub const IMPLEMENTATION: CellId = CellId::from_u128(0x24e1165744a040d7a4a4ff7d46f3b03c);

/// The kinds of Grap's forms, as a record's `isa` names them.
pub mod kind {
    use gid::CellId;

    pub const CALL: CellId = CellId::from_u128(0x8dd318828a4c42cf9f2d5d1ba9b068fe);
    pub const LAMBDA: CellId = CellId::from_u128(0xe4a0653134ca48379cab2e008bed03e7);
    pub const CLOSURE: CellId = CellId::from_u128(0x0e8180c5b2f0492291ce5150f2292697);
    pub const VALUE_WRAPPER: CellId = CellId::from_u128(0xf615f8d777294ffd9e5296faf34edd9d);
    /// A function the host implements: `{isa: foreign function,
    /// implementation, …description}`, its `implementation` the cell the
    /// host implements it under, which is its own cell where a library
    /// defines it.
    pub const FOREIGN_FUNCTION: CellId = CellId::from_u128(0xa394a3373e334363ab5a2965ec3204fa);
    /// A value that isn't there: `{isa: absent, reason, …details}`, its
    /// reason a cell (see [`crate::absent`]).
    pub const ABSENT: CellId = CellId::from_u128(0x6509cd9921ab4cf28657ac75a9976d71);
}
