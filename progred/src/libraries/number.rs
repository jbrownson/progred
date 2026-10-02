//! What every number representation shares: the argument labels of
//! binary and unary operations, and the scrub editing helper. The
//! operation identities stay with each representation until dispatch
//! evaluates arguments once per call.

use crate::display::{Layout, Partial, ProjectionInput, activatable, overlay_value};
use crate::libraries::representation::{Precedence, calls, infix_display, tagged};
use crate::libraries::{Library, absent, line_edit, logic, name};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue, Stage,
};
use gid::{CellId, Cells, Value};
use puri::handler::HasHandler;
use std::fmt::Display;
use std::rc::Rc;

pub(crate) mod scrub;
use scrub::{ScrubEvent, ScrubUpdate, on_scrub};

pub const ID: CellId = CellId::from_u128(0xc46d010325d3a1ec0f2a84dd3a9570ae);

pub mod vocabulary {
    use gid::CellId;

    pub const LEFT: CellId = CellId::from_u128(0x764f6afe17ba14e81f5ab61204be0bec);
    pub const RIGHT: CellId = CellId::from_u128(0x4f53ff25390f58472d31a6142644dec2);
    pub const OPERAND: CellId = CellId::from_u128(0x50a20d15e4ae56be51b882de9d58c676);
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::OPERAND, "operand"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "number",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::runtime_partial(|_| None),
    )
}

pub(crate) fn completions<N: std::str::FromStr + Display>(
    query: &str,
    representation: CellId,
    encode: impl FnOnce(N) -> Value,
) -> Vec<crate::display::Completion> {
    match query.trim() {
        "" => "0",
        number => number,
    }
    .parse::<N>()
    .ok()
    .map(|number| {
        crate::libraries::completion::select(number.to_string(), encode(number))
            .with_aliases([query])
            .with_detail(representation)
    })
    .into_iter()
    .collect()
}

const PIXELS_PER_STEP: f64 = 4.0;
const PIXELS_PER_DECADE: f64 = 24.0;
const DECADE_STRETCH: f64 = 1.5;

/// Comparisons and the arithmetic symbols read infix; named and one-operand
/// operations stay calls.
fn precedence<N>(spelling: &str, operation: &Operation<N>) -> Option<Precedence> {
    match operation {
        Operation::Comparison(_) => Some(Precedence::Comparison),
        Operation::Arithmetic(_) | Operation::Checked(..) => Precedence::of_symbol(spelling),
        Operation::Unary(_) | Operation::Predicate(_) | Operation::Conversion(_) => None,
    }
}

/// One numeric convention's identity and encodings. Each representation
/// keeps its own cells and operations; this generates the plumbing they
/// would otherwise each repeat.
#[derive(Clone, Copy)]
pub(crate) struct Convention<N> {
    pub name: &'static str,
    pub tag: CellId,
    pub update: CellId,
    pub left_not: CellId,
    pub right_not: CellId,
    pub operand_not: CellId,
    pub invalid_input: CellId,
    pub encode: fn(N) -> Value,
    pub runtime: fn(N) -> RuntimeValue,
    pub read: fn(&RuntimeValue) -> Option<N>,
    /// Evaluate an operand read as this number, with any accelerated path.
    pub eval: fn(&mut Context, Expression, &Environment) -> Result<Option<N>, Halt>,
}

#[derive(Clone, Copy)]
pub(crate) enum Operation<N> {
    Arithmetic(fn(N, N) -> N),
    /// Fails with the given reason instead of producing a result.
    Checked(fn(N, N) -> Option<N>, CellId),
    Comparison(fn(N, N) -> bool),
    Unary(fn(N) -> N),
    Predicate(fn(N) -> bool),
    /// From another representation's operand, which it reads itself.
    Conversion(fn(&RuntimeValue) -> Option<N>),
}

