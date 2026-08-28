//! An f64 Grap library. The number representation is ordinary Grap
//! data; arithmetic is supplied to the evaluator as Rust foreign
//! functions.

use crate::{Library, absent, line_edit, logic, name};
use gid::{CellId, Cells, Step, Value};
#[cfg(test)]
use grap_runtime as grap;
use grap_runtime::vocabulary::FUNCTION;
use grap_runtime::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
};
use progred_display::{
    Delim, Layout, ProjectionInput, ScrubEvent, ScrubUpdate, bracket, on_scrub, overlay_value, row,
};
use std::rc::Rc;

pub mod vocabulary {
    use gid::CellId;

    pub const F64: CellId = CellId::from_u128(0xed11fde03b7c2c1ba2fccc3cdba5d561);
    pub const ADD: CellId = CellId::from_u128(0x201af445eb7e2c270bb5ead10b781fc1);
    pub const MULTIPLY: CellId = CellId::from_u128(0xd6f384c439d9d69996d545df422efd79);
    pub const SUBTRACT: CellId = CellId::from_u128(0x08d1ebc7fd4ce62efec9671f73e9b645);
    pub const DIVIDE: CellId = CellId::from_u128(0xb08dd4c44eeea43ab3c7593ddeedd742);
    pub const SIN: CellId = CellId::from_u128(0x9f62e0f56d92be74f6ae36378d889262);
    pub const COS: CellId = CellId::from_u128(0xfb05b6c9b1565ca4add3732e8ddbe2b4);
    pub const LESS: CellId = CellId::from_u128(0xed44dbf5b4cdf5c952e1ef00f219b655);
    pub const EQUAL: CellId = CellId::from_u128(0x22ab9aa3e7ce4f4f79a7039e1cc23773);
    pub const FLOOR: CellId = CellId::from_u128(0xd007814c5f6a6c38b025605b399473d4);
    pub const LERP: CellId = CellId::from_u128(0x432ad7a31ef129e353e251419e690ca1);
    pub const OPERAND: CellId = CellId::from_u128(0x50a20d15e4ae56be51b882de9d58c676);
    pub const PI: CellId = CellId::from_u128(0x9cd591f37312e563f52b7374a6cef5c0);
    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    pub const START: CellId = CellId::from_u128(0x4b4fb6349d2fd798e7aafca85a2deca8);
    pub const END: CellId = CellId::from_u128(0x3a816b0af0160bc77ba1948b122b3f29);
    pub const AMOUNT: CellId = CellId::from_u128(0x55e66fc5eb91699cf12833fb0a15d4b6);
    /// The f64 line's write-back rule: parse the typed spelling,
    /// other fields carried; unparseable input declines.
    pub const UPDATE: CellId = CellId::from_u128(0x6b95d2e04c7a1f38b1a08e57d24c96fb);
    pub const LEFT_NOT_F64: CellId = CellId::from_u128(0x50c0d2fd8fe0325a8e0e41f79ce86eff);
    pub const RIGHT_NOT_F64: CellId = CellId::from_u128(0xcab77cffe8c38745dd8e748ece331409);
    pub const OPERAND_NOT_F64: CellId = CellId::from_u128(0x9c2a4845e1c67df6dd43ac1116e76441);
    pub const START_NOT_F64: CellId = CellId::from_u128(0x809d7bba33afaf8a673846f0c44cd2e6);
    pub const END_NOT_F64: CellId = CellId::from_u128(0x17081dd43c4af46c54408cd125eaf3e2);
    pub const AMOUNT_NOT_F64: CellId = CellId::from_u128(0xe1977104f3574cd37a99ef9083dde01f);
}

pub fn value(value: f64) -> Value {
    Value::record([(vocabulary::F64, Value::from(value.to_le_bytes().to_vec()))])
}

pub fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    fields
        .get(&vocabulary::F64)
        .and_then(Value::as_blob)
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(f64::from_le_bytes)
}

pub fn display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let number = read(input.value)?;
    let line = line_edit::layout(
        number.to_string(),
        grap_runtime::ffi(vocabulary::UPDATE),
        "",
        "",
    );
    if !number.is_finite() {
        return Some(line);
    }
    let original = input.value.clone();
    let target = input.targets.current();
    Some(on_scrub(
        line,
        target.hover,
        Rc::new(move || {
            let original = original.clone();
            let mut scrub = NumberScrub::new(number);
            Box::new(move |event| {
                let scrubbed = scrub.update(event);
                ScrubUpdate {
                    value: overlay_value(&original, value(scrubbed.value)),
                    spelling: Some(spelling(scrubbed.value, scrubbed.precision)),
                }
            })
        }),
    ))
}

