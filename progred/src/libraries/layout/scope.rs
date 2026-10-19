//! Evaluation-local layout output. Grap carries only ordinary values; the
//! borrowed FFIs emit into this Rust buffer, including through calls and loops.
use super::*;
use crate::display::host::{Host, Layout, ProjectionInput};
use ::grap::{Context, Evaluation, ForeignOverlay, RuntimeValue};
use std::cell::RefCell;
use vocabulary::*;

const FUNCTIONS: &[CellId] = &[
    ROW,
    COL,
    PAD,
    CENTER,
    OVERLAY,
    ALTERNATIVES,
    BRACKET,
    TEXT,
    SLOT,
    RULE,
    DESCEND,
    DESCEND_PATH,
    JUMP,
    AT,
    CANVAS,
    SELECTABLE,
    HOVER_BLOCK,
    PICKABLE,
    ON_EVENT,
];

pub(super) fn program(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<::grap::RuntimeValue, Halt> {
    match context.field(call, ::grap::vocabulary::EXPRESSION) {
        Some(expression) => {
            let closure = context.closure([], expression, environment);
            Ok(::grap::RuntimeValue::record([(LAYOUT_PROGRAM, closure)]))
        }
        None => Ok(context.missing_runtime_argument(::grap::vocabulary::EXPRESSION)),
    }
}

struct Output<W: Host> {
    children: RefCell<Vec<Layout<W>>>,
}

enum BuildError {
    Absent(RuntimeValue),
    Halt(Halt),
}

impl From<Halt> for BuildError {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

fn invalid(function: CellId) -> BuildError {
    BuildError::Absent(
        ::grap::absent::with_detail(
            INVALID_PROGRAM,
            ::grap::vocabulary::FUNCTION,
            function.into(),
        )
        .into(),
    )
}

impl<W: Host> Output<W> {
    fn collect<T>(
        &self,
        run: impl FnOnce() -> Result<T, Halt>,
    ) -> Result<(T, Vec<Layout<W>>), Halt> {
        let parent = self.children.take();
        let result = run();
        let children = self.children.replace(parent);
        result.map(|result| (result, children))
    }
}

/// Exactly one emitted root is a layout; no emission leaves the ordinary
/// value result alone. A halt or final absent drops the entire output.
pub fn run<W: Host>(
    site: impl Fn() -> crate::editing::Site,
    evaluate: impl FnOnce(&ForeignOverlay<'_>) -> Evaluation,
) -> (Evaluation, Option<Layout<W>>) {
    let output = Output {
        children: RefCell::new(Vec::new()),
    };
    let unit = RuntimeValue::record([]);
    let emit =
        |function, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
            match operation(function, context, call, environment, &output, &site) {
                Ok(layout) => Ok(context.effect(|| {
                    output.children.borrow_mut().push(layout);
                    unit.clone()
                })),
                Err(BuildError::Absent(value)) => Ok(value),
                Err(BuildError::Halt(halt)) => Err(halt),
            }
        };
    let mut evaluation = evaluate(&ForeignOverlay::new(FUNCTIONS, &emit));
    let mut children = output.children.into_inner();
    let layout = if !evaluation.completed || evaluation.result.is_absent() {
        None
    } else if children.len() > 1 {
        evaluation.result = ::grap::absent::value(INVALID_PROGRAM).into();
        None
    } else {
        children.pop()
    };
    (evaluation, layout)
}

pub(super) fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let function = input.value?.field(LAYOUT_PROGRAM)?;
    let (evaluation, layout) = run(
        || input.site(),
        |scope| input.env.apply_runtime_scoped(&function, &[], Some(scope)),
    );
    Some(layout.unwrap_or_else(|| {
        crate::display::at(
            [gid::Step::Key(
                crate::libraries::presentation::vocabulary::RESULT,
            )],
            &if evaluation.result.is_absent() {
                evaluation.result
            } else {
                RuntimeValue::record([
                    (::grap::absent::ABSENT, Value::from(INVALID_PROGRAM).into()),
                    (VALUE, evaluation.result),
                ])
            },
        )
    }))
}

fn argument<T>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    read: impl FnOnce(&RuntimeValue) -> Option<T>,
) -> Result<Option<T>, Halt> {
    match context.field(call, field) {
        Some(expression) => context
            .eval(expression, environment)
            .map(|value| read(&value)),
        None => Ok(None),
    }
}

fn number_argument(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    default: Option<f64>,
) -> Result<Option<f64>, Halt> {
    match context.field(call, field) {
        Some(expression) => context
            .eval_f64(expression, environment)
            .map(|value| value.filter(|value| value.is_finite())),
        None => Ok(default),
    }
}

fn single<W: Host>(children: Vec<Layout<W>>) -> Option<Layout<W>> {
    if children.len() == 1 {
        children.into_iter().next()
    } else {
        None
    }
}

