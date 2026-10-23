use super::*;
use crate::location::Location;
use gid::{Resolution, new_cell_id};

#[test]
fn a_trail_holds_the_path_it_was_built_along() {
    let [field, other] = [new_cell_id(), new_cell_id()];
    let parent = Trail::root(crate::test_document()).push(Step::Key(field));
    let child = parent.below(&[gid::Step::Follow(Resolution::Document)]);
    let path = vec![Step::Key(field), Step::Follow(Resolution::Document)];
    assert_eq!(child.to_vec(), path);
    assert_eq!(child, path);
    assert_eq!(child, Trail::of(&path));
    assert_eq!(child.len(), 2);
    assert_eq!(child.last(), Some(&Step::Follow(Resolution::Document)));
    assert_eq!(child.parent(), Some(&parent));
    assert!(parent.leads(&path));
    assert!(!child.leads(&path[..1]));
    // A sibling differs in its last step, and a cousin further up.
    assert_ne!(parent.push(Step::Key(other)), child);
    assert_ne!(
        Trail::root(crate::test_document())
            .push(Step::Key(other))
            .push(Step::Follow(Resolution::Document)),
        child
    );
    assert!(
        Trail::root(crate::test_document()).is_empty()
            && Trail::root(crate::test_document()) == Vec::<Step>::new()
    );
}

#[test]
fn a_trail_knows_where_its_jumps_and_computed_values_are() {
    let [field, cell] = [new_cell_id(), new_cell_id()];
    let data = Trail::root(crate::test_document()).push(Step::Key(field));
    assert!(data.data_from(0) && !data.computed());
    let followed = data.push(Step::Follow(Resolution::Document));
    let inside = followed.push(Step::Key(field));
    assert!(inside.data_from(0));
    let result = inside.push(Step::Computed).push(Step::Key(field));
    assert!(result.computed() && !result.data_from(2) && result.data_from(4));
    let jumped = result
        .push(Step::jump(Location::in_cell(
            cell,
            Resolution::Document,
            vec![],
        )))
        .push(Step::Key(field));
    assert!(!jumped.computed() && jumped.data_from(6) && !jumped.data_from(5));
    // It finds its location as the whole path does.
    for trail in [&data, &followed, &inside, &result, &jumped] {
        assert_eq!(trail.location(), super::super::location(&trail.to_vec()));
    }
}

#[test]
fn steps_after_a_start_compare_across_trails() {
    let [a, b, field] = [new_cell_id(), new_cell_id(), new_cell_id()];
    let below = |cell| {
        Trail::root(crate::test_document())
            .push(Step::Key(cell))
            .push(Step::Follow(Resolution::Document))
            .push(Step::Key(field))
    };
    assert!(below(a).same_after(2, &below(b), 2));
    assert!(!below(a).same_after(0, &below(b), 0));
    assert!(!below(a).same_after(2, &below(b), 1));
}
