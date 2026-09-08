use super::svg::write_cmds;
use super::*;

#[test]
fn grap_template_preview_evaluates_the_shared_cells_current_call() {
    let mut context = BenchContext::new();
    let mut doc = Document {
        root: Some(
            root_completions(&context.stack)
                .into_iter()
                .find(|offer| offer.display == crate::libraries::grap::vocabulary::GRAP.into())
                .unwrap()
                .value
                .instantiate(),
        ),
        cells: Cells::new(),
    };
    let cell = doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&crate::libraries::grap::vocabulary::GRAP)
        .unwrap()
        .as_cell()
        .unwrap();
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
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
            crate::libraries::number::vocabulary::LEFT,
            crate::libraries::f32::value(7.0),
        ),
        (
            crate::libraries::number::vocabulary::RIGHT,
            crate::libraries::f32::value(2.0),
        ),
    ];
    let shape = Value::record([(
        fidget::vocabulary::DIFFERENCE,
        Value::record(arguments.clone()),
    )]);
    for (function, result) in [
        (
            crate::libraries::f32::vocabulary::SUBTRACT,
            crate::libraries::f32::value(5.0),
        ),
        (fidget::vocabulary::DIFFERENCE, shape),
    ] {
        let expression = grap::call(function.into(), arguments.clone());
        doc.cells.set_value(cell, expression.clone());
        displayed.borrow_mut().clear();
        context.place(
            &doc,
            None,
            &Annotations::default(),
            600.0,
            None,
            None,
            Some(&pane.path),
        );
        assert!(displayed.borrow().contains(&result));
        assert!(!displayed.borrow().contains(&expression));
    }
}

#[test]
fn fidget_template_preview_uses_the_shared_cells_current_definition() {
    let mut context = BenchContext::new();
    let mut doc = Document {
        root: Some(
            root_completions(&context.stack)
                .into_iter()
                .find(|offer| offer.display == fidget::vocabulary::FIDGET.into())
                .unwrap()
                .value
                .instantiate(),
        ),
        cells: Cells::new(),
    };
    let cell = doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&fidget::vocabulary::FIDGET)
        .unwrap()
        .as_cell()
        .unwrap();
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    context.stack.projection = crate::projection::viewport::projection(
        &context.stack.projection,
        kurbo::Size::new(96.0, 64.0),
    );
    let sphere = |radius| {
        grap::call(
            fidget::vocabulary::SPHERE.into(),
            [(
                fidget::vocabulary::RADIUS,
                crate::libraries::f32::value(radius),
            )],
        )
    };
    let coverage = [None, Some(sphere(40.0)), Some(sphere(20.0))].map(|value| {
        if let Some(value) = value {
            doc.cells.set_value(cell, value);
        }
        let (bench, _) = context.place(
            &doc,
            None,
            &Annotations::default(),
            1400.0,
            None,
            None,
            Some(&pane.path),
        );
        bench.list.0.iter().find_map(|command| match command {
            DrawCmd::Image { image, .. } => Some(
                image
                    .data
                    .as_ref()
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .filter(|alpha| **alpha == 255)
                    .count(),
            ),
            _ => None,
        })
    });
    let [None, Some(larger), Some(smaller)] = coverage else {
        panic!("only the defined shapes should render: {coverage:?}");
    };
    assert!(
        larger > smaller && smaller > 0,
        "editing the shared cell changes the preview"
    );
}

