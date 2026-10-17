use super::*;
use crate::annotations::Annotations;
use crate::libraries::{control, fidget, presentation, text};
use crate::selection::{Stage, pending_edge};
use crate::test_support::{
    core_libraries, inserted_value, projected_completion_entries, root_completions, src,
};
use gid::{Cells, Document, Resolution, Step, new_cell_id};
use std::rc::Rc;

#[test]
fn completion_insertion_never_invents_or_overwrites_selection_policy() {
    let libraries = core_libraries();
    let root = crate::test_root();
    let doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_with_query(&root, vec![], "original query");
    let old_payload = pending.payload();
    for continuation in [
        None,
        Some(
            Rc::new(crate::site::grap(grap::lambda([], Value::record([])), []))
                as crate::site::Continuation,
        ),
    ] {
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            text::value("inserted"),
            None,
            continuation.as_ref(),
        )
        .unwrap();
        assert_eq!(prepared.document.root, Some(text::value("inserted")));
        assert!(!prepared.effects.selection_changed);
        assert_eq!(
            prepared.effects.selection,
            Some((vec![], old_payload.clone()))
        );
    }
    let custom = Value::record([
        (new_cell_id(), text::value("custom state")),
        // A payload is data even when it looks like an application.
        (grap::vocabulary::FUNCTION, new_cell_id().into()),
    ]);
    for payload in [
        custom.clone(),
        crate::libraries::absent::with_reason(gid::new_cell_id()),
    ] {
        let continuation = crate::selection::at(&[], payload.clone());
        let prepared = crate::completion::prepare(
            &src(&doc, &libraries),
            &pending,
            &Annotations::default(),
            text::value("inserted"),
            None,
            Some(&continuation),
        )
        .unwrap();
        assert!(prepared.effects.selection_changed);
        assert_eq!(
            prepared.effects.selection,
            (payload == custom).then(|| (vec![], custom.clone()))
        );
    }
}

#[test]
fn label_offer_explicitly_opens_its_missing_value() {
    let libraries = core_libraries();
    let root = crate::test_root();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = pending_edge(&root, &src(&doc, &libraries), vec![]).unwrap();
    let field = new_cell_id();
    let offer = crate::libraries::completion::insert(
        field,
        field.into(),
        Some(crate::selection::pending_at(&[])),
    );
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = Some(pending);
    (offer.activate)(&mut world);
    assert_eq!(world.model.doc.root, doc.root);
    assert!(!world.model.history.can_undo());
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), [Step::Key(field)]);
    assert_eq!(selected.stage(&world.sources()), Stage::Pending);
}

#[test]
fn root_completions_share_a_bare_cell_with_a_left_pane_and_open_its_definition() {
    let stack = crate::stack::load();
    let templates = [
        fidget::vocabulary::FIDGET.into(),
        crate::libraries::grap::vocabulary::GRAP.into(),
    ];
    let offers: Vec<_> = root_completions(&stack)
        .into_iter()
        .filter(|offer| templates.contains(&offer.display))
        .collect();
    assert_eq!(
        offers
            .iter()
            .map(|offer| offer.display.clone())
            .collect::<Vec<_>>(),
        templates
    );
    for (offer, field) in offers.iter().zip([
        fidget::vocabulary::FIDGET,
        crate::libraries::grap::vocabulary::GRAP,
    ]) {
        let annotations = Annotations::default();
        let world = crate::libraries::test_widgets::activate_completion(offer);
        let document = &world.model.doc;
        let inserted = document.root.clone().unwrap();
        let cell = inserted
            .as_record()
            .unwrap()
            .get(&field)
            .and_then(Value::as_cell)
            .unwrap();
        assert!(document.cells.value(cell).is_none());
        let panes = crate::workspace::declarations(document.root.as_ref());
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].side, crate::workspace::Side::Left);
        let pane = crate::spine::get(document.root.as_ref().unwrap(), &panes[0].path).unwrap();
        assert_eq!(
            pane,
            &if field == fidget::vocabulary::FIDGET {
                Value::record([
                    (presentation::vocabulary::VALUE, cell.into()),
                    (
                        presentation::vocabulary::VIEWPORT,
                        fidget::vocabulary::PREVIEW_3D.into(),
                    ),
                ])
            } else {
                Value::record([(presentation::vocabulary::RENDER, cell.into())])
            }
        );
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(
            selected.path(),
            [Step::Key(field), Step::Follow(Resolution::Document)]
        );
        assert_eq!(
            selected.stage(&src(document, &stack.libraries)),
            Stage::Pending
        );
        let choices = projected_completion_entries(document, selected);
        for value in [Value::record([]), Value::list([])] {
            assert!(
                choices
                    .iter()
                    .any(|choice| inserted_value(document, selected, choice) == Some(value.clone()))
            );
        }
        let filled = crate::completion::prepare(
            &src(document, &stack.libraries),
            selected,
            &annotations,
            Value::record([]),
            None,
            None,
        )
        .unwrap();
        assert_eq!(filled.document.root, document.root);
        assert_eq!(filled.document.cells.value(cell), Some(&Value::record([])));
    }
}

#[test]
fn completion_continuations_use_the_insertion_site_and_decline_atomically() {
    let stack = crate::stack::load();
    let root = crate::test_root();
    let field = new_cell_id();
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let pending = crate::selection::pending_value(&root, vec![Step::Key(field)]);
    let offer = root_completions(&stack)
        .into_iter()
        .find(|offer| offer.display == crate::libraries::grap::vocabulary::GRAP.into())
        .unwrap();
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = Some(crate::selection::pending_value(
        &root,
        vec![Step::Key(field)],
    ));
    (offer.activate)(&mut world);
    let inserted = world
        .model
        .doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&field)
        .unwrap()
        .clone();
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [
            Step::Key(field),
            Step::Key(crate::libraries::grap::vocabulary::GRAP),
            Step::Follow(Resolution::Document),
        ]
    );

    let declined = grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([
                    grap::call(
                        crate::libraries::selection::tests::grap_at(
                            &[],
                            crate::selection::payload::edge(),
                        ),
                        [],
                    ),
                    crate::libraries::absent::decline(),
                ]),
            )],
        ),
    );
    assert!(
        crate::completion::prepare(
            &src(&doc, &stack.libraries),
            &pending,
            &Annotations::default(),
            inserted,
            None,
            Some(&(Rc::new(crate::site::grap(declined, [])) as crate::site::Continuation)),
        )
        .is_none()
    );
    assert_eq!(doc.root, Some(Value::record([])));
}
