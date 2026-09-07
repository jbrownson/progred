use super::*;

#[test]
fn projection_target_appends_relative_steps() {
    let parent = gid::new_cell_id();
    let field = gid::new_cell_id();
    let hooks = Hooks::<Vec<Path>> {
        completions: None,
        select: Rc::new(|selections, path| selections.push(path)),
        select_payload: Rc::new(|selections, path, _| selections.push(path)),
        edit_line: Rc::new(|_, _, _, _| false),
        toggle: Rc::new(|_, _| {}),
        update_state: Rc::new(|_, _, _| false),
        edit: Rc::new(|_, _| false),
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
fn contextual_projection_is_local_whether_it_accepts_or_declines() {
    use progred_display::{at_local, descend, descend_local, partial, row};
    for use_at in [false, true] {
        for accepts in [false, true] {
            let field = new_cell_id();
            let child = new_cell_id();
            let sibling = new_cell_id();
            let nested = Value::record([(child, Value::from(vec![1]))]);
            let root = Value::record([(field, nested.clone()), (sibling, Value::from(vec![2]))]);
            let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
            let calls = seen.clone();
            let local = partial(
                move |input: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
                    calls.borrow_mut().push(input.value?.clone());
                    accepts.then(|| descend(Step::Key(child), None, None))
                },
            );
            let expected_root = root.clone();
            let ambient_seen = Rc::new(std::cell::RefCell::new(Vec::new()));
            let ambient_calls = ambient_seen.clone();
            let projection = Projection::new([partial(move |input| {
                ambient_calls.borrow_mut().push(input.value?.clone());
                (input.value == Some(&expected_root)).then(|| {
                    row(
                        0.0,
                        [
                            if use_at {
                                at_local(
                                    [Step::Key(field)],
                                    &nested,
                                    local.clone(),
                                    &input.default_projection,
                                )
                            } else {
                                descend_local(
                                    Step::Key(field),
                                    local.clone(),
                                    &input.default_projection,
                                )
                            },
                            descend(Step::Key(sibling), None, None),
                        ],
                    )
                })
            })]);
            let mut world = EditingWorld::new(
                &Document {
                    root: Some(root),
                    cells: Cells::new(),
                },
                &core_libraries(),
            );
            editing_frame_with_projection(&mut world, false, Some(&projection));
            assert_eq!(
                *seen.borrow(),
                [Value::record([(child, Value::from(vec![1]))])]
            );
            assert!(ambient_seen.borrow().contains(&Value::from(vec![1])));
            assert!(ambient_seen.borrow().contains(&Value::from(vec![2])));
            assert_eq!(ambient_seen.borrow().contains(&seen.borrow()[0]), !accepts);
        }
    }
}

#[test]
fn local_projection_receives_missing_values_without_leaking_through_follow_or_transient_roots() {
    use progred_display::{descend, descend_local, partial, transient};
    for mode in 0..3 {
        let cell = new_cell_id();
        let field = new_cell_id();
        let leaf = Value::from(vec![7]);
        let root = Value::record([(field, Value::Cell(cell))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let calls = seen.clone();
        let local = partial(
            move |input: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
                calls.borrow_mut().push(input.value.cloned());
                match mode {
                    0 => Some(descend(Step::Follow(gid::Resolution::Document), None, None)),
                    1 => Some(transient(&Value::from(vec![7]), 100)),
                    _ => None,
                }
            },
        );
        let expected_root = root.clone();
        let projection = Projection::new([partial(move |input| {
            (input.value == Some(&expected_root)).then(|| {
                descend_local(
                    Step::Key(if mode == 2 { cell } else { field }),
                    local.clone(),
                    &input.default_projection,
                )
            })
        })]);
        let mut cells = Cells::new();
        cells.set_value(cell, leaf.clone());
        let mut world = EditingWorld::new(
            &Document {
                root: Some(root),
                cells,
            },
            &core_libraries(),
        );
        editing_frame_with_projection(&mut world, false, Some(&projection));
        assert_eq!(
            *seen.borrow(),
            [if mode == 2 { None } else { Some(cell.into()) }]
        );
        assert!(!seen.borrow().contains(&Some(leaf)));
    }
}

#[test]
fn pane_entry_still_follows_cells_but_stops_at_the_first_non_cell() {
    let cell = new_cell_id();
    let field = new_cell_id();
    let definition = Value::record([(field, Value::from(vec![3]))]);
    let mut cells = Cells::new();
    cells.set_value(cell, definition.clone());
    let mut world = EditingWorld::new(
        &Document {
            root: Some(cell.into()),
            cells,
        },
        &core_libraries(),
    );
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let projection = Projection::default().with_entry(progred_display::partial(move |input| {
        calls.borrow_mut().push(input.value?.clone());
        None
    }));
    editing_frame_with_projection(&mut world, false, Some(&projection));
    assert_eq!(*seen.borrow(), [Value::Cell(cell), definition]);
}

#[test]
fn descents_replace_current_and_default_projections_independently() {
    use progred_display::{at_with_projection, descend, dim, partial, row};
    for use_at in [false, true] {
        let field = new_cell_id();
        let child = new_cell_id();
        let sibling = new_cell_id();
        let nested = Value::record([(child, Value::from(vec![1]))]);
        let root = Value::record([(field, nested.clone()), (sibling, Value::from(vec![2]))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let current_seen = seen.clone();
        let current = partial(
            move |input: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
                current_seen
                    .borrow_mut()
                    .push(("current", input.value?.clone()));
                None
            },
        );
        let child_seen = seen.clone();
        let children = partial(
            move |input: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
                child_seen
                    .borrow_mut()
                    .push(("children", input.value?.clone()));
                Some(dim("child"))
            },
        );
        let default_seen = seen.clone();
        let expected_root = root.clone();
        let projection = Projection::new([partial(move |input| {
            default_seen
                .borrow_mut()
                .push(("original", input.value?.clone()));
            (input.value == Some(&expected_root)).then(|| {
                row(
                    0.0,
                    [
                        if use_at {
                            at_with_projection(
                                [Step::Key(field)],
                                &nested,
                                Some(current.clone()),
                                Some(children.clone()),
                            )
                        } else {
                            descend(
                                Step::Key(field),
                                Some(current.clone()),
                                Some(children.clone()),
                            )
                        },
                        descend(Step::Key(sibling), None, None),
                    ],
                )
            })
        })]);
        let mut world = EditingWorld::new(
            &Document {
                root: Some(root.clone()),
                cells: Cells::new(),
            },
            &core_libraries(),
        );
        editing_frame_with_projection(&mut world, false, Some(&projection));
        assert_eq!(
            *seen.borrow(),
            [
                ("original", root),
                ("current", Value::record([(child, Value::from(vec![1]))])),
                ("children", Value::from(vec![1])),
                ("original", Value::from(vec![2])),
            ]
        );
    }
}

#[test]
fn explicit_scope_reaches_nested_containers_and_cells_but_not_siblings() {
    use progred_display::{at_scoped, descend, dim, partial, row};
    let scoped = new_cell_id();
    let ordinary = new_cell_id();
    let items = new_cell_id();
    let cell = new_cell_id();
    let nested = Value::record([(items, Value::list([Value::from(vec![1]), cell.into()]))]);
    let root = Value::record([(scoped, nested.clone()), (ordinary, nested.clone())]);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let special = partial(
        move |input: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
            input.value?.as_blob()?;
            calls.borrow_mut().push(input.value?.clone());
            Some(dim("scoped"))
        },
    );
    let expected_root = root.clone();
    let projection = Projection::new([partial(move |input| {
        (input.value == Some(&expected_root)).then(|| {
            row(
                0.0,
                [
                    at_scoped(
                        [Step::Key(scoped)],
                        &nested,
                        special.clone(),
                        &input.default_projection,
                    ),
                    descend(Step::Key(ordinary), None, None),
                ],
            )
        })
    })]);
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(vec![2]));
    let mut world = EditingWorld::new(
        &Document {
            root: Some(root),
            cells,
        },
        &core_libraries(),
    );
    editing_frame_with_projection(&mut world, false, Some(&projection));
    assert_eq!(*seen.borrow(), [Value::from(vec![1]), Value::from(vec![2])]);
}

