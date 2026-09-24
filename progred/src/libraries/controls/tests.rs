use super::*;

fn controls_output(
    controls: &Value,
    state: Option<&Value>,
    width: f64,
    frame: &widget::Context<'_, '_, crate::Editor, crate::frame::Hovered>,
) -> Result<(Vec<Widget>, Value), Value> {
    let callable = ::grap::evaluate(controls, &frame.inputs.sources, ::grap::DEFAULT_FUEL).result;
    super::controls_output(&callable, state, width, frame)
        .map(|(widgets, value)| (widgets, value.into_value()))
        .map_err(RuntimeValue::into_value)
}

fn apply_change(
    editor: &mut crate::Editor,
    scope: &crate::editing::Scope,
    root: &Root,
    path: &[gid::Step],
    handler: &Value,
    value: Value,
) {
    let callable = ::grap::evaluate(handler, &editor.sources(), ::grap::DEFAULT_FUEL).result;
    super::apply_change(editor, scope, root, path, &callable, value)
}

use crate::display::test_support::{ResolveForDispatch, with_context};
use crate::display::widget::project::Project;
use crate::libraries::control::vocabulary as c;
use measured::choices::{ChoiceBuild, resolve_choices};
use puri::{Affine, DrawCmd, DrawList, Placement, Shape};

const A: CellId = CellId::from_u128(1);
const B: CellId = CellId::from_u128(2);

struct Host(crate::libraries::Libraries);
impl display::Env for Host {
    fn apply_scoped(
        &self,
        function: &Value,
        args: &[(CellId, Value)],
        scope: Option<&ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation<gid::Value> {
        match scope {
            Some(scope) => {
                ::grap::apply_value_scoped(function, args.iter().cloned(), &self.0, scope, 10_000)
            }
            None => ::grap::apply_value(function, args.iter().cloned(), &self.0, 10_000),
        }
    }
    fn evaluate(&self, _: &Value) -> Value {
        unreachable!()
    }
}

#[derive(Default)]
struct Output(RefCell<Option<Value>>);
impl Project<crate::Editor, crate::frame::Hovered> for Output {
    fn descend(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        _: &[gid::Step],
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        unreachable!()
    }
    fn jump(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        _: Vec<gid::Step>,
        _: Vec<gid::Step>,
        _: display::Conject,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        unreachable!()
    }
    fn at(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        _: Vec<gid::Step>,
        value: ::grap::RuntimeValue,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        let height = value.field(HEIGHT).unwrap().as_f64().unwrap();
        self.0.replace(Some(value.into_value()));
        ChoiceLayout::fixed(widget::paint(
            Extent {
                width: 200.0,
                ascent: height,
                descent: 0.0,
            },
            |canvas, placement| {
                canvas.fill_shape(
                    placement.rect.into(),
                    puri::Color::from_rgb8(80, 140, 100).into(),
                    Affine::IDENTITY,
                );
            },
        ))
    }
}

fn quote(value: Value) -> Value {
    ::grap::call(c::QUOTE.into(), [(::grap::vocabulary::EXPRESSION, value)])
}
fn splice(value: Value) -> Value {
    Value::record([(c::UNQUOTE, value)])
}

#[test]
fn source_highlight_clips_its_rectangle_without_a_pane_sized_layer() {
    use crate::hover::SourceTrace;
    let source = SourceTrace::Stored(Rc::from([]));
    let rect = Rect::new(30.0, 40.0, 34.0, 56.0);
    for clip in [
        Rect::new(0.0, 0.0, 3000.0, 1800.0),
        Rect::new(32.0, 45.0, 33.0, 50.0),
    ] {
        with_context(&Output::default(), |context| {
            let decorate = crate::projection::source_link::decoration(source.clone())(context);
            let mut output = widget::HoverOutput::default();
            decorate(
                &mut widget::HoverContext::new(Default::default(), &mut output),
                Placement::new(rect, clip),
            );
            let frame = output.bind(widget::ResolvedHover {
                hovered_trace: Some(source.clone()),
                ..Default::default()
            });
            let mut drawing = DrawList::new();
            puri::frame::render(frame.renders, &mut drawing);
            assert!(matches!(drawing.0.as_slice(), [DrawCmd::Fill {
                shape: Shape::Rect(painted), transform, ..
            }] if *painted == rect.intersect(clip) && *transform == Affine::IDENTITY));
        });
    }
}

#[test]
fn cam_collection_is_shared_with_the_view_and_reused_with_its_hover_links() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let computations = crate::computations::Computations::from_sources(sources);
    let root = crate::workspace::Root::document();
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let mut previous: Option<RuntimeValue> = None;
    for width in [400.0, 600.0, 401.0] {
        let declaration = presentation::viewport_output(
            sources.resolve_path(&pane.path).unwrap(),
            &sources,
            width,
            600.0,
        )
        .unwrap();
        let controls = declaration
            .as_record()
            .unwrap()
            .get(&WITH_CONTROLS)
            .unwrap()
            .as_record()
            .unwrap()
            .get(&CONTROLS)
            .unwrap();
        with_context(&Output::default(), |context| {
            let inputs = crate::projection::Cx {
                focused: true,
                sources,
                computations: Some(&computations),
                view: &root,
                ..context.inputs.clone()
            };
            let mut context = widget::Context {
                inputs: &inputs,
                text: context.text,
                project: context.project,
                path: context.path,
                value: context.value,
            };
            let start = std::time::Instant::now();
            let controls = ::grap::evaluate(controls, &sources, ::grap::DEFAULT_FUEL).result;
            let (widgets, parameters) =
                super::controls_output(&controls, None, width, &context).unwrap();
            eprintln!("CAM controls width {width}: {:?}", start.elapsed());
            let tree = parameters
                .field(names["playback"])
                .unwrap()
                .field(ITEMS)
                .unwrap();
            if let Some(previous) = &previous {
                assert!(
                    previous.same_result(&tree),
                    "native leaf code and captures must be reused"
                );
            }
            previous = Some(tree.clone());
            assert_eq!(widgets.len(), 7, "radio, playback, and five grouping rows");
            // A genuinely generated leaf links to its producer after all mapping,
            // control declaration, collection, memo, and widget boundaries.
            let measured = widgets[2](&mut context);
            let point = puri_widgets::range_slider::RangeSlider::new(504, 0..504)
                .unwrap()
                .item_rect(Rect::new(PADDING_X, 0.0, width - PADDING_X, 20.0), 1.0, 0)
                .unwrap()
                .center();
            let placed = widget::frame::place(
                measured,
                Placement::root(Rect::new(0.0, 0.0, width, 20.0)),
                &widget::HoverInput {
                    pointer: Some(point),
                    ..Default::default()
                },
            );
            assert!(
                matches!(&placed.claim, Some((_,puri::hover::Claim::Direct(crate::frame::Hovered::Tree(crate::hover::Hover::Source(crate::hover::SourceTrace::InCell {cell,..}))))) if *cell==names["diagonal_groups"]),
                "{:?}",
                placed.claim
            );
        });
    }
}

