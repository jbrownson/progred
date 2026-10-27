use super::*;

#[test]
fn keys_read_as_the_tool_writes_them() {
    assert_eq!(
        key_combination("Return"),
        Some((Key::Named(NamedKey::Enter), Modifiers::empty()))
    );
    assert_eq!(
        key_combination("ctrl+z"),
        Some((Key::Character("z".into()), Modifiers::CONTROL))
    );
    assert_eq!(
        key_combination("ctrl+shift+Tab"),
        Some((
            Key::Named(NamedKey::Tab),
            Modifiers::CONTROL | Modifiers::SHIFT
        ))
    );
    assert_eq!(
        key_combination("super+Left"),
        Some((Key::Named(NamedKey::ArrowLeft), Modifiers::META))
    );
    assert_eq!(
        key_combination("space"),
        Some((Key::Character(" ".into()), Modifiers::empty()))
    );
    for unknown in ["", "hyper+a", "Return2", "ctrl+"] {
        assert_eq!(key_combination(unknown), None, "{unknown:?}");
    }
}

#[test]
fn modifiers_and_directions_read_as_named() {
    assert_eq!(held_modifiers(""), Some(ModifiersState::empty()));
    assert_eq!(
        held_modifiers("ctrl+shift"),
        Some(ModifiersState::CONTROL | ModifiersState::SHIFT)
    );
    assert_eq!(held_modifiers("ctrl+hyper"), None);
    assert_eq!(direction("down"), Some(Vec2::new(0.0, -1.0)));
    assert_eq!(direction("sideways"), None);
}

#[test]
fn coordinates_are_two_finite_numbers() {
    let number = crate::libraries::f64::value;
    assert_eq!(
        coordinate(&Value::list([number(3.0), number(4.5)])),
        Some(Point::new(3.0, 4.5))
    );
    for invalid in [
        Value::list([number(3.0)]),
        Value::list([number(3.0), number(f64::NAN)]),
        Value::list([number(3.0), text::value("4")]),
        number(3.0),
    ] {
        assert_eq!(coordinate(&invalid), None);
    }
}
