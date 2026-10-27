use super::*;
use crate::occurrence::Step;
use gid::new_cell_id;

#[test]
fn secondaries_in_a_cell_compare_their_paths_from_where_each_enters_it() {
    let cell = new_cell_id();
    let secondary = |path: Vec<Step>, relative_from| Secondary::InCell {
        cell,
        path: path.into(),
        relative_from,
    };
    let step = Step::Key(new_cell_id());
    assert_eq!(
        secondary(vec![step.clone()], 0),
        secondary(vec![Step::Key(new_cell_id()), step.clone()], 1)
    );
    assert_ne!(
        secondary(vec![step.clone()], 2),
        secondary(vec![step], 1),
        "a relative start past the path is not an empty one"
    );
}
