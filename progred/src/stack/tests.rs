use super::*;

#[test]
fn selected_libraries_supply_only_their_own_definitions() {
    let ids = [name::ID, text::ID, blob::ID, number::ID, f64::ID];
    let stack = load_selected(&ids).unwrap();
    assert_eq!(
        stack.libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        ids
    );
    assert!(stack.libraries.resolve(f64::vocabulary::F64).is_some());
    assert!(stack.libraries.resolve(f32::vocabulary::F32).is_none());
    for id in [fidget::ID, toolpath::ID, control::ID, grap_library::ID] {
        assert!(stack.libraries.resolve(id).is_none());
    }
}

#[test]
fn explicit_order_and_empty_selection_are_preserved_and_unknown_ids_fail() {
    let stack = load_selected(&[text::ID, name::ID, text::ID]).unwrap();
    assert_eq!(
        stack.libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        [text::ID, name::ID]
    );
    assert_eq!(load_selected(&[]).unwrap().libraries.iter().count(), 0);
    assert!(load_selected(&[gid::new_cell_id()]).is_err());
    assert_eq!(load().libraries.iter().count(), BUILT_INS.len());
}

#[test]
fn only_libraries_that_bring_a_projection_offer_one() {
    let stack = load_selected(&[name::ID, text::ID, random::ID, color::ID]).unwrap();
    assert_eq!(
        stack.projections().collect::<Vec<_>>(),
        [
            (text::ID, "text"),
            (color::ID, "color"),
            (workspace::vocabulary::LIBRARIES, "document libraries")
        ]
    );
}