#[test]
fn program_cursor_preserves_list_leaves_and_captures_sources_directly() {
    use tree::vocabulary as t;
    let leaf = |value| ::grap::call(t::LEAF.into(), [(VALUE, value)]);
    let group = |children| {
        ::grap::call(
            t::GROUP.into(),
            [(layout::vocabulary::CHILDREN, Value::list(children))],
        )
    };
    let payload = Value::list([f64::value(1.0), f64::value(2.0)]);
    let mut doc = gid::Document {
        root: Some(A.into()),
        cells: Cells::new(),
    };
    doc.cells.set_value(
        A,
        ::grap::lambda(
            [],
            group([
                leaf(payload.clone()),
                group([leaf(f64::value(1.0)), leaf(f64::value(2.0))]),
            ]),
        ),
    );
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let controls = ::grap::lambda(
        [],
        ::grap::call(
            TREE_PROGRAM_CURSOR.into(),
            [(KEY, quote(B.into())), (t::PROGRAM, quote(A.into()))],
        ),
    );
    with_context(&Output::default(), |context| {
        let inputs = crate::projection::Cx {
            focused: true,
            sources,
            ..context.inputs.clone()
        };
        let mut context = widget::Context {
            inputs: &inputs,
            text: context.text,
            project: context.project,
            path: context.path,
            value: context.value,
        };
        let (widgets, result) = controls_output(&controls, None, 200.0, &context).unwrap();
        let fields = result.as_record().unwrap();
        assert_eq!(
            fields.get(&ITEMS),
            Some(&Value::list([payload.clone(), payload]))
        );
        assert_eq!(fields.get(&RANGE), Some(&tree_range::encode(0..3)));
        assert_eq!(fields.get(&POSITION), Some(&f64::value(0.0)));
        assert_eq!(widgets.len(), 3, "playback and two grouping rows");
        drop(result);
        let placed = widget::frame::place(
            widgets[1](&mut context),
            Placement::root(Rect::new(0.0, 0.0, 200.0, 20.0)),
            &widget::HoverInput {
                pointer: Some(Point::new(30.0, 10.0)),
                ..Default::default()
            },
        );
        let Some((
            _,
            puri::hover::Claim::Direct(crate::frame::Hovered::Tree(crate::hover::Hover::Source(
                crate::hover::SourceTrace::InCell { cell, path, .. },
            ))),
        )) = &placed.claim
        else {
            panic!("{:?}", placed.claim)
        };
        assert_eq!(*cell, A);
        let target = path
            .iter()
            .try_fold(doc.cells.value(A).unwrap(), |value, step| match step {
                gid::Step::Key(key) => value.as_record()?.get(key),
                gid::Step::Element(position) => value.as_list()?.get(position),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            target
                .as_record()
                .unwrap()
                .get(&::grap::vocabulary::FUNCTION),
            Some(&t::LEAF.into())
        );
    });
}

#[test]
fn collect_tree_has_the_same_meaning_inside_controls_without_a_key() {
    use tree::vocabulary as t;
    let collect = ::grap::call(
        t::COLLECT.into(),
        [(
            t::PROGRAM,
            ::grap::lambda(
                [],
                ::grap::call(t::LEAF.into(), [(VALUE, Value::list([f64::value(7.0)]))]),
            ),
        )],
    );
    let libraries = crate::stack::load().libraries;
    let expected = ::grap::evaluate_value(&collect, &libraries, 1000).result;
    with_context(&Output::default(), |context| {
        let inputs = crate::projection::Cx {
            focused: true,
            sources: crate::sources::Sources {
                libraries: &libraries,
                ..context.inputs.sources
            },
            ..context.inputs.clone()
        };
        let context = widget::Context {
            inputs: &inputs,
            text: context.text,
            project: context.project,
            path: context.path,
            value: context.value,
        };
        let (widgets, result) =
            controls_output(&::grap::lambda([], collect), None, 200.0, &context).unwrap();
        assert!(widgets.is_empty());
        assert_eq!(result, expected);
        assert_eq!(result, Value::list([f64::value(7.0)]));
    });
}

#[test]
fn program_cursor_distinguishes_an_empty_group_from_an_empty_list_leaf() {
    use tree::vocabulary as t;
    let libraries = crate::stack::load().libraries;
    for (function, field, leaves, widget_count) in [
        (t::GROUP, layout::vocabulary::CHILDREN, 0, 0),
        (t::LEAF, VALUE, 1, 2),
    ] {
        let controls = ::grap::lambda(
            [],
            ::grap::call(
                TREE_PROGRAM_CURSOR.into(),
                [
                    (KEY, quote(A.into())),
                    (
                        t::PROGRAM,
                        ::grap::lambda(
                            [],
                            ::grap::call(function.into(), [(field, Value::list([]))]),
                        ),
                    ),
                ],
            ),
        );
        with_context(&Output::default(), |context| {
            let inputs = crate::projection::Cx {
                focused: true,
                sources: crate::sources::Sources {
                    libraries: &libraries,
                    ..context.inputs.sources
                },
                ..context.inputs.clone()
            };
            let context = widget::Context {
                inputs: &inputs,
                text: context.text,
                project: context.project,
                path: context.path,
                value: context.value,
            };
            let (widgets, result) = controls_output(&controls, None, 200.0, &context).unwrap();
            assert_eq!(widgets.len(), widget_count);
            let fields = result.as_record().unwrap();
            assert_eq!(fields.get(&ITEMS), Some(&Value::list([])));
            assert_eq!(fields.get(&RANGE), Some(&tree_range::encode(0..leaves)));
        });
    }
}

#[test]
fn program_cursor_returns_retained_runtime_leaves_with_and_without_memoization() {
    use tree::vocabulary as t;
    let libraries = crate::stack::load().libraries;
    let callback = ::grap::evaluate_at(
        &::grap::lambda([], f64::value(7.0)),
        Some(::grap::SourceOrigin::Stored(vec![gid::Step::Key(A)])),
        &libraries,
        1000,
    )
    .result;
    let maker = ::grap::evaluate_at(
        &::grap::lambda(
            [VALUE],
            ::grap::lambda(
                [],
                ::grap::call(
                    t::GROUP.into(),
                    [(
                        layout::vocabulary::CHILDREN,
                        Value::list([::grap::call(t::LEAF.into(), [(VALUE, VALUE.into())])]),
                    )],
                ),
            ),
        ),
        Some(::grap::SourceOrigin::Stored(vec![gid::Step::Key(B)])),
        &libraries,
        1000,
    )
    .result;
    let program = ::grap::apply(&maker, [(VALUE, callback.clone())], &libraries, 1000).result;
    let controls = ::grap::evaluate_runtime_at(
        &RuntimeValue::record([
            (::grap::vocabulary::PARAMS, RuntimeValue::list([])),
            (
                ::grap::vocabulary::BODY,
                RuntimeValue::record([
                    (
                        ::grap::vocabulary::FUNCTION,
                        Value::from(TREE_PROGRAM_CURSOR).into(),
                    ),
                    (KEY, quote(A.into()).into()),
                    (t::PROGRAM, program),
                ]),
            ),
        ]),
        None,
        &libraries,
        1000,
    )
    .result;
    with_context(&Output::default(), |context| {
        let sources = crate::sources::Sources {
            libraries: &libraries,
            ..context.inputs.sources
        };
        let computations = crate::computations::Computations::from_sources(sources);
        for memo in [None, Some(&computations)] {
            let inputs = crate::projection::Cx {
                sources,
                computations: memo,
                ..context.inputs.clone()
            };
            let context = widget::Context {
                inputs: &inputs,
                text: context.text,
                project: context.project,
                path: context.path,
                value: context.value,
            };
            for _ in 0..2 {
                let (widgets, result) =
                    super::controls_output(&controls, None, 200.0, &context).unwrap();
                assert_eq!(widgets.len(), 2);
                assert!(
                    result
                        .field(ITEMS)
                        .unwrap()
                        .list_get(0)
                        .unwrap()
                        .same_result(&callback)
                );
            }
        }
    });
}

fn slider(value: f64, max: f64) -> Value {
    ::grap::call(
        SLIDER.into(),
        [
            (VALUE, f64::value(value)),
            (ON_CHANGE, update_function()),
            (MAXIMUM, f64::value(max)),
        ],
    )
}

#[test]
fn controls_receive_arbitrary_state_and_an_update_callable() {
    let controls = ::grap::lambda([STATE], STATE.into());
    for state in [None, Some(f64::value(0.35)), Some(Value::list([A.into()]))] {
        let annotation = state.clone().map(|state| Value::record([(STATE, state)]));
        with_context(&Output::default(), |context| {
            let output = controls_output(&controls, annotation.as_ref(), 200.0, context);
            match state {
                Some(state) => {
                    let (widgets, result) = output.unwrap();
                    assert!(widgets.is_empty());
                    assert_eq!(result, state);
                }
                None => {
                    assert!(matches!(output, Err(error) if error == absent::with_reason(NO_STATE)))
                }
            }
            let (widgets, result) = controls_output(
                &::grap::lambda([UPDATE], UPDATE.into()),
                annotation.as_ref(),
                200.0,
                context,
            )
            .unwrap();
            assert!(widgets.is_empty());
            assert_eq!(result, update_function());
        });
    }
}

#[test]
fn slider_handler_can_store_a_float_and_preserves_other_annotations() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let root = editor.model.workspace.document_root().clone();
    let camera = crate::libraries::fidget::vocabulary::CAMERA;
    crate::editing::annotate(&mut editor, &root, &[], Value::record([(camera, A.into())]));
    let doc = editor.model.doc.clone();
    let control = slider_widget(
        Slider::new(0.0, 1.0, 0.2).unwrap(),
        180.0,
        update_function().into(),
    );
    let measured = with_context(&Output::default(), |context| control(context));
    let height = measured.extent.height();
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
        &Default::default(),
    );
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (100.0, height / 2.0).into(),
            ..Default::default()
        },
    };
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut editor, &event)
    );
    let annotation = editor.model.workspace.document.annotations.at(&[]).unwrap();
    assert_eq!(f64::read(&current_state(Some(annotation))), Some(0.5));
    assert_eq!(
        annotation.as_record().unwrap().get(&camera),
        Some(&A.into())
    );
    assert!(Rc::ptr_eq(&doc, &editor.model.doc));
}

