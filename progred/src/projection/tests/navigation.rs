use super::*;

#[test]
fn navigation_direction_accepts_modifiers_but_excludes_other_keys_and_releases() {
    use crate::navigate::{Direction, direction};

    for (key, expected) in [
        (NamedKey::ArrowLeft, Direction::Left),
        (NamedKey::ArrowRight, Direction::Right),
        (NamedKey::ArrowUp, Direction::Up),
        (NamedKey::ArrowDown, Direction::Down),
    ] {
        let event = arrow(key);
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
        direction(&KeyboardEvent {
            key: Key::Character("a".into()),
            modifiers: Modifiers::META,
            ..arrow(NamedKey::ArrowDown)
        }),
        None
    );
}

#[test]
fn command_a_selects_the_current_views_root() {
    let document = crate::test_root();
    let pane_path = vec![key("pane")];
    let pane = crate::workspace::Root::pane(pane_path.clone());
    let other_pane = crate::workspace::Root::pane(pane_path.clone());
    let child_path = vec![key("pane"), key("child")];
    let descends: Vec<_> = [
        (&document, child_path.clone()),
        (&document, pane_path.clone()),
        (&document, vec![]),
        (&other_pane, pane_path.clone()),
        (&pane, child_path.clone()),
        (&pane, pane_path.clone()),
    ]
    .into_iter()
    .map(|(root, path)| Descend {
        scope: Default::default(),
        root: Some(root.clone()),
        ..stop(path, 0.0, 0.0, 100.0, 20.0)
    })
    .collect();
    let event = KeyboardEvent {
        key: Key::Character("a".into()),
        modifiers: if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        },
        ..arrow(NamedKey::ArrowDown)
    };
    for (root, path) in [(&document, vec![]), (&pane, pane_path)] {
        let target = select_all(crate::modifiers::native(), &descends, Some(root), &event)
            .expect("Select All reaches the view root");
        assert_eq!(target.root.as_ref(), Some(root));
        assert_eq!(target.path.as_ref(), path);
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
        assert!(
            select_all(
                crate::modifiers::native(),
                &descends,
                Some(&document),
                &event
            )
            .is_none()
        );
    }
}

#[test]
fn set_collapse_is_directional_and_stays_sparse() {
    let lib = core_libraries();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("a"),
        crate::test_values::text("1"),
    )]);
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    assert!(set_fold(&sources, &mut collapse, &[], true));
    assert!(!set_fold(&sources, &mut collapse, &[], true));
    assert!(set_fold(&sources, &mut collapse, &[], false));
    assert!(!set_fold(&sources, &mut collapse, &[], false));
    // Matching the default stores nothing.
    assert!(collapse.at(&[]).is_none());
    // A leaf has nothing to fold.
    let leaf = vec![Step::Follow(gid::Resolution::Document), key("a")];
    assert!(!set_fold(&sources, &mut collapse, &leaf, true));
}
