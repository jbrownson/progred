use super::*;
use crate::display::test_support::{mouse, primary_press};
use crate::svg::write_cmds;
use puri::handler::test_support::DispatchShorthands;

#[test]
fn grap_template_preview_evaluates_the_shared_cells_current_call() {
    let mut context = BenchContext::new();
    let mut doc = Document {
        root: Some(
            root_completions(&context.stack)
                .into_iter()
                .find(|offer| offer.display == crate::libraries::grap::vocabulary::GRAP.into())
                .unwrap()
                .test_value(),
        ),
        cells: Cells::new(),
    };
    let cell = crate::test_support::value_at(
        doc.root.as_ref().unwrap(),
        &[
            crate::test_examples::main_part(),
            vec![gid::Step::Key(crate::libraries::grap::vocabulary::GRAP)],
        ]
        .concat(),
    )
    .and_then(gid::Value::as_cell)
    .unwrap();
    let pane = crate::workspace::panes(doc.root.as_ref()).remove(0);
    let displayed = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observe = displayed.clone();
    context.stack.projection = Projection {
        partial: crate::display::compose_partials([
            crate::display::partial(move |input| {
                observe.borrow_mut().push(input.value?.clone());
                None
            }),
            context.stack.pane_projection.partial.clone(),
        ]),
        ..context.stack.pane_projection.clone()
    };
    let arguments = [
        (
            crate::libraries::grap::numbers::vocabulary::LEFT,
            crate::libraries::f32::value(7.0),
        ),
        (
            crate::libraries::grap::numbers::vocabulary::RIGHT,
            crate::libraries::f32::value(2.0),
        ),
    ];
    let shape = ::grap::isa::record(
        fidget::vocabulary::kind::DIFFERENCE,
        [
            (fidget::vocabulary::LEFT, crate::libraries::f32::value(7.0)),
            (fidget::vocabulary::RIGHT, crate::libraries::f32::value(2.0)),
        ],
    );
    // A Grap call, and Grap quoting a Fidget field.
    for (expression, result) in [
        (
            grap::call(
                crate::libraries::grap::numbers::f32::SUBTRACT.into(),
                arguments.clone(),
            ),
            crate::libraries::f32::value(5.0),
        ),
        (
            grap::call(
                crate::libraries::grap::control::vocabulary::QUOTE.into(),
                [(grap::vocabulary::EXPRESSION, shape.clone())],
            ),
            shape,
        ),
    ] {
        doc.cells.set_value(cell, expression.clone());
        displayed.borrow_mut().clear();
        context.frame(BenchFrame {
            root: &crate::occurrence::of(&pane),
            ..BenchFrame::of(&doc, 600.0)
        });
        assert!(displayed.borrow().contains(&result));
        assert!(!displayed.borrow().contains(&expression));
    }
}

#[test]
fn iop_tree_projects_through_grap_into_puri_ink() {
    for size in [
        kurbo::Size::new(500.0, 500.0),
        kurbo::Size::new(720.0, 420.0),
        kurbo::Size::new(360.0, 640.0),
    ] {
        iop_tree_at_size(size);
    }
}

