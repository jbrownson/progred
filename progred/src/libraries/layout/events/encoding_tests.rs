use super::*;
use crate::display::test_support::ResolveForDispatch;
use crate::projection::Projection;
use crate::projection::test_support::editing_frame_with_projection;
use crate::view::ProjectionInput;
use gid::{Cells, Document, Record};
use layout_data::vocabulary as event_fields;
use puri::handler::PointerId;

/// The event a handler last stored at the root.
fn last_event(world: &crate::Editor) -> &Record {
    world
        .model
        .workspace
        .document
        .annotations
        .at(&[])
        .unwrap()
        .as_record()
        .unwrap()
}

fn last_kind(world: &crate::Editor) -> Option<CellId> {
    last_event(world)
        .get(&event_fields::EVENT_KIND)
        .and_then(Value::as_cell)
}

#[test]
fn grap_event_handlers_receive_all_event_kinds_at_the_projected_site() {
    fn probe(input: &ProjectionInput<'_>) -> Option<Layout> {
        input.value?.as_blob()?;
        let target = input.targets.current();
        layout_data::decode(
            &layout_data::on(
                layout_data::text_leaf("go", event_fields::NAME_FACE),
                grap::lambda(
                    [event_fields::EVENT],
                    grap::call(
                        crate::libraries::site::vocabulary::SET.into(),
                        [(
                            crate::libraries::site::vocabulary::VALUE,
                            event_fields::EVENT.into(),
                        )],
                    ),
                ),
            ),
            &target.select,
            &target.hover,
        )
    }
    let mut events = crate::test_editor(Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    });
    let projection = Projection::new([crate::display::partial(probe)]);
    let placed = editing_frame_with_projection(&mut events, false, Some(&projection));
    let handler = placed.resolve_for_dispatch();
    let mut state = PointerState::default();
    state.position.x = 1.0;
    state.position.y = 1.0;
    let mouse = PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    };
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: mouse,
            state: state.clone(),
        },
    ));
    assert_eq!(last_kind(&events), Some(event_fields::POINTER_DOWN));
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: mouse,
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(last_kind(&events), Some(event_fields::POINTER_MOVE));

    let touch = PointerInfo {
        pointer_id: PointerId::new(7),
        persistent_device_id: None,
        pointer_type: PointerType::Touch,
    };
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    assert_eq!(last_kind(&events), Some(event_fields::TOUCH_START));
    assert!(!last_event(&events).contains_key(&event_fields::BUTTON));
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: touch,
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(last_kind(&events), Some(event_fields::TOUCH_MOVE));
    assert!(handler.dispatch_pointer_up(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    assert_eq!(last_kind(&events), Some(event_fields::TOUCH_END));
    assert!(handler.dispatch_pointer_cancel(&mut events, &touch));
    assert_eq!(last_kind(&events), Some(event_fields::TOUCH_CANCEL));

    let scrolls = [1.0, -2.0].map(|y| PointerScrollEvent {
        pointer: mouse,
        delta: ScrollDelta::LineDelta(0.0, y),
        state: state.clone(),
    });
    assert!(
        handler
            .dispatch(
                &mut events,
                Event::Scroll(std::borrow::Cow::Borrowed(&scrolls)),
                &mut Default::default(),
            )
            .handled()
    );
    assert_eq!(last_kind(&events), Some(event_fields::SCROLL));
    let batch = last_event(&events)
        .get(&event_fields::CONTENT)
        .unwrap()
        .as_list()
        .unwrap();
    assert_eq!(
        batch
            .values()
            .map(|event| f64_convention::read(
                event
                    .as_record()
                    .unwrap()
                    .get(&event_fields::DELTA_Y)
                    .unwrap()
            )
            .unwrap())
            .collect::<Vec<_>>(),
        [1.0, -2.0]
    );

    let key = KeyboardEvent {
        modifiers: crate::modifiers::command(),
        ..Default::default()
    };
    assert!(handler.dispatch_key(&mut events, &key));
    assert_eq!(last_kind(&events), Some(event_fields::KEY));
    assert_eq!(
        last_event(&events).get(&event_fields::MODIFIERS),
        Some(&Value::list([Value::Cell(event_fields::COMMAND)]))
    );

    assert!(handler.dispatch_ime(&mut events, &ImeEvent::Commit("entered".into())));
    assert_eq!(last_kind(&events), Some(event_fields::IME));
    assert_eq!(
        last_event(&events).get(&event_fields::CONTENT),
        Some(&text::value("entered"))
    );

    let outside = PointerState {
        position: (-100.0, -100.0).into(),
        ..Default::default()
    };
    let button = PointerButtonEvent {
        pointer: touch,
        button: None,
        state: outside.clone(),
    };
    assert!(!handler.dispatch_pointer_down(&mut events, &button));
    assert!(
        !handler
            .dispatch_scroll(
                &mut events,
                &PointerScrollEvent {
                    pointer: touch,
                    state: outside.clone(),
                    delta: ScrollDelta::LineDelta(0.0, 1.0),
                }
            )
            .handled()
    );
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: touch,
            current: outside,
            coalesced: vec![],
            predicted: vec![],
        }
    ));
    assert!(handler.dispatch_pointer_up(&mut events, &button));
}
