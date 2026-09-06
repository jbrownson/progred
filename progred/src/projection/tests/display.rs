use super::*;

#[test]
fn projection_target_appends_relative_steps() {
    let parent = gid::new_cell_id();
    let field = gid::new_cell_id();
    let hooks = Hooks::<Vec<Path>> {
        completions: None,
        select: Rc::new(|selections, path| selections.push(path)),
        select_payload: Rc::new(|selections, path, _| selections.push(path)),
        start_edit: Rc::new(|_, _, _| {}),
        toggle: Rc::new(|_, _| {}),
        update_state: Rc::new(|_, _, _| false),
        edit: Rc::new(|_| None),
        pick: Rc::new(|_, _| false),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_, _| false),
        apply: Rc::new(|_, _, _, _| false),
        point: Rc::new(|_, _, _, _, _| false),
        state_drag: Rc::new(|_, _, _, _, _| {}),
        scrub: Rc::new(|_, _, _, _, _| false),
        select_source: Rc::new(|_, _, _| {}),
        commit_value: Rc::new(|_, _, _| {}),
        commit_label: Rc::new(|_, _, _, _| {}),
        set_completion_view: Rc::new(|_, _, _, _| {}),
    };
    let target = projection_target(&[Step::Key(parent)], &hooks, vec![Step::Key(field)]);
    assert_eq!(
        target.hover,
        Hover::Value(Rc::from(vec![Step::Key(parent), Step::Key(field)]))
    );
    let mut selections = Vec::new();
    assert!((target.select)(&mut selections));
    assert!((target.select_with)(&mut selections, Value::record([])));
    assert_eq!(
        selections,
        [
            vec![Step::Key(parent), Step::Key(field)],
            vec![Step::Key(parent), Step::Key(field)]
        ]
    );
}

#[test]
fn contextual_projection_precedes_and_falls_through_to_the_ambient_projection() {
    struct NoEval;
    impl progred_display::Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("projection evaluated")
        }
    }

    let ambient = Projection::new([progred_display::partial(ambient_probe)]);
    let value = Value::record([]);
    let target = |_| progred_display::ProjectionTarget {
        select: Rc::new(|_: &mut ()| false),
        select_with: Rc::new(|_: &mut (), _| false),
        hover: Hover::Value(Rc::from([])),
    };
    let apply = |projection: &Projection<()>| {
        projection
            .apply(&progred_display::ProjectionInput {
                env: &NoEval,
                value: &value,
                scale_factor: 1.0,
                writable: true,
                selection: None,
                pending: None,
                state: None,
                targets: progred_display::ProjectionTargets::new(&target),
            })
            .unwrap()
    };
    let text = |layout| match layout {
        progred_display::Layout::Leaf(puri::Leaf::Text { text, .. }) => text,
        _ => panic!("probe returns text"),
    };

    assert_eq!(
        text(apply(
            &contextual_projection(
                Some(&ambient),
                Some(vec![progred_display::partial(contextual_probe)]),
            )
            .unwrap()
        )),
        "contextual"
    );
    assert_eq!(
        text(apply(
            &contextual_projection(
                Some(&ambient),
                Some(vec![progred_display::partial(declining_probe)]),
            )
            .unwrap()
        )),
        "ambient"
    );
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
    let doc = Document {
        root: Some(Value::from(a)),
        cells,
    };
    let lib = core_libraries();
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    let reentry = vec![Step::Follow(gid::Resolution::Document), key("next")];
    // Space's toggle expands the default-collapsed re-entry.
    assert!(toggle_fold(&sources, &mut collapse, &reentry));
    assert!(!crate::annotations::collapsed(&collapse, &reentry, true));
    // The next turn defaults collapsed at its own deeper path and
    // expands the same way — follow the cycle as far as wanted.
    let deeper: Vec<Step> = reentry.iter().chain(reentry.iter()).cloned().collect();
    assert!(crate::annotations::collapsed(&collapse, &deeper, true));
    assert!(toggle_fold(&sources, &mut collapse, &deeper));
    assert!(!crate::annotations::collapsed(&collapse, &deeper, true));
    // Toggling back restores the default (the override is sparse).
    assert!(toggle_fold(&sources, &mut collapse, &deeper));
    assert!(crate::annotations::collapsed(&collapse, &deeper, true));
    assert!(collapse.at(&deeper).is_none());
}

