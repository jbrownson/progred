use crate::SourceOrigin;
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
/// Why a function couldn't use its arguments, carrying why each it couldn't
/// use under its label.
pub const INVALID_ARGUMENTS: CellId = CellId::from_u128(0xaeb37f359ed1437cbc1adee0a05ac60f);

pub const CELL: CellId = CellId::from_u128(0x7ba69298ef92cf954456784b66e86032);
pub const VALUE: CellId = CellId::from_u128(0xcfeaf191b71e06afef86c27ee43eb936);
pub const CYCLE: CellId = CellId::from_u128(0x2776d28585d66ef8a747a8bc6dad7381);
/// Where the call an absent is about is, as [`crate::source`] records an
/// origin.
pub const CALL: CellId = CellId::from_u128(0x31232e62ec458b0e6e201f3a7c1c10fe);

pub fn with_detail(reason: CellId, field: CellId, detail: Value) -> Value {
    Value::record([(ABSENT, Value::from(reason)), (field, detail)])
}

/// Why a call has no argument under `cell`: carrying where the call is,
/// when it has a source, so the argument's place can be found.
pub fn missing_argument(cell: CellId, call: Option<&SourceOrigin>) -> Value {
    Value::record(
        [
            (ABSENT, Value::from(MISSING_ARGUMENT)),
            (CELL, Value::from(cell)),
        ]
        .into_iter()
        .chain(call.map(|call| (CALL, crate::source::value(call)))),
    )
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

/// The longest chain of causes an absent grows: past it, an absent passes on
/// unchanged instead of gaining another layer, so a loop that keeps using one,
/// such as a sum over a list, keeps its root causes and first uses without
/// growing a layer per element.
pub const CHAIN_LIMIT: usize = 8;

/// An absent for `reason` about `call`, naming where the call is when it
/// has a source, and carrying each value it couldn't use, absent or not,
/// under its field: its causes are those that are absent, so absents make a
/// tree. An absent the same as one before it isn't repeated, so a value used
/// twice doesn't double the tree. Where a chain of causes would grow past
/// [`CHAIN_LIMIT`], the longest passes on unchanged instead.
pub fn caused_by_all(
    reason: CellId,
    call: Option<&SourceOrigin>,
    details: impl IntoIterator<Item = (CellId, Value)>,
) -> Value {
    let mut fields: Vec<(CellId, Value)> = Vec::new();
    for (field, detail) in details {
        if !(is_absent(&detail) && fields.iter().any(|(_, seen)| *seen == detail)) {
            fields.push((field, detail));
        }
    }
    match fields.iter().max_by_key(|(_, detail)| chain_length(detail)) {
        Some((_, longest)) if chain_length(longest) >= CHAIN_LIMIT => longest.clone(),
        _ => Value::record(
            [(ABSENT, Value::from(reason))]
                .into_iter()
                .chain(call.map(|call| (CALL, crate::source::value(call))))
                .chain(fields),
        ),
    }
}

/// The absents among `value`'s details, which caused it.
pub fn causes(value: &Value) -> impl Iterator<Item = &Value> {
    is_absent(value)
        .then(|| value.as_record())
        .flatten()
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter(|(field, detail)| *field != ABSENT && is_absent(detail))
        .map(|(_, detail)| detail)
}

/// The absents at the ends of `value`'s causes, the first things that went
/// wrong, which a check for a particular reason looks at: `value` itself
/// when nothing caused it.
pub fn root_causes(value: &Value) -> Vec<&Value> {
    let causes: Vec<_> = causes(value).collect();
    if causes.is_empty() {
        vec![value]
    } else {
        causes.into_iter().flat_map(root_causes).collect()
    }
}

/// How many absents the longest chain of `value`'s causes holds: none for a
/// value that isn't absent.
pub fn chain_length(value: &Value) -> usize {
    if is_absent(value) {
        1 + causes(value).map(chain_length).max().unwrap_or(0)
    } else {
        0
    }
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
