//! Folding through the projected occurrence's own key handling, with the
//! default the projection supplies.

use super::*;
use crate::driver::Driver;
use ui_events::keyboard::Modifiers;

fn view_annotations(world: &crate::Editor) -> Annotations {
    world
        .model
        .workspace
        .view(world.model.workspace.document_root())
        .unwrap()
        .annotations
        .clone()
}

/// Select the occurrence at `path` and press `key` there with `modifiers`,
/// returning the resulting fold annotations.
fn press(driver: &mut Driver, path: &[Step], key: Key, modifiers: Modifiers) -> Annotations {
    driver.select(path);
    driver.keystroke(key, modifiers);
    view_annotations(driver.editor())
}

fn space(driver: &mut Driver, path: &[Step]) -> Annotations {
    press(driver, path, Key::Character(" ".into()), Modifiers::empty())
}

fn driver_of(doc: &Document) -> Driver {
    Driver::new(crate::test_editor((doc).clone()), Driver::VIEWPORT)
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
    let mut driver = driver_of(&Document {
        root: Some(Value::from(a)),
        cells,
    });
    let reentry = vec![Step::Follow(gid::Resolution::Document), key("next")];
    // Space expands the default-collapsed re-entry.
    let folds = space(&mut driver, &reentry);
    assert!(!crate::annotations::folded(folds.at(&reentry), true));
    // The next turn defaults collapsed at its own deeper path and
    // expands the same way — follow the cycle as far as wanted.
    let deeper: Vec<Step> = reentry.iter().chain(reentry.iter()).cloned().collect();
    assert!(crate::annotations::folded(folds.at(&deeper), true));
    let folds = space(&mut driver, &deeper);
    assert!(!crate::annotations::folded(folds.at(&deeper), true));
    // Toggling back restores the default (the override is sparse).
    let folds = space(&mut driver, &deeper);
    assert!(crate::annotations::folded(folds.at(&deeper), true));
    assert!(folds.at(&deeper).is_none());
}

#[test]
fn any_valued_cell_and_any_container_collapse() {
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("kind"),
        crate::libraries::text::value("building"),
    )]);
    let mut driver = driver_of(&doc);
    // A plain (non-cycle) cell collapses to ( … ) via the same toggle.
    let folds = space(&mut driver, &[]);
    assert!(crate::annotations::folded(folds.at(&[]), false));
    // Its record collapses too — layout never enters into it, so inline
    // literals toggle exactly like block forms.
    let record = [Step::Follow(gid::Resolution::Document)];
    space(&mut driver, &[]);
    let folds = space(&mut driver, &record);
    assert!(crate::annotations::folded(folds.at(&record), false));
}

#[test]
fn fold_keys_are_directional_and_stay_sparse() {
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("a"),
        crate::libraries::text::value("1"),
    )]);
    let mut driver = driver_of(&doc);
    for (key, closed) in [
        (NamedKey::ArrowUp, true),
        (NamedKey::ArrowUp, true),
        (NamedKey::ArrowDown, false),
        (NamedKey::ArrowDown, false),
    ] {
        let folds = press(
            &mut driver,
            &[],
            Key::Named(key),
            crate::test_support::command(),
        );
        assert_eq!(crate::annotations::folded(folds.at(&[]), false), closed);
    }
    // Matching the default stores nothing.
    assert!(view_annotations(driver.editor()).at(&[]).is_none());
    // A leaf has nothing to fold.
    let leaf = vec![Step::Follow(gid::Resolution::Document), key("a")];
    let folds = press(
        &mut driver,
        &leaf,
        Key::Named(NamedKey::ArrowUp),
        crate::test_support::command(),
    );
    assert!(folds.at(&leaf).is_none());
}

#[test]
fn raw_opens_only_the_cells_it_reaches_directly() {
    let [shared, user, alias, uses, inner, first, second] = [(); 7].map(|_| new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(shared, Value::record([(inner, f64::value(1.0))]));
    cells.set_value(user, Value::record([(uses, Value::Cell(shared))]));
    cells.set_value(alias, Value::Cell(shared));
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (first, user.into()),
            (second, alias.into()),
        ])),
        cells,
    });
    let follow = Step::Follow(gid::Resolution::Document);
    world.model.workspace.document.projection = crate::workspace::Projection::Raw;
    let frame = editing_frame(&mut world);
    let drawn = |path: &[Step]| frame.landmarks.landmark(&path).is_some();
    // The cell the document reaches opens, and a reference inside it stays
    // folded, so a shared cell isn't expanded again at every use.
    assert!(drawn(&[Step::Key(first), follow.clone(), Step::Key(uses)]));
    assert!(!drawn(&[
        Step::Key(first),
        follow.clone(),
        Step::Key(uses),
        follow.clone(),
        Step::Key(inner),
    ]));
    // A cell that is only a reference to another opens through to it.
    assert!(drawn(&[
        Step::Key(second),
        follow.clone(),
        follow,
        Step::Key(inner),
    ]));
}