#[test]
fn change_handlers_read_current_state_and_stage_writes_until_completion() {
    fn field_update(field: CellId, other: CellId) -> Value {
        ::grap::lambda(
            [STATE, VALUE, UPDATE],
            ::grap::call(
                UPDATE.into(),
                [(
                    VALUE,
                    quote(Value::record([
                        (field, splice(VALUE.into())),
                        (
                            other,
                            splice(::grap::call(
                                c::MATCH.into(),
                                [
                                    (c::VALUE, STATE.into()),
                                    (
                                        c::CASES,
                                        Value::list([
                                            Value::record([
                                                (
                                                    c::PATTERN,
                                                    Value::record([(
                                                        other,
                                                        Value::record([(c::BIND, other.into())]),
                                                    )]),
                                                ),
                                                (::grap::vocabulary::EXPRESSION, other.into()),
                                            ]),
                                            Value::record([
                                                (
                                                    c::PATTERN,
                                                    Value::record([(c::BIND, other.into())]),
                                                ),
                                                (::grap::vocabulary::EXPRESSION, f64::value(0.0)),
                                            ]),
                                        ]),
                                    ),
                                ],
                            )),
                        ),
                    ])),
                )],
            ),
        )
    }
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let root = editor.model.workspace.document_root().clone();
    let scope = crate::editing::Scope::default();
    let first = field_update(A, B);
    let second = field_update(B, A);
    for (handler, value) in [(&first, 0.2), (&second, 0.8), (&first, 0.4)] {
        apply_change(&mut editor, &scope, &root, &[], handler, f64::value(value));
    }
    assert_eq!(
        current_state(editor.model.workspace.document.annotations.at(&[])),
        Value::record([(A, f64::value(0.4)), (B, f64::value(0.8))])
    );
    let previous = current_state(editor.model.workspace.document.annotations.at(&[]));
    let rejected = ::grap::lambda(
        [VALUE, UPDATE],
        ::grap::call(
            c::DO.into(),
            [(
                c::EXPRESSIONS,
                Value::list([
                    ::grap::call(UPDATE.into(), [(VALUE, VALUE.into())]),
                    absent::decline(),
                ]),
            )],
        ),
    );
    apply_change(&mut editor, &scope, &root, &[], &rejected, f64::value(0.9));
    assert_eq!(
        current_state(editor.model.workspace.document.annotations.at(&[])),
        previous
    );
    let recovered = ::grap::lambda(
        [VALUE, UPDATE],
        ::grap::call(
            c::DO.into(),
            [(
                c::EXPRESSIONS,
                Value::list([
                    ::grap::call(UPDATE.into(), [(VALUE, VALUE.into())]),
                    absent::with_reason(INVALID_INPUT),
                ]),
            )],
        ),
    );
    apply_change(&mut editor, &scope, &root, &[], &recovered, f64::value(0.6));
    assert_eq!(
        current_state(editor.model.workspace.document.annotations.at(&[])),
        f64::value(0.6)
    );
}

