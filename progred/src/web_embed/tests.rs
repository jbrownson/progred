use super::*;
use crate::selection;
use gid::{Step, Value};

#[test]
fn content_height_waits_for_a_measured_window() {
    let region = |rect| crate::display::widget::frame::ViewRegion {
        root: Root::document(),
        rect,
        maximum: kurbo::Vec2::ZERO,
        content: Some(kurbo::Vec2::new(300.0, 120.5)),
    };
    assert_eq!(content_height(&[region(kurbo::Rect::ZERO)], 2.0), None);
    assert_eq!(
        content_height(&[region(kurbo::Rect::new(0.0, 40.0, 600.0, 400.0))], 2.0),
        Some(141.0)
    );
}

#[test]
fn a_slot_showing_the_cell_above_says_it_is_the_same_value() {
    let [a, b, c, shared, other] = [(); 5].map(|_| gid::new_cell_id());
    let slots = [(a, Level::Full), (b, Level::Raw), (c, Level::Raw)];
    let root = Value::record([
        (a, Value::Cell(shared)),
        (b, Value::Cell(shared)),
        (c, Value::Cell(other)),
    ]);
    assert_eq!(same_as_above(&slots, Some(&root)), [false, true, false]);
    let copies = Value::record([
        (a, crate::libraries::f64::value(1.0)),
        (b, crate::libraries::f64::value(1.0)),
    ]);
    assert_eq!(same_as_above(&slots[..2], Some(&copies)), [false, false]);
    let libraries = crate::library::Libraries::default();
    assert_eq!(
        Level::Raw.caption(&libraries, true),
        "The same value in the base projection: no libraries"
    );
    assert!(
        Level::Plain
            .caption(&libraries, true)
            .starts_with("The same value, drawn with the name")
    );
}

#[test]
fn tutorial_slots_require_distinct_valid_ids() {
    for ids in [
        "",
        "first",
        "9940ece27410c72a5308a544890ccc71,",
        "9940ece27410c72a5308a544890ccc71,9940ece27410c72a5308a544890ccc71",
        "9940ece27410c72a5308a544890ccc71,9940ece27410c72a5308a544890ccc71:raw",
        "9940ece27410c72a5308a544890ccc71:fancy",
        "9940ece27410c72a5308a544890ccc71:",
    ] {
        assert!(tutorial_slots(Some(ids), Default::default(), &Default::default()).is_err());
    }
    assert!(tutorial_slots(None, Default::default(), &Default::default()).is_ok());
    for ids in [
        "9940ece27410c72a5308a544890ccc71",
        "9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417:plain,5e716c07490849f072b4e9017dd6230d:raw",
    ] {
        assert!(tutorial_slots(Some(ids), Default::default(), &Default::default()).is_ok());
    }
}

#[test]
fn library_option_distinguishes_default_empty_subset_and_invalid_input() {
    let ids = [crate::libraries::name::ID, crate::libraries::text::ID];
    let option = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        libraries(Some(&option))
            .unwrap()
            .libraries
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(libraries(Some("")).unwrap().libraries.iter().count(), 0);
    assert!(libraries(None).unwrap().libraries.iter().count() > ids.len());
    assert!(libraries(Some("text")).is_err());
    assert!(libraries(Some(&format!("{option},"))).is_err());
    assert!(libraries(Some(&gid::new_cell_id().to_string())).is_err());
}

#[test]
fn reports_initial_state_edits_and_undo_but_not_hover_or_paint() {
    let mut editor = crate::test_editor(
        crate::gid_text::parse(include_str!("../../fixtures/values.gid"))
            .unwrap()
            .0,
    );
    let mut changes = Changes::default();
    let initial = changes.sample(&editor).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&initial).unwrap()["selection"],
        serde_json::Value::Null
    );
    editor.pointer = Some(kurbo::Point::new(100.0, 100.0));
    assert!(changes.sample(&editor).is_none());
    let original = editor.model.doc.clone();
    Rc::make_mut(&mut editor.model.doc).root = Some(Value::list([]));
    assert!(changes.sample(&editor).is_some());
    editor.model.doc = original;
    assert_eq!(changes.sample(&editor).unwrap(), initial);
}

#[test]
fn reports_pending_selection_and_committed_value_at_the_same_path() {
    let mut editor = crate::test_editor(
        crate::gid_text::parse(r#"{"root": ["apples", "pears", "plums"]}"#)
            .unwrap()
            .0,
    );
    let mut changes = Changes::default();
    changes.sample(&editor);
    let list = editor.model.doc.root.as_ref().unwrap().as_list().unwrap();
    let positions: Vec<_> = list.keys().cloned().collect();
    let position = gid::position::between(positions.first(), positions.get(1)).unwrap();
    editor.model.selection = Some(selection::Selection::edge(
        &editor.model.workspace.document.root,
        vec![Step::Element(position.clone())],
    ));
    let pending: serde_json::Value =
        serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
    assert_eq!(pending["selection"]["stage"], "pending");
    assert_eq!(pending["selection"]["view"], "document");
    assert_eq!(pending["selection"]["value"], serde_json::Value::Null);
    let mut items = list.clone();
    items.insert(position, crate::libraries::text::value("peaches"));
    Rc::make_mut(&mut editor.model.doc).root = Some(Value::List(items));
    let committed: serde_json::Value =
        serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
    assert_eq!(committed["selection"]["stage"], "value");
    assert_eq!(
        committed["selection"]["value"],
        serde_json::to_value(crate::libraries::text::value("peaches")).unwrap()
    );
    editor.model.selection = Some(selection::Selection::edge(
        &editor.model.workspace.document.root,
        vec![],
    ));
    let whole: serde_json::Value = serde_json::from_str(&changes.sample(&editor).unwrap()).unwrap();
    assert_eq!(whole["selection"]["path"], serde_json::json!({"list": []}));
    assert!(changes.sample(&editor).is_none());
}

mod frames;
