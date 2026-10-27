use super::*;
fn leaf() -> Value {
    Value::record([])
}

#[test]
fn ranges_follow_occurrences_and_irregular_nested_lists() {
    let tree = Value::list([
        Value::list([Value::list([leaf(), leaf()]), leaf()]),
        Value::list([leaf(), leaf()]),
    ]);
    let all = Selection::new(&tree, None);
    assert_eq!(
        all.rows.iter().map(|r| r.slider.count).collect::<Vec<_>>(),
        [2, 3, 5]
    );
    assert_eq!(all.leaves, 0..5);
    let chosen = all.select(2, 1..2, None).select(0, 0..1, None);
    assert_eq!(chosen.leaves, 3..4);
    assert_eq!(Selection::new(&tree, Some(&chosen.state())).leaves, 3..4);
    assert_eq!(
        all.select(2, 0..1, None)
            .select(1, 0..1, None)
            .select(0, 1..2, None)
            .leaves,
        1..2
    );
    assert_eq!(Selection::new(&Value::list([]), None).leaves, 0..0);
}

#[test]
fn coarser_changes_store_all_for_finer_rows() {
    let tree = Value::list([Value::list([leaf(), leaf()]), Value::list([leaf(), leaf()])]);
    let all = Selection::new(&tree, None);
    assert_eq!(all.state(), Value::list([ALL.into(), ALL.into()]));
    let first = all.select(1, 0..1, None);
    let restored = Selection::new(&tree, Some(&first.state())).select_all(1);
    assert_eq!(restored.leaves, 0..4);
    assert_eq!(restored.state(), all.state());
    let chosen = all.select(0, 1..3, None);
    let narrowed = chosen.select(1, 0..1, None);
    assert_eq!(narrowed.leaves, 0..2);
    assert_eq!(narrowed.intent[0], Intent::All);
    assert_eq!(
        Selection::new(&tree, Some(&narrowed.state()))
            .select_all(1)
            .state(),
        all.state()
    );
    let (widgets, _) = cursor::<crate::Editor>(
        &tree,
        Some(&cursor_state(&narrowed, 1.5)),
        0.0,
        TREE_CURSOR,
        200.0,
        None,
    );
    assert_eq!(widgets.len(), 3);
    assert_eq!(narrowed.intent[0], Intent::All);
}

#[test]
fn stored_paths_track_insertions_and_survive_missing_endpoints() {
    let original = Value::list([leaf(), leaf(), leaf(), leaf()]);
    let positions: Vec<_> = original.as_list().unwrap().keys().cloned().collect();
    let all = Selection::new(&original, None);
    let chosen = all.select(0, 1..3, None);
    let full_span = all.select(0, 0..4, None);
    let mut edited = original.as_list().unwrap().clone();
    edited.set(
        &gid::position::between(None, positions.first()).unwrap(),
        leaf(),
    );
    edited.set(
        &gid::position::between(Some(&positions[1]), Some(&positions[2])).unwrap(),
        leaf(),
    );
    edited.set(
        &gid::position::between(positions.last(), None).unwrap(),
        leaf(),
    );
    let edited_value = Value::List(edited.clone());
    assert_eq!(
        Selection::new(&edited_value, Some(&chosen.state())).leaves,
        2..5
    );
    assert_eq!(
        Selection::new(&edited_value, Some(&full_span.state())).leaves,
        1..6
    );
    assert_eq!(
        Selection::new(&edited_value, Some(&all.state())).leaves,
        0..7
    );
    let stored_cursor = cursor_state(&all, 2.25);
    let (_, result) = cursor::<crate::Editor>(
        &edited_value,
        Some(&stored_cursor),
        0.0,
        TREE_CURSOR,
        200.0,
        None,
    );
    assert_eq!(
        result
            .as_record()
            .unwrap()
            .get(&POSITION)
            .and_then(f64::read),
        Some(4.25)
    );
    edited.set(&positions[1], None);
    edited.set(&positions[2], None);
    let survivors = Selection::new(&Value::List(edited), Some(&chosen.state()));
    assert_eq!(survivors.leaves, 2..3);
    assert_eq!(survivors.state(), chosen.state());
}

#[test]
fn missing_ranges_and_hidden_rows_do_not_destroy_state() {
    let tree = Value::list([Value::list([]), leaf(), leaf()]);
    let chosen = Selection::new(&tree, None).select(0, 1..2, None);
    let position = tree.as_list().unwrap().keys().last().unwrap();
    let edited = Value::List(tree.as_list().unwrap().with(&position, None));
    let stale = Selection::new(&edited, Some(&chosen.state()));
    assert_eq!(stale.leaves, 0..1);
    assert_eq!(stale.state(), chosen.state());
    assert_eq!(Selection::new(&tree, Some(&stale.state())).leaves, 1..2);
    let empty = Selection::new(&Value::list([]), Some(&chosen.state()));
    assert_eq!(empty.state(), chosen.state());
    assert!(empty.rows.is_empty());
    for malformed in [Value::record([]), Value::list([f64::value(5.0)])] {
        assert_eq!(Selection::new(&tree, Some(&malformed)).leaves, 0..2);
    }
}

#[test]
fn cursor_keeps_its_leaf_position_only_inside_the_selected_range() {
    let tree = Value::list([Value::list([leaf(), leaf()]), Value::list([leaf(), leaf()])]);
    let all = Selection::new(&tree, None);
    let first = all.select(1, 0..1, None);
    let second = all.select(1, 1..2, None);
    assert_eq!(first.position(1.25), 1.25);
    assert_eq!(second.position(1.25), 2.0);
    let (widgets, value) = cursor::<crate::Editor>(
        &tree,
        Some(&cursor_state(&second, 2.75)),
        0.0,
        TREE_CURSOR,
        200.0,
        None,
    );
    assert_eq!(widgets.len(), 3);
    let value = value.as_record().unwrap();
    assert_eq!(value.get(&RANGE), Some(&encode(2..4)));
    assert_eq!(value.get(&POSITION).and_then(f64::read), Some(2.75));
    let (widgets, _) =
        cursor::<crate::Editor>(&Value::list([]), None, 0.0, TREE_CURSOR, 200.0, None);
    assert!(widgets.is_empty());
}