#[test]
fn record_combinator_chooses_a_projection_for_each_field() {
    use progred_display::{dim, partial, structure};
    let first = new_cell_id();
    let second = new_cell_id();
    let ordinary = new_cell_id();
    let value = Value::from(vec![1]);
    let root = Value::record([
        (first, value.clone()),
        (second, value.clone()),
        (ordinary, value),
    ]);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observe = |key| {
        let seen = seen.clone();
        partial(
            move |_: &progred_display::ProjectionInput<'_, EditingWorld, Hover>| {
                seen.borrow_mut().push(key);
                Some(dim("field"))
            },
        )
    };
    let first_projection = observe(first);
    let second_projection = observe(second);
    let default_projection = observe(ordinary);
    let projection = Projection::new([
        structure::record(move |key| match key {
            key if key == first => Some(first_projection.clone()),
            key if key == second => Some(second_projection.clone()),
            _ => None,
        }),
        default_projection,
    ]);
    let mut world = EditingWorld::new(
        &Document {
            root: Some(root),
            cells: Cells::new(),
        },
        &core_libraries(),
    );
    editing_frame_with_projection(&mut world, false, Some(&projection));
    let mut actual = seen.borrow().clone();
    actual.sort();
    let mut expected = [first, second, ordinary];
    expected.sort();
    assert_eq!(actual, expected);
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
        input.value?.as_blob()?;
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
                edit_line: Rc::new(|_, _, _, _| false),
                toggle: Rc::new(|_, _| {}),
                update_state: Rc::new(|_, _, _| false),
                edit: Rc::new(|_, _| false),
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
    for everything in [false, true] {
        let mut pending = crate::selection::pending_with_query(
            &crate::workspace::Root::document(),
            Vec::new(),
            "",
        );
        pending.set_completion_view(24.0, 2, everything);
        pending
            .edit_query(|line| line.handle_ime(&puri::handler::ImeEvent::Commit("ab".to_string())));
        assert_eq!(pending.choice(), 0);
        assert_eq!(pending.completion_scroll(), 0.0);
        assert_eq!(pending.completion_everything(), everything);
        assert_eq!(selection_payload::query(&pending.payload()), Some("ab"));
        assert_eq!(
            selection_payload::completion_everything(&pending.payload()),
            everything
        );
        assert_eq!(selection_payload::query(&pending.payload()), Some("ab"));
        assert_eq!(pending.choice(), 0);
        assert_eq!(pending.completion_everything(), everything);
        pending.edit_query(|line| {
            line.set_text("");
            true
        });
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
        input.value?.as_blob()?;
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
            edit_line: Rc::new(|_, _, _, _| false),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_, _| false),
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
