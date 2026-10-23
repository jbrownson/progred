use super::*;

#[test]
fn payloads_round_trip_by_stage() {
    let edge = edge();
    assert_eq!(stage(&edge), Some(vocabulary::EDGE));
    assert_eq!(query(&edge), None);

    let pending = with_completion_view(&pending("asd"), 0.0, 2);
    assert_eq!(stage(&pending), Some(vocabulary::PENDING));
    assert_eq!(query(&pending), Some("asd"));
    assert_eq!(choice(&pending), Some(2));

    let label = label("nm");
    assert_eq!(stage(&label), Some(vocabulary::LABEL));
    assert_eq!(label.as_record().unwrap().len(), 3);
}

#[test]
fn the_whole_editor_round_trips_through_the_payload() {
    let mut line = LineEditState::from_parts(
        "hëllo",
        2,
        4,
        Some(("ab".to_string(), Some((0, 2)))),
        Some((Point::new(4.5, 6.0), 2)),
    );
    // 2 is inside ë (bytes 1..3): clamps back to its boundary.
    assert_eq!(line.selection_offsets(), (1, 4));
    let encoded = with_editor(&pending(""), &line, true);
    let decoded = editor_line(&encoded, query(&encoded).unwrap_or(""));
    assert_eq!(decoded.text(), "hëllo");
    assert_eq!(decoded.selection_offsets(), line.selection_offsets());
    assert_eq!(decoded.preedit_parts(), line.preedit_parts());
    assert_eq!(decoded.drag_parts(), line.drag_parts());

    // Ending the composition and drag removes their fields.
    line = LineEditState::from_parts("hëllo", 1, 4, None, None);
    let settled = with_editor(&encoded, &line, true);
    let fields = settled.as_record().unwrap();
    assert!(!fields.contains_key(&vocabulary::PREEDIT));
    assert!(!fields.contains_key(&vocabulary::DRAG));
}

#[test]
fn changing_an_owned_query_resets_position() {
    let changed = with_editor(
        &with_completion_view(&pending("old"), 24.0, 2),
        &LineEditState::from_parts("new", 3, 3, None, None),
        true,
    );
    assert_eq!(query(&changed), Some("new"));
    assert_eq!(choice(&changed), Some(0));
    assert_eq!(completion_scroll(&changed), Some(0.0));

    let unchanged = with_editor(
        &with_completion_view(&pending("same"), 24.0, 2),
        &LineEditState::from_parts("same", 4, 4, None, None),
        true,
    );
    assert_eq!(choice(&unchanged), Some(2));
    assert_eq!(completion_scroll(&unchanged), Some(24.0));
}

#[test]
fn junk_reads_none() {
    assert_eq!(stage(&Value::record([])), None);
    let junk = Value::record([(vocabulary::CHOICE, super::f64_convention::value(-1.5))]);
    assert_eq!(choice(&junk), None);
}
