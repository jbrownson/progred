use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::text;

use crate::display::test_support::{ProjectionCall, inspect};
use std::rc::Rc;

fn decoded(value: &Value) -> Option<Recorded<crate::Editor, crate::frame::Hovered>> {
    let select: crate::display::Action = Rc::new(|_| false);
    decode(
        value,
        &select,
        &crate::libraries::test_widgets::hover(vec![]),
    )
    .map(|layout| layout.record())
}

#[test]
fn traversal_uses_the_path_library_for_every_step() {
    let steps = vec![
        Step::Key(gid::new_cell_id()),
        Step::Element(gid::position::between(None, None).unwrap()),
        Step::Follow(gid::Resolution::Document),
        Step::Follow(gid::Resolution::Library(gid::new_cell_id())),
    ];
    for step in &steps {
        assert!(
            matches!((decoded(&descend_step(step.clone()))).map(|layout| inspect(&layout)),
                Some(ProjectionCall::Descend { step: decoded, .. }) if decoded == *step
            )
        );
    }
    let child = text::value("child");
    let value = node(
        vocabulary::AT,
        Value::record([
            (vocabulary::STEPS, crate::libraries::path::value(&steps)),
            (vocabulary::VALUE, child.clone()),
        ]),
    );
    assert!(matches!((decoded(&value)).map(|layout| inspect(&layout)),
        Some(ProjectionCall::At { steps: decoded, value, .. }) if decoded == steps && value == child
    ));
    let value = node(
        vocabulary::DESCEND_PATH,
        crate::libraries::path::value(&steps),
    );
    assert!(matches!(decoded(&value).map(|layout| inspect(&layout)),
        Some(ProjectionCall::DescendPath { steps: decoded, .. }) if decoded == steps
    ));
    let document = vec![Step::Key(gid::new_cell_id())];
    let value = node(
        vocabulary::JUMP,
        Value::record([
            (vocabulary::STEPS, crate::libraries::path::value(&steps)),
            (
                vocabulary::DOCUMENT_PATH,
                crate::libraries::path::value(&document),
            ),
        ]),
    );
    assert!(matches!(decoded(&value).map(|layout| inspect(&layout)),
        Some(ProjectionCall::Jump { steps: decoded, document: target, .. }) if decoded == steps && target == crate::location::Location::root(document.as_slice())
    ));
}

#[test]
fn drawing_constructor_keeps_native_program_code_and_captures() {
    let library = library();
    let host = crate::libraries::TestHost(|cell| {
        library
            .definitions
            .get(cell)
            .cloned()
            .map(|definition| (gid::Resolution::Document, definition))
            .into_iter()
            .collect()
    });
    let captured = gid::new_cell_id();
    let program = ::grap::evaluate_at(
        &::grap::call(
            ::grap::lambda([captured], ::grap::lambda([], captured.into())),
            [(captured, number(7.0))],
        ),
        Some(::grap::SourceOrigin::Stored(vec![Step::Key(
            gid::new_cell_id(),
        )])),
        &host,
        100,
    )
    .result;
    let configuration = ::grap::RuntimeValue::record([
        (vocabulary::WIDTH, ::grap::RuntimeValue::f64(12.0)),
        (vocabulary::PROGRAM, program.clone()),
    ]);
    let result = ::grap::apply(
        &Value::from(vocabulary::DRAWING).into(),
        [(presentation::vocabulary::VALUE, configuration.clone())],
        &host,
        100,
    );
    assert!(result.completed);
    let drawn = result.result.field(vocabulary::DRAWING).unwrap();
    assert!(drawn.same_result(&configuration));
    let retained = drawn.field(vocabulary::PROGRAM).unwrap();
    assert!(retained.same_result(&program));
    assert_eq!(
        ::grap::apply(&retained, [], &host, 100).result.as_f64(),
        Some(7.0)
    );
}

#[test]
fn drawing_is_an_ordinary_projection_function() {
    let library = library();
    let configuration = Value::record([(vocabulary::WIDTH, number(12.0))]);
    let evaluation = crate::libraries::test_apply(
        &Value::from(vocabulary::DRAWING),
        [(presentation::vocabulary::VALUE, configuration.clone())],
        |cell| library.value(cell).cloned(),
        &library.functions(),
        20,
    );
    assert_eq!(
        evaluation.result,
        Value::record([(vocabulary::DRAWING, configuration)])
    );
}

