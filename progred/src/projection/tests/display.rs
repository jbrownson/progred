use super::*;
use crate::display::widget::style;
use crate::projection::{Layout, ProjectionInput};

#[test]
fn contextual_projection_is_local_whether_it_accepts_or_declines() {
    use crate::display::{
        at_with_projection, compose_partials, descend, descend_local, partial, row,
    };
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
                move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                    calls.borrow_mut().push(input.value?.clone());
                    accepts.then(|| descend([Step::Key(child)], None, None))
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
                                at_with_projection(
                                    [Step::Key(field)],
                                    &nested,
                                    Some(compose_partials([
                                        local.clone(),
                                        input.default_projection.clone(),
                                    ])),
                                    Some(input.default_projection.clone()),
                                )
                            } else {
                                descend_local(
                                    [Step::Key(field)],
                                    local.clone(),
                                    &input.default_projection,
                                )
                            },
                            descend([Step::Key(sibling)], None, None),
                        ],
                    )
                })
            })]);
            let mut world = editing_world(
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
fn local_projection_receives_missing_values_without_leaking_through_follow_or_at() {
    use crate::display::{at, descend, descend_local, partial};
    for mode in 0..3 {
        let cell = new_cell_id();
        let field = new_cell_id();
        let leaf = Value::from(vec![7]);
        let root = Value::record([(field, Value::Cell(cell))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let calls = seen.clone();
        let local = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                calls.borrow_mut().push(input.value.cloned());
                match mode {
                    0 => Some(descend(
                        [Step::Follow(gid::Resolution::Document)],
                        None,
                        None,
                    )),
                    1 => Some(at([Step::Key(field)], Value::from(vec![7]))),
                    _ => None,
                }
            },
        );
        let expected_root = root.clone();
        let projection = Projection::new([partial(move |input| {
            (input.value == Some(&expected_root)).then(|| {
                descend_local(
                    [Step::Key(if mode == 2 { cell } else { field })],
                    local.clone(),
                    &input.default_projection,
                )
            })
        })]);
        let mut cells = Cells::new();
        cells.set_value(cell, leaf.clone());
        let mut world = editing_world(
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
    let mut world = editing_world(
        &Document {
            root: Some(cell.into()),
            cells,
        },
        &core_libraries(),
    );
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let projection = Projection::default().with_entry(crate::display::partial(move |input| {
        calls.borrow_mut().push(input.value?.clone());
        None
    }));
    editing_frame_with_projection(&mut world, false, Some(&projection));
    assert_eq!(*seen.borrow(), [Value::Cell(cell), definition]);
}

#[test]
fn descents_replace_current_and_default_projections_independently() {
    use crate::display::{at_with_projection, descend, dim, partial, row};
    for use_at in [false, true] {
        let field = new_cell_id();
        let child = new_cell_id();
        let sibling = new_cell_id();
        let nested = Value::record([(child, Value::from(vec![1]))]);
        let root = Value::record([(field, nested.clone()), (sibling, Value::from(vec![2]))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let current_seen = seen.clone();
        let current = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                current_seen
                    .borrow_mut()
                    .push(("current", input.value?.clone()));
                None
            },
        );
        let child_seen = seen.clone();
        let children = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
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
                                [Step::Key(field)],
                                Some(current.clone()),
                                Some(children.clone()),
                            )
                        },
                        descend([Step::Key(sibling)], None, None),
                    ],
                )
            })
        })]);
        let mut world = editing_world(
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
    use crate::display::{at_with_projection, compose_partials, descend, dim, partial, row};
    let scoped = new_cell_id();
    let ordinary = new_cell_id();
    let items = new_cell_id();
    let cell = new_cell_id();
    let nested = Value::record([(items, Value::list([Value::from(vec![1]), cell.into()]))]);
    let root = Value::record([(scoped, nested.clone()), (ordinary, nested.clone())]);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let special = partial(
        move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
            input.value?.as_blob()?;
            calls.borrow_mut().push(input.value?.clone());
            Some(dim("scoped"))
        },
    );
    let expected_root = root.clone();
    let projection = Projection::new([partial(move |input| {
        (input.value == Some(&expected_root)).then(|| {
            let scoped_projection =
                compose_partials([special.clone(), input.default_projection.clone()]);
            row(
                0.0,
                [
                    at_with_projection(
                        [Step::Key(scoped)],
                        &nested,
                        Some(scoped_projection.clone()),
                        Some(scoped_projection),
                    ),
                    descend([Step::Key(ordinary)], None, None),
                ],
            )
        })
    })]);
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(vec![2]));
    let mut world = editing_world(
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
    use crate::display::{dim, partial};
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
            move |_: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                seen.borrow_mut().push(key);
                Some(dim("field"))
            },
        )
    };
    let first_projection = observe(first);
    let second_projection = observe(second);
    let default_projection = observe(ordinary);
    let projection = Projection::new([
        crate::display::test_support::record_partial(move |key| match key {
            key if key == first => Some(first_projection.clone()),
            key if key == second => Some(second_projection.clone()),
            _ => None,
        }),
        default_projection,
    ]);
    let mut world = editing_world(
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
fn partials_receive_selection_and_annotations_positionally() {
    // The first library code ever to SEE editor state — as data,
    // positionally: the payload only at the selected path, the
    // annotation record only at its own.
    fn probe(input: &ProjectionInput<'_>) -> Option<Layout> {
        input.value?.as_blob()?;
        Some(crate::display::dim(
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
    let projection: Projection = Projection::new([crate::display::partial(probe)]);
    let styles = style::editor(style::Theme::Light.palette(), 1.0);
    let mut fonts = crate::fonts::bundled_font_context();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let computations = crate::computations::Computations::from_sources(Sources {
        doc: &doc,
        libraries: &lib,
    });
    let mut width = |selection: Option<&Selection>, annotations: &Annotations| {
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        project(
            ProjectDescription {
                focused: true,
                computations: &computations,
                view: &crate::test_root(),
                completions: None,
                sources: Sources {
                    doc: &doc,
                    libraries: &lib,
                },
                root: doc.root.as_ref(),
                root_path: &[],
                selection,
                source_selection: selection,
                annotations,
                raw: false,
                styles: &styles,
                width: 500.0,

                projection: &projection,
            },
            &mut tcx,
        )
        .extent
        .width
    };
    let empty = Annotations::default();
    let cold = width(None, &empty);
    let selected = width(
        Some(&crate::selection::Selection::edge(
            &crate::test_root(),
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
fn a_projection_defined_as_data_realizes() {
    // The display language's data form, decoded with the PROVIDED
    // intents and realized through the ordinary pipeline — the same
    // boundary a Grap-backed library projection can use.
    fn probe(input: &ProjectionInput<'_>) -> Option<Layout> {
        use crate::libraries::layout::{test_support as data, vocabulary};
        input.value?.as_blob()?;
        data::decode(
            &data::selectable(data::row(
                4.0,
                [
                    data::text_leaf("from", vocabulary::NAME_FACE),
                    data::text_leaf("data", vocabulary::DIM_FACE),
                ],
            )),
            &input.site(),
        )
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = core_libraries();
    let projection: Projection = Projection::new([crate::display::partial(probe)]);
    let styles = style::editor(style::Theme::Light.palette(), 1.0);
    let mut fonts = crate::fonts::bundled_font_context();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let empty = Annotations::default();
    let computations = crate::computations::Computations::from_sources(Sources {
        doc: &doc,
        libraries: &lib,
    });
    let measured = project(
        ProjectDescription {
            focused: true,
            computations: &computations,
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &doc,
                libraries: &lib,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &empty,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: &projection,
        },
        &mut tcx,
    );
    assert!(measured.extent.width > 0.0);
    let placed = crate::display::widget::frame::place(
        measured,
        uig::Placement::root(measured_rect(500.0)),
        &Default::default(),
    );
    let mut pointer = crate::frame::DispatchContext::new(
        None,
        Some(crate::frame::Hovered::Tree(Hover::Value(Rc::from([])))),
    );
    assert!(placed.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut crate::test_editor(doc.clone()),
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