#[test]
fn controls_declaration_reuses_tracked_inputs_but_not_effectful_or_untracked_arguments() {
    use crate::{computations::Computations, libraries::Definitions};
    use std::cell::Cell;

    // Constructing the declaration only evaluates its arguments. Running the
    // widgets is a separate interpretation, and must still happen each frame.
    for mode in ["pure", "effectful", "untracked"] {
        let (function, input, unrelated, library) = (
            gid::new_cell_id(),
            gid::new_cell_id(),
            gid::new_cell_id(),
            gid::new_cell_id(),
        );
        let runs = Rc::new(Cell::new(0));
        let implementation = ForeignFunction::new({
            let runs = runs.clone();
            move |context, call, environment| {
                runs.set(runs.get() + 1);
                let argument = context.field(call, VALUE).unwrap();
                let value = context.eval(argument, environment)?;
                if mode == "effectful" {
                    context.effect(|| ());
                }
                Ok(value)
            }
        });
        let implementation = if mode == "untracked" {
            implementation
        } else {
            implementation.tracked()
        };
        let mut definitions = Definitions::default();
        definitions.insert(
            function,
            ::grap::Definition::foreign(Value::record([]), implementation),
        );
        let mut libraries = crate::stack::load().libraries;
        libraries.insert(library, definitions);
        let mut doc = gid::Document {
            root: None,
            cells: Cells::new(),
        };
        doc.cells.set_value(input, f64::value(1.0));
        let expression = ::grap::call(
            WITH_CONTROLS.into(),
            [
                (CONTROLS, ::grap::lambda([], Value::record([]))),
                (VIEW, ::grap::lambda([VALUE], VALUE.into())),
                (
                    VALUE,
                    ::grap::call(function.into(), [(VALUE, input.into())]),
                ),
                (WIDTH, f64::value(200.0)),
                (HEIGHT, f64::value(300.0)),
            ],
        );
        let computations = Computations::default();
        let root = Root::document();
        let evaluate = |doc: &gid::Document| {
            computations.begin(Rc::new(doc.clone()), libraries.clone());
            let result = computations.evaluate(&root, &[], &expression, 1000);
            result
                .as_record()
                .unwrap()
                .get(&WITH_CONTROLS)
                .unwrap()
                .as_record()
                .unwrap()
                .get(&VALUE)
                .unwrap()
                .clone()
        };
        assert_eq!(evaluate(&doc), f64::value(1.0));
        assert_eq!(evaluate(&doc), f64::value(1.0));
        doc.cells.set_value(unrelated, Value::record([]));
        assert_eq!(evaluate(&doc), f64::value(1.0));
        assert_eq!(runs.get(), if mode == "pure" { 1 } else { 3 }, "{mode}");
        doc.cells.set_value(input, f64::value(2.0));
        assert_eq!(evaluate(&doc), f64::value(2.0));
        assert_eq!(runs.get(), if mode == "pure" { 2 } else { 4 }, "{mode}");
    }
}

#[test]
fn controls_overlay_the_full_height_view_and_supply_their_values() {
    let host = Host(crate::stack::load().libraries);
    let controls = ::grap::lambda(
        [],
        quote(Value::list([
            splice(slider(0.25, 1.0)),
            splice(slider(5.0, 10.0)),
            splice(::grap::call(
                RADIO.into(),
                [
                    (KEY, quote(CellId::from_u128(3).into())),
                    (
                        OPTIONS,
                        quote(Value::list([
                            name::record("First", [(VALUE, f64::value(10.0))]),
                            name::record("Second", [(VALUE, f64::value(20.0))]),
                        ])),
                    ),
                ],
            )),
        ])),
    );
    let view = ::grap::lambda(
        [VALUE, WIDTH, HEIGHT, PARAMETERS],
        quote(Value::record([
            (HEIGHT, splice(HEIGHT.into())),
            (PARAMETERS, splice(PARAMETERS.into())),
        ])),
    );
    let call = ::grap::call(
        WITH_CONTROLS.into(),
        [
            (CONTROLS, controls),
            (VIEW, view),
            (VALUE, Value::record([])),
            (WIDTH, f64::value(200.0)),
            (HEIGHT, f64::value(300.0)),
        ],
    );
    let declaration = ::grap::evaluate_value(&call, &host.0, 10_000).result;
    let target = |_| unreachable!();
    let input = ProjectionInput {
        env: &host,
        value: Some(&declaration),
        default_projection: display::partial(|_| None),
        scale_factor: 1.0,
        writable: false,
        selection: None,
        pending: None,
        state: None,
        targets: display::ProjectionTargets::new(&target),
    };
    let output = Output::default();
    let mut build = ChoiceBuild::default();
    // The test's ordinary projection interpreter records the viewport arguments.
    let graph = with_context(&output, |context| {
        let libraries = &host.0;
        let inputs = crate::projection::Cx {
            focused: true,
            edits: Default::default(),
            sources: crate::sources::Sources {
                doc: context.inputs.sources.doc,
                libraries,
            },
            ..context.inputs.clone()
        };
        let mut context = widget::Context {
            inputs: &inputs,
            text: context.text,
            project: context.project,
            path: context.path,
            value: context.value,
        };
        display(&input.with_value(input.value.map(::grap::RuntimeValue::from).as_ref()))
            .unwrap()
            .measure(&mut context, &mut build)
    });
    let measured = resolve_choices(build.finish(graph), 200.0, false);
    assert_eq!(measured.extent.height(), 300.0);
    let result = output.0.borrow();
    let result = result.as_ref().unwrap().as_record().unwrap();
    assert_eq!(f64::read(result.get(&HEIGHT).unwrap()), Some(300.0));
    let values: Vec<_> = result
        .get(&PARAMETERS)
        .unwrap()
        .as_list()
        .unwrap()
        .values()
        .map(|v| f64::read(v).unwrap())
        .collect();
    assert_eq!(values, [0.25, 5.0, 10.0]);

    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, 300.0)),
        &Default::default(),
    );
    let frame = placed.bind(Default::default());
    let mut drawing = DrawList::new();
    puri::frame::render(frame.renders, &mut drawing);
    assert!(matches!(
        drawing.0.first(),
        Some(DrawCmd::Fill { shape: Shape::Rect(rect), .. })
            if *rect == Rect::new(0.0, 0.0, 200.0, 300.0)
    ));
    assert!(
        drawing.0[1..].iter().all(|cmd| !matches!(
            cmd,
            DrawCmd::Fill {
                shape: Shape::Rect(_) | Shape::RoundedRect(_),
                ..
            }
        )),
        "controls paint above the full view without a background strip hiding it"
    );
    let dispatch = frame.handler.unwrap();
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let mut event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (5.0, 1.0).into(),
            ..Default::default()
        },
    };
    assert!(
        !dispatch.dispatch_pointer_down(&mut editor, &event),
        "the controls leave the top of the view interactive"
    );
    event.state.position = (5.0, 299.0).into();
    assert!(
        dispatch.dispatch_pointer_down(&mut editor, &event),
        "the overlay consumes presses in its bottom padding too"
    );
    assert!(
        editor
            .model
            .workspace
            .document
            .annotations
            .at(&[])
            .is_none(),
        "padding blocks orbit without activating a control"
    );
    event.state.position = (201.0, 299.0).into();
    assert!(!dispatch.dispatch_pointer_down(&mut editor, &event));
}

