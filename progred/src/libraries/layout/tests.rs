use super::test_support::*;
use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::text;

use crate::display::test_support::{ProjectionCall, inspect};

fn decoded(value: &Value) -> Option<Recorded<crate::Editor, crate::frame::Hovered>> {
    decode(value, &crate::libraries::test_widgets::test_site()).map(|layout| layout.record())
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
                Some(ProjectionCall::Descend { steps: decoded, .. }) if decoded == [step.clone()]
            )
        );
    }
    let child = text::value("child");
    let value = layout_node(
        vocabulary::kind::AT,
        [
            (
                vocabulary::STEPS,
                crate::libraries::editor::path::value(&steps),
            ),
            (vocabulary::VALUE, child.clone()),
        ],
    );
    assert!(matches!((decoded(&value)).map(|layout| inspect(&layout)),
        Some(ProjectionCall::At { steps: decoded, value, .. }) if decoded == steps && value == child
    ));
    let value = layout_node(
        vocabulary::kind::DESCEND_PATH,
        [(
            vocabulary::STEPS,
            crate::libraries::editor::path::value(&steps),
        )],
    );
    assert!(matches!(decoded(&value).map(|layout| inspect(&layout)),
        Some(ProjectionCall::Descend { steps: decoded, .. }) if decoded == steps
    ));
    let document = vec![Step::Key(gid::new_cell_id())];
    let value = layout_node(
        vocabulary::kind::JUMP,
        [
            (
                vocabulary::STEPS,
                crate::libraries::editor::path::value(&steps),
            ),
            (
                vocabulary::DOCUMENT_PATH,
                crate::libraries::editor::path::value(&document),
            ),
        ],
    );
    assert!(matches!(decoded(&value).map(|layout| inspect(&layout)),
        Some(ProjectionCall::Jump { steps: decoded, document: target, .. }) if decoded == steps && target == crate::location::Location::root(document.as_slice())
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
        selectable_node(row(
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
        ProjectionCall::Descend { steps, .. } if *steps == [Step::Key(vocabulary::GAP)]
    ));
    let (left, child, right) = crate::display::test_support::delimited(&forms[1]);
    crate::libraries::test_widgets::assert_delimiter(left, Delim::Brace, puri::delim::Side::Open);
    crate::libraries::test_widgets::assert_delimiter(right, Delim::Brace, puri::delim::Side::Close);
    let Recorded::Col { children, .. } = child else {
        panic!("col inside");
    };
    assert!(matches!(
        &inspect(&(&children[0])),
        ProjectionCall::Descend { steps, .. } if *steps == [Step::Follow(gid::Resolution::Document)]
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
            &decoded(&selectable_node(text_leaf("h", vocabulary::LABEL_FACE))).unwrap()
        ),
        Some(puri::hover::Claim::Direct(
            crate::libraries::test_widgets::hover(vec![])
        ))
    );
    assert_eq!(
        crate::libraries::test_widgets::claim(
            &decoded(&hover_block(layout_node(vocabulary::kind::SLOT, []))).unwrap()
        ),
        Some(puri::hover::Claim::Occludes)
    );
    let handler = ::grap::lambda(
        [],
        ::grap::call(
            crate::libraries::editor::site::vocabulary::SET.into(),
            [(
                crate::libraries::editor::site::vocabulary::VALUE,
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
fn a_path_that_does_not_start_by_moving_is_junk() {
    let stroked = |elements: Vec<Value>| {
        drawing(
            10.0,
            10.0,
            0.0,
            [stroke(
                path(elements),
                1.0,
                Value::from(vocabulary::INK_FACE),
            )],
        )
    };
    assert!(decoded(&stroked(vec![line_to(5.0, 5.0), move_to(0.0, 0.0)])).is_none());
    assert!(decoded(&stroked(vec![move_to(0.0, 0.0), line_to(5.0, 5.0)])).is_some());
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
    let bad_face = layout_node(
        vocabulary::kind::TEXT,
        [
            (vocabulary::CONTENT, text::value("x")),
            (vocabulary::PAINT, Value::Cell(vocabulary::GAP)),
        ],
    );
    assert!(decoded(&bad_face).is_none());
}

mod programs;

#[test]
fn center_and_rule_decode_from_data_as_a_program_emits_them() {
    let value = col(
        0,
        0.0,
        [
            layout_node(
                vocabulary::kind::CENTER,
                [(vocabulary::CHILD, text_leaf("1", vocabulary::NAME_FACE))],
            ),
            layout_node(vocabulary::kind::RULE, []),
            layout_node(
                vocabulary::kind::CENTER,
                [(vocabulary::CHILD, text_leaf("2", vocabulary::NAME_FACE))],
            ),
        ],
    );
    let Some(Recorded::Col { children, .. }) = decoded(&value) else {
        panic!("a fraction's column decodes");
    };
    assert_eq!(children.len(), 3);
}
