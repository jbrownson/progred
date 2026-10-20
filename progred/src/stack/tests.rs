use super::*;

#[test]
fn selected_libraries_supply_only_their_own_definitions() {
    let ids = [name::ID, text::ID, f64::ID];
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
fn only_libraries_that_bring_a_projection_or_offers_can_be_switched_off() {
    let stack = load_selected(&[name::ID, text::ID, random::ID, color::ID, declared::ID]).unwrap();
    assert_eq!(
        stack.switchable().collect::<Vec<_>>(),
        [
            (name::ID, "name"),
            (text::ID, "text"),
            (color::ID, "color"),
            (declared::ID, "document libraries")
        ]
    );
}

#[test]
fn a_preset_is_read_from_what_an_area_shows() {
    use crate::workspace::Shown;
    let stack = load();
    for preset in Preset::ALL {
        assert_eq!(stack.preset(&preset.shown()), Some(preset));
    }
    // Every box unchecked by hand is Raw; one checked again is a set of
    // its own.
    let unchecked = Shown::AllBut(stack.switchable().map(|(library, _)| library).collect());
    assert_eq!(stack.preset(&unchecked), Some(Preset::Raw));
    assert_eq!(stack.preset(&Shown::Only(vec![text::ID])), None);
}

#[test]
fn a_library_switched_off_offers_nothing() {
    let stack = load_selected(&[name::ID, text::ID, color::ID]).unwrap();
    let offered = |provider: &crate::display::CompletionProvider| {
        provider(&crate::test_support::bare_request(
            crate::display::CompletionKind::Value,
            crate::display::CompletionScope::Everything,
            "",
        ))
        .unwrap_or_default()
        .iter()
        .any(|offer| offer.display == "new color".into())
    };
    assert!(offered(&stack.completions));
    assert!(offered(&stack.completions_without(&[])));
    assert!(!offered(&stack.completions_without(&[color::ID])));
}
