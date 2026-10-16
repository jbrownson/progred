use super::*;

#[test]
fn fields_compose_at_a_path_and_prune_when_cleared() {
    let other = CellId::from_u128(7);
    let mut annotations = Annotations::default();
    let path = [Step::Follow(gid::Resolution::Document)];
    set_collapsed(&mut annotations, &path, false, true);
    annotations.set_field(&path, other, Some(Value::from(vec![9u8])));
    assert!(collapsed(&annotations, &path, false));
    assert!(annotations.field(&path, other).is_some());

    set_collapsed(&mut annotations, &path, false, false);
    assert!(!collapsed(&annotations, &path, false));
    assert!(annotations.field(&path, other).is_some());

    annotations.set_field(&path, other, None);
    assert!(annotations.at(&path).is_none());
}

#[test]
fn absent_overrides_fall_to_the_cycle_default() {
    let annotations = Annotations::default();
    assert!(collapsed(
        &annotations,
        &[Step::Follow(gid::Resolution::Document)],
        true
    ));
    assert!(!collapsed(
        &annotations,
        &[Step::Follow(gid::Resolution::Document)],
        false
    ));
}
