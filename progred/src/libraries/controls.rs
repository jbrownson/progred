//! Evaluation-local controls: native widgets emit alongside ordinary Grap values.
use super::{Definitions, Library, absent, f64, layout, name, presentation, tree};
use crate::display::widget::view::Root;
use crate::display::{self, Layout, ProjectionInput, widget};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, ForeignOverlay, Halt,
    RuntimeValue,
};
use gid::{CellId, Cells, Value};
use measured::{Extent, choices::ChoiceLayout};
use puri::handler::HasHandler;
use puri::{Point, Rect};
use puri_widgets::slider::Slider;
use std::{cell::RefCell, rc::Rc};

#[cfg(test)]
mod tests;
pub(crate) mod tree_range;

pub const ID: CellId = CellId::from_u128(0x666ba40b81028e32c78bdc1665d6c3a9);
pub mod vocabulary {
    use gid::CellId;
    pub const WITH_CONTROLS: CellId = CellId::from_u128(0x987acea0ded8eadb18f6b926e945cb0a);
    pub const CONTROLS: CellId = CellId::from_u128(0xb9d3424b750ee1fbba4df638d995e236);
    pub const VIEW: CellId = CellId::from_u128(0x047d43dfc6ba836cd75a7c840f4b9df7);
    pub const PARAMETERS: CellId = CellId::from_u128(0x6f0875ea2329b14673435cc13e2836d0);
    pub const SLIDER: CellId = CellId::from_u128(0xe3c52729da1f3b54e99ef208ad9d9201);
    pub const RADIO: CellId = CellId::from_u128(0xd3e46e8851004f6786bb6cf771ad1d87);
    pub const TREE_RANGE: CellId = CellId::from_u128(0xe6031ee7046216b43b9fa9c08bdcd656);
    pub const TREE_CURSOR: CellId = CellId::from_u128(0x21781a5dfb25048f053375147e251b80);
    pub const TREE_PROGRAM_CURSOR: CellId = CellId::from_u128(0x0b8e1e3410cfd16c0b36e18be2fcb8a3);
    pub const RANGE: CellId = CellId::from_u128(0x870bd3c9e1c2d00a7884be4e9173ed68);
    pub const POSITION: CellId = CellId::from_u128(0x199e09fd8b4d41632a90d103d278d6ea);
    pub const ITEMS: CellId = CellId::from_u128(0x685f4fc766c54035725efdd652943c7b);
    pub const ALL: CellId = CellId::from_u128(0x9f39d6c33f96ca6bbc09b1561170abb6);
    pub const OPTIONS: CellId = CellId::from_u128(0xa84188d71fa018de8ad68c74da7e18e2);
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
) -> Result<RuntimeValue, Halt> {
    let mut fields = Vec::new();
    for key in [CONTROLS, VIEW, VALUE, WIDTH, HEIGHT] {
        let Some(expression) = context.field(call, key) else {
            return Ok(context.missing_runtime_argument(key));
        };
        let value = context.eval_runtime(expression, environment)?;
        if value.is_absent() {
            return Ok(value);
        }
        fields.push((key, value));
    }
    Ok(RuntimeValue::record([(
        WITH_CONTROLS,
        RuntimeValue::record(fields),
    )]))
}

fn read_state(state: Option<&Value>, key: CellId) -> Option<&Value> {
    state?.as_record()?.get(&STATE)?.as_record()?.get(&key)
}

fn set_state(state: Option<&Value>, key: CellId, value: Value) -> Value {
    let mut fields = state
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    let mut controls = fields
        .get(&STATE)
        .and_then(Value::as_record)
        .cloned()
        .unwrap_or_default();
    controls.insert(key, value);
    fields.insert(STATE, Value::Record(controls));
    Value::Record(fields)
}

struct Drag {
    root: Root,
    path: gid::Path,
    edits: crate::editing::Scope,
    key: CellId,
    slider: Slider,
    value: Rc<dyn Fn(f64) -> Value>,
    rect: Rect,
    scale: f64,
}

impl widget::gesture::Gesture<crate::Editor> for Drag {
    fn advance(&mut self, editor: &mut crate::Editor, samples: &[Point]) -> bool {
        if let Some(point) = samples.last() {
            let mut editor = self.edits.open(crate::editing::Access::new(editor));
            let state = editor.annotation(&self.root, &self.path);
            let value = set_state(
                state,
                self.key,
                (self.value)(self.slider.value_at(self.rect, self.scale, *point)),
            );
            editor.annotate(&self.root, &self.path, value);
        }
        false
    }
}

fn slider_widget(key: CellId, slider: Slider, width: f64) -> Widget {
    slider_widget_with(key, slider, width, Vec::new(), Rc::new(f64::value))
}