struct Scrubbed {
    value: f64,
    precision: f64,
}

const SCRUB_PIXELS_PER_STEP: f64 = 4.0;
const SCRUB_PIXELS_PER_DECADE: f64 = 24.0;
const SCRUB_DECADE_STRETCH: f64 = 1.5;

struct NumberScrub {
    base: f64,
    raw: f64,
    displayed: f64,
}

impl NumberScrub {
    fn new(start: f64) -> Self {
        Self {
            base: if start == 0.0 {
                0.01
            } else {
                10.0_f64
                    .powf(start.abs().log10().floor() - 2.0)
                    .min(1.0)
            },
            raw: start,
            displayed: start,
        }
    }

    fn update(&mut self, event: ScrubEvent) -> Scrubbed {
        let gain = 10.0_f64.powf(vertical_decades(event.distance_y).clamp(-16.0, 16.0));
        let scale = self.base * gain;
        let precision = nice_precision(scale);
        let horizontal_scale = scale / gain.max(1.0).cbrt();
        self.raw += event.movement_x * horizontal_scale / SCRUB_PIXELS_PER_STEP;
        let candidate = rounded(self.raw, precision);
        self.displayed = if event.movement_x > 0.0 {
            self.displayed.max(candidate)
        } else if event.movement_x < 0.0 {
            self.displayed.min(candidate)
        } else {
            self.displayed
        };
        Scrubbed {
            value: self.displayed,
            precision,
        }
    }
}

fn vertical_decades(distance_y: f64) -> f64 {
    let distance = distance_y.abs();
    let decades = (1.0 + (SCRUB_DECADE_STRETCH - 1.0) * distance / SCRUB_PIXELS_PER_DECADE)
        .log(SCRUB_DECADE_STRETCH);
    -distance_y.signum() * decades
}

fn nice_precision(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        let magnitude = 10.0_f64.powf(scale.log10().floor());
        let normalized = scale / magnitude;
        let coefficient = if normalized < 2.0 {
            1.0
        } else if normalized < 5.0 {
            2.0
        } else {
            5.0
        };
        coefficient * magnitude
    } else {
        scale
    }
}

fn rounded(value: f64, step: f64) -> f64 {
    if value.is_finite() && step.is_finite() && step > 0.0 {
        let snapped = (value / step).round() * step;
        let decimal_places = (-step.log10().floor()).max(0.0);
        let decimal_scale = 10.0_f64.powf(decimal_places);
        if decimal_scale.is_finite() {
            (snapped * decimal_scale).round() / decimal_scale
        } else {
            snapped
        }
    } else {
        value
    }
}

