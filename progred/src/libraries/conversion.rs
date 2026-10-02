//! Conversions between number types, belonging to neither type.

use crate::libraries::{Library, f32, name};
use ::grap::ForeignFunctions;
use gid::{CellId, Cells};

pub const ID: CellId = CellId::from_u128(0x20540168af7321cb256f4e275e9389f9);

pub mod vocabulary {
    use gid::CellId;

    pub const F32_FROM_F64: CellId = CellId::from_u128(0x73bd2225d2091ba893a5570e8e9bacff);
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::F32_FROM_F64,
        f32::convention().conversion(|operand| operand.as_f64().map(|number| number as f32)),
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::F32_FROM_F64, name::record("f32 from f64", []));
    Library::named(
        ID,
        "conversion",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::runtime_partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::{f64, number};
    use gid::Value;

    #[test]
    fn f32_from_f64_rounds_at_the_explicit_numeric_boundary() {
        let evaluate = |expression: &Value| {
            crate::libraries::test_evaluate(expression, |_| None, &functions(), 20).result
        };
        let convert = |operand| {
            evaluate(&::grap::call(
                vocabulary::F32_FROM_F64.into(),
                [(number::vocabulary::OPERAND, operand)],
            ))
        };
        for number in [
            0.0,
            -0.0,
            0.1,
            -3.25,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::INFINITY,
        ] {
            let result = convert(f64::value(number));
            assert_eq!(
                f32::read(&result).unwrap().to_bits(),
                (number as f32).to_bits()
            );
        }
        assert!(f32::read(&convert(f64::value(f64::NAN))).unwrap().is_nan());
        let invalid = f32::value(1.0);
        assert_eq!(
            convert(invalid.clone()),
            ::grap::absent::with_detail(
                f32::vocabulary::INVALID_INPUT,
                number::vocabulary::OPERAND,
                invalid,
            )
        );
        assert!(::grap::absent::is_absent(&evaluate(&::grap::call(
            vocabulary::F32_FROM_F64.into(),
            [],
        ))));
    }
}
