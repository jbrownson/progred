//! A deterministic, explicitly-threaded random stream for Grap. The
//! state is an opaque blob; sampling returns both the value and the
//! successor state, so evaluation itself remains deterministic.

use crate::{Library, absent, f64, name};
use gid::{Cells, Value};
use grap_runtime::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};

pub mod vocabulary {
    use gid::CellId;

    pub const RESET: CellId = CellId::from_u128(0xbced0768fa647daf95a663b3e626b4ae);
    pub const BETWEEN: CellId = CellId::from_u128(0xe8a99ada91800bca33709da0db80c29f);
    pub const STATE: CellId = CellId::from_u128(0x1c80fb9f0035b66347ae1529e42af98b);
    pub const MIN: CellId = CellId::from_u128(0x61213005b01abca2d4fb351cc4fe4d98);
    pub const MAX: CellId = CellId::from_u128(0x4be40301b427aeed939ef079ab63cf92);
    pub const VALUE: CellId = CellId::from_u128(0x725980a1344dbdce099fcdf189f77ba9);
    pub const INVALID_STATE: CellId = CellId::from_u128(0x8780e75d0f3b5e561e29a2dfa648ec44);
}

const INITIAL_STATE: u64 = 0x4d595df4d0f33173;

fn state(value: u64) -> Value {
    Value::from(value.to_le_bytes().to_vec())
}

fn read_state(value: &Value) -> Option<u64> {
    value
        .as_blob()
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_le_bytes)
}

fn evaluated(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    field: gid::CellId,
) -> Result<Option<Value>, Halt> {
    context
        .field(call, field)
        .map(|value| context.eval(value, environment))
        .transpose()
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::RESET,
            ForeignFunction::new(|_, _, _| Ok(state(INITIAL_STATE))),
        )
        .register(
            vocabulary::BETWEEN,
            ForeignFunction::new(|context, call, environment| {
                let Some(current) = evaluated(context, call, environment, vocabulary::STATE)?
                else {
                    return Ok(context.missing_argument(vocabulary::STATE));
                };
                let Some(min) = evaluated(context, call, environment, vocabulary::MIN)? else {
                    return Ok(context.missing_argument(vocabulary::MIN));
                };
                let Some(max) = evaluated(context, call, environment, vocabulary::MAX)? else {
                    return Ok(context.missing_argument(vocabulary::MAX));
                };
                let (Some(current), Some(min), Some(max)) =
                    (read_state(&current), f64::read(&min), f64::read(&max))
                else {
                    return Ok(Value::from(vocabulary::INVALID_STATE));
                };
                let next = current
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let unit = ((next >> 11) as f64) / ((1_u64 << 53) as f64);
                Ok(Value::record([
                    (vocabulary::STATE, state(next)),
                    (vocabulary::VALUE, f64::value(min + (max - min) * unit)),
                ]))
            }),
        )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::RESET, "reset random"),
        (vocabulary::BETWEEN, "random between"),
        (vocabulary::STATE, "random state"),
        (vocabulary::MIN, "minimum"),
        (vocabulary::MAX, "maximum"),
        (vocabulary::VALUE, "random value"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    cells.set_value(
        vocabulary::INVALID_STATE,
        absent::named("invalid random state"),
    );
    Library {
        cells,
        functions: functions(),
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grap_runtime as grap;

    #[test]
    fn reset_streams_repeat() {
        let sample = || {
            grap::evaluate(
                &grap::call(
                    Value::from(vocabulary::BETWEEN),
                    [
                        (
                            vocabulary::STATE,
                            grap::call(Value::from(vocabulary::RESET), []),
                        ),
                        (vocabulary::MIN, f64::value(-2.0)),
                        (vocabulary::MAX, f64::value(3.0)),
                    ],
                ),
                |_| None,
                &functions(),
                20,
            )
            .result
        };
        assert_eq!(sample(), sample());
    }
}