#[test]
fn fidget_viewport_projects_an_image_at_the_assigned_size() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/fidget.gid"
    )))
    .expect("the Fidget demo parses");
    let declaration = crate::workspace::declarations(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("the preview is declared as a pane");
    let source = Some(declaration.path.as_slice());
    let mut context = BenchContext::new();
    context.stack.projection = crate::projection::viewport::projection(
        &context.stack.projection,
        kurbo::Size::new(160.0, 96.0),
    );
    let (bench, extent) = context.place(
        &doc,
        None,
        &Annotations::default(),
        1400.0,
        None,
        None,
        source,
    );
    let image = bench
        .list
        .0
        .iter()
        .find_map(|command| match command {
            DrawCmd::Image { image, .. } => Some(image),
            _ => None,
        })
        .expect("the Fidget projection paints an image");
    let alphas = image.data.as_ref().iter().skip(3).step_by(4);
    assert_eq!((image.width, image.height), (160, 96));
    assert_eq!((extent.width, extent.height()), (160.0, 96.0));
    assert!(alphas.clone().any(|alpha| *alpha == 0));
    assert!(alphas.clone().any(|alpha| *alpha == 255));
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
    let declaration = crate::workspace::declarations(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("the picture is declared as a pane");
    let source = Some(declaration.path.as_slice());
    let mut context = BenchContext::new();
    context.stack.projection =
        crate::projection::viewport::projection(&context.stack.projection, size);
    let (bench, extent) = context.place(
        &doc,
        None,
        &Annotations::default(),
        1400.0,
        None,
        None,
        source,
    );
    let (rebuilt, _) = context.place(
        &doc,
        None,
        &Annotations::default(),
        1400.0,
        None,
        None,
        source,
    );
    eprintln!("IoP tree rebuilt frame: {:.1?}", rebuilt.frame_elapsed);
    assert_eq!(rebuilt.list.0.len(), bench.list.0.len());
    let outer = match bench.list.0.first() {
        Some(DrawCmd::Fill { transform, .. }) => *transform,
        _ => panic!("the scene starts with the sky fill"),
    };
    let (linked, _) = context.place(
        &doc,
        None,
        &Annotations::default(),
        1400.0,
        Some(outer * Point::new(10.0, 10.0)),
        None,
        source,
    );
    assert!(
        matches!(
            &linked.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Drawing(
                crate::hover::SourceTrace::InCell { .. }
            ))))
        ),
        "unexpected drawing link: {:?}",
        linked.hit
    );
    let native_iterations = 16;
    let native_start = std::time::Instant::now();
    let (native, native_stats) = (1..native_iterations).fold(
        {
            let mut frame = DrawList::new();
            let stats = super::iop_tree_native::draw(&mut frame, size.width, size.height, outer);
            (frame, stats)
        },
        |_, _| {
            let mut frame = DrawList::new();
            let stats = super::iop_tree_native::draw(&mut frame, size.width, size.height, outer);
            std::hint::black_box(&frame);
            (frame, stats)
        },
    );
    let native_elapsed = native_start.elapsed() / native_iterations;
    let grap_elapsed = bench.frame_elapsed;
    eprintln!(
        "IoP tree: Grap {grap_elapsed:.1?}, native {native_elapsed:.1?}, {:.0}x",
        grap_elapsed.as_secs_f64() / native_elapsed.as_secs_f64(),
    );
    assert_eq!((extent.width, extent.height()), (size.width, size.height));
    assert_eq!(native_stats.branches, 511);
    assert_eq!(native_stats.blossoms, 7_680);
    assert_eq!(native.0.len(), bench.list.0.len());
    let mut grap_svg = String::new();
    let mut native_svg = String::new();
    write_cmds(&mut grap_svg, &bench.list.0);
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
) -> Measured<Placed<crate::Editor>> {
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let cx = Cx {
        view: &crate::test_root(),
        completions: None,
        sources: Sources { doc, libraries },
        raw: false,
        annotations: &annotations,
        styles: &styles,
        selection: None,
        scrub_spelling: None,
        secondary: None,
        selected_trace: None,
        source: Source::Stored,
        fuel: std::cell::Cell::new(100),
    };
    crate::projection::drawing::program_leaf(
        &cx,
        &[],
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
                    crate::libraries::color::value(Color::BLACK),
                ),
            ],
        ),
    )
}

