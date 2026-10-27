use super::*;
use gid::new_cell_id;

#[test]
fn a_path_reads_from_its_last_jump_and_not_inside_a_computed_value() {
    let [field, cell, other] = [new_cell_id(), new_cell_id(), new_cell_id()];
    let inside = Location::in_cell(cell, Resolution::Document, vec![gid::Step::Key(other)]);
    assert_eq!(
        location(&[Step::Key(field)]),
        Some(Location::root(vec![gid::Step::Key(field)]))
    );
    let jumped = [
        Step::Key(field),
        Step::jump(Location::in_cell(cell, Resolution::Document, vec![])),
        Step::Key(other),
    ];
    assert_eq!(location(&jumped), Some(inside.clone()));
    let result = [Step::Key(field), Step::Computed, Step::Key(other)];
    assert_eq!(location(&result), None);
    assert!(Trail::of(&result).computed());
    // A cell followed inside a computed value starts its location again.
    let followed = [
        Step::Key(field),
        Step::Computed,
        Step::jump(Location::in_cell(cell, Resolution::Document, vec![])),
        Step::Key(other),
    ];
    assert_eq!(location(&followed), Some(inside));
    assert!(!Trail::of(&followed).computed());
}

#[test]
fn occurrence_paths_round_trip_as_data() {
    let [field, cell] = [new_cell_id(), new_cell_id()];
    let path = vec![
        Step::Key(field),
        Step::Follow(Resolution::Document),
        Step::jump(Location::in_cell(
            cell,
            Resolution::Document,
            vec![gid::Step::Key(field)],
        )),
        Step::Computed,
        Step::Element(gid::position::spread(1).remove(0)),
    ];
    assert_eq!(from_value(&value(&path)), Some(path));
    // A document path reads as the occurrence path of the same steps.
    let document = vec![gid::Step::Key(field)];
    assert_eq!(
        from_value(&::grap::path::value(&document)),
        Some(of(&document))
    );
}
