use super::*;

/// One convention field at a path.
fn field<'a>(annotations: &'a Annotations, path: &[Step], key: CellId) -> Option<&'a Value> {
    annotations.at(path)?.as_record()?.get(&key)
}

#[test]
fn fields_compose_at_a_path_and_prune_when_cleared() {
    let other = CellId::from_u128(7);
    let mut annotations = Annotations::default();
    let path = [Step::Follow(gid::Resolution::Document)];
    set_folded(&mut annotations, &path, false, true);
    annotations.set_field(&path, other, Some(Value::from(vec![9u8])));
    assert!(folded(annotations.at(&path), false));
    assert!(field(&annotations, &path, other).is_some());

    set_folded(&mut annotations, &path, false, false);
    assert!(!folded(annotations.at(&path), false));
    assert!(field(&annotations, &path, other).is_some());

    annotations.set_field(&path, other, None);
    assert!(annotations.at(&path).is_none());
}

#[test]
fn absent_overrides_fall_to_the_cycle_default() {
    let annotations = Annotations::default();
    assert!(folded(
        annotations.at(&[Step::Follow(gid::Resolution::Document)]),
        true
    ));
    assert!(!folded(
        annotations.at(&[Step::Follow(gid::Resolution::Document)]),
        false
    ));
}

#[test]
fn annotations_move_under_a_prefix_with_their_values() {
    let (field, value) = (CellId::from_u128(1), Value::record([]));
    let [outer, inner] = [CellId::from_u128(2), CellId::from_u128(3)];
    let mut annotations = Annotations::default();
    annotations.set_field(&[], field, Some(value.clone()));
    annotations.set_field(&[Step::Key(inner)], field, Some(value.clone()));
    let moved = annotations.under(&[Step::Key(outer)]);
    let record = Value::record([(field, value)]);
    assert_eq!(moved.at(&[Step::Key(outer)]), Some(&record));
    assert_eq!(
        moved.at(&[Step::Key(outer), Step::Key(inner)]),
        Some(&record)
    );
    assert_eq!(moved.at(&[]), None);
    assert_eq!(moved.at(&[Step::Key(inner)]), None);
}
