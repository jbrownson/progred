use super::*;
use crate::test_support::key_down;
use crate::test_values::key;
use kurbo::Rect;
use std::rc::Rc;
use ui_events::keyboard::{KeyState, Modifiers};

fn stop(path: Vec<Step>, x0: f64, y0: f64, x1: f64, y1: f64) -> Landmark<()> {
    Landmark {
        root: None,
        path: path.into(),
        rect: Rect::new(x0, y0, x1, y1),
        select: Rc::new(|_, _| true),
        secondary: None,
        selected: false,
        tab_stop: false,
        required: false,
    }
}

#[test]
fn navigation_direction_accepts_modifiers_but_excludes_other_keys_and_releases() {
    use crate::navigate::direction;
    use puri::handler::NavigationDirection;

    for (key, expected) in [
        (NamedKey::ArrowLeft, NavigationDirection::Left),
        (NamedKey::ArrowRight, NavigationDirection::Right),
        (NamedKey::ArrowUp, NavigationDirection::Up),
        (NamedKey::ArrowDown, NavigationDirection::Down),
    ] {
        let event = key_down(key, Modifiers::empty());
        assert_eq!(direction(&event), Some(expected));
        assert_eq!(
            direction(&KeyboardEvent {
                state: KeyState::Up,
                ..event.clone()
            }),
            None
        );
        for modifiers in [
            Modifiers::SHIFT,
            Modifiers::ALT,
            Modifiers::CONTROL,
            Modifiers::META,
            Modifiers::SHIFT | Modifiers::ALT | Modifiers::CONTROL | Modifiers::META,
        ] {
            assert_eq!(
                direction(&KeyboardEvent {
                    modifiers,
                    ..event.clone()
                }),
                Some(expected)
            );
        }
    }
    assert_eq!(
        direction(&key_down(Key::Character("a".into()), Modifiers::META)),
        None
    );
}

#[test]
fn command_a_selects_the_current_views_root() {
    let document = crate::test_root();
    let pane_path = vec![key("pane")];
    let pane = crate::display::widget::view::Root::pane(pane_path.clone());
    let other_pane = crate::display::widget::view::Root::pane(pane_path.clone());
    let child_path = vec![key("pane"), key("child")];
    let landmarks: Vec<_> = [
        (&document, child_path.clone()),
        (&document, pane_path.clone()),
        (&document, vec![]),
        (&other_pane, pane_path.clone()),
        (&pane, child_path.clone()),
        (&pane, pane_path.clone()),
    ]
    .into_iter()
    .map(|(root, path)| Landmark {
        root: Some(root.clone()),
        ..stop(crate::occurrence::of(&path), 0.0, 0.0, 100.0, 20.0)
    })
    .collect();
    let event = key_down(Key::Character("a".into()), crate::test_support::command());
    for (root, path) in [(&document, vec![]), (&pane, pane_path)] {
        let target = select_all(crate::modifiers::native(), &landmarks, root, &event)
            .expect("Select All reaches the view root");
        assert_eq!(target.root.as_ref(), Some(root));
        assert_eq!(target.path, path);
    }
    for event in [
        KeyboardEvent {
            state: KeyState::Up,
            ..event.clone()
        },
        KeyboardEvent {
            modifiers: Modifiers::empty(),
            ..event.clone()
        },
        KeyboardEvent {
            modifiers: event.modifiers | Modifiers::SHIFT,
            ..event.clone()
        },
        KeyboardEvent {
            modifiers: event.modifiers | Modifiers::ALT,
            ..event.clone()
        },
    ] {
        assert!(select_all(crate::modifiers::native(), &landmarks, &document, &event).is_none());
    }
}

mod dead_ends;
mod frames;
mod logical;
mod own_marks;
mod slots;
