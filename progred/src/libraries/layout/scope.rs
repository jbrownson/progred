//! Evaluation-local layout output. Grap carries only ordinary values; the
//! borrowed FFIs emit into this Rust buffer, including through calls and loops.
use super::*;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::editor::path;
use ::grap::{Context, Evaluation, RuntimeValue};
use std::cell::RefCell;
use vocabulary::*;

const FUNCTIONS: &[CellId] = &[
    ROW,
    COL,
    PAD,
    CENTER,
    BORDER,
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
    DRAWING,
    SELECTABLE,
    HOVER_BLOCK,
    PICKABLE,
    ON_EVENT,
];

pub(super) fn program(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<::grap::RuntimeValue, ::grap::Early> {
    let closure = context.closure(
        [],
        call.argument(::grap::vocabulary::EXPRESSION)?,
        environment,
    );
    Ok(::grap::isa::runtime(
        kind::LAYOUT_PROGRAM,
        [(PROGRAM, closure)],
    ))
}

/// Why an operation emits no layout.
enum BuildError {
    /// The call's arguments don't describe one.
    Invalid,
    /// A child's computation returned this absent value.
    Absent(RuntimeValue),
    Halt(Halt),
}

impl From<Halt> for BuildError {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

/// Exactly one emitted root is a layout; no emission leaves the ordinary
/// value result alone. A halt or final absent drops the entire output.
pub fn run<W: Host>(
    site: impl Fn() -> crate::editing::Site,
    evaluate: impl FnOnce(ForeignFunctions<'_>) -> Evaluation,
) -> (Evaluation, Option<Layout<W>>) {
    let output = RefCell::new(Vec::new());
    let unit = RuntimeValue::record([]);
    let emit =
        |function, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
            match operation(function, context, call, environment, &output, &site) {
                Ok(layout) => Ok(context.effect(|| {
                    output.borrow_mut().push(layout);
                    unit.clone()
                })),
                Err(BuildError::Invalid) => Ok(::grap::absent::with_detail(
                    INVALID_PROGRAM,
                    ::grap::vocabulary::FUNCTION,
                    function.into(),
                )
                .into()),
                Err(BuildError::Absent(value)) => Ok(value),
                Err(BuildError::Halt(halt)) => Err(halt),
            }
        };
    let mut evaluation = evaluate(ForeignFunctions::shared(FUNCTIONS, &emit));
    let mut children = output.into_inner();
    let layout = if evaluation.result.absent_reason().is_some() {
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
    let function = input.value?.field(PROGRAM)?;
    let (evaluation, layout) = run(
        || input.site(),
        |scope| input.env.apply_runtime_scoped(&function, &[], scope),
    );
    Some(layout.unwrap_or_else(|| {
        crate::libraries::presentation::result_layout(
            if evaluation.result.absent_reason().is_some() {
                evaluation.result
            } else {
                ::grap::absent::runtime(
                    INVALID_PROGRAM,
                    [(::grap::absent::VALUE, evaluation.result)],
                )
            },
        )
    }))
}

/// `field`'s argument, evaluated; a missing one is invalid.
fn evaluated(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
) -> Result<RuntimeValue, BuildError> {
    Ok(context.eval(call.field(field).ok_or(BuildError::Invalid)?, environment)?)
}

/// `field`'s argument, read by `read`; a missing or unreadable one is invalid.
fn argument<T>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    read: impl FnOnce(&RuntimeValue) -> Option<T>,
) -> Result<T, BuildError> {
    optional_argument(context, call, environment, field, read)?.ok_or(BuildError::Invalid)
}

/// `field`'s argument, read by `read`, when the call has one; an unreadable
/// one is invalid.
fn optional_argument<T>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    read: impl FnOnce(&RuntimeValue) -> Option<T>,
) -> Result<Option<T>, BuildError> {
    call.field(field)
        .map(|expression| read(&context.eval(expression, environment)?).ok_or(BuildError::Invalid))
        .transpose()
}

/// `field`'s argument as a finite number; `default`, if given, stands in for
/// a missing one.
fn number_argument(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
    default: Option<f64>,
) -> Result<f64, BuildError> {
    optional_number(context, call, environment, field)?
        .or(default)
        .ok_or(BuildError::Invalid)
}

/// `field`'s argument as a finite number, when the call has one; one that
/// isn't a finite number is invalid.
fn optional_number(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    field: CellId,
) -> Result<Option<f64>, BuildError> {
    call.field(field)
        .map(|expression| {
            context
                .eval_f64(expression, environment)?
                .filter(|value| value.is_finite())
                .ok_or(BuildError::Invalid)
        })
        .transpose()
}

fn read_steps(value: &RuntimeValue) -> Option<Vec<Step>> {
    path::read(value.as_value())
}

/// The layouts `body` emits, collected apart from the enclosing call's; no
/// borrow of `output` is held while it runs.
fn emitted<W: Host>(
    context: &mut Context,
    body: Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Vec<Layout<W>>, BuildError> {
    let parent = output.take();
    let result = context.eval(body, environment);
    let children = output.replace(parent);
    match result? {
        result if result.absent_reason().is_some() => Err(BuildError::Absent(result)),
        _ => Ok(children),
    }
}

/// The layouts the call's `children` emit.
fn children<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Vec<Layout<W>>, BuildError> {
    let body = call.field(CHILDREN).ok_or(BuildError::Invalid)?;
    emitted(context, body, environment, output)
}

/// The one layout the call's `child` emits.
fn single_child<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let body = call.field(CHILD).ok_or(BuildError::Invalid)?;
    <[_; 1]>::try_from(emitted(context, body, environment, output)?)
        .map(|[child]| child)
        .map_err(|_| BuildError::Invalid)
}

fn operation<W: Host>(
    function: CellId,
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
    site: &impl Fn() -> crate::editing::Site,
) -> Result<Layout<W>, BuildError> {
    match function {
        ROW => row(context, call, environment, output),
        COL => col(context, call, environment, output),
        PAD => pad(context, call, environment, output),
        CENTER => single_child(context, call, environment, output).map(crate::display::center),
        BORDER => single_child(context, call, environment, output).map(crate::display::border),
        OVERLAY => children(context, call, environment, output).map(layout_overlay),
        ALTERNATIVES => alternatives(context, call, environment, output),
        BRACKET => bracket(context, call, environment, output),
        TEXT => text(context, call, environment),
        SLOT => Ok(slot()),
        RULE => Ok(crate::display::rule()),
        DESCEND => descend(context, call, environment),
        DESCEND_PATH => descend_path(context, call, environment),
        JUMP => jump(context, call, environment),
        AT => at(context, call, environment),
        DRAWING => drawing(context, call, environment),
        SELECTABLE => {
            single_child(context, call, environment, output).map(|child| selectable(child, &site()))
        }
        HOVER_BLOCK => single_child(context, call, environment, output).map(block_hover),
        PICKABLE => pickable(context, call, environment, output, site),
        ON_EVENT => on_event(context, call, environment, output),
        _ => unreachable!("only advertised layout functions reach this scope"),
    }
}

fn row<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let gap = number_argument(context, call, environment, GAP, Some(0.0))?;
    // A row given a baseline aligns tops, as a column does.
    let baseline = optional_number(context, call, environment, BASELINE)?;
    let children = children(context, call, environment, output)?;
    Ok(match baseline {
        Some(baseline) => crate::display::aligned_row(
            crate::display::RowAlignment::Top {
                baseline: baseline_index(baseline, children.len()).ok_or(BuildError::Invalid)?,
            },
            gap,
            children,
        ),
        None => crate::display::row(gap, children),
    })
}