fn spelling(value: f64, precision: f64) -> String {
    if precision >= 1.0 {
        value.round().to_string()
    } else {
        let decimal_places = (-precision.log10()).round().clamp(0.0, 16.0) as usize;
        format!("{value:.decimal_places$}")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Comparison,
    Sum,
    Product,
}

fn precedence(function: CellId) -> Option<Precedence> {
    match function {
        vocabulary::ADD | vocabulary::SUBTRACT => Some(Precedence::Sum),
        vocabulary::MULTIPLY | vocabulary::DIVIDE => Some(Precedence::Product),
        vocabulary::LESS | vocabulary::EQUAL => Some(Precedence::Comparison),
        _ => None,
    }
}

fn expression_precedence(value: &Value) -> Option<Precedence> {
    let fields = value.as_record()?;
    (fields.len() == 3).then_some(())?;
    fields.get(&vocabulary::LEFT)?;
    fields.get(&vocabulary::RIGHT)?;
    fields
        .get(&FUNCTION)?
        .as_cell()
        .and_then(precedence)
}

fn operand<World, Hover: Clone>(
    field: CellId,
    value: &Value,
    parent: Precedence,
) -> Layout<World, Hover> {
    let child = crate::grap::expression_descend(Step::Key(field));
    match expression_precedence(value) {
        Some(child_precedence)
            if child_precedence < parent
                || (child_precedence == parent
                    && (field == vocabulary::RIGHT || parent == Precedence::Comparison)) =>
        {
            bracket(Delim::Paren, child)
        }
        _ => child,
    }
}

pub fn binary_display<World, Hover: Clone>(
    input: &ProjectionInput<'_, World, Hover>,
) -> Option<Layout<World, Hover>> {
    let fields = input.value.as_record()?;
    let function = fields.get(&FUNCTION)?;
    let precedence = precedence(function.as_cell()?)?;
    let left = fields.get(&vocabulary::LEFT)?;
    let right = fields.get(&vocabulary::RIGHT)?;
    (fields.len() == 3).then_some(row(
        6.0,
        [
            operand(vocabulary::LEFT, left, precedence),
            crate::grap::shallow_descend(Step::Key(FUNCTION)),
            operand(vocabulary::RIGHT, right, precedence),
        ],
    ))
}

pub fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .with_f64_representation(read, value)
        .register(
            vocabulary::UPDATE,
            ForeignFunction::new(|context, call, environment| {
                let Some(current) = context.field(call, line_edit::vocabulary::CURRENT) else {
                    return Ok(context.missing_argument(line_edit::vocabulary::CURRENT));
                };
                let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                    return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
                };
                let current = context.eval(current, environment)?;
                let input = context.eval(input, environment)?;
                Ok(crate::text::read(&input)
                    .and_then(|text| text.trim().parse::<f64>().ok())
                    .map(|number| overlay_value(&current, value(number)))
                    .unwrap_or_else(crate::absent::value))
            }),
        )
        .register(
            vocabulary::ADD,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left + right)
            }),
        )
        .register(
            vocabulary::MULTIPLY,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left * right)
            }),
        )
        .register(
            vocabulary::SUBTRACT,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left - right)
            }),
        )
        .register(
            vocabulary::DIVIDE,
            ForeignFunction::runtime(|context, call, environment| {
                binary(context, call, environment, |left, right| left / right)
            }),
        )
        .register(
            vocabulary::SIN,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::sin)
            }),
        )
        .register(
            vocabulary::COS,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::cos)
            }),
        )
        .register(
            vocabulary::FLOOR,
            ForeignFunction::runtime(|context, call, environment| {
                unary(context, call, environment, f64::floor)
            }),
        )
        .register(vocabulary::LERP, ForeignFunction::runtime(lerp))
        .register(
            vocabulary::LESS,
            ForeignFunction::runtime(|context, call, environment| {
                binary_value(context, call, environment, |left, right| {
                    RuntimeValue::from_value(logic::value(left < right))
                })
            }),
        )
        .register(
            vocabulary::EQUAL,
            ForeignFunction::runtime(|context, call, environment| {
                binary_value(context, call, environment, |left, right| {
                    RuntimeValue::from_value(logic::value(left == right))
                })
            }),
        )
}

fn lerp(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(start) = context.field(call, vocabulary::START) else {
        return Ok(context.missing_runtime_argument(vocabulary::START));
    };
    let Some(end) = context.field(call, vocabulary::END) else {
        return Ok(context.missing_runtime_argument(vocabulary::END));
    };
    let Some(amount) = context.field(call, vocabulary::AMOUNT) else {
        return Ok(context.missing_runtime_argument(vocabulary::AMOUNT));
    };
    let start = context.eval_runtime(start, environment)?;
    let end = context.eval_runtime(end, environment)?;
    let amount = context.eval_runtime(amount, environment)?;
    Ok(match (
        start.as_f64(read),
        end.as_f64(read),
        amount.as_f64(read),
    ) {
        (Some(start), Some(end), Some(amount)) => {
            RuntimeValue::f64(start + (end - start) * amount, value)
        }
        (None, _, _) => absent::with_reason(vocabulary::START_NOT_F64).into(),
        (_, None, _) => absent::with_reason(vocabulary::END_NOT_F64).into(),
        (_, _, None) => absent::with_reason(vocabulary::AMOUNT_NOT_F64).into(),
    })
}

fn binary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<RuntimeValue, Halt> {
    binary_value(context, call, environment, |left, right| {
        RuntimeValue::f64(operation(left, right), value)
    })
}