#[test]
fn drawing_records_once_per_visible_frame_for_hover_and_paint() {
    let shape_function = new_cell_id();
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = calls.clone();
    let library = crate::libraries::Library::<(), ()>::new(
        crate::libraries::Definitions::from_parts(
            Cells::new(),
            grap::ForeignFunctions::default().register(
                shape_function,
                grap::ForeignFunction::new(move |_, _, _| {
                    count.set(count.get() + 1);
                    Ok(layout_data::rect(0.0, 0.0, 10.0, 10.0))
                }),
            ),
        ),
        crate::display::partial(|_| None),
    );
    let libraries = Libraries::from_contributions([(new_cell_id(), library)]).0;
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let bounds = Rect::new(0.0, 0.0, 40.0, 40.0);
    for expected in 1..=2 {
        let selected = Rc::new(std::cell::RefCell::new(Vec::new()));
        let picked = selected.clone();
        let placed = measured::place(
            drawing_frame(&doc, &libraries, shape_function),
            Placement::root(bounds),
        );
        assert_eq!(calls.get(), expected - 1);
        let placed = placed.run(&placed::HoverInput {
            pointer: Some(Point::new(5.0, 5.0)),
            ..Default::default()
        });
        for _ in 0..2 {
            assert!(matches!(
                placed.claim.clone().map(|(_, claim)| claim),
                Some(Claim::Direct(_))
            ));
        }
        assert_eq!(calls.get(), expected);
        assert!(selected.borrow().is_empty());
        let source = SourceTrace::Stored(Rc::from([Step::Key(layout_data::vocabulary::PROGRAM)]));
        let target = Hovered::Tree(Hover::Drawing(source.clone()));
        assert_eq!(
            placed.claim.clone().map(|(_, claim)| claim),
            Some(Claim::Direct(target.clone()))
        );
        let mut pointer = placed::DispatchContext::new(None, Some(target));
        pointer.descends = Rc::from([crate::navigate::Descend {
            root: Some(crate::test_root()),
            path: Rc::from([Step::Key(layout_data::vocabulary::PROGRAM)]),
            rect: bounds,
            select: Rc::new(move |_, _| {
                picked.borrow_mut().push(source.clone());
                true
            }),
        }]);
        let mut state = ui_events::pointer::PointerState::default();
        state.position.x = 5.0;
        state.position.y = 5.0;
        state.modifiers =
            ui_events::keyboard::Modifiers::META | ui_events::keyboard::Modifiers::CONTROL;
        let event = ui_events::pointer::PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: ui_events::pointer::PointerInfo {
                pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: ui_events::pointer::PointerType::Mouse,
            },
            state,
        };
        assert!(placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
            &mut crate::test_editor(doc.clone()),
            &event,
            &mut pointer,
        ));
        assert_eq!(selected.borrow().len(), 1);
        settle(placed);
        assert_eq!(calls.get(), expected);
    }
    let clipped = measured::place(
        drawing_frame(&doc, &libraries, shape_function),
        Placement::new(bounds, Rect::new(50.0, 50.0, 60.0, 60.0)),
    )
    .run(&Default::default());
    settle(clipped);
    assert_eq!(calls.get(), 2);
}

#[test]
fn drawing_frames_observe_missing_and_changed_foreign_definitions() {
    let shape_function = new_cell_id();
    let library_id = new_cell_id();
    let library = |width| {
        let mut cells = Cells::new();
        cells.set_value(shape_function, name::record("shape", []));
        Libraries::from_contributions([(
            library_id,
            crate::libraries::Library::<(), ()>::named(
                library_id,
                "shape",
                crate::libraries::Definitions::from_parts(
                    cells,
                    grap::ForeignFunctions::default().register(
                        shape_function,
                        grap::ForeignFunction::new(move |_, _, _| {
                            Ok(layout_data::rect(0.0, 0.0, width, 10.0))
                        }),
                    ),
                ),
                crate::display::partial(|_| None),
            ),
        )])
        .0
    };
    let before = library(10.0);
    let after = library(20.0);
    assert_eq!(
        before.first_value(shape_function),
        after.first_value(shape_function)
    );
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    for (libraries, expected) in [
        (Libraries::default(), vec![]),
        (before, vec![10.0]),
        (after, vec![20.0]),
        (Libraries::default(), vec![]),
    ] {
        let placed = measured::place(
            drawing_frame(&doc, &libraries, shape_function),
            Placement::root(Rect::new(0.0, 0.0, 40.0, 40.0)),
        )
        .run(&Default::default());
        let drawing = settle(placed);
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
