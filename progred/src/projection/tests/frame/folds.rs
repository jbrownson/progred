//! Folding through the projected occurrence's own key handling, with the
//! default the projection supplies.

use super::*;

fn view_annotations(world: &crate::Editor) -> Annotations {
    world
        .model
        .workspace
        .view(world.model.workspace.document_root())
        .unwrap()
        .annotations
        .clone()
}

fn frame(world: &mut crate::Editor) -> placed::HoverOutput<crate::Editor> {
    let annotations = view_annotations(world);
    editing_frame_with_annotations(world, false, None, None, &annotations)
}

/// Select the occurrence at `path` and press `key` there, returning the
/// resulting fold annotations.
fn press(world: &mut crate::Editor, path: &[Step], key: KeyboardEvent) -> Annotations {
    let selected = frame(world);
    let occurrence = selected
        .descends
        .iter()
        .find(|landmark| landmark.path.as_ref() == path)
        .expect("projected occurrence");
    assert!((occurrence.select)(world, None));
    frame(world)
        .resolve_for_dispatch()
        .dispatch_key(world, &key);
    view_annotations(world)
}

fn space() -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Character(" ".into()),
        state: KeyState::Down,
        ..Default::default()
    }
}

fn command(key: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        modifiers: if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        },
        ..Default::default()
    }
}

fn world_of(doc: &Document) -> crate::Editor {
    editing_world(doc, &core_libraries())
}

#[test]
fn cycles_collapse_by_default_and_expand_turn_by_turn() {
    // A: { next: A } — the re-entry at [Follow, next] repeats the
    // root value.
    let a = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        a,
        Value::record([(crate::test_values::label("next"), Value::from(a))]),
    );
    let mut world = world_of(&Document {
        root: Some(Value::from(a)),
        cells,
    });
    let reentry = vec![Step::Follow(gid::Resolution::Document), key("next")];
    // Space expands the default-collapsed re-entry.
    let folds = press(&mut world, &reentry, space());
    assert!(!crate::annotations::collapsed(&folds, &reentry, true));
    // The next turn defaults collapsed at its own deeper path and
    // expands the same way — follow the cycle as far as wanted.
    let deeper: Vec<Step> = reentry.iter().chain(reentry.iter()).cloned().collect();
    assert!(crate::annotations::collapsed(&folds, &deeper, true));
    let folds = press(&mut world, &deeper, space());
    assert!(!crate::annotations::collapsed(&folds, &deeper, true));
    // Toggling back restores the default (the override is sparse).
    let folds = press(&mut world, &deeper, space());
    assert!(crate::annotations::collapsed(&folds, &deeper, true));
    assert!(folds.at(&deeper).is_none());
}

#[test]
fn any_valued_cell_and_any_container_collapse() {
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("kind"),
        crate::test_values::text("building"),
    )]);
    let mut world = world_of(&doc);
    // A plain (non-cycle) cell collapses to ( … ) via the same toggle.
    let folds = press(&mut world, &[], space());
    assert!(crate::annotations::collapsed(&folds, &[], false));
    // Its record collapses too — layout never enters into it, so inline
    // literals toggle exactly like block forms.
    let record = [Step::Follow(gid::Resolution::Document)];
    press(&mut world, &[], space());
    let folds = press(&mut world, &record, space());
    assert!(crate::annotations::collapsed(&folds, &record, false));
}

#[test]
fn fold_keys_are_directional_and_stay_sparse() {
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("a"),
        crate::test_values::text("1"),
    )]);
    let mut world = world_of(&doc);
    for (key, closed) in [
        (NamedKey::ArrowUp, true),
        (NamedKey::ArrowUp, true),
        (NamedKey::ArrowDown, false),
        (NamedKey::ArrowDown, false),
    ] {
        let folds = press(&mut world, &[], command(key));
        assert_eq!(crate::annotations::collapsed(&folds, &[], false), closed);
    }
    // Matching the default stores nothing.
    assert!(view_annotations(&world).at(&[]).is_none());
    // A leaf has nothing to fold.
    let leaf = vec![Step::Follow(gid::Resolution::Document), key("a")];
    let folds = press(&mut world, &leaf, command(NamedKey::ArrowUp));
    assert!(folds.at(&leaf).is_none());
}

#[test]
fn a_folded_named_cell_keeps_its_name_unless_raw() {
    let node = new_cell_id();
    let next = crate::test_values::label("next");
    let mut cells = Cells::new();
    cells.set_value(node, name::record("node", [(next, Value::from(node))]));
    let mut world = world_of(&Document {
        root: Some(Value::from(node)),
        cells,
    });
    let follow = Step::Follow(gid::Resolution::Document);
    let reentry = vec![follow.clone(), Step::Key(next)];
    let label = [&reentry[..], &[follow, Step::Key(name::vocabulary::NAME)]].concat();
    let inside = [&reentry[..], &reentry[..]].concat();
    let shown = |world: &mut crate::Editor, raw| {
        editing_frame(world, raw)
            .descends
            .iter()
            .map(|d| d.path.to_vec())
            .collect::<Vec<_>>()
    };
    let named = shown(&mut world, false);
    assert!(named.contains(&label));
    assert!(!named.contains(&inside));
    assert!(!shown(&mut world, true).contains(&label));
}