fn col<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let gap = number_argument(context, call, environment, GAP, Some(0.0))?;
    let baseline = number_argument(context, call, environment, BASELINE, Some(0.0))?;
    let children = children(context, call, environment, output)?;
    Ok(crate::display::col(
        baseline_index(baseline, children.len()).ok_or(BuildError::Invalid)?,
        gap,
        children,
    ))
}

fn pad<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let insets = (
        number_argument(context, call, environment, LEFT, Some(0.0))?,
        number_argument(context, call, environment, TOP, Some(0.0))?,
        number_argument(context, call, environment, RIGHT, Some(0.0))?,
        number_argument(context, call, environment, BOTTOM, Some(0.0))?,
    );
    single_child(context, call, environment, output)
        .map(|child| crate::display::padding(insets.into(), child))
}

fn alternatives<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    match children(context, call, environment, output)? {
        children if children.is_empty() => Err(BuildError::Invalid),
        children => Ok(crate::display::alternatives(children)),
    }
}

fn bracket<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let delim = argument(context, call, environment, DELIM, read_delim)?;
    single_child(context, call, environment, output)
        .map(|child| crate::display::bracket(delim, child))
}

fn text<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let paint = optional_argument(context, call, environment, PAINT, |value| {
        read_paint(value.as_value())
    })?
    .unwrap_or(Paint::Face(Face::Ink));
    let text = argument(context, call, environment, CONTENT, |value| {
        value.as_text().map(str::to_owned)
    })?;
    Ok(leaf(Leaf::Text {
        text,
        paint,
        script: puri::text::Script::Normal,
    }))
}