#[test]
fn radio_options_require_distinct_values_and_preserve_callable_data() {
    let callable = Value::record([(::grap::vocabulary::FFI, SLIDER.into())]);
    let option = name::record("A callable", [(VALUE, callable.clone())]);
    let options = radio_options(&Value::list([option.clone()])).unwrap();
    assert_eq!(options[0].value, callable);
    assert!(radio_options(&Value::list([option.clone(), option])).is_none());
    assert!(radio_options(&Value::list([])).is_none());
    assert!(radio_options(&Value::list([name::record("No value", [])])).is_none());
}

#[test]
fn radio_dispatch_updates_value_without_touching_playback_camera_or_document() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let original = editor.model.doc.clone();
    let root = editor.model.workspace.document_root().clone();
    let camera = crate::libraries::fidget::vocabulary::CAMERA;
    let camera_value = Value::record([(A, f64::value(0.3))]);
    crate::editing::annotate(
        &mut editor,
        &root,
        &[],
        Value::record([
            (camera, camera_value.clone()),
            (STATE, Value::record([(A, f64::value(0.35))])),
        ]),
    );
    let first = Value::record([(::grap::vocabulary::FFI, SLIDER.into())]);
    let second = Value::record([(::grap::vocabulary::FFI, RADIO.into())]);
    let control = radio_widget(
        B,
        vec![
            RadioOption {
                label: "First".into(),
                value: first.clone(),
            },
            RadioOption {
                label: "Second".into(),
                value: second.clone(),
            },
        ],
        first,
        400.0,
    );
    let measured = with_context(&Output::default(), |context| control(context));
    let extent = measured.extent;
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, extent.width, extent.height())),
        &Default::default(),
    );
    let dispatch = placed.resolve_for_dispatch();
    let mut event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (extent.width + 10.0, extent.height() / 2.0).into(),
            ..Default::default()
        },
    };
    assert!(!dispatch.dispatch_pointer_down(&mut editor, &event));
    event.state.position = (extent.width - PADDING_X - 1.0, extent.height() / 2.0).into();
    assert!(dispatch.dispatch_pointer_down(&mut editor, &event));
    let state = editor.model.workspace.document.annotations.at(&[]);
    assert_eq!(read_state(state, B), Some(&second));
    assert_eq!(read_state(state, A).and_then(f64::read), Some(0.35));
    assert_eq!(
        state.unwrap().as_record().unwrap().get(&camera),
        Some(&camera_value)
    );
    assert!(Rc::ptr_eq(&original, &editor.model.doc));
}

#[test]
fn slider_dispatch_updates_only_its_own_view_state_and_keeps_camera_fields() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let original = editor.model.doc.clone();
    let root = editor.model.workspace.document_root().clone();
    let camera = crate::libraries::fidget::vocabulary::CAMERA;
    crate::editing::annotate(
        &mut editor,
        &root,
        &[],
        Value::record([(camera, Value::record([]))]),
    );
    let control = slider_widget_with(
        A,
        Slider::new(0.0, 1.0, 0.2).unwrap(),
        180.0,
        Vec::new(),
        Rc::new(f64::value),
    );
    let measured = with_context(&Output::default(), |context| control(context));
    let height = measured.extent.height();
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
        &Default::default(),
    );
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (100.0, height - 18.0).into(),
            ..Default::default()
        },
    };
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut editor, &event)
    );
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), A).and_then(f64::read),
        Some(0.5)
    );
    editor.advance_gesture(&[Point::new(-100.0, -100.0), Point::new(1000.0, -100.0)]);
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), A).and_then(f64::read),
        Some(1.0)
    );
    assert!(
        editor
            .model
            .workspace
            .document
            .annotations
            .at(&[])
            .unwrap()
            .as_record()
            .unwrap()
            .contains_key(&camera)
    );
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), B),
        None
    );
    assert!(Rc::ptr_eq(&editor.model.doc, &original));
    editor.finish_gesture();
}

#[test]
fn range_stack_uses_row_hit_height_without_extra_vertical_padding() {
    let tree = Value::list([
        Value::list([A.into(), A.into()]),
        Value::list([A.into(), A.into()]),
    ]);
    let selection = tree_range::Selection::new(&tree, None);
    let measured = with_context(&Output::default(), |context| {
        measured::col(
            0,
            0.0,
            selection
                .widgets(A, 180.0, None)
                .map(|row| row(context))
                .collect(),
        )
    });
    assert_eq!(measured.extent.height(), 40.0);
    let frame = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, 40.0)),
        &Default::default(),
    )
    .bind(Default::default());
    let mut drawing = DrawList::new();
    puri::frame::render(frame.renders, &mut drawing);
    let handles: Vec<_> = drawing
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Fill {
                shape: Shape::Rect(rect),
                ..
            } if rect.width() == 2.0 => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(handles.len(), 4);
    assert_eq!(handles[0].height(), 16.0);
    assert_eq!(handles[2].y0 - handles[0].y1, 4.0);
}