fn iop_tree_at_size(size: kurbo::Size) {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/iop-tree.gid"
    )))
    .expect("the IoP tree demo parses");
    let pane = crate::workspace::panes(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("the picture is a pane");
    let source = crate::occurrence::of(&pane);
    let mut context = BenchContext::new();
    context.stack.projection =
        crate::projection::viewport::projection(&context.stack.projection, size);
    let (bench, extent) = context.frame(BenchFrame {
        root: &source,
        ..BenchFrame::of(&doc, 1400.0)
    });
    let (rebuilt, _) = context.frame(BenchFrame {
        root: &source,
        ..BenchFrame::of(&doc, 1400.0)
    });
    assert_eq!(rebuilt.list.0.len(), bench.list.0.len());
    let (ground, scene) = bench.list.0.split_first().expect("the pane paints");
    assert!(matches!(
        ground,
        DrawCmd::Fill {
            shape: Shape::RoundedRect(_),
            brush: Brush::Solid(color),
            ..
        } if *color == crate::display::widget::style::Theme::Light.palette().readonly_ground
    ));
    let outer = match scene.first() {
        Some(DrawCmd::Fill { transform, .. }) => *transform,
        _ => panic!("the scene starts with the sky fill"),
    };
    let (linked, _) = context.frame(BenchFrame {
        pointer: Some(outer * Point::new(10.0, 10.0)),
        root: &source,
        ..BenchFrame::of(&doc, 1400.0)
    });
    assert!(
        matches!(
            &linked.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Calls(calls))))
                if matches!(calls.sources().next(), Some(crate::display::widget::source::SourceTrace::InCell { .. }))
        ),
        "unexpected drawing link: {:?}",
        linked.hit
    );
    let mut native = DrawList::new();
    let native_stats = super::iop_tree_native::draw(&mut native, size.width, size.height, outer);
    assert_eq!((extent.width, extent.height()), (size.width, size.height));
    assert_eq!(native_stats.branches, 511);
    assert_eq!(native_stats.blossoms, 7_680);
    assert_eq!(native.0.len(), scene.len());
    let mut grap_svg = String::new();
    let mut native_svg = String::new();
    write_cmds(&mut grap_svg, scene);
    write_cmds(&mut native_svg, &native.0);
    assert_eq!(native_svg, grap_svg);
    assert!(bench.list.0.iter().any(|command| matches!(
        command,
        DrawCmd::Fill {
            brush: Brush::Gradient(_),
            ..
        }
    )));
    assert!(
        bench
            .list
            .0
            .iter()
            .filter(|command| matches!(
                command,
                DrawCmd::Fill {
                    shape: Shape::Circle(_),
                    ..
                }
            ))
            .count()
            > 7_000
    );
}

fn drawing_frame(
    doc: &Document,
    libraries: &Libraries,
    shape_function: CellId,
) -> Measured<HoverPass<crate::Editor>> {
    let drawing = layout_data::test_support::program_drawing(
        40.0,
        0.0,
        40.0,
        100,
        grap::call(
            Value::from(layout_data::vocabulary::FILL),
            [
                (
                    layout_data::vocabulary::SHAPE,
                    grap::call(Value::from(shape_function), []),
                ),
                (
                    layout_data::vocabulary::PAINT,
                    crate::libraries::color::tests::value(Color::BLACK),
                ),
            ],
        ),
    );
    let layout =
        layout_data::test_support::decode(&drawing, &crate::libraries::test_widgets::test_site())
            .expect("a program drawing decodes");
    let mut build = measured::choices::ChoiceBuild::default();
    let prepared = crate::display::test_support::with_sources(
        Sources { doc, libraries },
        &crate::display::test_support::NoProject,
        |context| layout.measure(context, &mut build),
    );
    measured::choices::resolve_choices(build.finish(prepared), 40.0)
}