#[test]
fn border_retains_native_callbacks_and_their_call_origins() {
    use ::grap::{RuntimeValue, SourceOrigin};
    use std::{cell::RefCell, rc::Rc};

    let library = library();
    let sink = gid::new_cell_id();
    let captured = gid::new_cell_id();
    let origin = gid::new_cell_id();
    let traces = Rc::new(RefCell::new(Vec::new()));
    let host = crate::libraries::TestHost(|cell: CellId| {
        let definition = if cell == sink {
            let traces = traces.clone();
            Some(::grap::Definition::foreign(
                Value::record([]),
                ForeignFunction::new(move |context, call, environment| {
                    traces.borrow_mut().push(context.call_trace().unwrap());
                    context.eval(context.field(call, captured).unwrap(), environment)
                }),
            ))
        } else {
            library.definitions.get(cell).cloned()
        };
        definition
            .map(|definition| (gid::Resolution::Document, definition))
            .into_iter()
            .collect()
    });
    let callback = ::grap::evaluate_at(
        &::grap::lambda([], number(7.0)),
        Some(SourceOrigin::Stored(vec![Step::Key(gid::new_cell_id())])),
        &host,
        100,
    )
    .result;
    let maker = ::grap::evaluate_at(
        &::grap::lambda(
            [captured],
            ::grap::lambda(
                [presentation::vocabulary::VALUE],
                ::grap::call(sink.into(), [(captured, captured.into())]),
            ),
        ),
        Some(SourceOrigin::Stored(vec![Step::Key(origin)])),
        &host,
        100,
    )
    .result;
    let projection = ::grap::apply(&maker, [(captured, callback.clone())], &host, 100).result;
    let bordered = ::grap::apply(
        &Value::from(vocabulary::BORDER).into(),
        [(presentation::vocabulary::PROJECTION, projection)],
        &host,
        100,
    );
    assert!(bordered.completed);
    assert!(
        traces.borrow().is_empty(),
        "composition must not run the projection"
    );
    // The constructed closure outlives the evaluation that captured it.
    for _ in 0..2 {
        let result = ::grap::apply_expression(
            &bordered.result,
            [(presentation::vocabulary::VALUE, RuntimeValue::record([]))],
            &host,
            100,
        );
        assert!(result.completed);
        let retained = result
            .result
            .field(vocabulary::BORDER)
            .unwrap()
            .field(vocabulary::AT)
            .unwrap()
            .field(vocabulary::VALUE)
            .unwrap();
        assert!(retained.same_result(&callback));
        assert_eq!(
            ::grap::apply(&retained, [], &host, 100).result.as_f64(),
            Some(7.0)
        );
    }
    let traces = traces.borrow();
    assert_eq!(traces.len(), 2);
    let expected = SourceOrigin::Stored(vec![
        Step::Key(origin),
        Step::Key(::grap::vocabulary::BODY),
        Step::Key(::grap::vocabulary::BODY),
    ]);
    for trace in traces.iter() {
        assert_eq!(trace.origins().next(), Some(&expected));
    }
}

#[test]
fn border_preserves_expression_facing_foreign_arguments() {
    let library = library();
    let data = gid::new_cell_id();
    let value = number(12.0);
    let host = crate::libraries::TestHost(|cell: CellId| {
        let definition = if cell == data {
            Some(::grap::Definition::Value(value.clone()))
        } else {
            library.definitions.get(cell).cloned()
        };
        definition
            .map(|definition| (gid::Resolution::Document, definition))
            .into_iter()
            .collect()
    });
    for (projection, expected) in [
        (
            ::grap::ffi(vocabulary::DRAWING),
            ::grap::RuntimeValue::record([(vocabulary::DRAWING, (&value).into())]),
        ),
        (
            ::grap::lambda(
                [presentation::vocabulary::VALUE],
                presentation::vocabulary::VALUE.into(),
            ),
            ::grap::RuntimeValue::from(Value::from(data)),
        ),
    ] {
        let wrapper = ::grap::evaluate(
            &::grap::call(
                vocabulary::BORDER.into(),
                [(presentation::vocabulary::PROJECTION, projection)],
            ),
            &host,
            100,
        );
        let result = ::grap::apply_expression(
            &wrapper.result,
            [(presentation::vocabulary::VALUE, Value::from(data).into())],
            &host,
            100,
        );
        assert!(result.completed);
        let output = result
            .result
            .field(vocabulary::BORDER)
            .unwrap()
            .field(vocabulary::AT)
            .unwrap()
            .field(vocabulary::VALUE)
            .unwrap();
        assert_eq!(output.to_value(), expected.to_value());
    }
}

