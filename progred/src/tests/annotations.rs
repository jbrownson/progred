use super::*;

/// One convention field at a path.
fn field<'a>(annotations: &'a Annotations, path: &[Step], key: CellId) -> Option<&'a Value> {
    annotations.at(path)?.as_record()?.get(&key)
}

#[test]
fn fields_compose_at_a_path_and_prune_when_cleared() {
    let other = CellId::from_u128(7);
    let mut annotations = Annotations::default();
    let path = [Step::Follow];
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
    assert!(folded(annotations.at(&[Step::Follow]), true));
    assert!(!folded(annotations.at(&[Step::Follow]), false));
}
