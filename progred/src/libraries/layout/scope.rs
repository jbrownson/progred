//! Evaluation-local layout output. Grap carries only ordinary values; the
//! borrowed FFIs emit into this Rust buffer, including through calls and loops.
use super::*;
use crate::libraries::absent;
use ::grap::{Context, Evaluation, ForeignOverlay};
use std::cell::RefCell;
use vocabulary::*;

const FUNCTIONS: &[CellId] = &[
    ROW,
    COL,
    PAD,
    OVERLAY,
    ALTERNATIVES,
    BRACKET,
    TEXT,
    SLOT,
    DESCEND,
    DESCEND_PATH,
    JUMP,
    AT,
    CANVAS,
    SELECTABLE,
    HOVERABLE,
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

struct Output {
    children: RefCell<Vec<Layout<crate::Editor, crate::frame::Hovered>>>,
}

enum BuildError {
    Absent(Value),
    Halt(Halt),
}

impl From<Halt> for BuildError {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

fn invalid(function: CellId) -> BuildError {
    BuildError::Absent(::grap::absent::with_detail(
        INVALID_PROGRAM,
        ::grap::vocabulary::FUNCTION,
        function.into(),
    ))
}

impl Output {
    fn collect<T>(
        &self,
        run: impl FnOnce() -> Result<T, Halt>,
    ) -> Result<(T, Vec<Layout<crate::Editor, crate::frame::Hovered>>), Halt> {
        let parent = self.children.take();
        let result = run();
        let children = self.children.replace(parent);
        result.map(|result| (result, children))
    }
}

/// Exactly one emitted root is a layout; no emission leaves the ordinary
/// value result alone. A halt or final absent drops the entire output.
pub fn run(
    target: impl Fn() -> ProjectionTarget<crate::Editor, crate::frame::Hovered>,
    evaluate: impl FnOnce(&ForeignOverlay<'_>) -> Evaluation<gid::Value>,
) -> (
    Evaluation<Value>,
    Option<Layout<crate::Editor, crate::frame::Hovered>>,
) {
    let output = Output {
        children: RefCell::new(Vec::new()),
    };
    let unit = Value::record([]);
    let emit =
        |function, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
            match operation(function, context, call, environment, &output, &target) {
                Ok(layout) => Ok(context.effect(|| {
                    output.children.borrow_mut().push(layout);
                    unit.clone()
                })),
                Err(BuildError::Absent(value)) => Ok(value),
                Err(BuildError::Halt(halt)) => Err(halt),
            }
        };
    let mut evaluation = evaluate(&ForeignOverlay::from_value(FUNCTIONS, &emit));
    let mut children = output.children.into_inner();
    let layout = if !evaluation.completed || absent::is_absent(&evaluation.result) {
        None
    } else if children.len() > 1 {
        evaluation.result = absent::with_reason(INVALID_PROGRAM);
        None
    } else {
        children.pop()
    };
    (evaluation, layout)
}

pub(super) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, ::grap::RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let function = input.value?.field(LAYOUT_PROGRAM)?;
    let (evaluation, layout) = run(
        || input.targets.current(),
        |scope| {
            input
                .env
                .apply_runtime_scoped(&function, &[], Some(scope))
                .into_value()
        },
    );
    Some(layout.unwrap_or_else(|| {
        crate::display::at(
            [gid::Step::Key(
                crate::libraries::presentation::vocabulary::RESULT,
            )],
            &if absent::is_absent(&evaluation.result) {
                evaluation.result
            } else {
                ::grap::absent::with_detail(INVALID_PROGRAM, VALUE, evaluation.result)
            },
        )
    }))
}

fn argument<T>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    read: impl FnOnce(&Value) -> Option<T>,
) -> Result<Option<T>, Halt> {
    match context.field(call, field) {
        Some(expression) => context
            .eval_to_value(expression, environment)
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

fn single(
    children: Vec<Layout<crate::Editor, crate::frame::Hovered>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    if children.len() == 1 {
        children.into_iter().next()
    } else {
        None
    }
}

fn operation(
    function: CellId,
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &Output,
    target: &impl Fn() -> ProjectionTarget<crate::Editor, crate::frame::Hovered>,
) -> Result<Layout<crate::Editor, crate::frame::Hovered>, BuildError> {
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
            let (result, children) =
                output.collect(|| context.eval_to_value($body, environment))?;
            if absent::is_absent(&result) {
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
            let baseline = if function == COL {
                number!(BASELINE, Some(0.0))
            } else {
                0.0
            };
            let body = need!(context.field(call, CHILDREN));
            let children = children!(body);
            match function {
                ROW => crate::display::row(gap, children),
                COL if baseline >= 0.0
                    && baseline.fract() == 0.0
                    && (children.is_empty() || baseline < children.len() as f64) =>
                {
                    crate::display::col(baseline as usize, gap, children)
                }
                OVERLAY => layout_overlay(children),
                ALTERNATIVES if !children.is_empty() => alternatives(children),
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
        BRACKET => {
            let delim = arg!(DELIM, |value| match value.as_cell()? {
                PAREN => Some(Delim::Paren),
                SQUARE => Some(Delim::Bracket),
                CURLY => Some(Delim::Brace),
                _ => None,
            });
            let body = need!(context.field(call, CHILD));
            let children = children!(body);
            bracket(delim, need!(single(children)))
        }
        TEXT => {
            let paint = match context.field(call, PAINT) {
                Some(_) => arg!(PAINT, read_paint),
                None => Paint::Face(Face::Ink),
            };
            let text = arg!(CONTENT, |value| text::read(value).map(str::to_owned));
            leaf(Leaf::Text {
                text,
                paint,
                script: puri::text::Script::Normal,
            })
        }
        SLOT => slot(),
        DESCEND => descend(arg!(STEP, crate::libraries::path::read_step), None, None),
        DESCEND_PATH => crate::display::descend_path(arg!(STEPS, crate::libraries::path::read)),
        JUMP => crate::display::jump(
            arg!(STEPS, crate::libraries::path::read),
            arg!(DOCUMENT_PATH, crate::libraries::path::read),
        ),
        AT => {
            let steps = arg!(STEPS, crate::libraries::path::read);
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
            let fuel = number!(FUEL, Some(::grap::DEFAULT_FUEL as f64));
            if fuel < 0.0 || fuel.fract() != 0.0 || fuel > usize::MAX as f64 {
                return Err(invalid(function));
            }
            let program = context.eval(need!(context.field(call, PROGRAM)), environment)?;
            crate::display::drawing_program(
                crate::display::widget::Extent {
                    width,
                    ascent,
                    descent,
                },
                fuel as usize,
                program.into(),
            )
        }
        SELECTABLE | HOVERABLE | HOVER_BLOCK | PICKABLE | ON_EVENT => {
            let value = if function == PICKABLE {
                Some(arg!(VALUE, |value| Some(value.clone())))
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
                SELECTABLE => {
                    let target = target();
                    on_activate(child, target.hover, target.select)
                }
                HOVERABLE => on_hover(child, target().hover),
                HOVER_BLOCK => block_hover(child),
                PICKABLE => pickable(child, target().hover, need!(value)),
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