fn binary_value(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64, f64) -> RuntimeValue,
) -> Result<RuntimeValue, Halt> {
    let Some(left) = context.field(call, vocabulary::LEFT) else {
        return Ok(context.missing_runtime_argument(vocabulary::LEFT));
    };
    let Some(right) = context.field(call, vocabulary::RIGHT) else {
        return Ok(context.missing_runtime_argument(vocabulary::RIGHT));
    };
    let left = context.eval_runtime(left, environment)?;
    let right = context.eval_runtime(right, environment)?;
    Ok(match (left.as_f64(read), right.as_f64(read)) {
        (Some(left), Some(right)) => operation(left, right),
        (None, _) => absent::with_reason(vocabulary::LEFT_NOT_F64).into(),
        (_, None) => absent::with_reason(vocabulary::RIGHT_NOT_F64).into(),
    })
}

fn unary(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
    operation: impl FnOnce(f64) -> f64,
) -> Result<RuntimeValue, Halt> {
    let Some(operand) = context.field(call, vocabulary::OPERAND) else {
        return Ok(context.missing_runtime_argument(vocabulary::OPERAND));
    };
    let operand = context.eval_runtime(operand, environment)?;
    Ok(operand
        .as_f64(read)
        .map(|operand| RuntimeValue::f64(operation(operand), value))
        .unwrap_or_else(|| absent::with_reason(vocabulary::OPERAND_NOT_F64).into()))
}

