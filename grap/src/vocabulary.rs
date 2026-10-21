use gid::CellId;

pub const FUNCTION: CellId = CellId::from_u128(0x751fca4373debdd0b7e6eb73e08d684b);
pub const PARAMS: CellId = CellId::from_u128(0x195b378d0d31d90ab0d7366c15346b70);
pub const BODY: CellId = CellId::from_u128(0x986143866eda2e2fbf9ab8484357a0c9);
pub const VALUE: CellId = CellId::from_u128(0x5adde9ececa6ad57c81a3b2907e75e08);
/// A closure's lambda, as written: beside it are the environment it closes
/// over and where its body came from.
pub const LAMBDA: CellId = CellId::from_u128(0xe070bde324bb9d90e1d457a2fdba6bf7);
pub const ENVIRONMENT: CellId = CellId::from_u128(0xe910025c710c25c43d0a5b296378f374);
pub const FFI: CellId = CellId::from_u128(0x912adb7252d689659b6de9eeeb827658);
pub const EVALUATE: CellId = CellId::from_u128(0xacfc5e50881292518dab3cec77cf43ee);
pub const EXPRESSION: CellId = CellId::from_u128(0xccc55b0eb63b9f564ea74436094d4014);
