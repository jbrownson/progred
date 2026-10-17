use super::*;
use crate::placed::ViewRegion;
use crate::test_support::arrow;
use crate::test_values::key;
use std::rc::Rc;
use ui_events::keyboard::{KeyState, Modifiers};

#[test]
fn reveal_moves_a_partly_visible_target_fully_into_view() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 180.0, 220.0, 30.0, 200.0, 12.0, 1.0),
        152.0
    );
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 10.0, 50.0, 30.0, 200.0, 12.0, 1.0),
        88.0
    );
}

#[test]
fn reveal_leaves_fully_visible_targets_alone_even_inside_the_margin() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 30.0, 200.0, 30.0, 200.0, 12.0, 1.0),
        120.0
    );
}

#[test]
fn reveal_aligns_an_oversized_target_at_its_leading_edge() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 80.0, 500.0, 30.0, 200.0, 12.0, 1.0),
        158.0
    );
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 42.0, 462.0, 30.0, 200.0, 12.0, 1.0),
        120.0
    );
}

#[test]
fn a_target_starting_beyond_the_viewport_is_revealed() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, 220.0, 260.0, 30.0, 200.0, 12.0, 1.0),
        192.0
    );
}

#[test]
fn reveal_clamps_an_offset_left_stale_by_a_resize() {
    assert_eq!(
        reveal_axis(300.0, 100.0, 80.0, 120.0, 30.0, 200.0, 12.0, 1.0),
        100.0
    );
}

#[test]
fn reveal_uses_the_requested_view_and_converts_pixels_to_scroll_units() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    let root = editor.model.workspace.document_root().clone();
    let other = Root::document();
    let descends = [
        Descend {
            scope: Default::default(),
            root: Some(other.clone()),
            path: Rc::from([]),
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            select: Rc::new(|_, _| false),
            secondary: None,
            selected: false,
            tab_stop: false,
        },
        Descend {
            scope: Default::default(),
            root: Some(root.clone()),
            path: Rc::from([]),
            rect: Rect::new(0.0, 1_000.0, 20.0, 1_060.0),
            select: Rc::new(|_, _| false),
            secondary: None,
            selected: false,
            tab_stop: false,
        },
    ];
    let regions = [ViewRegion {
        root: root.clone(),
        rect: Rect::new(0.0, 0.0, 400.0, 200.0),
        maximum: Vec2::new(0.0, 1_000.0),
        content: None,
    }];
    let geometry = Geometry {
        descends: &descends,
        view_regions: &regions,
        scale: 2.0,
    };
    geometry.reveal_path(&mut editor, &other, &[]);
    assert_eq!(editor.model.workspace.document.scroll, Vec2::ZERO);
    geometry.reveal_path(&mut editor, &root, &[]);
    assert_eq!(
        editor.model.workspace.document.scroll,
        Vec2::new(0.0, 442.0)
    );
}

fn stop(path: Vec<Step>, x0: f64, y0: f64, x1: f64, y1: f64) -> Descend<()> {
    Descend {
        scope: Default::default(),
        root: None,
        path: Rc::from(path),
        rect: Rect::new(x0, y0, x1, y1),
        select: Rc::new(|_, _| true),
        secondary: None,
        selected: false,
        tab_stop: false,
    }
}

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
        modifiers: crate::modifiers::command(),
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

mod frames;
mod logical;
mod slots;