#[test]
fn any_valued_cell_and_any_container_collapse() {
    let lib = core_libraries();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("kind"),
        crate::test_values::text("building"),
    )]);
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    // A plain (non-cycle) cell collapses to ( … ) via the same
    // toggle.
    assert!(toggle_fold(&sources, &mut collapse, &[]));
    assert!(crate::annotations::collapsed(&collapse, &[], false));
    // Its record collapses too — layout never enters into it, so
    // inline literals toggle exactly like block forms.
    assert!(toggle_fold(
        &sources,
        &mut collapse,
        &[Step::Follow(gid::Resolution::Document)]
    ));
    assert!(crate::annotations::collapsed(
        &collapse,
        &[Step::Follow(gid::Resolution::Document)],
        false
    ));
    // A valueless location declines.
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    assert!(!toggle_fold(
        &src(&empty, &lib),
        &mut collapse,
        &[] as &[Step]
    ));
}

#[test]
fn the_sample_document_shows_the_constructs() {
    let doc = sample_document();
    let lib = crate::stack::load::<()>().libraries;
    let sources = src(&doc, &lib);
    // The root is an inline record of roles.
    assert!(doc.root.as_ref().unwrap().as_record().is_some());
    let roof = sources
        .resolve_path(&[key("shape")])
        .unwrap()
        .as_cell()
        .unwrap();
    assert_eq!(
        sources
            .value(roof, &gid::Resolution::Document)
            .and_then(name::read),
        Some("roof")
    );
    // The material cell is referenced and fully bare.
    let material = sources
        .resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("material"),
        ])
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(
        sources
            .value(material, &gid::Resolution::Document)
            .is_none()
    );
    // The stroke cell is a name-only ordinary record, referenced
    // as a label.
    let stroke = sources
        .value(roof, &gid::Resolution::Document)
        .unwrap()
        .as_record()
        .unwrap()
        .keys()
        .copied()
        .find(|cell| {
            sources
                .value(*cell, &gid::Resolution::Document)
                .and_then(name::read)
                == Some("stroke")
        })
        .unwrap();
    assert_eq!(
        sources
            .value(stroke, &gid::Resolution::Document)
            .and_then(name::read),
        Some("stroke")
    );
    // The style cell is shared by the root and the roof.
    assert_eq!(
        sources.resolve_path(&[key("style")]),
        sources.resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("style")
        ])
    );
    // Points hold inline records; the swatch is a blob.
    let points = sources
        .resolve_path(&[
            key("shape"),
            Step::Follow(gid::Resolution::Document),
            key("points"),
        ])
        .unwrap();
    let origin = points
        .as_list()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(matches!(
        sources
            .value(origin, &gid::Resolution::Document)
            .and_then(|value| value.as_record())
            .and_then(|fields| fields.get(&crate::test_values::label("at"))),
        Some(Value::Record(_))
    ));
    assert!(
        sources
            .resolve_path(&[
                key("style"),
                Step::Follow(gid::Resolution::Document),
                key("swatch")
            ])
            .unwrap()
            .as_blob()
            .is_some()
    );
    // Documents round trip with convention data unchanged.
    let json = serde_json::to_string(&doc).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.root, doc.root);
    assert_eq!(loaded.cells.value(roof).and_then(name::read), Some("roof"));
    assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
}