pub fn library<World, Hover: Clone>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, name) in [
        (vocabulary::F64, "f64"),
        (vocabulary::UPDATE, "f64 update"),
        (vocabulary::ADD, "+"),
        (vocabulary::MULTIPLY, "*"),
        (vocabulary::SUBTRACT, "-"),
        (vocabulary::DIVIDE, "/"),
        (vocabulary::SIN, "sin"),
        (vocabulary::COS, "cos"),
        (vocabulary::LESS, "<"),
        (vocabulary::EQUAL, "=="),
        (vocabulary::FLOOR, "floor"),
        (vocabulary::LERP, "lerp"),
        (vocabulary::OPERAND, "operand"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::START, "start"),
        (vocabulary::END, "end"),
        (vocabulary::AMOUNT, "amount"),
    ] {
        cells.set_value(cell, name::record(name, []));
    }
    for (cell, name) in [
        (vocabulary::LEFT_NOT_F64, "left is not f64"),
        (vocabulary::RIGHT_NOT_F64, "right is not f64"),
        (vocabulary::OPERAND_NOT_F64, "operand is not f64"),
        (vocabulary::START_NOT_F64, "start is not f64"),
        (vocabulary::END_NOT_F64, "end is not f64"),
        (vocabulary::AMOUNT_NOT_F64, "amount is not f64"),
    ] {
        cells.set_value(cell, absent::named_reason(name));
    }
    cells.set_value(
        vocabulary::PI,
        overlay_value(
            &value(std::f64::consts::PI),
            name::record("π", []),
        ),
    );
    Library {
        cells,
        functions: functions(),
        projections: vec![binary_display::<World, Hover>, display::<World, Hover>],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    struct TestEnv;

    impl progred_display::Env for TestEnv {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            unreachable!()
        }
    }

    fn target(_: Vec<Step>) -> progred_display::ProjectionTarget<(), ()> {
        progred_display::ProjectionTarget {
            select: std::rc::Rc::new(|_| false),
            hover: (),
        }
    }

    fn projection_input(value: &Value) -> ProjectionInput<'_, (), ()> {
        ProjectionInput {
            env: &TestEnv,
            value,
            selection: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&target),
        }
    }

    fn call(function: CellId, left: Value, right: Value) -> Value {
        grap::call(
            Value::from(function),
            [(vocabulary::LEFT, left), (vocabulary::RIGHT, right)],
        )
    }

    #[test]
    fn representation_is_library_data() {
        assert_eq!(read(&value(2.5)), Some(2.5));
        assert_eq!(read(&Value::from(b"2.5".to_vec())), None);

        let extra = new_cell_id();
        let with_extra = Value::record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(b"degrees".to_vec())),
        );
        assert_eq!(read(&with_extra), Some(2.5));
        let update = |input: &str| {
            grap::evaluate(
                &grap::call(
                    grap::ffi(vocabulary::UPDATE),
                    [
                        (line_edit::vocabulary::CURRENT, with_extra.clone()),
                        (line_edit::vocabulary::INPUT, crate::text::value(input)),
                    ],
                ),
                |_| None,
                &functions(),
                100,
            )
            .result
        };
        // Unparseable input declines as an absent — the editor drops
        // the write whole.
        assert!(crate::absent::is_absent(&update("junk")));
        assert_eq!(
            update("3"),
            Value::record(
                value(3.0)
                    .as_record()
                    .unwrap()
                    .clone()
                    .update(extra, Value::from(b"degrees".to_vec())),
            )
        );
    }

    #[test]
    fn scrubbing_uses_the_sensitivitys_decimal_precision() {
        let mut scrub = NumberScrub::new(100.0);

        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: 0.0,
                })
                .value,
            101.0,
        );
        assert_eq!(
            NumberScrub::new(0.5)
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: 0.0,
                })
                .value,
            0.501,
        );

        let one_decimal_place = SCRUB_PIXELS_PER_DECADE;
        let mut scrub = NumberScrub::new(100.0);
        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 4.0,
                    distance_y: one_decimal_place,
                })
                .value,
            100.1,
        );
        assert_eq!(
            scrub
                .update(ScrubEvent {
                    movement_x: 16.0,
                    distance_y: -one_decimal_place,
                })
                .value,
            120.0,
        );
    }

    #[test]
    fn rightward_motion_never_lowers_the_displayed_value() {
        let mut scrub = NumberScrub::new(100.0);
        let first = scrub
            .update(ScrubEvent {
                movement_x: 16.0,
                distance_y: 0.0,
            })
            .value;
        let scale_changed = scrub
            .update(ScrubEvent {
                movement_x: 0.0,
                distance_y: -SCRUB_PIXELS_PER_DECADE,
            })
            .value;
        let moved_right = scrub
            .update(ScrubEvent {
                movement_x: 0.1,
                distance_y: -SCRUB_PIXELS_PER_DECADE,
            })
            .value;

        assert_eq!(first, 104.0);
        assert_eq!(scale_changed, first);
        assert!(moved_right >= scale_changed);
    }

    #[test]
    fn scrub_spelling_retains_the_active_decimal_precision() {
        assert_eq!(spelling(100.0, 0.1), "100.0");
        assert_eq!(spelling(100.1, 0.1), "100.1");
        assert_eq!(spelling(100.0, 1.0), "100");
        assert_eq!(spelling(110.0, 10.0), "110");
    }

    #[test]
    fn a_gesture_fixes_its_scale_from_the_starting_value() {
        assert_eq!(NumberScrub::new(0.1234838495).base, 0.001);
        assert_eq!(NumberScrub::new(123.0).base, 1.0);
        assert_eq!(NumberScrub::new(1234.0).base, 1.0);
    }

    #[test]
    fn precision_uses_one_two_five_steps() {
        assert_eq!(nice_precision(0.01), 0.01);
        assert_eq!(nice_precision(0.02), 0.02);
        assert_eq!(nice_precision(0.05), 0.05);
        assert_eq!(nice_precision(0.1), 0.1);
        assert_eq!(nice_precision(2.0), 2.0);
        assert_eq!(nice_precision(5.0), 5.0);
    }

    #[test]
    fn vertical_decades_spread_out_as_they_get_coarser() {
        let close = |left: f64, right: f64| (left - right).abs() < 1e-12;

        assert!(close(vertical_decades(-24.0), 1.0));
        assert!(close(vertical_decades(-60.0), 2.0));
        assert!(close(vertical_decades(-114.0), 3.0));
        assert!(close(vertical_decades(60.0), -2.0));
    }

    #[test]
    fn coarse_precision_grows_horizontal_sensitivity_sublinearly() {
        let horizontal_scale = |gain: f64| gain / gain.max(1.0).cbrt();

        assert_eq!(horizontal_scale(0.01), 0.01);
        assert_eq!(horizontal_scale(1.0), 1.0);
        assert!((horizontal_scale(1_000.0) - 100.0).abs() < 1e-12);
    }

    #[test]
    fn rust_supplies_arithmetic_to_grap() {
        let add = call(vocabulary::ADD, value(2.0), value(3.0));
        let multiply = call(vocabulary::MULTIPLY, add, value(4.0));
        assert_eq!(
            grap::evaluate(&multiply, |_| None, &functions(), 20).result,
            value(20.0)
        );
    }

    #[test]
    fn rust_supplies_lerp_to_grap() {
        let expression = grap::call(
            Value::from(vocabulary::LERP),
            [
                (vocabulary::START, value(10.0)),
                (vocabulary::END, value(20.0)),
                (vocabulary::AMOUNT, value(0.25)),
            ],
        );
        assert_eq!(
            grap::evaluate(&expression, |_| None, &functions(), 20).result,
            value(12.5)
        );
    }

    #[test]
    fn runtime_lowering_preserves_an_enriched_value_through_a_binding() {
        let parameter = new_cell_id();
        let metadata = new_cell_id();
        let enriched = Value::Record(
            value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(b"kept".to_vec())),
        );
        let expression = grap::call(
            grap::lambda([parameter], Value::from(parameter)),
            [(parameter, enriched.clone())],
        );

        assert_eq!(
            grap::evaluate(&expression, |_| None, &functions(), 20).result,
            enriched
        );
    }

    #[test]
    fn binary_notation_descends_through_source_fields_and_preserves_precedence() {
        let product = call(vocabulary::MULTIPLY, value(2.0), value(3.0));
        let sum = call(vocabulary::ADD, value(1.0), product);
        let layout = binary_display(&projection_input(&sum)).unwrap();
        let Layout::Row { children, .. } = layout else {
            panic!("binary notation is a row");
        };
        assert!(matches!(
            &children[0],
            Layout::Descend {
                step: Step::Key(field),
                ..
            } if *field == vocabulary::LEFT
        ));
        assert!(matches!(
            &children[1],
            Layout::Descend {
                step: Step::Key(field),
                projection: Some(projection),
                ..
            } if *field == FUNCTION && projection.len() == 1
        ));
        assert!(matches!(
            &children[2],
            Layout::Descend {
                step: Step::Key(field),
                ..
            } if *field == vocabulary::RIGHT
        ));

        let product = call(
            vocabulary::MULTIPLY,
            call(vocabulary::ADD, value(1.0), value(2.0)),
            value(3.0),
        );
        let layout = binary_display(&projection_input(&product)).unwrap();
        let Layout::Row { children, .. } = layout else {
            panic!("binary notation is a row");
        };
        assert!(matches!(&children[0], Layout::Surround { .. }));
    }

    #[test]
    fn binary_notation_declines_calls_with_unshown_fields() {
        let extra = new_cell_id();
        let call = grap::call(
            Value::from(vocabulary::ADD),
            [
                (vocabulary::LEFT, value(1.0)),
                (vocabulary::RIGHT, value(2.0)),
                (extra, value(3.0)),
            ],
        );
        assert!(binary_display(&projection_input(&call)).is_none());
    }

    #[test]
    fn type_absences_are_library_values() {
        let left = call(vocabulary::ADD, Value::from(b"two".to_vec()), value(3.0));
        let right = call(vocabulary::ADD, value(2.0), Value::from(b"three".to_vec()));
        assert_eq!(
            grap::evaluate(&left, |_| None, &functions(), 10).result,
            absent::with_reason(vocabulary::LEFT_NOT_F64)
        );
        assert_eq!(
            grap::evaluate(&right, |_| None, &functions(), 10).result,
            absent::with_reason(vocabulary::RIGHT_NOT_F64)
        );
    }

    #[test]
    fn library_names_are_ordinary_facts_for_random_identities() {
        let library = library::<(), ()>();
        assert_eq!(
            library.cells.value(vocabulary::F64).and_then(name::read),
            Some("f64")
        );
        assert_eq!(
            library.cells.value(vocabulary::ADD).and_then(name::read),
            Some("+")
        );
        assert_eq!(
            library.cells.value(vocabulary::PI).and_then(name::read),
            Some("π")
        );
        assert_eq!(
            library.cells.value(vocabulary::PI).and_then(read),
            Some(std::f64::consts::PI)
        );
        assert_eq!(
            library
                .cells
                .value(vocabulary::LEFT_NOT_F64)
                .and_then(name::read),
            Some("left is not f64")
        );
        assert_eq!(
            library
                .cells
                .value(vocabulary::RIGHT_NOT_F64)
                .and_then(name::read),
            Some("right is not f64")
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::LEFT_NOT_F64)),
            Some(vocabulary::LEFT_NOT_F64)
        );
        assert_eq!(
            absent::reason(&absent::with_reason(vocabulary::RIGHT_NOT_F64)),
            Some(vocabulary::RIGHT_NOT_F64)
        );
        assert!(library.cells.value(vocabulary::ADD).is_some());
    }
}