#[test]
fn border_interprets_captured_projection_syntax_only_when_invoked() {
    use std::{cell::Cell, rc::Rc};

    let library = library();
    let syntax = gid::new_cell_id();
    let resolve = gid::new_cell_id();
    let runs = Rc::new(Cell::new(0));
    let host = crate::libraries::TestHost(|cell: CellId| {
        let definition = if cell == syntax {
            Some(::grap::Definition::foreign(
                Value::record([]),
                ForeignFunction::new(move |_, _, _| Ok(::grap::call(resolve.into(), []).into())),
            ))
        } else if cell == resolve {
            let runs = runs.clone();
            Some(::grap::Definition::foreign(
                Value::record([]),
                ForeignFunction::new(move |_, _, _| {
                    runs.set(runs.get() + 1);
                    Ok(::grap::ffi(vocabulary::DRAWING).into())
                }),
            ))
        } else {
            library.definitions.get(cell).cloned()
        };
        definition
            .map(|definition| (gid::Resolution::Document, definition))
            .into_iter()
            .collect()
    });
    let wrapper = ::grap::evaluate(
        &::grap::call(
            vocabulary::BORDER.into(),
            [(
                presentation::vocabulary::PROJECTION,
                ::grap::call(syntax.into(), []),
            )],
        ),
        &host,
        100,
    );
    assert!(wrapper.completed);
    assert_eq!(runs.get(), 0);
    for expected_runs in 1..=2 {
        let result = ::grap::apply_expression(
            &wrapper.result,
            [(
                presentation::vocabulary::VALUE,
                ::grap::RuntimeValue::f64(12.0),
            )],
            &host,
            100,
        );
        assert!(result.completed);
        assert_eq!(runs.get(), expected_runs);
        assert_eq!(
            result
                .result
                .field(vocabulary::BORDER)
                .unwrap()
                .field(vocabulary::AT)
                .unwrap()
                .field(vocabulary::VALUE)
                .unwrap()
                .field(vocabulary::DRAWING)
                .unwrap()
                .as_f64(),
            Some(12.0)
        );
    }
}

#[test]
fn border_composes_with_an_ordinary_projection() {
    let library = library();
    let configuration = Value::record([(vocabulary::WIDTH, number(12.0))]);
    let composition = crate::libraries::test_evaluate(
        &::grap::call(
            Value::from(vocabulary::BORDER),
            [(
                presentation::vocabulary::PROJECTION,
                ::grap::ffi(vocabulary::DRAWING),
            )],
        ),
        |cell| library.value(cell).cloned(),
        &library.functions(),
        20,
    );
    assert!(
        composition
            .result
            .as_record()
            .is_some_and(|fields| fields.contains_key(&::grap::vocabulary::CLOSURE))
    );
    let evaluation = crate::libraries::test_apply(
        &composition.result,
        [(presentation::vocabulary::VALUE, configuration.clone())],
        |cell| library.value(cell).cloned(),
        &library.functions(),
        50,
    );
    let Some(Recorded::After { child, .. }) = decoded(&evaluation.result) else {
        panic!(
            "the composed projection returns a border: {:?}",
            evaluation.result
        );
    };
    assert!(matches!(&inspect(&(child.as_ref())),
        ProjectionCall::At { value, .. }
            if value == &Value::record([(vocabulary::DRAWING, configuration)])
    ));
}

#[test]
fn invalid_column_baselines_decline_before_measurement() {
    let child = || text_leaf("one", vocabulary::NAME_FACE);
    assert!(decoded(&col(0, 0.0, [child()])).is_some());
    assert!(decoded(&col(1, 0.0, [child()])).is_none());
    assert!(decoded(&col(usize::MAX, 0.0, [child()])).is_none());
    assert!(decoded(&col(0, 0.0, [])).is_some());
}

#[test]
fn border_wraps_any_decoded_layout() {
    let value = bordered(text_leaf("inside", vocabulary::NAME_FACE));
    let Some(Recorded::After { child, .. }) = decoded(&value) else {
        panic!("border decodes");
    };
    assert!(matches!(
        child.as_ref(),
        Recorded::Leaf(Leaf::Text { text, .. }) if text == "inside"
    ));
}