#[test]
fn partials_receive_selection_and_annotations_positionally() {
    // The first library code ever to SEE editor state — as data,
    // positionally: the payload only at the selected path, the
    // annotation record only at its own.
    fn probe(
        input: &progred_display::ProjectionInput<'_, (), Hover>,
    ) -> Option<progred_display::Layout<(), Hover>> {
        input.value.as_blob()?;
        Some(progred_display::dim(
            match (input.selection.is_some(), input.state.is_some()) {
                (true, _) => "selected here",
                (false, true) => "annotated here",
                (false, false) => "cold",
            },
        ))
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = core_libraries();
    let projection: Projection<()> = Projection::new([progred_display::partial(probe)]);
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut width = |selection: Option<&Selection>, annotations: &Annotations| {
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        project::<(), crate::frame::Paint>(
            ProjectDescription {
                sources: Sources {
                    doc: &doc,
                    libraries: &lib,
                },
                root: doc.root.as_ref(),
                root_path: &[],
                selection,
                scrub_spelling: None,
                source_selection: selection,
                annotations,
                raw: false,
                styles: &styles,
                width: 500.0,

                projection: Some(&projection),
            },
            &mut tcx,
            Hooks::<()> {
                completions: None,
                select: Rc::new(|_, _| {}),
                select_payload: Rc::new(|_, _, _| {}),
                start_edit: Rc::new(|_, _, _| {}),
                toggle: Rc::new(|_, _| {}),
                update_state: Rc::new(|_, _, _| false),
                edit: Rc::new(|_| None),
                pick: Rc::new(|_, _| false),
                insert: Rc::new(|_, _| {}),
                delete: Rc::new(|_, _| false),
                apply: Rc::new(|_, _, _, _| false),
                point: Rc::new(|_, _, _, _, _| false),
                state_drag: Rc::new(|_, _, _, _, _| {}),
                scrub: Rc::new(|_, _, _, _, _| false),
                select_source: Rc::new(|_, _, _| {}),
                commit_value: Rc::new(|_, _, _| {}),
                commit_label: Rc::new(|_, _, _, _| {}),
                set_completion_view: Rc::new(|_, _, _, _| {}),
            },
        )
        .extent
        .width
    };
    let empty = Annotations::default();
    let cold = width(None, &empty);
    let selected = width(
        Some(&crate::selection::bare_edge(
            &crate::workspace::Root::document(),
            Vec::new(),
        )),
        &empty,
    );
    let mut marked = Annotations::default();
    marked.set_field(&[], crate::annotations::FOLD, Some(Value::from(vec![1u8])));
    let annotated = width(None, &marked);
    assert_ne!(cold, selected);
    assert_ne!(cold, annotated);
    assert_ne!(selected, annotated);
}

#[test]
fn the_pending_payload_is_derived_from_the_live_editor() {
    let mut doc = Rc::new(Document {
        root: None,
        cells: Cells::new(),
    });
    let lib = core_libraries();
    for everything in [false, true] {
        let mut pending = crate::selection::pending_with_query(
            &crate::workspace::Root::document(),
            Vec::new(),
            "",
        );
        pending.set_completion_view(24.0, 2, everything);
        pending
            .edit_mut()
            .unwrap()
            .handle_ime(&puri::handler::ImeEvent::Commit("ab".to_string()));
        assert_eq!(pending.choice(), 0);
        assert_eq!(pending.completion_scroll(), 0.0);
        assert_eq!(pending.completion_everything(), everything);
        assert_eq!(selection_payload::query(&pending.payload()), Some("ab"));
        assert_eq!(
            selection_payload::completion_everything(&pending.payload()),
            everything
        );
        write_through(&mut doc, &lib, &mut pending);
        assert_eq!(selection_payload::query(&pending.payload()), Some("ab"));
        assert_eq!(pending.choice(), 0);
        assert_eq!(pending.completion_everything(), everything);
        pending.edit_mut().unwrap().set_text("");
        assert_eq!(pending.choice(), 0);
        assert_eq!(pending.completion_everything(), everything);
    }
}

#[test]
fn a_projection_defined_as_data_realizes() {
    // The display language's data form, decoded with the PROVIDED
    // intents and realized through the ordinary pipeline — the same
    // boundary a Grap-backed library projection can use.
    fn probe(
        input: &progred_display::ProjectionInput<'_, (), Hover>,
    ) -> Option<progred_display::Layout<(), Hover>> {
        use progred_libraries::layout as data;
        input.value.as_blob()?;
        let target = input.targets.current();
        data::decode(
            &data::selectable(data::row(
                4.0,
                [
                    data::text_leaf("from", data::vocabulary::NAME_FACE),
                    data::text_leaf("data", data::vocabulary::DIM_FACE),
                ],
            )),
            &target.select,
            &target.hover,
        )
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = core_libraries();
    let projection: Projection<()> = Projection::new([progred_display::partial(probe)]);
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let empty = Annotations::default();
    let measured = project::<(), crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                libraries: &lib,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            scrub_spelling: None,
            source_selection: None,
            annotations: &empty,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(&projection),
        },
        &mut tcx,
        Hooks::<()> {
            completions: None,
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|_, _, _| {}),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    assert!(measured.extent.width > 0.0);
    let placed = measured::place(
        measured,
        puri::geometry::Placement::root(measured_rect(500.0)),
    );
    let mut pointer = crate::placed::DispatchContext::new(
        None,
        Some(crate::frame::Hovered::Tree(Hover::Value(Rc::from([])))),
    );
    assert!(placed.handler.unwrap().dispatch_pointer_down_with(
        &mut (),
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState::default(),
        },
        &mut pointer,
    ));
}