#[test]
fn stored_tree_sources_are_captured_by_widgets_not_inserted_into_items() {
    use crate::hover::{Hover, SourceTrace};
    use gid::Step;
    use puri::handler::{Event, Modifiers};
    use puri::hover::Claim;

    let (owner, library, probe) = (gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id());
    let items = Value::list([A.into(), A.into()]);
    let positions: Vec<_> = items.as_list().unwrap().keys().cloned().collect();
    let mut definitions = crate::libraries::Definitions::default();
    definitions.insert(
        owner,
        ::grap::Definition::Value(::grap::lambda(
            [],
            ::grap::call(probe.into(), [(ITEMS, items.clone())]),
        )),
    );
    let mut libraries = crate::stack::load().libraries;
    libraries.insert(library, definitions);
    let captured = RefCell::new(None);
    let emit = |_, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
        let expression = context.field(call, ITEMS).unwrap();
        captured.replace(stored_tree_items(context, expression.clone()));
        context.eval_to_value(expression, environment)
    };
    let result = ::grap::apply_value_scoped(
        &owner.into(),
        [],
        &libraries,
        &ForeignOverlay::from_value(&[probe], &emit),
        1000,
    );
    assert!(result.completed);
    assert_eq!(result.result, items);
    let decorate = captured.into_inner().expect("a stored list has a source");
    let sources: Vec<_> = positions
        .iter()
        .map(|position| SourceTrace::InCell {
            cell: owner,
            source: gid::Resolution::Library(library),
            path: Rc::from([
                Step::Key(::grap::vocabulary::BODY),
                Step::Key(ITEMS),
                Step::Element(position.clone()),
            ]),
        })
        .collect();
    let selection = tree_range::Selection::new(&result.result, None);
    let point = Point::new(50.0, 10.0);

    let build = |selected: Option<SourceTrace>, hovered: Option<SourceTrace>| {
        let measured = with_context(&Output::default(), |context| {
            let inputs = crate::projection::Cx {
                focused: true,
                selected_trace: selected,
                ..context.inputs.clone()
            };
            let mut context = widget::Context {
                inputs: &inputs,
                text: context.text,
                project: context.project,
                path: context.path,
                value: context.value,
            };
            selection
                .widgets(A, 180.0, Some(decorate.clone()))
                .next()
                .unwrap()(&mut context)
        });
        let placed = widget::frame::place(
            measured,
            Placement::root(Rect::new(0.0, 0.0, 200.0, 20.0)),
            &widget::HoverInput {
                pointer: Some(point),
                ..Default::default()
            },
        );
        assert_eq!(
            placed.claim.as_ref().map(|(_, c)| c),
            Some(&Claim::Direct(crate::frame::Hovered::Tree(Hover::Source(
                sources[0].clone()
            ))))
        );
        placed.bind(widget::ResolvedHover {
            hovered_trace: hovered,
            ..Default::default()
        })
    };
    let draws = |selected, hovered| {
        let frame = build(selected, hovered);
        let mut drawing = DrawList::new();
        puri::frame::render(frame.renders, &mut drawing);
        let styles = widget::style::editor(widget::style::Theme::Light.palette(), 1.0);
        drawing
            .0
            .iter()
            .filter(|cmd| {
                matches!(cmd, DrawCmd::Fill { brush, .. }
                if *brush == styles.selection_wash || *brush == styles.accent_wash.brush)
            })
            .count()
    };
    assert_eq!(draws(None, None), 0);
    assert_eq!(draws(Some(sources[0].clone()), None), 1);
    assert_eq!(draws(None, Some(sources[1].clone())), 1);
    assert_eq!(draws(Some(sources[0].clone()), Some(sources[0].clone())), 1);

    let frame = build(None, None);
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let root = editor.model.workspace.document_root().clone();
    let mut input = crate::placed::DispatchContext::new(
        None,
        Some(crate::frame::Hovered::Tree(Hover::Source(
            sources[0].clone(),
        ))),
    );
    // No visible source: Cmd-click still belongs to the source link and must not move the range.
    let mut event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (point.x, point.y).into(),
            modifiers: Modifiers::META | Modifiers::CONTROL,
            ..Default::default()
        },
    };
    let handler = frame.handler.unwrap();
    assert!(handler.dispatch_pointer_down_with(&mut editor, &event, &mut input));
    assert!(
        read_state(
            editor
                .model
                .workspace
                .view(&root)
                .unwrap()
                .annotations
                .at(&[]),
            A
        )
        .is_none()
    );
    assert!(editor.model.selection.is_none());
    editor.pointer = Some(point);
    assert!(
        !handler
            .dispatch(&mut editor, Event::HoverChanged, &mut input)
            .handled()
    );
    event.state.modifiers = Modifiers::default();
    assert!(handler.dispatch_pointer_down_with(&mut editor, &event, &mut input));
    assert!(
        read_state(
            editor
                .model
                .workspace
                .view(&root)
                .unwrap()
                .annotations
                .at(&[]),
            A
        )
        .is_some()
    );
    editor.finish_gesture();
}

#[test]
fn generated_tree_items_are_not_misattributed_to_the_argument_expression() {
    let (owner, library, probe) = (gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id());
    let items = Value::list([A.into(), B.into()]);
    let mut definitions = crate::libraries::Definitions::default();
    definitions.insert(
        owner,
        ::grap::Definition::Value(::grap::lambda(
            [],
            ::grap::call(probe.into(), [(ITEMS, quote(items.clone()))]),
        )),
    );
    let mut libraries = crate::stack::load().libraries;
    libraries.insert(library, definitions);
    let emit = |_, context: &mut Context<'_>, call: &Expression, environment: &Environment| {
        let expression = context.field(call, ITEMS).unwrap();
        assert!(stored_tree_items(context, expression.clone()).is_none());
        context.eval_to_value(expression, environment)
    };
    assert_eq!(
        ::grap::apply_value_scoped(
            &owner.into(),
            [],
            &libraries,
            &ForeignOverlay::from_value(&[probe], &emit),
            1000
        )
        .result,
        items
    );
}