fn slider_widget_with(
    key: CellId,
    slider: Slider,
    width: f64,
    ticks: Vec<puri_widgets::slider::TickLevel>,
    value: Rc<dyn Fn(f64) -> Value>,
) -> Widget {
    let ticks: Rc<[puri_widgets::slider::TickLevel]> = ticks.into();
    Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let root = context.inputs.view.clone();
        let path = context.path.to_vec();
        let edits = context.inputs.edits.clone();
        let value = value.clone();
        let ticks = ticks.clone();
        let rail = widget::leaf(
            Extent {
                width: width * scale,
                ascent: puri_widgets::slider::HEIGHT * scale,
                descent: 0.0,
            },
            move |output, placement| {
                output.claim(puri::hover::Probe::occludes(placement));
                output.render(move |canvas, _| {
                    slider.draw_with_ticks(canvas, placement.rect, scale, &ticks)
                });
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
                            edits: edits.clone(),
                            key,
                            slider,
                            value: value.clone(),
                            rect: placement.rect,
                            scale,
                        }),
                        &[point],
                    );
                    true
                });
            },
        );
        measured::pad((PADDING_X * scale, PADDING_Y * scale).into(), rail)
    })
}

#[derive(Clone)]
struct RadioOption {
    label: String,
    value: Value,
}

fn radio_options(value: &Value) -> Option<Vec<RadioOption>> {
    let mut options = Vec::<RadioOption>::new();
    for value in value.as_list()?.values() {
        let fields = value.as_record()?;
        let label = name::read(value)?.to_owned();
        let value = fields.get(&VALUE)?.clone();
        if options.iter().any(|option| option.value == value) {
            return None;
        }
        options.push(RadioOption { label, value });
    }
    (!options.is_empty()).then_some(options)
}

fn radio_widget(key: CellId, options: Vec<RadioOption>, selected: Value, width: f64) -> Widget {
    Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let mut buttons = Vec::new();
        for option in &options {
            let label = puri::text::text(context.text, &option.label, &context.inputs.styles.label);
            let label_extent = widget::extent(label.metrics());
            let indicator_size = puri_widgets::radio::SIZE * scale;
            let radio = puri_widgets::radio::Radio {
                selected: option.value == selected,
            };
            let indicator = widget::paint(
                Extent {
                    width: indicator_size,
                    ascent: indicator_size / 2.0,
                    descent: indicator_size / 2.0,
                },
                move |canvas, placement| radio.draw(canvas, placement.rect, scale),
            );
            let label = widget::paint(label_extent, move |canvas, placement| {
                label.place(canvas, placement)
            });
            let button = measured::centered_row(4.0 * scale, vec![indicator, label]);
            let root = context.inputs.view.clone();
            let path = context.path.to_vec();
            let edits = context.inputs.edits.clone();
            let value = option.value.clone();
            buttons.push(widget::before_place(button, move |placement, output| {
                output.claim(puri::hover::Probe::occludes(placement));
                puri::interact::clickable(output, placement, move |editor: &mut crate::Editor| {
                    let mut editor = edits.open(crate::editing::Access::new(editor));
                    let state = editor.annotation(&root, &path);
                    let state = set_state(state, key, value.clone());
                    editor.annotate(&root, &path, state);
                });
            }));
        }
        let gap = 12.0 * scale;
        let row_width = buttons.iter().map(|b| b.extent.width).sum::<f64>()
            + gap * buttons.len().saturating_sub(1) as f64;
        let group = if row_width <= width * scale {
            measured::row(gap, buttons)
        } else {
            measured::col(0, 4.0 * scale, buttons)
        };
        measured::pad((PADDING_X * scale, PADDING_Y * scale).into(), group)
    })
}

fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let fields = input.value?.as_record()?.get(&WITH_CONTROLS)?.as_record()?;
    let controls = fields.get(&CONTROLS)?.clone();
    let view = fields.get(&VIEW)?.clone();
    let source = fields.get(&VALUE)?.clone();
    let width = f64::read(fields.get(&WIDTH)?)?;
    let height = f64::read(fields.get(&HEIGHT)?)?;
    if !width.is_finite() || !height.is_finite() || width <= 2.0 * PADDING_X || height <= 0.0 {
        return None;
    }
    let state = input.state.cloned();
    Some(Layout::program(Rc::new(move |context, build| {
        let (widgets, parameters) = match controls_output(&controls, state.as_ref(), width, context)
        {
            Ok(output) => output,
            Err(error) => {
                return display::at([gid::Step::Key(presentation::vocabulary::RESULT)], &error)
                    .measure(context, build);
            }
        };
        let controls: Vec<_> = widgets.iter().map(|widget| widget(context)).collect();
        let result = ::grap::apply(
            &view,
            [
                (VALUE, source.clone()),
                (PARAMETERS, parameters),
                (WIDTH, f64::value(width)),
                (HEIGHT, f64::value(height)),
            ],
            &context.inputs.sources,
            ::grap::DEFAULT_FUEL,
        );
        let content = display::at(
            [gid::Step::Key(
                crate::libraries::presentation::vocabulary::RESULT,
            )],
            &result.result,
        )
        .measure(context, build);
        if controls.is_empty() {
            return content;
        }
        let controls =
            widget::before_place(measured::col(0, 0.0, controls), |placement, output| {
                output.claim(puri::hover::Probe::occludes(placement));
                // The padding is part of the control surface too, not an orbit handle.
                output.handler().on_pointer_down(move |_, event| {
                    placement.contains(Point::new(event.state.position.x, event.state.position.y))
                });
            });
        ChoiceLayout::attach(content, ChoiceLayout::fixed(controls), |view, controls| {
            measured::overlay_into(view, controls, |placement, extent| {
                Some(puri::Placement::new(
                    Rect::from_origin_size(
                        (placement.rect.x0, placement.rect.y1 - extent.height()),
                        extent.size(),
                    ),
                    placement.clip_rect.intersect(placement.rect),
                ))
            })
        })
    })))
}

fn controls_output(
    controls: &Value,
    state: Option<&Value>,
    width: f64,
    frame: &widget::Context<'_, '_, crate::Editor, crate::frame::Hovered>,
) -> Result<(Vec<Widget>, Value), Value> {
    let widgets: RefCell<Vec<Widget>> = RefCell::new(Vec::new());
    let emit = |function, context: &mut Context<'_>, call, environment: &Environment| {
        let Some(key) = context.field(call, KEY) else {
            return Ok(context.missing_argument(KEY));
        };
        let key = context.eval(key, environment)?;
        let Some(key) = key.as_cell() else {
            return Ok(absent::with_reason(INVALID_INPUT));
        };
        if function == TREE_PROGRAM_CURSOR {
            let Some(program) = context.field(call, tree::vocabulary::PROGRAM) else {
                return Ok(context.missing_argument(tree::vocabulary::PROGRAM));
            };
            let program = context.eval(program, environment)?;
            if absent::is_absent(&program) {
                return Ok(program);
            }
            let fuel = match context.field(call, layout::vocabulary::FUEL) {
                Some(expression) => match context.eval_f64(expression, environment)? {
                    Some(n) if n >= 0.0 && n <= usize::MAX as f64 && n.fract() == 0.0 => n as usize,
                    _ => return Ok(absent::with_reason(INVALID_INPUT)),
                },
                None => ::grap::DEFAULT_FUEL,
            };
            let initial = match context.field(call, INITIAL) {
                Some(expression) => match context.eval_f64(expression, environment)? {
                    Some(n) if n.is_finite() => n,
                    _ => return Ok(absent::with_reason(INVALID_INPUT)),
                },
                None => 0.0,
            };
            let result = match frame.inputs.computations {
                Some(computations) => tree::prepared(
                    computations,
                    &frame.inputs.view,
                    &frame
                        .path
                        .iter()
                        .cloned()
                        .chain([gid::Step::Key(key)])
                        .collect::<Vec<_>>(),
                    program,
                    fuel,
                ),
                None => Rc::new(tree::build(&program, &frame.inputs.sources, fuel)),
            };
            let built = match result.as_ref() {
                Ok(tree) => tree,
                Err(error) => return Ok(error.clone()),
            };
            let root = built.root.clone();
            let decorate: tree_range::ItemDecoration = Rc::new(move |key| {
                let source = crate::hover::from_grap(root.at(key)?.source.clone()?, None)?;
                Some(crate::projection::source_link::decoration(source))
            });
            let (controls, cursor) = tree_range::cursor_from_tree(
                tree_range::emitted_tree(&built.root, Vec::new()),
                read_state(state, key),
                initial,
                key,
                width - 2.0 * PADDING_X,
                Some(decorate),
            );
            let value = Value::record(
                cursor
                    .as_record()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (*key, value.clone()))
                    .chain([(ITEMS, built.items.clone())]),
            );
            return Ok(context.effect(|| {
                widgets.borrow_mut().extend(controls);
                value
            }));
        }
        if function == TREE_RANGE || function == TREE_CURSOR {
            let Some(expression) = context.field(call, ITEMS) else {
                return Ok(context.missing_argument(ITEMS));
            };
            let decorate = stored_tree_items(context, expression);
            let items = context.eval(expression, environment)?;
            if absent::is_absent(&items) {
                return Ok(items);
            }
            if function == TREE_CURSOR {
                let initial = match context.field(call, INITIAL) {
                    Some(expression) => match context.eval_f64(expression, environment)? {
                        Some(n) if n.is_finite() => n,
                        _ => return Ok(absent::with_reason(INVALID_INPUT)),
                    },
                    None => 0.0,
                };
                let (controls, result) = tree_range::cursor(
                    &items,
                    read_state(state, key),
                    initial,
                    key,
                    width - 2.0 * PADDING_X,
                    decorate,
                );
                return Ok(context.effect(|| {
                    widgets.borrow_mut().extend(controls);
                    result
                }));
            }
            let selection = tree_range::Selection::new(&items, read_state(state, key));
            return Ok(context.effect(|| {
                widgets.borrow_mut().extend(selection.widgets(
                    key,
                    width - 2.0 * PADDING_X,
                    decorate,
                ));
                tree_range::encode(selection.leaves.clone())
            }));
        }
        if function == RADIO {
            let Some(expression) = context.field(call, OPTIONS) else {
                return Ok(context.missing_argument(OPTIONS));
            };
            let options = context.eval(expression, environment)?;
            let Some(options) = radio_options(&options) else {
                return Ok(absent::with_reason(INVALID_INPUT));
            };
            let initial = match context.field(call, INITIAL) {
                Some(expression) => context.eval(expression, environment)?,
                None => options[0].value.clone(),
            };
            if !options.iter().any(|option| option.value == initial) {
                return Ok(absent::with_reason(INVALID_INPUT));
            }
            let selected = read_state(state, key)
                .filter(|value| options.iter().any(|option| option.value == **value))
                .cloned()
                .unwrap_or(initial);
            return Ok(context.effect(|| {
                widgets.borrow_mut().push(radio_widget(
                    key,
                    options,
                    selected.clone(),
                    width - 2.0 * PADDING_X,
                ));
                selected
            }));
        }
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
        let value = read_state(state, key)
            .and_then(f64::read)
            .filter(|value| value.is_finite())
            .unwrap_or(initial);
        let Some(slider) = Slider::new(min, max, value) else {
            return Ok(absent::with_reason(INVALID_INPUT));
        };
        Ok(context.effect(|| {
            widgets
                .borrow_mut()
                .push(slider_widget(key, slider, width - 2.0 * PADDING_X));
            f64::value(slider.value)
        }))
    };
    let evaluation = ::grap::apply_scoped(
        controls,
        [],
        &frame.inputs.sources,
        &ForeignOverlay::new(
            &[SLIDER, RADIO, TREE_RANGE, TREE_CURSOR, TREE_PROGRAM_CURSOR],
            &emit,
        ),
        ::grap::DEFAULT_FUEL,
    );
    if !evaluation.completed || absent::is_absent(&evaluation.result) {
        Err(evaluation.result)
    } else {
        Ok((widgets.into_inner(), evaluation.result))
    }
}

