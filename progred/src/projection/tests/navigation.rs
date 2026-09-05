use super::*;

#[test]
fn arrows_walk_rows_down_and_lines_across() {
    // A block record: field `a` hugs a flat record on line one,
    // field `b` drops a two-row block, field `e` closes. Settled
    // in placement order — children before parents.
    let a = || vec![key("a")];
    let a1 = || vec![key("a"), key("p")];
    let a2 = || vec![key("a"), key("q")];
    let b = || vec![key("b")];
    let c = || vec![key("b"), key("c")];
    let d = || vec![key("b"), key("d")];
    let e = || vec![key("e")];
    let ds = vec![
        stop(a1(), 60.0, 2.0, 80.0, 18.0),
        stop(a2(), 100.0, 2.0, 120.0, 18.0),
        stop(a(), 40.0, 2.0, 140.0, 18.0),
        stop(c(), 60.0, 24.0, 80.0, 40.0),
        stop(d(), 60.0, 44.0, 80.0, 60.0),
        stop(b(), 20.0, 22.0, 280.0, 62.0),
        stop(e(), 40.0, 66.0, 80.0, 82.0),
        stop(vec![], 0.0, 0.0, 300.0, 84.0),
    ];
    // Nothing selected: any arrow lands on the root.
    assert_eq!(stepped(&ds, None, NamedKey::ArrowDown), Some(vec![]));
    // Down walks every row in reading order, entering the open
    // block; up reverses it exactly.
    let rows = [vec![], a(), b(), c(), d(), e()];
    for pair in rows.windows(2) {
        let (above, below) = (&pair[0], &pair[1]);
        assert_eq!(
            stepped(&ds, Some(above.clone()), NamedKey::ArrowDown),
            Some(below.clone())
        );
        assert_eq!(
            stepped(&ds, Some(below.clone()), NamedKey::ArrowUp),
            Some(above.clone())
        );
    }
    assert_eq!(stepped(&ds, Some(e()), NamedKey::ArrowDown), None);
    assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowUp), None);
    // Right walks the hugged line's content; the walk ends with
    // the line, and left retraces it back out to the row.
    assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowRight), Some(a1()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowRight), Some(a2()));
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowRight), None);
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowLeft), Some(a1()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowLeft), Some(a()));
    // Left from a row widens to the parent; the root has none.
    assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowLeft), Some(vec![]));
    assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowLeft), None);
    // Mid-line, down exits to the next row and up collects to the
    // line's own stop.
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowDown), Some(b()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowUp), Some(a()));
    // A dropped block is entered by down, never right.
    assert_eq!(stepped(&ds, Some(b()), NamedKey::ArrowRight), None);
}

#[test]
fn command_a_selects_the_current_views_root() {
    let document = crate::workspace::Root::document();
    let pane_path = vec![key("pane")];
    let pane = crate::workspace::Root::pane(pane_path.clone());
    let other_pane = crate::workspace::Root::pane(pane_path.clone());
    let child_path = vec![key("pane"), key("child")];
    let doc = Document {
        root: Some(Value::record([(
            crate::test_values::label("pane"),
            Value::record([(crate::test_values::label("child"), Value::record([]))]),
        )])),
        cells: Cells::new(),
    };
    let libraries = Libraries::default();
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
        let selection = Selection::edge(root, &src(&doc, &libraries), child_path.clone());
        for selection in [None, Some(&selection)] {
            let target = step_selection(&descends, Some(root), selection, LINE, &event)
                .expect("Select All reaches the view root");
            assert_eq!(target.root.as_ref(), Some(root));
            assert_eq!(target.path.as_ref(), path);
        }
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
        assert!(step_selection(&descends, Some(&document), None, LINE, &event).is_none());
    }
}

#[test]
fn navigation_declines_modified_keys_releases_and_other_keys() {
    let ds = vec![
        stop(vec![key("a")], 0.0, 2.0, 60.0, 18.0),
        stop(vec![], 0.0, 0.0, 300.0, 40.0),
    ];
    let shifted = KeyboardEvent {
        modifiers: Modifiers::SHIFT,
        ..arrow(NamedKey::ArrowDown)
    };
    assert!(step_selection(&ds, None, None, LINE, &shifted).is_none());
    let released = KeyboardEvent {
        state: KeyState::Up,
        ..arrow(NamedKey::ArrowDown)
    };
    assert!(step_selection(&ds, None, None, LINE, &released).is_none());
    assert!(step_selection(&ds, None, None, LINE, &arrow(NamedKey::Escape)).is_none());
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
