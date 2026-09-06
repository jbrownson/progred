//! An open f32 convention used by libraries whose host boundary is
//! single precision. It remains ordinary GID data and ordinary Grap
//! library behavior.

use crate::{Library, absent, line_edit, logic, name, number};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0xf8daecede6e48de724408cfb0e3090f8);
use grap_runtime::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use progred_display::{Layout, ProjectionInput, overlay_value};

pub mod vocabulary {
    use gid::CellId;

    pub const F32: CellId = CellId::from_u128(0x64810cfeb0631ca8875e282d1ad4af79);
    pub const UPDATE: CellId = CellId::from_u128(0x9c34c242d73e090cbd62de1242ad74ae);
    pub const SUM: CellId = CellId::from_u128(0x257e5967d826e69d28929cf365667ae1);
    pub const SUBTRACT: CellId = CellId::from_u128(0x5cd7408dab067d8b92c7a1bd9cda7b05);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd4df1fe63a95cda53c19ad7220054c24);
    pub const DIVIDE: CellId = CellId::from_u128(0xfbf1ebc4eeeea21f86af583f6ad85e71);
    pub const LESS: CellId = CellId::from_u128(0x3dde12d1f97ccca65c579ae2703bb07a);
    pub const EQUAL: CellId = CellId::from_u128(0xe0637a60944f8dd9afd2fb4c28214dc3);
    pub const LEFT_NOT_F32: CellId = CellId::from_u128(0x2f7a7dfd96df51008a563a36e075e50b);
    pub const RIGHT_NOT_F32: CellId = CellId::from_u128(0xd4a7b035dc57953dca5c1d20cb591d0d);
}

pub fn value(number: f32) -> Value {
    Value::record([(vocabulary::F32, Value::from(number.to_le_bytes().to_vec()))])
}

pub fn completions(query: &str) -> Vec<progred_display::Completion> {
    number::completions(query, vocabulary::F32, value)
}

pub fn read(value: &Value) -> Option<f32> {
    value
        .as_record()?
        .get(&vocabulary::F32)?
        .as_blob()?
        .try_into()
        .ok()
        .map(f32::from_le_bytes)
}

impl number::Scrubbable for f32 {
    fn magnitude(self) -> f64 {
        self.into()
    }

    fn minimum_precision() -> f64 {
        0.0
    }

    fn scrubbable(self) -> bool {
        self.is_finite()
    }

    fn from_offset(start: Self, offset: f64, precision: f64) -> Self {
        number::rounded(f64::from(start) + offset, precision) as f32
    }

    fn spelling(self, precision: f64) -> String {
        if precision >= 1.0 {
            self.round().to_string()
        } else {
            let decimal_places = (-precision.log10()).round().clamp(0.0, 8.0) as usize;
            format!("{self:.decimal_places$}")
        }
    }
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    Some(number::layout(
        input,
        read(input.value)?,
        vocabulary::F32,
        vocabulary::UPDATE,
        value,
    ))
}

fn update(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
        return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
    };
    let current = context
        .field(call, line_edit::vocabulary::CURRENT)
        .map(|current| context.eval(current, environment))
        .transpose()?;
    let input = context.eval(input, environment)?;
    Ok(
        match crate::text::read(&input).and_then(|text| text.trim().parse().ok()) {
            Some(number) => current
                .as_ref()
                .map(|current| overlay_value(current, value(number)))
                .unwrap_or_else(|| value(number)),
            None => absent::value(),
        },
    )
}

fn binary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f32, f32) -> Value,
) -> Result<Value, Halt> {
    let Some(left) = context.field(call, number::vocabulary::LEFT) else {
        return Ok(context.missing_argument(number::vocabulary::LEFT));
    };
    let Some(right) = context.field(call, number::vocabulary::RIGHT) else {
        return Ok(context.missing_argument(number::vocabulary::RIGHT));
    };
    let left = read(&context.eval(left, environment)?);
    let right = read(&context.eval(right, environment)?);
    Ok(match (left, right) {
        (Some(left), Some(right)) => operation(left, right),
        (None, _) => absent::with_reason(vocabulary::LEFT_NOT_F32),
        (_, None) => absent::with_reason(vocabulary::RIGHT_NOT_F32),
    })
}

fn arithmetic(operation: fn(f32, f32) -> f32) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        binary(context, call, environment, |left, right| {
            value(operation(left, right))
        })
    })
}

fn comparison(operation: fn(f32, f32) -> bool) -> ForeignFunction {
    ForeignFunction::new(move |context, call, environment| {
        binary(context, call, environment, |left, right| {
            logic::value(operation(left, right))
        })
    })
}

