use gid::{CellId, Value};

pub const ABSENT: CellId = CellId::from_u128(0xd9c0a7145a38859a245640d3469cbcd4);
pub const FUEL_EXHAUSTED: CellId = CellId::from_u128(0x513628d759c04b3e7088b575e555a80e);
pub const MISSING_CELL: CellId = CellId::from_u128(0xa5a1b4e3d0df96bd11af00f0780136ff);
pub const CELL_CYCLE: CellId = CellId::from_u128(0x150e0fc7e38d1670f41283c3d23a9b8d);
pub const MALFORMED_LAMBDA: CellId = CellId::from_u128(0xfbf5894d7f62b6d0048d17d26851b415);
pub const INVALID_PARAMETER: CellId = CellId::from_u128(0x93ca0e9199372c46ee24bd4c508e178c);
pub const NOT_CALLABLE: CellId = CellId::from_u128(0x8624488c2d10d2a4b84560dfa99a38e6);
pub const MISSING_ARGUMENT: CellId = CellId::from_u128(0x8b2f0db36e5c3d35595eb5666cc89c78);
pub const INVALID_ENVIRONMENT: CellId = CellId::from_u128(0x152f2cac01f072317ab5746c5befdf9c);
pub const NO_ALTERNATIVE: CellId = CellId::from_u128(0x3bf0543fa73a0cee9036317cdb5cacd9);
pub const EFFECTFUL_DECLINE: CellId = CellId::from_u128(0xdc2651b863aa8aaf2df25ccb0c1fef03);
pub const DECLINED: CellId = CellId::from_u128(0x1cab38a2c8cffe5c077adc29f74c2ddf);
pub const CAUSES: CellId = CellId::from_u128(0x2f34365ec4dce76324f482c08afe6aba);

pub const CELL: CellId = CellId::from_u128(0x7ba69298ef92cf954456784b66e86032);
pub const VALUE: CellId = CellId::from_u128(0xcfeaf191b71e06afef86c27ee43eb936);
pub const CYCLE: CellId = CellId::from_u128(0x2776d28585d66ef8a747a8bc6dad7381);

pub fn with_detail(reason: CellId, field: CellId, detail: Value) -> Value {
    Value::record([(ABSENT, Value::from(reason)), (field, detail)])
}

pub fn value(reason: CellId) -> Value {
    Value::record([(ABSENT, Value::from(reason))])
}

pub fn reason(value: &Value) -> Option<CellId> {
    value.as_record()?.get(&ABSENT).and_then(Value::as_cell)
}

pub fn is_absent(value: &Value) -> bool {
    reason(value).is_some()
}

pub fn decline() -> Value {
    value(DECLINED)
}

pub fn from_causes(causes: impl IntoIterator<Item = Value>) -> Value {
    match <[Value; 1]>::try_from(causes.into_iter().collect::<Vec<_>>()) {
        Ok([cause]) => cause,
        Err(causes) => Value::record([
            (ABSENT, Value::from(NO_ALTERNATIVE)),
            (CAUSES, Value::list(causes)),
        ]),
    }
}