#[test]
fn a_layout_round_trips_from_data() {
    let value = options([
        selectable(row(
            4.0,
            [
                text_leaf("shape", vocabulary::NAME_FACE),
                descend_key(vocabulary::GAP),
            ],
        )),
        bracketed(
            vocabulary::CURLY,
            col(
                0,
                2.0,
                [
                    descend_follow(gid::Resolution::Document),
                    text_leaf("…", vocabulary::DIM_FACE),
                ],
            ),
        ),
    ]);
    let Some(Recorded::Alternatives(forms)) = decoded(&value) else {
        panic!("alternatives decode");
    };
    assert_eq!(forms.len(), 2);
    let Recorded::Before { child, .. } = &forms[0] else {
        panic!("selectable claims the hover");
    };
    let Recorded::Before { child, .. } = child.as_ref() else {
        panic!("selectable attaches the provided select");
    };
    let Recorded::Row { gap, children, .. } = child.as_ref() else {
        panic!("row inside");
    };
    assert_eq!(*gap, 4.0);
    assert!(matches!(
        &children[0],
        Recorded::Leaf(Leaf::Text {
            text,
            paint: Paint::Face(Face::Name),
            ..
        }) if text == "shape"
    ));
    assert!(matches!(&inspect(&(&children[1])),
        ProjectionCall::Descend { step: Step::Key(key), .. } if *key == vocabulary::GAP
    ));
    let (left, child, right) = crate::display::test_support::delimited(&forms[1]);
    crate::libraries::test_widgets::assert_delimiter(
        left,
        Delim::Brace,
        crate::display::Side::Open,
    );
    crate::libraries::test_widgets::assert_delimiter(
        right,
        Delim::Brace,
        crate::display::Side::Close,
    );
    let Recorded::Col { children, .. } = child else {
        panic!("col inside");
    };
    assert!(matches!(
        &inspect(&(&children[0])),
        ProjectionCall::Descend {
            step: Step::Follow(gid::Resolution::Document),
            ..
        }
    ));
}

#[test]
fn intents_and_leaves_decode() {
    assert_eq!(
        crate::libraries::test_widgets::picked(
            &decoded(&pick_target(
                text_leaf("k", vocabulary::ID_FACE),
                Value::Cell(vocabulary::GAP),
            ))
            .unwrap()
        ),
        Some(Value::Cell(vocabulary::GAP))
    );
    assert_eq!(
        crate::libraries::test_widgets::claim(
            &decoded(&selectable(text_leaf("h", vocabulary::LABEL_FACE))).unwrap()
        ),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![])
        ))
    );
    assert_eq!(
        crate::libraries::test_widgets::claim(
            &decoded(&hover_block(node(vocabulary::SLOT, Value::record([])))).unwrap()
        ),
        Some(puri::hover::Claim::Occludes)
    );
    let handler = ::grap::lambda(
        [],
        ::grap::call(
            crate::libraries::site::vocabulary::SET.into(),
            [(
                crate::libraries::site::vocabulary::VALUE,
                Value::from(vec![7u8]),
            )],
        ),
    );
    assert_eq!(
        crate::libraries::test_widgets::event_annotation(
            &decoded(&on(text_leaf("go", vocabulary::NAME_FACE), handler.clone())).unwrap()
        ),
        Some(Value::from(vec![7u8]))
    );
}

#[test]
fn puri_commands_are_ordinary_display_data() {
    let display = drawing(
        20.0,
        8.0,
        2.0,
        [
            fill(
                rounded_rect(0.0, 0.0, 20.0, 10.0, 2.0),
                Value::from(vocabulary::DIM_FACE),
            ),
            stroke(
                circle(10.0, 5.0, 4.0),
                1.0,
                Value::from(vocabulary::NAME_FACE),
            ),
            clip(
                rect(0.0, 0.0, 20.0, 10.0),
                [stroke(
                    path([
                        move_to(0.0, 0.0),
                        line_to(5.0, 5.0),
                        quad_to(7.0, 3.0, 10.0, 5.0),
                        curve_to(11.0, 6.0, 12.0, 4.0, 15.0, 5.0),
                        close(),
                    ]),
                    2.0,
                    Value::from(vocabulary::INK_FACE),
                )],
            ),
        ],
    );
    let Some(Recorded::Leaf(Leaf::Drawing(drawing))) = decoded(&display) else {
        panic!("Puri drawing leaf");
    };
    assert_eq!(
        (drawing.width, drawing.ascent, drawing.descent),
        (20.0, 8.0, 2.0)
    );
    assert_eq!(drawing.commands.len(), 3);
    assert!(matches!(
        drawing.commands[1],
        Command::Stroke { shape: Shape::Circle(_), ref style, .. } if style.width == 1.0
    ));
    assert!(matches!(
        drawing.commands[2],
        Command::Clip { ref children, .. }
            if matches!(children.as_slice(), [Command::Stroke { shape: Shape::Path(_), .. }])
    ));
}

#[test]
fn junk_falls_through_whole() {
    assert!(decoded(&Value::record([])).is_none());
    assert!(decoded(&text::value("plain text is not a layout")).is_none());
    let broken = row(
        1.0,
        [text_leaf("ok", vocabulary::NAME_FACE), Value::record([])],
    );
    assert!(decoded(&broken).is_none());
    let bad_face = node(
        vocabulary::TEXT,
        Value::record([
            (vocabulary::CONTENT, text::value("x")),
            (vocabulary::PAINT, Value::Cell(vocabulary::GAP)),
        ]),
    );
    assert!(decoded(&bad_face).is_none());
}

mod programs;
