//! Evaluation-local controls: native widgets emit alongside ordinary Grap values.
use super::{Library, f64, layout, name, presentation, tree};
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{self, widget};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, ForeignOverlay, Halt,
    RuntimeValue,
};
use gid::{CellId, Value};
use kurbo::{Point, Rect};
use measured::{Extent, choices::ChoiceLayout};
use puri::handler::HasHandler;
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
    pub const UPDATE: CellId = CellId::from_u128(0x551c68d60e0d993c301ee8418c26de72);
    pub const ON_CHANGE: CellId = CellId::from_u128(0xc7c092998288d5492108bc7988d4729c);
    pub const NO_STATE: CellId = CellId::from_u128(0xa447ceb4dea2cb19e5feebaaace4a89b);
    pub const INVALID_INPUT: CellId = CellId::from_u128(0x696753238d1730b42d341ef86a2e2fd0);
    pub const OUTPUT_REQUIRED: CellId = CellId::from_u128(0xec88743fae51737b557a2c57aae7712b);
}
use layout::vocabulary::{HEIGHT, WIDTH};
use presentation::vocabulary::VALUE;
use vocabulary::*;

type Widget<W> = widget::Widget<W, <W as Host>::Hover>;
/// What a slider does with the value it's dragged to.
type Change = Rc<dyn Fn(&mut crate::editing::Session<'_>, &crate::editing::Site, f64)>;

const PADDING_X: f64 = 10.0;
const PADDING_Y: f64 = 4.0;

fn constructor(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let mut fields = Vec::new();
    for key in [CONTROLS, VIEW, VALUE, WIDTH, HEIGHT] {
        let Some(expression) = context.field(call, key) else {
            return Ok(context.missing_runtime_argument(key));
        };
        let value = context.eval(expression, environment)?;
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

/// Store `value` under `key` in the control state at `site`.
fn store(
    session: &mut crate::editing::Session<'_>,
    site: &crate::editing::Site,
    key: CellId,
    value: Value,
) {
    let mut edit = site.open(session);
    let state = set_state(edit.state(), key, value);
    edit.set_state(state);
}

fn current_state(annotation: Option<&Value>) -> Value {
    annotation
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&STATE))
        .cloned()
        .unwrap_or_else(|| ::grap::absent::value(NO_STATE))
}

fn update_function() -> Value {
    Value::record([(::grap::vocabulary::FFI, UPDATE.into())])
}

fn apply_change(
    session: &mut crate::editing::Session<'_>,
    site: &crate::editing::Site,
    handler: &::grap::RuntimeValue,
    value: Value,
) {
    let state = current_state(site.open(session).state());
    let staged = RefCell::new(None);
    let update = |_, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
        let Some(expression) = context.field(call, VALUE) else {
            return Ok(context.missing_argument(VALUE));
        };
        let value = context.eval_to_value(expression, environment)?;
        Ok(context.effect(|| {
            staged.replace(Some(value));
            Value::record([])
        }))
    };
    let result = ::grap::apply(
        handler,
        [
            (VALUE, value.into()),
            (STATE, state.into()),
            (UPDATE, update_function().into()),
        ],
        &session.sources(),
        Some(&ForeignOverlay::from_value(&[UPDATE], &update)),
        ::grap::DEFAULT_FUEL,
    );
    if result.completed
        && !result.result.declines()
        && let Some(value) = staged.into_inner()
    {
        let mut edit = site.open(session);
        let mut annotation = edit
            .state()
            .and_then(Value::as_record)
            .cloned()
            .unwrap_or_default();
        annotation.insert(STATE, value);
        edit.set_state(Value::Record(annotation));
    }
}

struct Drag {
    site: crate::editing::Site,
    change: Change,
    slider: Slider,
    rect: Rect,
    scale: f64,
}

impl widget::gesture::Gesture for Drag {
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool {
        if let Some(point) = samples.last() {
            (self.change)(
                session,
                &self.site,
                self.slider.value_at(self.rect, self.scale, *point),
            );
        }
        false
    }
}

/// A slider whose changes go to its Grap `on change` handler.
fn handler_slider<W: Host>(slider: Slider, width: f64, handler: ::grap::RuntimeValue) -> Widget<W> {
    slider_control(
        slider,
        width,
        Vec::new(),
        Rc::new(move |session, site, value| {
            apply_change(session, site, &handler, f64::value(value));
        }),
    )
}

