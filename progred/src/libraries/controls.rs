//! Evaluation-local controls: native widgets emit alongside ordinary Grap values.
use super::{Definitions, Library, absent, f64, layout, name, presentation};
use crate::display::widget::view::Root;
use crate::display::{self, Layout, ProjectionInput, widget};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, ForeignOverlay, Halt,
};
use gid::{CellId, Cells, Value};
use measured::{Extent, choices::ChoiceLayout};
use puri::handler::HasHandler;
use puri::{Point, Rect};
use puri_widgets::slider::Slider;
use std::{cell::RefCell, rc::Rc};

#[cfg(test)]
mod tests;

pub const ID: CellId = CellId::from_u128(0x666ba40b81028e32c78bdc1665d6c3a9);
pub mod vocabulary {
    use gid::CellId;
    pub const WITH_CONTROLS: CellId = CellId::from_u128(0x987acea0ded8eadb18f6b926e945cb0a);
    pub const CONTROLS: CellId = CellId::from_u128(0xb9d3424b750ee1fbba4df638d995e236);
    pub const VIEW: CellId = CellId::from_u128(0x047d43dfc6ba836cd75a7c840f4b9df7);
    pub const PARAMETERS: CellId = CellId::from_u128(0x6f0875ea2329b14673435cc13e2836d0);
    pub const SLIDER: CellId = CellId::from_u128(0xe3c52729da1f3b54e99ef208ad9d9201);
    pub const KEY: CellId = CellId::from_u128(0x600ed992016bae93f3c9ab48a9d7d7b5);
    pub const MINIMUM: CellId = CellId::from_u128(0x27d7b8d3c3a6a902ed495684e7bcd4d5);
    pub const MAXIMUM: CellId = CellId::from_u128(0x30113361e78c2e47bb88176ac834cac1);
    pub const INITIAL: CellId = CellId::from_u128(0x133b05e33c166614831f01de2365f49c);
    pub const STATE: CellId = CellId::from_u128(0x26f287e5868627eacc4b5a2981932e55);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x696753238d1730b42d341ef86a2e2fd0);
    pub const OUTPUT_REQUIRED: CellId = CellId::from_u128(0xec88743fae51737b557a2c57aae7712b);
}
use layout::vocabulary::{HEIGHT, WIDTH};
use presentation::vocabulary::VALUE;
use vocabulary::*;

type Widget = widget::Widget<crate::Editor, crate::frame::Hovered>;

const PADDING_X: f64 = 10.0;
const PADDING_Y: f64 = 4.0;

fn constructor(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<Value, Halt> {
    let mut fields = Vec::new();
    for key in [CONTROLS, VIEW, VALUE, WIDTH, HEIGHT] {
        let Some(expression) = context.field(call, key) else {
            return Ok(context.missing_argument(key));
        };
        let value = context.eval(expression, environment)?;
        if absent::is_absent(&value) {
            return Ok(value);
        }
        fields.push((key, value));
    }
    Ok(Value::record([(WITH_CONTROLS, Value::record(fields))]))
}

fn read_state(state: Option<&Value>, key: CellId) -> Option<f64> {
    f64::read(state?.as_record()?.get(&STATE)?.as_record()?.get(&key)?)
        .filter(|value| value.is_finite())
}

fn set_state(state: Option<&Value>, key: CellId, value: f64) -> Value {
    let mut fields = state
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    let mut controls = fields
        .get(&STATE)
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    controls.insert(key, f64::value(value));
    fields.insert(STATE, Value::Record(controls));
    Value::Record(fields)
}

struct Drag {
    root: Root,
    path: gid::Path,
    key: CellId,
    slider: Slider,
    rect: Rect,
    scale: f64,
}

impl widget::gesture::Gesture<crate::Editor> for Drag {
    fn advance(&mut self, editor: &mut crate::Editor, samples: &[Point]) -> bool {
        if let Some(point) = samples.last() {
            let state = editor
                .model
                .workspace
                .view(&self.root)
                .and_then(|v| v.annotations.at(&self.path));
            let value = set_state(
                state,
                self.key,
                self.slider.value_at(self.rect, self.scale, *point),
            );
            crate::editing::annotate(editor, &self.root, &self.path, value);
        }
        false
    }
}

fn slider_widget(key: CellId, label: String, slider: Slider, width: f64) -> Widget {
    Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let root = context.inputs.view.clone();
        let path = context.path.to_vec();
        let label = puri::text::text(
            context.text,
            &format!("{label}  {:.3}", slider.value),
            &context.inputs.styles.label,
        );
        let metrics = label.metrics();
        let label = widget::paint(widget::extent(metrics), move |canvas, placement| {
            label.place(canvas, placement)
        });
        let rail = widget::leaf(
            Extent {
                width: width * scale,
                ascent: puri_widgets::slider::HEIGHT * scale,
                descent: 0.0,
            },
            move |output, placement| {
                output.claim(puri::hover::Probe::occludes(placement));
                output.render(move |canvas, _| slider.draw(canvas, placement.rect, scale));
                output.handler().on_pointer_down(move |editor, event| {
                    let point = Point::new(event.state.position.x, event.state.position.y);
                    if !puri::interact::is_primary_contact(event) || !placement.contains(point) {
                        return false;
                    }
                    crate::editing::start_gesture(
                        editor,
                        Box::new(Drag {
                            root: root.clone(),
                            path: path.clone(),
                            key,
                            slider,
                            rect: placement.rect,
                            scale,
                        }),
                        &[point],
                    );
                    true
                });
            },
        );
        measured::pad(
            (PADDING_X * scale, PADDING_Y * scale).into(),
            measured::col(0, 0.0, vec![label, rail]),
        )
    })
}

fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&WITH_CONTROLS)?.as_record()?;
    let controls = fields.get(&CONTROLS)?;
    let view = fields.get(&VIEW)?.clone();
    let source = fields.get(&VALUE)?.clone();
    let width = f64::read(fields.get(&WIDTH)?)?;
    let height = f64::read(fields.get(&HEIGHT)?)?;
    if !width.is_finite() || !height.is_finite() || width <= 2.0 * PADDING_X || height <= 0.0 {
        return None;
    }
    let widgets: RefCell<Vec<Widget>> = RefCell::new(Vec::new());
    let emit = |_, context: &mut Context<'_>, call, environment: &Environment| {
        let Some(key) = context.field(call, KEY) else {
            return Ok(context.missing_argument(KEY));
        };
        let key = context.eval(key, environment)?;
        let Some(key) = key.as_cell() else {
            return Ok(absent::with_reason(INVALID_INPUT));
        };
        let mut number = |field, default| -> Result<Option<f64>, Halt> {
            match context.field(call, field) {
                Some(expression) => context.eval_f64(expression, environment),
                None => Ok(Some(default)),
            }
        };
        let (Some(min), Some(max), Some(initial)) = (
            number(MINIMUM, 0.0)?,
            number(MAXIMUM, 1.0)?,
            number(INITIAL, 0.0)?,
        ) else {
            return Ok(absent::with_reason(INVALID_INPUT));
        };
        let Some(slider) = Slider::new(min, max, read_state(input.state, key).unwrap_or(initial))
        else {
            return Ok(absent::with_reason(INVALID_INPUT));
        };
        let label = input.env.name(key).unwrap_or("").to_owned();
        Ok(context.effect(|| {
            widgets
                .borrow_mut()
                .push(slider_widget(key, label, slider, width - 2.0 * PADDING_X));
            f64::value(slider.value)
        }))
    };
    let evaluation =
        input
            .env
            .apply_scoped(controls, &[], Some(&ForeignOverlay::new(&[SLIDER], &emit)));
    if !evaluation.completed || absent::is_absent(&evaluation.result) {
        return Some(display::transient(
            &evaluation.result,
            evaluation.remaining_fuel,
        ));
    }
    let widgets = widgets.into_inner();
    Some(Layout::program(Rc::new(move |context, build| {
        let controls: Vec<_> = widgets.iter().map(|widget| widget(context)).collect();
        let controls_height: f64 = controls.iter().map(|c| c.extent.height()).sum();
        let view_height = (height - controls_height / context.inputs.styles.scale).max(0.0);
        let result = ::grap::apply(
            &view,
            [
                (VALUE, source.clone()),
                (PARAMETERS, evaluation.result.clone()),
                (WIDTH, f64::value(width)),
                (HEIGHT, f64::value(view_height)),
            ],
            &context.inputs.sources,
            evaluation.remaining_fuel,
        );
        let content =
            context
                .project
                .transient(context.text, build, result.result, result.remaining_fuel);
        ChoiceLayout::col(
            0,
            0.0,
            std::iter::once(content)
                .chain(controls.into_iter().map(ChoiceLayout::fixed))
                .collect(),
        )
    })))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (key, label) in [
        (WITH_CONTROLS, "with controls"),
        (CONTROLS, "controls"),
        (VIEW, "view"),
        (PARAMETERS, "parameters"),
        (SLIDER, "slider"),
        (KEY, "key"),
        (MINIMUM, "minimum"),
        (MAXIMUM, "maximum"),
        (INITIAL, "initial"),
        (STATE, "control state"),
        (INVALID_INPUT, "invalid control input"),
        (OUTPUT_REQUIRED, "control output required"),
    ] {
        cells.set_value(key, name::record(label, []));
    }
    let functions = ForeignFunctions::default()
        .register(WITH_CONTROLS, ForeignFunction::new(constructor))
        .register(
            SLIDER,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        );
    Library::named(
        ID,
        "controls",
        Definitions::from_parts(cells, functions),
        display::partial(display),
    )
}