fn operation<W: Host>(
    function: CellId,
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &Output<W>,
    site: &impl Fn() -> crate::editing::Site,
) -> Result<Layout<W>, BuildError> {
    macro_rules! need {
        ($value:expr) => {
            match $value {
                Some(value) => value,
                None => return Err(invalid(function)),
            }
        };
    }
    macro_rules! arg {
        ($field:expr, $read:expr) => {
            need!(argument(context, call, environment, $field, $read)?)
        };
    }
    macro_rules! number {
        ($field:expr, $default:expr) => {
            need!(number_argument(
                context,
                call,
                environment,
                $field,
                $default
            )?)
        };
    }
    macro_rules! children {
        ($body:expr) => {{
            let (result, children) = output.collect(|| context.eval($body, environment))?;
            if result.is_absent() {
                return Err(BuildError::Absent(result));
            }
            children
        }};
    }
    let layout = match function {
        ROW | COL | OVERLAY | ALTERNATIVES => {
            let gap = if matches!(function, ROW | COL) {
                number!(GAP, Some(0.0))
            } else {
                0.0
            };
            // A row given a baseline aligns tops, as a column does.
            let baseline = match function {
                COL => Some(number!(BASELINE, Some(0.0))),
                ROW if context.field(call, BASELINE).is_some() => Some(number!(BASELINE, None)),
                _ => None,
            };
            let body = need!(context.field(call, CHILDREN));
            let children = children!(body);
            let baseline = match baseline {
                Some(baseline) => Some(need!(baseline_index(baseline, children.len()))),
                None => None,
            };
            match (function, baseline) {
                (ROW, None) => crate::display::row(gap, children),
                (ROW, Some(baseline)) => crate::display::aligned_row(
                    crate::display::RowAlignment::Top { baseline },
                    gap,
                    children,
                ),
                (COL, Some(baseline)) => crate::display::col(baseline, gap, children),
                (OVERLAY, _) => layout_overlay(children),
                (ALTERNATIVES, _) if !children.is_empty() => alternatives(children),
                _ => return Err(invalid(function)),
            }
        }
        PAD => {
            let insets = (
                number!(LEFT, Some(0.0)),
                number!(TOP, Some(0.0)),
                number!(RIGHT, Some(0.0)),
                number!(BOTTOM, Some(0.0)),
            );
            let body = need!(context.field(call, CHILD));
            let children = children!(body);
            crate::display::padding(insets.into(), need!(single(children)))
        }
        CENTER => {
            let body = need!(context.field(call, CHILD));
            let children = children!(body);
            crate::display::center(need!(single(children)))
        }
        BRACKET => {
            let delim = arg!(DELIM, read_delim);
            let body = need!(context.field(call, CHILD));
            let children = children!(body);
            bracket(delim, need!(single(children)))
        }
        TEXT => {
            let paint = match context.field(call, PAINT) {
                Some(_) => arg!(PAINT, |value| read_paint(value.as_value())),
                None => Paint::Face(Face::Ink),
            };
            let text = arg!(CONTENT, |value| value.as_text().map(str::to_owned));
            leaf(Leaf::Text {
                text,
                paint,
                script: puri::text::Script::Normal,
            })
        }
        SLOT => slot(),
        RULE => crate::display::rule(),
        DESCEND => descend(
            [arg!(STEP, |value| crate::libraries::path::read_step(
                value.as_value()
            ))],
            None,
            None,
        ),
        DESCEND_PATH => descend(
            arg!(STEPS, |value| crate::libraries::path::read(
                value.as_value()
            )),
            None,
            None,
        ),
        JUMP => crate::display::jump(
            arg!(STEPS, |value| crate::libraries::path::read(
                value.as_value()
            )),
            arg!(DOCUMENT_PATH, |value| crate::libraries::path::read(
                value.as_value()
            )),
        ),
        AT => {
            let steps = arg!(STEPS, |value| crate::libraries::path::read(
                value.as_value()
            ));
            crate::display::at(
                steps,
                context.eval(need!(context.field(call, VALUE)), environment)?,
            )
        }
        CANVAS => {
            let width = number!(WIDTH, None);
            let ascent = number!(ASCENT, Some(0.0));
            let descent = number!(DESCENT, None);
            if width < 0.0 || ascent < 0.0 || descent < 0.0 {
                return Err(invalid(function));
            }
            let Some(fuel) =
                ::grap::f64::to_usize(number!(FUEL, Some(::grap::DEFAULT_FUEL as f64)))
            else {
                return Err(invalid(function));
            };
            let program = context.eval(need!(context.field(call, PROGRAM)), environment)?;
            super::drawing::program(
                crate::display::widget::Extent {
                    width,
                    ascent,
                    descent,
                },
                fuel,
                program,
            )
        }
        SELECTABLE | HOVER_BLOCK | PICKABLE | ON_EVENT => {
            let value = if function == PICKABLE {
                Some(context.eval(need!(context.field(call, VALUE)), environment)?)
            } else {
                None
            };
            let handler = if function == ON_EVENT {
                Some(context.eval(need!(context.field(call, HANDLER)), environment)?)
            } else {
                None
            };
            let body = need!(context.field(call, CHILD));
            let children = children!(body);
            let child = need!(single(children));
            match function {
                SELECTABLE => activatable(child, &site()),
                HOVER_BLOCK => block_hover(child),
                PICKABLE => pickable(child, W::Hover::from(site().hover()), need!(value)),
                ON_EVENT => on_event(child, need!(handler)),
                _ => unreachable!(),
            }
        }
        _ => unreachable!("only advertised layout functions reach this scope"),
    };
    Ok(layout)
}

#[cfg(test)]
mod tests;