fn stored_tree_items(
    context: &Context,
    expression: Expression,
) -> Option<tree_range::ItemDecoration> {
    context.value(expression).as_list()?;
    let source = crate::hover::from_grap(context.source_origin(expression)?, None)?;
    Some(Rc::new(move |key| {
        Some(crate::projection::source_link::decoration(
            source.descendant(
                &key.iter()
                    .cloned()
                    .map(gid::Step::Element)
                    .collect::<Vec<_>>(),
            ),
        ))
    }))
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (key, label) in [
        (WITH_CONTROLS, "with controls"),
        (CONTROLS, "controls"),
        (VIEW, "view"),
        (PARAMETERS, "parameters"),
        (SLIDER, "slider"),
        (RADIO, "radio"),
        (TREE_RANGE, "tree range"),
        (TREE_CURSOR, "tree cursor"),
        (TREE_PROGRAM_CURSOR, "tree program cursor"),
        (RANGE, "range"),
        (POSITION, "position"),
        (ITEMS, "items"),
        (ALL, "all"),
        (OPTIONS, "options"),
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
        .register(
            WITH_CONTROLS,
            ForeignFunction::runtime(constructor).tracked(),
        )
        .register(
            SLIDER,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        )
        .register(
            RADIO,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        )
        .register(
            TREE_RANGE,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        )
        .register(
            TREE_CURSOR,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        )
        .register(
            TREE_PROGRAM_CURSOR,
            ForeignFunction::new(|_, _, _| Ok(absent::with_reason(OUTPUT_REQUIRED))),
        );
    Library::named(
        ID,
        "controls",
        Definitions::from_parts(cells, functions),
        display::partial(display),
    )
}
