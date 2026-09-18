use super::*;
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
    ) -> ::grap::Evaluation {
        match scope {
            Some(scope) => {
                ::grap::apply_scoped(function, args.iter().cloned(), &self.0, scope, 10_000)
            }
            None => ::grap::apply(function, args.iter().cloned(), &self.0, 10_000),
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
        value: Value,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        let height = f64::read(value.as_record().unwrap().get(&HEIGHT).unwrap()).unwrap();
        self.0.replace(Some(value));
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
fn slider(key: CellId, initial: f64, max: f64) -> Value {
    ::grap::call(
        SLIDER.into(),
        [
            (KEY, quote(key.into())),
            (INITIAL, f64::value(initial)),
            (MAXIMUM, f64::value(max)),
        ],
    )
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
        let implementation = ForeignFunction::runtime({
            let runs = runs.clone();
            move |context, call, environment| {
                runs.set(runs.get() + 1);
                let argument = context.field(call, VALUE).unwrap();
                let value = context.eval_runtime(argument, environment)?;
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
            splice(slider(A, 0.25, 1.0)),
            splice(slider(B, 5.0, 10.0)),
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
    let declaration = ::grap::evaluate(&call, &host.0, 10_000).result;
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
        display(&input).unwrap().measure(&mut context, &mut build)
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
    let control = slider_widget(A, Slider::new(0.0, 1.0, 0.2).unwrap(), 180.0);
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
                .widgets(A, 180.0)
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
fn tree_cursor_stacks_disjoint_rows_without_extra_frame_padding() {
    let tree = Value::list([
        Value::list([A.into(), A.into()]),
        Value::list([A.into(), A.into()]),
    ]);
    let (rows, _) = tree_range::cursor(&tree, None, 0.25, A, 180.0);
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
    let control = selection.widgets(A, 180.0).last().unwrap();
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
                tree_range::cursor(&tree, Some(&old), 0.0, A, 180.0).0
            } else {
                selection.widgets(A, 180.0).collect()
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
    let (widgets, _) = tree_range::cursor(&tree, None, 2.75, A, 180.0);
    click(&mut editor, widgets.last().unwrap(), 150.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(2.75));
    let (widgets, result) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0);
    assert_eq!(
        result.as_record().unwrap().get(&RANGE),
        Some(&tree_range::encode(2..4))
    );
    click(&mut editor, &widgets[0], 100.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(3.0));
    let (widgets, _) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0);
    click(&mut editor, widgets.last().unwrap(), 50.0, 1);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(tree_cursor_position(&tree, state), Some(0.0));
    assert_eq!(
        tree_range::cursor(&tree, Some(state), 0.0, A, 180.0)
            .1
            .as_record()
            .unwrap()
            .get(&RANGE),
        Some(&tree_range::encode(0..2))
    );
    let (widgets, _) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0);
    click(&mut editor, widgets.last().unwrap(), 50.0, 2);
    let state = read_state(editor.model.workspace.document.annotations.at(&[]), A).unwrap();
    assert_eq!(
        state.as_record().unwrap().get(&RANGE),
        Some(&Value::list([ALL.into(), ALL.into()]))
    );
    let (_, result) = tree_range::cursor(&tree, Some(state), 0.0, A, 180.0);
    assert_eq!(
        result.as_record().unwrap().get(&RANGE),
        Some(&tree_range::encode(0..4))
    );
    assert!(Rc::ptr_eq(&editor.model.doc, &original));
}

fn tree_cursor_position(tree: &Value, state: &Value) -> Option<f64> {
    tree_range::cursor(tree, Some(state), 0.0, A, 180.0)
        .1
        .as_record()?
        .get(&POSITION)
        .and_then(f64::read)
}