#[test]
fn drawing_records_once_per_visible_frame_for_hover_and_paint() {
    let shape_function = new_cell_id();
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let library = crate::library::Library::<(), ()>::new(crate::library::Definitions::from_parts(
        Cells::new(),
        grap::ForeignFunctions::default().register(
            shape_function,
            grap::ForeignFunction::from_value(move |_, _, _| {
                count.set(count.get() + 1);
                Ok(layout_data::test_support::rect(0.0, 0.0, 10.0, 10.0))
            }),
        ),
    ));
    let libraries = Libraries::from_contributions([(new_cell_id(), library)]).0;
    let doc = Document::default();
    let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
    for expected in 1..=2 {
        let selected = Rc::new(std::cell::RefCell::new(Vec::new()));
        let picked = selected.clone();
        assert_eq!(calls.get(), expected - 1);
        let placed = crate::display::widget::frame::place(
            drawing_frame(&doc, &libraries, shape_function),
            Placement::root(bounds),
            &crate::display::widget::HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                ..Default::default()
            },
        );
        for _ in 0..2 {
            assert_eq!(
                placed
                    .hover_geometry
                    .probe(Some(Point::new(5.0, 5.0)), None, 0.0),
                placed.claim,
            );
        }
        assert_eq!(calls.get(), expected);
        assert!(selected.borrow().is_empty());
        // The drawing is the root's value, so its program is at `program`.
        let program = [gid::Step::Key(layout_data::vocabulary::PROGRAM)];
        let source = SourceTrace::Stored(Rc::from(program.clone()));
        let Some((_, Claim::Direct(target))) = placed.claim.clone() else {
            panic!("expected a drawing source claim")
        };
        let Hovered::Tree(Hover::Calls(ref trace)) = target else {
            panic!("expected a drawing call trace")
        };
        assert_eq!(trace.sources().next(), Some(source.clone()));
        let mut pointer = crate::frame::DispatchContext::new(None, Some(target));
        pointer.landmarks = Rc::from([crate::display::widget::navigation::Landmark {
            root: Some(crate::test_root()),
            path: crate::occurrence::of(&program).into(),
            rect: bounds,
            select: Rc::new(move |_, _| {
                picked.borrow_mut().push(source.clone());
                true
            }),
            secondary: None,
            selected: false,
            tab_stop: false,
        }]);
        let mut state = ui_events::pointer::PointerState::default();
        state.position.x = 5.0;
        state.position.y = 5.0;
        state.modifiers = crate::test_support::command();
        let event = ui_events::pointer::PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: mouse(),
            state,
        };
        let frame = placed.bind(Default::default());
        assert!(frame.handler.unwrap().dispatch_pointer_down_with(
            &mut crate::test_editor(doc.clone()),
            &event,
            &mut pointer,
        ));
        assert_eq!(selected.borrow().len(), 1);
        puri::frame::render(frame.renders, &mut DrawList::new());
        for (point, hits) in [
            (Point::new(5.0, 5.0), true),
            (Point::new(30.0, 30.0), false),
        ] {
            assert_eq!(
                frame.hover_geometry.probe(Some(point), None, 0.0).is_some(),
                hits,
            );
        }
        assert_eq!(calls.get(), expected);
    }
    let clipped = crate::display::widget::frame::place(
        drawing_frame(&doc, &libraries, shape_function),
        Placement::new(bounds, Rect::new(50.0, 50.0, 60.0, 60.0)),
        &Default::default(),
    );
    settle(clipped, &src(&doc, &libraries));
    assert_eq!(calls.get(), 2);
}

#[test]
fn drawing_frames_observe_missing_and_changed_foreign_definitions() {
    let shape_function = new_cell_id();
    let library_id = new_cell_id();
    let library = |width| {
        Libraries::from_contributions([(
            library_id,
            crate::libraries::name::named_library::<crate::Editor>(
                library_id,
                "shape",
                [(shape_function, "shape")],
                grap::ForeignFunctions::default().register(
                    shape_function,
                    grap::ForeignFunction::from_value(move |_, _, _| {
                        Ok(layout_data::test_support::rect(0.0, 0.0, width, 10.0))
                    }),
                ),
            ),
        )])
        .0
    };
    let before = library(10.0);
    let after = library(20.0);
    assert_eq!(
        crate::test_support::first_value(&before, shape_function),
        crate::test_support::first_value(&after, shape_function)
    );
    let doc = Document::default();
    for (libraries, expected) in [
        (Libraries::default(), vec![]),
        (before, vec![10.0]),
        (after, vec![20.0]),
        (Libraries::default(), vec![]),
    ] {
        let placed = crate::display::widget::frame::place(
            drawing_frame(&doc, &libraries, shape_function),
            Placement::root(Rect::new(0.0, 0.0, 40.0, 40.0)),
            &Default::default(),
        );
        let drawing = settle(placed, &src(&doc, &libraries));
        let widths: Vec<_> = drawing
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::Rect(rect),
                    ..
                } => Some(rect.width()),
                _ => None,
            })
            .collect();
        assert_eq!(widths, expected);
    }
}