#[test]
fn tree_cursor_stacks_disjoint_rows_without_extra_frame_padding() {
    let tree = Value::list([
        Value::list([A.into(), A.into()]),
        Value::list([A.into(), A.into()]),
    ]);
    let (rows, _) = tree_range::cursor(&tree, None, 0.25, A, 180.0, None);
    let measured = with_context(&Output::default(), |context| {
        measured::col(0, 0.0, rows.iter().map(|row| row(context)).collect())
    });
    assert_eq!(measured.extent.width, 200.0);
    assert_eq!(measured.extent.height(), 76.0);
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, 76.0)),
        &Default::default(),
    );
    let frame = placed.bind(Default::default());
    let mut drawing = DrawList::new();
    puri::frame::render(frame.renders, &mut drawing);
    assert!(drawing.0.iter().all(|cmd| !matches!(
        cmd,
        DrawCmd::Fill {
            shape: Shape::RoundedRect(_),
            ..
        }
    )));
    let markers: Vec<_> = drawing
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Fill {
                shape: Shape::Rect(rect),
                ..
            } if rect.height() == 2.0 => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(
        markers,
        [
            Rect::new(15.0, 52.0, 57.5, 54.0),
            Rect::new(15.0, 72.0, 100.0, 74.0)
        ]
    );

    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (100.0, 66.0).into(),
            ..Default::default()
        },
    };
    assert!(
        frame
            .handler
            .unwrap()
            .dispatch_pointer_down(&mut editor, &event)
    );
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    let selection = tree_range::Selection::new(&tree, state.as_record().unwrap().get(&RANGE));
    assert_eq!(selection.leaves, 2..4);
    assert_eq!(selection.intent[0], puri_widgets::tree_slider::Intent::All);
    editor.advance_gesture(&[Point::new(20.0, 15.0)]);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    let selection = tree_range::Selection::new(&tree, state.as_record().unwrap().get(&RANGE));
    assert_eq!(selection.leaves, 0..4);
    assert_eq!(
        selection.intent[0],
        puri_widgets::tree_slider::Intent::All,
        "crossing a row keeps dragging the original range, not playback or a finer range"
    );
}

#[test]
fn tree_range_dispatch_resets_finer_ranges_and_preserves_other_view_state() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let original = editor.model.doc.clone();
    let root = editor.model.workspace.document_root().clone();
    let camera = crate::libraries::fidget::vocabulary::CAMERA;
    let tree = Value::list([
        Value::list([A.into(), A.into()]),
        Value::list([A.into(), A.into()]),
    ]);
    let original_selection = tree_range::Selection::new(&tree, None).select(0, 0..1);
    let old = original_selection.state();
    let annotation = Value::record([
        (camera, Value::record([])),
        (
            STATE,
            Value::record([(A, old.clone()), (B, f64::value(0.35))]),
        ),
    ]);
    crate::editing::annotate(&mut editor, &root, &[], annotation);
    let selection = tree_range::Selection::new(&tree, Some(&old));
    let control = selection.widgets(A, 180.0, None).last().unwrap();
    let measured = with_context(&Output::default(), |context| control(context));
    let height = measured.extent.height();
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
        &Default::default(),
    );
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (150.0, height / 2.0).into(),
            ..Default::default()
        },
    };
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut editor, &event)
    );
    let state = editor.model.workspace.document.annotations.at(&[]);
    let chosen = read_state(state, A).unwrap();
    assert_eq!(chosen, &original_selection.select(1, 1..2).state());
    assert_eq!(tree_range::Selection::new(&tree, Some(chosen)).leaves, 2..4);
    assert_eq!(
        tree_range::Selection::new(&tree, Some(chosen)).intent[0],
        puri_widgets::tree_slider::Intent::All
    );
    assert_eq!(read_state(state, B).and_then(f64::read), Some(0.35));
    assert!(state.unwrap().as_record().unwrap().contains_key(&camera));
    editor.advance_gesture(&[Point::new(150.0, 0.0), Point::new(-100.0, -100.0)]);
    let state = editor.model.workspace.document.annotations.at(&[]);
    assert_eq!(
        read_state(state, A),
        Some(&original_selection.select(1, 0..2).state())
    );
    assert_eq!(
        tree_range::Selection::new(&tree, read_state(state, A)).leaves,
        0..4
    );
    assert!(Rc::ptr_eq(&original, &editor.model.doc));
    editor.finish_gesture();
}

#[test]
fn notched_slider_double_click_selects_its_full_range_without_starting_a_drag() {
    for cursor in [false, true] {
        for finest in [false, true] {
            let mut editor = crate::test_editor(gid::Document {
                root: None,
                cells: Cells::new(),
            });
            let original = editor.model.doc.clone();
            let root = editor.model.workspace.document_root().clone();
            let tree = Value::list([
                Value::list([B.into(), B.into()]),
                Value::list([B.into(), B.into()]),
            ]);
            let selection = tree_range::Selection::new(&tree, None)
                .select(1, 0..1)
                .select(0, 1..2);
            let ranges = selection.state();
            let old = if cursor {
                tree_range::cursor_state(&selection, 1.5)
            } else {
                ranges.clone()
            };
            let camera = crate::libraries::fidget::vocabulary::CAMERA;
            crate::editing::annotate(
                &mut editor,
                &root,
                &[],
                Value::record([
                    (camera, Value::record([])),
                    (
                        STATE,
                        Value::record([(A, old.clone()), (B, f64::value(0.35))]),
                    ),
                ]),
            );
            let selection = tree_range::Selection::new(&tree, Some(&ranges));
            let widgets: Vec<_> = if cursor {
                tree_range::cursor(&tree, Some(&old), 0.0, A, 180.0, None).0
            } else {
                selection.widgets(A, 180.0, None).collect()
            };
            let control = if finest {
                &widgets[usize::from(cursor)]
            } else {
                widgets.last().unwrap()
            };
            let measured = with_context(&Output::default(), |context| control(context));
            let height = measured.extent.height();
            let dispatch = widget::frame::place(
                measured,
                Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
                &Default::default(),
            )
            .resolve_for_dispatch();
            let mut event = puri::handler::PointerButtonEvent {
                button: Some(puri::handler::PointerButton::Primary),
                pointer: puri::handler::PointerInfo {
                    pointer_id: None,
                    persistent_device_id: None,
                    pointer_type: puri::handler::PointerType::Mouse,
                },
                state: puri::handler::PointerState {
                    position: (201.0, height / 2.0).into(),
                    count: 2,
                    ..Default::default()
                },
            };
            assert!(!dispatch.dispatch_pointer_down(&mut editor, &event));
            event.state.position = (100.0, height / 2.0).into();
            event.button = Some(puri::handler::PointerButton::Secondary);
            assert!(!dispatch.dispatch_pointer_down(&mut editor, &event));
            event.button = Some(puri::handler::PointerButton::Primary);
            assert!(dispatch.dispatch_pointer_down(&mut editor, &event));
            assert!(!editor.advance_gesture(&[Point::new(-100.0, -100.0)]));
            let expected = selection.select_all(if finest { 0 } else { 1 });
            let expected = if cursor {
                tree_range::cursor_state(&expected, 1.5)
            } else {
                expected.state()
            };
            let state = editor.model.workspace.document.annotations.at(&[]);
            assert_eq!(read_state(state, A), Some(&expected));
            assert_eq!(read_state(state, B).and_then(f64::read), Some(0.35));
            assert!(state.unwrap().as_record().unwrap().contains_key(&camera));
            assert!(Rc::ptr_eq(&original, &editor.model.doc));
        }
    }
}