/// A slider that stores `value` of where it's dragged under `key`.
fn keyed_slider<W: Host>(
    key: CellId,
    slider: Slider,
    width: f64,
    ticks: Vec<puri_widgets::slider::TickLevel>,
    value: impl Fn(f64) -> Value + 'static,
) -> Widget<W> {
    slider_control(
        slider,
        width,
        ticks,
        Rc::new(move |session, site, position| store(session, site, key, value(position))),
    )
}

fn slider_control<W: Host>(
    slider: Slider,
    width: f64,
    ticks: Vec<puri_widgets::slider::TickLevel>,
    change: Change,
) -> Widget<W> {
    let ticks: Rc<[puri_widgets::slider::TickLevel]> = ticks.into();
    Rc::new(move |context| {
        let scale = context.viewing().scale();
        let site = context.site();
        let change = change.clone();
        let ticks = ticks.clone();
        let rail = widget::leaf(
            Extent {
                width: width * scale,
                ascent: puri_widgets::slider::HEIGHT * scale,
                descent: 0.0,
            },
            move |output, placement| {
                output.claim(puri::hover::Probe::occludes(placement));
                output.render(move |canvas, _| slider.draw(canvas, placement.rect, scale, &ticks));
                output
                    .handler()
                    .on_pointer_down(move |world: &mut W, event| {
                        let point = Point::new(event.state.position.x, event.state.position.y);
                        if !puri::interact::is_primary_contact(event) || !placement.contains(point)
                        {
                            return false;
                        }
                        world.session().start_gesture(
                            Box::new(Drag {
                                site: site.clone(),
                                change: change.clone(),
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

fn radio_widget<W: Host>(
    key: CellId,
    options: Vec<RadioOption>,
    selected: Value,
    width: f64,
) -> Widget<W> {
    Rc::new(move |context| {
        let scale = context.viewing().scale();
        let mut buttons = Vec::new();
        for option in &options {
            let label = context.faced_text(&option.label, crate::display::Face::Label);
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
            let site = context.site();
            let value = option.value.clone();
            buttons.push(widget::before_place(button, move |placement, output| {
                output.claim(puri::hover::Probe::occludes(placement));
                puri::interact::clickable(output, placement, move |world: &mut W| {
                    store(&mut world.session(), &site, key, value.clone())
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

fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let fields = input.value?.field(WITH_CONTROLS)?;
    let controls = fields.field(CONTROLS)?;
    let view = fields.field(VIEW)?;
    let source = fields.field(VALUE)?;
    let width = fields.field(WIDTH)?.as_f64()?;
    let height = fields.field(HEIGHT)?.as_f64()?;
    (width.is_finite() && height.is_finite() && width > 2.0 * PADDING_X && height > 0.0)
        .then_some(())?;
    let state = input.state.cloned();
    Some(Layout::program(Rc::new(
        move |context, build| match controls_output(&controls, state.as_ref(), width, context) {
            Err(error) => display::at([gid::Step::Key(presentation::vocabulary::RESULT)], &error)
                .measure(context, build),
            Ok((widgets, parameters)) => {
                let controls: Vec<_> = widgets.iter().map(|widget| widget(context)).collect();
                let result = ::grap::apply(
                    &view,
                    [
                        (VALUE, source.clone()),
                        (PARAMETERS, parameters),
                        (WIDTH, ::grap::RuntimeValue::f64(width)),
                        (HEIGHT, ::grap::RuntimeValue::f64(height)),
                    ],
                    context.viewing().host(),
                    None,
                    ::grap::DEFAULT_FUEL,
                );
                let content = display::at(
                    [gid::Step::Key(presentation::vocabulary::RESULT)],
                    &result.result,
                )
                .measure(context, build);
                if controls.is_empty() {
                    content
                } else {
                    controls_over(content, controls)
                }
            }
        },
    )))
}

/// `view` with `controls` in a column over its bottom.
fn controls_over<W: Host>(
    view: ChoiceLayout<widget::HoverPass<W, W::Hover>>,
    controls: Vec<measured::Measured<widget::HoverPass<W, W::Hover>>>,
) -> ChoiceLayout<widget::HoverPass<W, W::Hover>> {
    let controls = widget::before_place(measured::col(0, 0.0, controls), |placement, output| {
        output.claim(puri::hover::Probe::occludes(placement));
        // The padding is part of the control surface too, not an orbit handle.
        output.handler().on_pointer_down(move |_, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
        });
    });
    ChoiceLayout::attach(view, ChoiceLayout::fixed(controls), |view, controls| {
        measured::overlay_into(view, controls, |placement, extent| {
            Some(uig::Placement::new(
                Rect::from_origin_size(
                    (placement.rect.x0, placement.rect.y1 - extent.height()),
                    extent.size(),
                ),
                placement.clip_rect.intersect(placement.rect),
            ))
        })
    })
}

fn controls_output<W: Host>(
    controls: &::grap::RuntimeValue,
    state: Option<&Value>,
    width: f64,
    frame: &widget::Context<'_, '_, W, W::Hover>,
) -> Result<(Vec<Widget<W>>, RuntimeValue), RuntimeValue> {
    let widgets: RefCell<Vec<Widget<W>>> = RefCell::new(Vec::new());
    let emit = |function,
                context: &mut Context<'_>,
                call: &Expression,
                environment: &Environment| {
        if function == SLIDER {
            let Some(expression) = context.field(call, VALUE) else {
                return Ok(context.missing_runtime_argument(VALUE));
            };
            let Some(value) = context.eval_f64(expression, environment)? else {
                return Ok(::grap::absent::value(INVALID_INPUT).into());
            };
            let Some(expression) = context.field(call, ON_CHANGE) else {
                return Ok(context.missing_runtime_argument(ON_CHANGE));
            };
            let handler = context.eval(expression, environment)?;
            if handler.is_absent() {
                return Ok(handler);
            }
            let mut number = |field, default| -> Result<Option<f64>, Halt> {
                match context.field(call, field) {
                    Some(expression) => context.eval_f64(expression, environment),
                    None => Ok(Some(default)),
                }
            };
            let (Some(min), Some(max)) = (number(MINIMUM, 0.0)?, number(MAXIMUM, 1.0)?) else {
                return Ok(::grap::absent::value(INVALID_INPUT).into());
            };
            let Some(slider) = Slider::new(min, max, value) else {
                return Ok(::grap::absent::value(INVALID_INPUT).into());
            };
            return Ok(context.effect(|| {
                widgets
                    .borrow_mut()
                    .push(handler_slider(slider, width - 2.0 * PADDING_X, handler));
                RuntimeValue::f64(slider.value)
            }));
        }
        let Some(key) = context.field(call, KEY) else {
            return Ok(context.missing_runtime_argument(KEY));
        };
        let key = context.eval(key, environment)?;
        let Some(key) = key.as_cell() else {
            return Ok(::grap::absent::value(INVALID_INPUT).into());
        };
        if function == TREE_PROGRAM_CURSOR {
            let Some(program) = context.field(call, tree::vocabulary::PROGRAM) else {
                return Ok(context.missing_runtime_argument(tree::vocabulary::PROGRAM));
            };
            let program = context.eval(program, environment)?;
            if program.is_absent() {
                return Ok(program);
            }
            let fuel = match context.field(call, layout::vocabulary::FUEL) {
                Some(expression) => match context
                    .eval_f64(expression, environment)?
                    .and_then(::grap::f64::to_usize)
                {
                    Some(fuel) => fuel,
                    None => return Ok(::grap::absent::value(INVALID_INPUT).into()),
                },
                None => ::grap::DEFAULT_FUEL,
            };
            let initial = match context.field(call, INITIAL) {
                Some(expression) => match context.eval_f64(expression, environment)? {
                    Some(n) if n.is_finite() => n,
                    _ => return Ok(::grap::absent::value(INVALID_INPUT).into()),
                },
                None => 0.0,
            };
            let result = tree::prepared(
                frame.viewing().computations(),
                &frame.site().below(&[gid::Step::Key(key)]),
                program,
                fuel,
            );
            let built = match result.as_ref() {
                Ok(tree) => tree,
                Err(error) => return Ok(error.clone()),
            };
            let root = built.root.clone();
            let decorate: tree_range::ItemDecoration<W> = Rc::new(move |key| {
                let source = crate::hover::from_grap(root.at(key)?.source.clone()?, None)?;
                Some(crate::display::widget::source_link::decoration(source))
            });
            let (controls, cursor) = tree_range::cursor_from_tree(
                tree_range::emitted_tree(&built.root, Vec::new()),
                read_state(state, key),
                initial,
                key,
                width - 2.0 * PADDING_X,
                Some(decorate),
            );
            let value = RuntimeValue::record(
                cursor
                    .into_iter()
                    .map(|(key, value)| (key, value.into()))
                    .chain([(ITEMS, built.items.clone())]),
            );
            return Ok(context.effect(|| {
                widgets.borrow_mut().extend(controls);
                value
            }));
        }
        if function == TREE_RANGE || function == TREE_CURSOR {
            let Some(expression) = context.field(call, ITEMS) else {
                return Ok(context.missing_runtime_argument(ITEMS));
            };
            let decorate = stored_item_links(context, expression.clone());
            let items = context.eval_to_value(expression, environment)?;
            if ::grap::absent::is_absent(&items) {
                return Ok(items.into());
            }
            if function == TREE_CURSOR {
                let initial = match context.field(call, INITIAL) {
                    Some(expression) => match context.eval_f64(expression, environment)? {
                        Some(n) if n.is_finite() => n,
                        _ => return Ok(::grap::absent::value(INVALID_INPUT).into()),
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
                    result.into()
                }));
            }
            let selection = tree_range::Selection::new(&items, read_state(state, key));
            return Ok(context.effect(|| {
                widgets.borrow_mut().extend(selection.widgets(
                    key,
                    width - 2.0 * PADDING_X,
                    decorate,
                ));
                tree_range::encode(selection.leaves.clone()).into()
            }));
        }
        if function == RADIO {
            let Some(expression) = context.field(call, OPTIONS) else {
                return Ok(context.missing_runtime_argument(OPTIONS));
            };
            let options = context.eval_to_value(expression, environment)?;
            let Some(options) = radio_options(&options) else {
                return Ok(::grap::absent::value(INVALID_INPUT).into());
            };
            let initial = match context.field(call, INITIAL) {
                Some(expression) => Some(context.eval_to_value(expression, environment)?),
                None => options.first().map(|option| option.value.clone()),
            };
            let Some(initial) =
                initial.filter(|initial| options.iter().any(|option| option.value == *initial))
            else {
                return Ok(::grap::absent::value(INVALID_INPUT).into());
            };
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
                selected.into()
            }));
        }
        Ok(::grap::absent::value(INVALID_INPUT).into())
    };
    let evaluation = ::grap::apply(
        controls,
        [
            (STATE, current_state(state).into()),
            (UPDATE, update_function().into()),
        ],
        frame.viewing().host(),
        Some(&ForeignOverlay::new(
            &[SLIDER, RADIO, TREE_RANGE, TREE_CURSOR, TREE_PROGRAM_CURSOR],
            &emit,
        )),
        ::grap::DEFAULT_FUEL,
    );
    if !evaluation.completed || evaluation.result.is_absent() {
        Err(evaluation.result)
    } else {
        Ok((widgets.into_inner(), evaluation.result))
    }
}

/// When `expression` is a list stored in source, a link from each of its
/// items to where that item is stored.
fn stored_item_links<W: Host>(
    context: &Context,
    expression: Expression,
) -> Option<tree_range::ItemDecoration<W>> {
    context.value(&expression).as_list()?;
    let source = crate::hover::from_grap(context.source_origin(&expression)?, None)?;
    Some(Rc::new(move |key| {
        Some(crate::display::widget::source_link::decoration(
            source.descendant(
                &key.iter()
                    .cloned()
                    .map(gid::Step::Element)
                    .collect::<Vec<_>>(),
            ),
        ))
    }))
}

pub fn library<W: Host>() -> Library<W> {
    let functions = ForeignFunctions::default()
        .register(
            WITH_CONTROLS,
            ForeignFunction::new(constructor)
                .parameters([CONTROLS, VIEW, VALUE, WIDTH, HEIGHT])
                .tracked(),
        )
        .register(
            SLIDER,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([VALUE, ON_CHANGE])
                .tracked(),
        )
        .register(
            UPDATE,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([VALUE])
                .tracked(),
        )
        .register(
            RADIO,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([KEY, OPTIONS])
                .tracked(),
        )
        .register(
            TREE_RANGE,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([KEY, ITEMS])
                .tracked(),
        )
        .register(
            TREE_CURSOR,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([KEY, ITEMS])
                .tracked(),
        )
        .register(
            TREE_PROGRAM_CURSOR,
            ForeignFunction::from_value(|_, _, _| Ok(::grap::absent::value(OUTPUT_REQUIRED)))
                .parameters([KEY, tree::vocabulary::PROGRAM])
                .tracked(),
        );
    name::named_library(
        ID,
        "controls",
        [
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
            (UPDATE, "update"),
            (ON_CHANGE, "on change"),
            (NO_STATE, "no control state"),
            (INVALID_INPUT, "invalid control input"),
            (OUTPUT_REQUIRED, "control output required"),
        ],
        functions,
    )
    .with_projection(display::runtime_partial(display))
}