#[test]
fn drawing_source_reveal_is_an_ordinary_hover_modifier_and_pick_handler() {
    use puri::handler::Event;

    let mut editor = crate::test_editor(Document::default());
    let source_view = editor.model.workspace.document_root().clone();
    let drawing_view =
        crate::display::widget::view::Root::pane(vec![gid::Step::Key(new_cell_id())]);
    let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
    let output = crate::display::widget::frame::place(
        crate::display::widget::view::in_view(
            drawing_frame(&editor.model.doc, &editor.stack.libraries, new_cell_id()),
            drawing_view.clone(),
        ),
        Placement::root(bounds),
        &Default::default(),
    )
    .bind(Default::default());
    let handler = output.handler.unwrap();
    let path: Rc<[gid::Step]> = Rc::from([gid::Step::Key(layout_data::vocabulary::PROGRAM)]);
    let mut input = crate::frame::DispatchContext::new(
        Some(drawing_view.clone()),
        Some(Hovered::Tree(Hover::Source(SourceTrace::Stored(
            path.clone(),
        )))),
    );
    input.landmarks = Rc::from([crate::display::widget::navigation::Landmark {
        root: Some(source_view.clone()),
        path: crate::occurrence::of(&path).into(),
        rect: Rect::new(0.0, 500.0, 20.0, 530.0),
        select: Rc::new({
            let path = path.clone();
            move |editor: &mut crate::Editor, _| {
                crate::editing::Edit::new(editor.session()).select(&crate::occurrence::of(&path));
                true
            }
        }),
        secondary: None,
        selected: false,
        tab_stop: false,
    }]);
    input.view_regions = Rc::from([crate::display::widget::frame::ViewRegion {
        root: source_view.clone(),
        rect: Rect::new(0.0, 0.0, 200.0, 100.0),
        maximum: kurbo::Vec2::new(0.0, 1_000.0),
        content: None,
    }]);
    input.pointer = Some(Point::new(5.0, 5.0));
    assert!(
        !handler
            .dispatch(&mut editor, Event::HoverChanged, &mut input)
            .handled()
    );
    assert_eq!(
        editor.model.workspace.view(&source_view).unwrap().scroll.y,
        0.0
    );

    let modifiers = crate::test_support::command();
    input.modifiers = modifiers;
    for event in [Event::HoverChanged, Event::ModifiersChanged(&modifiers)] {
        editor
            .model
            .workspace
            .view_mut(&source_view)
            .unwrap()
            .scroll = kurbo::Vec2::ZERO;
        assert!(handler.dispatch(&mut editor, event, &mut input).handled());
        assert_eq!(
            editor.model.workspace.view(&source_view).unwrap().scroll.y,
            442.0
        );
        assert!(editor.model.selection.is_none());
    }
    editor
        .model
        .workspace
        .view_mut(&source_view)
        .unwrap()
        .scroll = kurbo::Vec2::ZERO;
    input.root = Some(source_view.clone());
    assert!(
        !handler
            .dispatch(&mut editor, Event::HoverChanged, &mut input)
            .handled()
    );
    assert_eq!(
        editor.model.workspace.view(&source_view).unwrap().scroll.y,
        0.0
    );

    input.root = Some(drawing_view);
    let event = primary_press(Point::new(5.0, 5.0), modifiers);
    editor.pressed = true;
    assert!(handler.dispatch_pointer_down_with(&mut editor, &event, &mut input));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.root(), &source_view);
    assert_eq!(selected.path(), path.as_ref());
    assert_eq!(
        editor.model.workspace.view(&source_view).unwrap().scroll.y,
        442.0
    );
}