#[test]
fn tree_cursor_current_item_click_preserves_finer_filters_until_dragging_out() {
    let tree = Value::list([
        Value::list([B.into(), B.into()]),
        Value::list([B.into(), B.into()]),
    ]);
    for position in [1.5, 2.0] {
        for (x, count, range) in [
            (50.0, 1, 1..2),
            (150.0, 1, 2..4),
            (15.0, 1, 0..2),
            (50.0, 2, 0..4),
        ] {
            let mut editor = crate::test_editor(gid::Document {
                root: None,
                cells: Cells::new(),
            });
            let root = editor.model.workspace.document_root().clone();
            let selection = tree_range::Selection::new(&tree, None)
                .select(1, 0..1)
                .select(0, 1..2);
            let old = tree_range::cursor_state(&selection, position);
            let annotation = set_state(None, A, old.clone());
            crate::editing::annotate(&mut editor, &root, &[], annotation.clone());
            let (widgets, before) = tree_range::cursor(&tree, Some(&old), 0.0, A, 180.0, None);
            let control = widgets.last().unwrap();
            let measured = with_context(&Output::default(), |context| control(context));
            let height = measured.extent.height();
            let dispatch = widget::frame::place(
                measured,
                Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
                &Default::default(),
            )
            .resolve_for_dispatch();
            let event = puri::handler::PointerButtonEvent {
                button: Some(puri::handler::PointerButton::Primary),
                pointer: puri::handler::PointerInfo {
                    pointer_id: None,
                    persistent_device_id: None,
                    pointer_type: puri::handler::PointerType::Mouse,
                },
                state: puri::handler::PointerState {
                    position: (x, height / 2.0).into(),
                    count,
                    ..Default::default()
                },
            };
            assert!(dispatch.dispatch_pointer_down(&mut editor, &event));
            let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
            let (_, result) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None);
            assert_eq!(
                result.as_record().unwrap().get(&RANGE),
                Some(&tree_range::encode(range))
            );
            if x == 50.0 && count == 1 {
                assert_eq!(
                    editor.model.workspace.document.annotations.at(&[]),
                    Some(&annotation)
                );
                assert_eq!(
                    result, before,
                    "a current-item click leaves preview inputs unchanged"
                );
                editor.advance_gesture(&[Point::new(55.0, height / 2.0)]);
                assert_eq!(
                    editor.model.workspace.document.annotations.at(&[]),
                    Some(&annotation)
                );
                editor.advance_gesture(&[Point::new(150.0, height / 2.0)]);
                let state =
                    read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
                assert_eq!(
                    tree_range::Selection::new(&tree, state.as_record().unwrap().get(&RANGE))
                        .leaves,
                    0..4
                );
                editor.advance_gesture(&[Point::new(50.0, height / 2.0)]);
                let state =
                    read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
                assert_eq!(
                    tree_range::Selection::new(&tree, state.as_record().unwrap().get(&RANGE))
                        .leaves,
                    0..2
                );
            }
            editor.finish_gesture();
        }
    }
}

#[test]
fn tree_cursor_updates_range_and_position_together() {
    fn click(editor: &mut crate::Editor, control: &Widget, x: f64, count: u8) {
        let measured = with_context(&Output::default(), |context| control(context));
        let height = measured.extent.height();
        let placed = widget::frame::place(
            measured,
            Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
            &Default::default(),
        );
        let event = puri::handler::PointerButtonEvent {
            button: Some(puri::handler::PointerButton::Primary),
            pointer: puri::handler::PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: puri::handler::PointerType::Mouse,
            },
            state: puri::handler::PointerState {
                position: (x, height / 2.0).into(),
                count,
                ..Default::default()
            },
        };
        assert!(
            placed
                .resolve_for_dispatch()
                .dispatch_pointer_down(editor, &event)
        );
        editor.finish_gesture();
    }
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let original = editor.model.doc.clone();
    let tree = Value::list([
        Value::list([B.into(), B.into()]),
        Value::list([B.into(), B.into()]),
    ]);
    let (widgets, _) = tree_range::cursor(&tree, None, 2.75, A, 180.0, None);
    click(&mut editor, widgets.last().unwrap(), 150.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(2.75));
    let (widgets, result) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None);
    assert_eq!(
        result.as_record().unwrap().get(&RANGE),
        Some(&tree_range::encode(2..4))
    );
    click(&mut editor, &widgets[0], 100.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(3.0));
    let (widgets, _) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None);
    click(&mut editor, widgets.last().unwrap(), 50.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(0.0));
    assert_eq!(
        tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None)
            .1
            .as_record()
            .unwrap()
            .get(&RANGE),
        Some(&tree_range::encode(0..2))
    );
    let (widgets, _) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None);
    click(&mut editor, widgets.last().unwrap(), 50.0, 2);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(
        state.as_record().unwrap().get(&RANGE),
        Some(&Value::list([ALL.into(), ALL.into()]))
    );
    let (_, result) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0, None);
    assert_eq!(
        result.as_record().unwrap().get(&RANGE),
        Some(&tree_range::encode(0..4))
    );
    assert!(Rc::ptr_eq(&editor.model.doc, &original));
}

fn tree_cursor_position(tree: &Value, state: &Value) -> Option<f64> {
    tree_range::cursor(tree, Some(state), 0.0, A, 180.0, None)
        .1
        .as_record()?
        .get(&POSITION)
        .and_then(f64::read)
}
