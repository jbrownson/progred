use super::*;

#[test]
fn selected_libraries_supply_only_their_own_definitions() {
    let ids = [name::ID, text::ID, f64::ID];
    let stack = load_selected(&ids).unwrap();
    assert_eq!(
        stack.libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        ids
    );
    assert!(crate::test_support::first_value(&stack.libraries, f64::vocabulary::F64).is_some());
    assert!(crate::test_support::first_value(&stack.libraries, f32::vocabulary::F32).is_none());
    for id in [fidget::ID, toolpath::ID, grap_library::ID] {
        assert!(crate::test_support::first_value(&stack.libraries, id).is_none());
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

type Drew = std::rc::Rc<std::cell::Cell<Option<&'static str>>>;

/// A projection drawing every value, noting `name` as what drew it.
fn drawing(drew: &Drew, name: &'static str) -> Partial {
    let drew = drew.clone();
    crate::display::runtime_partial(move |_| {
        drew.set(Some(name));
        Some(crate::display::dim(name))
    })
}

/// What drew `value` through `projection`: the name a drawing noted, else
/// "something else", or nothing where it declined.
fn drawn_by(projection: &Projection, drew: &Drew, value: &gid::Value) -> Option<&'static str> {
    drew.set(None);
    let value = grap::RuntimeValue::from(value);
    (projection.partial())(&crate::display::ProjectionInput {
        provided: projection.provided().clone(),
        ..crate::libraries::test_widgets::projection_input(
            &crate::display::test_support::NoEval,
            &value,
        )
    })
    .map(|_| drew.get().unwrap_or("something else"))
}

#[test]
fn a_record_draws_as_its_kind_provides_before_any_library_tries_it() {
    let drew = Drew::default();
    let (first, second, kind) = (gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id());
    let stack = compose([
        (
            first,
            Library::default().with_projection(drawing(&drew, "first library")),
        ),
        (
            second,
            Library::default().with_projections_for([(kind, drawing(&drew, "kind"))]),
        ),
    ]);
    let of_kind = grap::isa::record(kind, []);
    assert_eq!(drawn_by(&stack.projection, &drew, &of_kind), Some("kind"));
    // A kind providing nothing, or no kind said, is the libraries' to draw.
    for value in [
        grap::isa::record(gid::new_cell_id(), []),
        gid::Value::record([]),
    ] {
        assert_eq!(
            drawn_by(&stack.projection, &drew, &value),
            Some("first library")
        );
    }
    // Switched off, a library's cells provide nothing.
    let (without, _) = stack.without(&[second]);
    assert_eq!(drawn_by(&without, &drew, &of_kind), Some("first library"));
}

#[test]
fn a_call_draws_as_its_function_provides_else_as_a_call() {
    let drew = Drew::default();
    let (library, function) = (gid::new_cell_id(), gid::new_cell_id());
    let stack = compose([
        (grap_library::ID, grap_library::library()),
        (
            library,
            Library::default().with_projections_for([(function, drawing(&drew, "function"))]),
        ),
    ]);
    let call = |function: gid::CellId| grap::call(function.into(), []);
    assert_eq!(
        drawn_by(&stack.projection, &drew, &call(function)),
        Some("function")
    );
    assert_eq!(
        drawn_by(&stack.projection, &drew, &call(gid::new_cell_id())),
        Some("something else")
    );
}

#[test]
fn each_library_s_own_cell_holds_the_library() {
    use crate::library::vocabulary::{CELLS, kind::LIBRARY};
    let stack = load();
    let fields = crate::test_support::first_value(&stack.libraries, presentation::ID)
        .and_then(gid::Value::as_record)
        .unwrap();
    assert_eq!(::grap::isa::of(fields), Some(LIBRARY));
    assert_eq!(
        fields.get(&name::vocabulary::NAME).and_then(text::read),
        Some("presentation")
    );
    let cells: Vec<_> = fields
        .get(&CELLS)
        .and_then(gid::Value::as_list)
        .unwrap()
        .values()
        .map(|cell| cell.as_cell().unwrap())
        .collect();
    assert!(cells.contains(&presentation::vocabulary::kind::OUTLINE));
    assert!(!cells.contains(&presentation::ID));
    let (_, definitions) = stack
        .libraries
        .iter()
        .find(|(id, _)| *id == presentation::ID)
        .unwrap();
    let names: Vec<_> = cells
        .iter()
        .map(|cell| name::read(definitions.value(*cell).unwrap()).unwrap())
        .collect();
    assert!(names.is_sorted(), "{names:?}");
}