pub fn functions() -> ForeignFunctions {
    [
        (vocabulary::SUM, arithmetic(|left, right| left + right)),
        (vocabulary::SUBTRACT, arithmetic(|left, right| left - right)),
        (vocabulary::MULTIPLY, arithmetic(|left, right| left * right)),
        (vocabulary::DIVIDE, arithmetic(|left, right| left / right)),
        (vocabulary::LESS, comparison(|left, right| left < right)),
        (vocabulary::EQUAL, comparison(|left, right| left == right)),
    ]
    .into_iter()
    .fold(
        ForeignFunctions::default().register(vocabulary::UPDATE, ForeignFunction::new(update)),
        |functions, (cell, function)| functions.register(cell, function),
    )
}

pub fn library<World: 'static, Hover: Clone + 'static>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::F32, "f32"),
        (vocabulary::UPDATE, "f32 update"),
        (vocabulary::SUM, "+"),
        (vocabulary::SUBTRACT, "-"),
        (vocabulary::MULTIPLY, "*"),
        (vocabulary::DIVIDE, "/"),
        (vocabulary::LESS, "<"),
        (vocabulary::EQUAL, "=="),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    for (cell, reason) in [
        (vocabulary::LEFT_NOT_F32, "left is not f32"),
        (vocabulary::RIGHT_NOT_F32, "right is not f32"),
    ] {
        cells.set_value(cell, absent::named_reason(reason));
    }
    Library::named(
        ID,
        "f32",
        crate::Definitions::from_parts(cells, functions()),
        progred_display::partial(display::<World, Hover>),
    )
    .with_completions(|request| {
        (request.scope == progred_display::CompletionScope::Everything
            && request.kind == progred_display::CompletionKind::Value)
            .then(|| completions(request.query))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    fn call(function: CellId, left: Value, right: Value) -> Value {
        grap_runtime::call(
            Value::from(function),
            [
                (number::vocabulary::LEFT, left),
                (number::vocabulary::RIGHT, right),
            ],
        )
    }

    fn evaluate(expression: &Value) -> Value {
        crate::test_evaluate(expression, |_| None, &functions(), 20).result
    }

    #[test]
    fn representation_is_open_library_data() {
        assert_eq!(read(&value(1.25)), Some(1.25));
        assert_eq!(read(&Value::from(1.25_f32.to_le_bytes().to_vec())), None);

        let metadata = new_cell_id();
        let enriched = Value::record(
            value(1.25)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(vec![1])),
        );
        assert_eq!(read(&enriched), Some(1.25));
    }

    #[test]
    fn spelling_round_trips_through_the_update_function() {
        let updated = crate::test_evaluate(
            &grap_runtime::call(
                grap_runtime::ffi(vocabulary::UPDATE),
                [(line_edit::vocabulary::INPUT, crate::text::value("3.5"))],
            ),
            |_| None,
            &functions(),
            20,
        );

        assert_eq!(read(&updated.result), Some(3.5));
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let sum = call(vocabulary::SUM, value(2.0), value(3.0));
        assert_eq!(
            evaluate(&call(vocabulary::MULTIPLY, sum, value(4.0))),
            value(20.0)
        );
        let difference = call(vocabulary::SUBTRACT, value(7.0), value(1.0));
        assert_eq!(
            evaluate(&call(vocabulary::DIVIDE, difference, value(4.0))),
            value(1.5)
        );
    }

    #[test]
    fn comparisons_are_logic_values() {
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(1.0), value(2.0))),
            logic::value(true)
        );
        assert_eq!(
            evaluate(&call(vocabulary::LESS, value(2.0), value(1.0))),
            logic::value(false)
        );
        assert_eq!(
            evaluate(&call(vocabulary::EQUAL, value(2.0), value(2.0))),
            logic::value(true)
        );
        assert_eq!(
            evaluate(&call(vocabulary::EQUAL, value(2.0), value(2.5))),
            logic::value(false)
        );
    }

    #[test]
    fn operands_of_other_representations_decline() {
        assert_eq!(
            evaluate(&call(vocabulary::SUM, crate::f64::value(2.0), value(3.0))),
            absent::with_reason(vocabulary::LEFT_NOT_F32)
        );
        assert_eq!(
            evaluate(&call(
                vocabulary::SUM,
                value(2.0),
                Value::from(b"three".to_vec())
            )),
            absent::with_reason(vocabulary::RIGHT_NOT_F32)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library::<(), ()>();
        assert_eq!(
            library.value(vocabulary::SUM).and_then(name::read),
            Some("+")
        );
        assert_eq!(
            library.value(vocabulary::EQUAL).and_then(name::read),
            Some("==")
        );
        assert_eq!(
            library.value(vocabulary::LEFT_NOT_F32).and_then(name::read),
            Some("left is not f32")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F32)),
            Some(vocabulary::RIGHT_NOT_F32)
        );
    }
}