fn descend<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let step = argument(context, call, environment, STEP, |value| {
        path::read_step(value.as_value())
    })?;
    Ok(crate::display::descend([step], None, None))
}

fn descend_path<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let steps = argument(context, call, environment, STEPS, read_steps)?;
    Ok(crate::display::descend(steps, None, None))
}

fn jump<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let steps = argument(context, call, environment, STEPS, read_steps)?;
    let document = argument(context, call, environment, DOCUMENT_PATH, read_steps)?;
    Ok(crate::display::jump(
        steps,
        crate::location::Location::root(document),
        None,
        None,
    ))
}

fn at<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let steps = argument(context, call, environment, STEPS, read_steps)?;
    Ok(crate::display::at(
        steps,
        evaluated(context, call, environment, VALUE)?,
        None,
        None,
    ))
}

fn drawing<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<Layout<W>, BuildError> {
    let width = number_argument(context, call, environment, WIDTH, None)?;
    let ascent = number_argument(context, call, environment, ASCENT, Some(0.0))?;
    let descent = number_argument(context, call, environment, DESCENT, None)?;
    let extent = (width >= 0.0 && ascent >= 0.0 && descent >= 0.0)
        .then_some(crate::display::widget::Extent {
            width,
            ascent,
            descent,
        })
        .ok_or(BuildError::Invalid)?;
    let fuel = fuel(optional_number(context, call, environment, FUEL)?.map(Some))
        .ok_or(BuildError::Invalid)?;
    let program = evaluated(context, call, environment, PROGRAM)?;
    // An evaluated program has no stored location.
    Ok(super::drawing::program(extent, fuel, None, program))
}

fn pickable<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
    site: &impl Fn() -> crate::editing::Site,
) -> Result<Layout<W>, BuildError> {
    let value = evaluated(context, call, environment, VALUE)?;
    single_child(context, call, environment, output)
        .map(|child| crate::display::pickable(child, W::Hover::from(site().hover()), value))
}

fn on_event<W: Host>(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
    output: &RefCell<Vec<Layout<W>>>,
) -> Result<Layout<W>, BuildError> {
    let handler = evaluated(context, call, environment, HANDLER)?;
    single_child(context, call, environment, output).map(|child| super::on_event(child, handler))
}

#[cfg(test)]
#[path = "tests/scope.rs"]
mod tests;