/// A convention's generated cells, functions, and projected call forms,
/// which a representation extends with its own before assembling.
pub(crate) struct Parts {
    pub cells: Cells,
    pub functions: ForeignFunctions,
    pub calls: Vec<CellId>,
    /// The calls that read infix, and how tightly each binds.
    pub infix: Vec<(CellId, Precedence)>,
    /// The infix operators by spelling, for typing after a number.
    pub symbols: Vec<(&'static str, CellId)>,
}

impl<N: Scrubbable + std::str::FromStr> Convention<N> {
    pub fn parts(
        self,
        operations: impl IntoIterator<Item = (CellId, &'static str, Operation<N>)>,
    ) -> Parts {
        let mut cells = Cells::new();
        cells.set_value(self.tag, name::record(self.name, []));
        cells.set_value(
            self.update,
            name::record(format!("{} update", self.name), []),
        );
        for (cell, reason) in [
            (self.left_not, format!("left is not {}", self.name)),
            (self.right_not, format!("right is not {}", self.name)),
            (self.operand_not, format!("operand is not {}", self.name)),
            (self.invalid_input, format!("invalid {} input", self.name)),
        ] {
            cells.set_value(cell, absent::named_reason(reason));
        }
        operations.into_iter().fold(
            Parts {
                cells,
                functions: ForeignFunctions::default()
                    .register(self.update, self.update_function()),
                calls: Vec::new(),
                infix: Vec::new(),
                symbols: Vec::new(),
            },
            |mut parts, (cell, spelling, operation)| {
                if let Some(precedence) = precedence(spelling, &operation) {
                    parts.infix.push((cell, precedence));
                    parts.symbols.push((spelling, cell));
                }
                parts.cells.set_value(cell, name::record(spelling, []));
                parts.functions = parts.functions.register(cell, self.operation(operation));
                parts.calls.push(cell);
                parts
            },
        )
    }

    pub fn library(
        self,
        id: CellId,
        parts: Parts,
        before: impl IntoIterator<Item = Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> Library<crate::Editor, crate::frame::Hovered> {
        let Parts {
            cells,
            functions,
            calls: operations,
            infix,
            symbols,
        } = parts;
        let infix: Rc<[(CellId, Precedence)]> = infix.into();
        let symbols: Rc<[(&'static str, CellId)]> = symbols.into();
        Library::named(
            id,
            self.name,
            crate::libraries::Definitions::from_parts(cells, functions),
            crate::display::compose_partials(before.into_iter().chain([
                crate::display::runtime_partial(move |input| {
                    infix_display(self.tag, &infix, input)
                }),
                calls(self.tag, operations),
                crate::display::runtime_partial(move |input| {
                    let layout = self.display(input)?;
                    Some(if input.writable && input.selection.is_some() {
                        typed_operators::<N>(layout, symbols.clone())
                    } else {
                        layout
                    })
                }),
            ])),
        )
        .with_completions(move |request| {
            (request.scope == crate::display::CompletionScope::Everything
                && request.kind == crate::display::CompletionKind::Value)
                .then(|| self.completions(request.query))
        })
    }

    /// A number reads as its spelling, then any other fields, including one
    /// being added.
    pub fn display(
        self,
        input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, RuntimeValue>,
    ) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
        layout(input, (self.read)(input.value?)?, self.tag, self.encode)
    }

    pub fn completions(self, query: &str) -> Vec<crate::display::Completion> {
        completions(query, self.tag, self.encode)
    }

    fn operation(self, operation: Operation<N>) -> ForeignFunction {
        match operation {
            Operation::Arithmetic(apply) => {
                self.binary(move |left, right| (self.runtime)(apply(left, right)))
            }
            Operation::Checked(apply, failure) => self.binary(move |left, right| {
                apply(left, right)
                    .map(self.runtime)
                    .unwrap_or_else(|| absent::with_reason(failure).into())
            }),
            Operation::Comparison(apply) => {
                self.binary(move |left, right| logic::value(apply(left, right)).into())
            }
            Operation::Unary(apply) => self.unary(move |operand| (self.runtime)(apply(operand))),
            Operation::Predicate(apply) => {
                self.unary(move |operand| logic::value(apply(operand)).into())
            }
            Operation::Conversion(convert) => self.conversion(convert),
        }
    }

    fn unary(self, operation: impl Fn(N) -> RuntimeValue + 'static) -> ForeignFunction {
        let operation = Rc::new(operation);
        ForeignFunction::staged(move |context, call| {
            match context.field(call, vocabulary::OPERAND) {
                Some(operand) => {
                    let operation = operation.clone();
                    Rc::new(move |context, environment| {
                        Ok(match (self.eval)(context, operand.clone(), environment)? {
                            Some(operand) => operation(operand),
                            None => absent::with_reason(self.operand_not).into(),
                        })
                    })
                }
                None => {
                    Rc::new(|context, _| Ok(context.missing_runtime_argument(vocabulary::OPERAND)))
                }
            }
        })
        .parameters([vocabulary::OPERAND])
        .tracked()
    }

    fn conversion(self, convert: fn(&RuntimeValue) -> Option<N>) -> ForeignFunction {
        ForeignFunction::staged(move |context, call| {
            match context.field(call, vocabulary::OPERAND) {
                Some(operand) => Rc::new(move |context, environment| {
                    let operand = context.eval(operand.clone(), environment)?;
                    Ok(match convert(&operand) {
                        Some(number) => (self.runtime)(number),
                        None => ::grap::absent::with_detail(
                            self.invalid_input,
                            vocabulary::OPERAND,
                            operand.to_value(),
                        )
                        .into(),
                    })
                }),
                None => {
                    Rc::new(|context, _| Ok(context.missing_runtime_argument(vocabulary::OPERAND)))
                }
            }
        })
        .parameters([vocabulary::OPERAND])
        .tracked()
    }

    /// Each call site finds its operand expressions once; its stage only
    /// evaluates them.
    fn binary(self, operation: impl Fn(N, N) -> RuntimeValue + 'static) -> ForeignFunction {
        let operation = Rc::new(operation);
        ForeignFunction::staged(move |context, call| {
            let missing = |cell| -> Stage {
                Rc::new(move |context, _| Ok(context.missing_runtime_argument(cell)))
            };
            match (
                context.field(call, vocabulary::LEFT),
                context.field(call, vocabulary::RIGHT),
            ) {
                (Some(left), Some(right)) => {
                    let operation = operation.clone();
                    Rc::new(move |context, environment| {
                        let left = (self.eval)(context, left.clone(), environment)?;
                        let right = (self.eval)(context, right.clone(), environment)?;
                        Ok(match (left, right) {
                            (Some(left), Some(right)) => operation(left, right),
                            (None, _) => absent::with_reason(self.left_not).into(),
                            (_, None) => absent::with_reason(self.right_not).into(),
                        })
                    })
                }
                (None, _) => missing(vocabulary::LEFT),
                (_, None) => missing(vocabulary::RIGHT),
            }
        })
        .parameters([vocabulary::LEFT, vocabulary::RIGHT])
        .tracked()
    }

    /// Text from a line control. A missing location has no current value.
    fn update_function(self) -> ForeignFunction {
        ForeignFunction::from_value(move |context, call, environment| {
            let Some(input) = context.field(call, line_edit::vocabulary::INPUT) else {
                return Ok(context.missing_argument(line_edit::vocabulary::INPUT));
            };
            let current = context
                .field(call, line_edit::vocabulary::CURRENT)
                .map(|current| context.eval_to_value(current, environment))
                .transpose()?;
            let input = context.eval_to_value(input, environment)?;
            Ok(crate::libraries::text::read(&input)
                .and_then(|text| edit(text, current.as_ref(), self.encode))
                .unwrap_or_else(|| {
                    ::grap::absent::with_detail(
                        self.invalid_input,
                        line_edit::vocabulary::INPUT,
                        input.clone(),
                    )
                }))
        })
        .parameters([line_edit::vocabulary::INPUT])
        .tracked()
    }
}

/// A selected number takes its type's single-character operators as typing:
/// one typed at the end of a whole number makes the number its left operand,
/// with the right operand's picker open. A sign or exponent that doesn't yet
/// make a number still goes into its text.
fn typed_operators<N: std::str::FromStr + 'static>(
    child: Layout<crate::Editor, crate::frame::Hovered>,
    symbols: Rc<[(&'static str, CellId)]>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    crate::display::widget::after(
        child,
        Rc::new(move |context| {
            let root = context.inputs.view.clone();
            let path = context.path.to_vec();
            let edits = context.inputs.edits.clone();
            let symbols = symbols.clone();
            Box::new(move |output, _| {
                output.handler().on_key(move |editor, event| {
                    wrap_typed::<N>(editor, event, &root, &path, &edits, &symbols)
                });
            })
        }),
    )
}

fn wrap_typed<N: std::str::FromStr>(
    editor: &mut crate::Editor,
    event: &ui_events::keyboard::KeyboardEvent,
    root: &crate::workspace::Root,
    path: &[gid::Step],
    edits: &crate::editing::Scope,
    symbols: &[(&'static str, CellId)],
) -> bool {
    if !event.state.is_down()
        || event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
    {
        return false;
    }
    let ui_events::keyboard::Key::Character(typed) = &event.key else {
        return false;
    };
    let Some(&(_, operator)) = symbols
        .iter()
        .find(|(spelling, _)| *spelling == typed.as_str())
    else {
        return false;
    };
    let Some(current) = editor
        .model
        .selection
        .as_ref()
        .filter(|current| current.root() == root && current.path() == path)
    else {
        return false;
    };
    if let Some(line) = current.value_edit() {
        let (anchor, focus) = line.selection_offsets();
        if anchor != focus || focus != line.text().len() || line.text().trim().parse::<N>().is_err()
        {
            return false;
        }
    }
    let Some(left) = current.value(&editor.sources()).cloned() else {
        return false;
    };
    let wrapped = ::grap::call(Value::from(operator), [(vocabulary::LEFT, left)]);
    if !edits
        .open(crate::editing::Access::new(editor))
        .replace(path, wrapped)
    {
        return false;
    }
    let mut right = crate::selection::pending_value(
        root,
        path.iter()
            .cloned()
            .chain([gid::Step::Key(vocabulary::RIGHT)])
            .collect(),
    );
    right.set_scope(edits.clone());
    editor.model.selection = Some(right);
    true
}

pub(crate) trait Scrubbable: Copy + Display + PartialOrd + 'static {
    fn magnitude(self) -> f64;
    fn minimum_precision() -> f64;
    fn scrubbable(self) -> bool;
    fn from_offset(start: Self, offset: f64, precision: f64) -> Self;
    fn spelling(self, precision: f64) -> String;
}

pub(crate) fn layout<N: Scrubbable + std::str::FromStr>(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, ::grap::RuntimeValue>,
    number: N,
    representation: CellId,
    encode: fn(N) -> Value,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let original = input.value?;
    let line = tagged(
        input,
        line_edit::layout(
            number.to_string(),
            line_edit::native(move |spelling, current| edit(spelling, current, encode)),
            "",
            "",
        ),
        representation,
    );
    let target = input.targets.current();
    let line = activatable(line, target.hover.clone(), target.select);
    let line = if number.scrubbable() {
        let original = original.clone();
        on_scrub(
            line,
            target.hover,
            Rc::new(move || {
                let original = original.to_value();
                let mut scrub = NumberScrub::new(number);
                Box::new(move |event| {
                    let scrubbed = scrub.update(event);
                    ScrubUpdate {
                        value: overlay_value(&original, encode(scrubbed.value)),
                        spelling: Some(scrubbed.value.spelling(scrubbed.precision)),
                    }
                })
            }),
        )
    } else {
        line
    };
    Some(crate::display::structure::with_extra_fields(
        input,
        |key| key == representation || key == name::vocabulary::NAME,
        name::with_name(input, line),
    ))
}

pub(crate) fn edit<N: std::str::FromStr>(
    spelling: &str,
    current: Option<&Value>,
    encode: fn(N) -> Value,
) -> Option<Value> {
    let value = encode(spelling.trim().parse().ok()?);
    Some(
        current
            .map(|current| overlay_value(current, value.clone()))
            .unwrap_or(value),
    )
}

struct Scrubbed<N> {
    value: N,
    precision: f64,
}

struct NumberScrub<N> {
    start: N,
    base: f64,
    offset: f64,
    displayed: N,
}

impl<N: Scrubbable> NumberScrub<N> {
    fn new(start: N) -> Self {
        Self {
            start,
            base: initial_precision(start.magnitude(), N::minimum_precision()),
            offset: 0.0,
            displayed: start,
        }
    }

    fn update(&mut self, event: ScrubEvent) -> Scrubbed<N> {
        let gain = 10.0_f64.powf(vertical_decades(event.distance_y).clamp(-16.0, 16.0));
        let scale = (self.base * gain).max(N::minimum_precision());
        let precision = nice_precision(scale);
        let horizontal_scale = scale / gain.max(1.0).cbrt();
        self.offset += event.movement_x * horizontal_scale / PIXELS_PER_STEP;
        let candidate = N::from_offset(self.start, self.offset, precision);
        self.displayed = if event.movement_x > 0.0 {
            partial_max(self.displayed, candidate)
        } else if event.movement_x < 0.0 {
            partial_min(self.displayed, candidate)
        } else {
            self.displayed
        };
        Scrubbed {
            value: self.displayed,
            precision,
        }
    }
}

fn partial_max<N: Copy + PartialOrd>(left: N, right: N) -> N {
    if right > left { right } else { left }
}

fn partial_min<N: Copy + PartialOrd>(left: N, right: N) -> N {
    if right < left { right } else { left }
}

fn initial_precision(magnitude: f64, minimum: f64) -> f64 {
    (if magnitude == 0.0 {
        0.01
    } else {
        10.0_f64
            .powf(magnitude.abs().log10().floor() - 2.0)
            .min(1.0)
    })
    .max(minimum)
}

fn vertical_decades(distance_y: f64) -> f64 {
    let distance = distance_y.abs();
    let decades = (1.0 + (DECADE_STRETCH - 1.0) * distance / PIXELS_PER_DECADE).log(DECADE_STRETCH);
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

pub(crate) fn rounded(value: f64, step: f64) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_convention_updates_a_missing_location_from_text() {
        use crate::libraries::{f32, f64, text, u64};
        for (library, update, spelling, expected) in [
            (
                f64::library(),
                f64::vocabulary::UPDATE,
                "2.5",
                f64::value(2.5),
            ),
            (
                f32::library(),
                f32::vocabulary::UPDATE,
                "2.5",
                f32::value(2.5),
            ),
            (u64::library(), u64::vocabulary::UPDATE, "2", u64::value(2)),
        ] {
            let call = ::grap::call(
                update.into(),
                [(line_edit::vocabulary::INPUT, text::value(spelling))],
            );
            let functions = library.functions();
            assert_eq!(
                crate::libraries::test_evaluate(&call, |_| None, &functions, 20).result,
                expected
            );
        }
    }

    #[test]
    fn numeric_completions_offer_zero_for_empty_queries_but_not_invalid_numbers() {
        for (complete, zero, representation) in [
            (
                crate::libraries::f32::completions as fn(&str) -> Vec<crate::display::Completion>,
                crate::libraries::f32::value(0.0),
                crate::libraries::f32::vocabulary::F32,
            ),
            (
                crate::libraries::f64::completions,
                crate::libraries::f64::value(0.0),
                crate::libraries::f64::vocabulary::F64,
            ),
            (
                crate::libraries::u64::completions,
                crate::libraries::u64::value(0),
                crate::libraries::u64::vocabulary::U64,
            ),
        ] {
            for query in ["", " \t\n", "0"] {
                let offers = complete(query);
                let [offer] = offers.as_slice() else {
                    panic!("expected one {representation} zero offer for {query:?}");
                };
                assert_eq!(offer.display, "0".into());
                assert_eq!(offer.detail, Some(representation.into()));
                assert_eq!(offer.preview.as_ref(), Some(&zero));
            }
            for query in ["not a number", "-", ".", "1e"] {
                assert!(complete(query).is_empty(), "{representation}: {query:?}");
            }
        }
    }

    #[test]
    fn scrubbing_uses_the_active_decimal_precision() {
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

        let one_decimal_place = PIXELS_PER_DECADE;
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
                distance_y: -PIXELS_PER_DECADE,
            })
            .value;
        let moved_right = scrub
            .update(ScrubEvent {
                movement_x: 0.1,
                distance_y: -PIXELS_PER_DECADE,
            })
            .value;

        assert_eq!(first, 104.0);
        assert_eq!(scale_changed, first);
        assert!(moved_right >= scale_changed);
    }

    #[test]
    fn float_spelling_retains_the_active_decimal_precision() {
        assert_eq!(100.0.spelling(0.1), "100.0");
        assert_eq!(100.1.spelling(0.1), "100.1");
        assert_eq!(100.0.spelling(1.0), "100");
        assert_eq!(110.0.spelling(10.0), "110");
    }

    #[test]
    fn a_gesture_fixes_its_scale_from_the_starting_value() {
        assert_eq!(NumberScrub::new(0.1234838495).base, 0.001);
        assert_eq!(NumberScrub::new(123.0).base, 1.0);
        assert_eq!(NumberScrub::new(1234.0).base, 1.0);
        assert_eq!(NumberScrub::new(0_u64).base, 1.0);
        assert_eq!(NumberScrub::new(123_u64).base, 1.0);
    }

    #[test]
    fn integer_precision_stops_at_one() {
        let scrubbed = NumberScrub::new(42_u64).update(ScrubEvent {
            movement_x: 4.0,
            distance_y: 100.0,
        });

        assert_eq!(scrubbed.precision, 1.0);
        assert_eq!(scrubbed.value, 43);
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
}
