use super::*;

#[test]
fn runtime_render_preserves_native_closures_through_evaluation() {
    use crate::display as d;
    use std::cell::Cell;
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    });
    let sources = crate::sources::Sources {
        doc: &world.model.doc,
        libraries: &world.stack.libraries,
    };
    let closure = ::grap::evaluate_at(
        &::grap::lambda([], Value::record([])),
        Some(::grap::SourceOrigin::Stored(vec![Step::Key(
            gid::new_cell_id(),
        )])),
        &sources,
        100,
    )
    .result;
    let expression = ::grap::RuntimeValue::record([(::grap::vocabulary::VALUE, closure.clone())]);
    let value = ::grap::RuntimeValue::record([(
        crate::libraries::presentation::vocabulary::RENDER,
        expression,
    )]);
    let visits = Rc::new(Cell::new(0));
    let child = d::runtime_partial({
        let visits = visits.clone();
        move |input| {
            input.value?.same_result(&closure).then(|| {
                let visits = visits.clone();
                let closure = closure.clone();
                d::Layout::program(Rc::new(move |context, build| {
                    assert!(
                        context.value.unwrap().same_result(&closure),
                        "widget preparation must retain the native callback too"
                    );
                    visits.set(visits.get() + 1);
                    d::text("retained callback").measure(context, build)
                }))
            })
        }
    });
    let projection = Projection::new([d::runtime_partial(move |_| {
        let children = d::compose_partials([
            d::runtime_partial(crate::libraries::presentation::display),
            child.clone(),
        ]);
        Some(d::at_with_projection(
            [],
            value.clone(),
            Some(children.clone()),
            Some(children),
        ))
    })]);
    for expected in 1..=2 {
        let _frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        assert_eq!(visits.get(), expected);
    }
}

#[test]
fn structural_runtime_lists_and_records_preserve_callable_children_and_paths() {
    use crate::display as d;
    use std::cell::Cell;

    let field = gid::new_cell_id();
    let result = gid::new_cell_id();
    struct Empty;
    impl ::grap::Host for Empty {
        fn resolve(&self, _: CellId) -> Option<(gid::Resolution, ::grap::Definition)> {
            None
        }
    }
    let closure = ::grap::evaluate(&::grap::lambda([], Value::record([])), &Empty, 100).result;
    let list = ::grap::RuntimeValue::list([closure.clone()]);
    let position = list.list_positions().unwrap()[0].clone();
    let value = ::grap::RuntimeValue::record([(field, list)]);
    let visits = Rc::new(Cell::new(0));
    let child = d::runtime_partial({
        let visits = visits.clone();
        move |input| {
            input.value?.same_result(&closure).then(|| {
                visits.set(visits.get() + 1);
                d::text("retained closure")
            })
        }
    });
    let projection = Projection::new([d::runtime_partial(move |_| {
        let children = d::compose_partials([
            child.clone(),
            d::structure::list(None),
            d::structure::record(|_| None),
        ]);
        Some(d::at_with_projection(
            [Step::Key(result)],
            value.clone(),
            Some(children.clone()),
            Some(children),
        ))
    })]);
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    });
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert_eq!(visits.get(), 1);
    let path = vec![Step::Key(result), Step::Key(field), Step::Element(position)];
    assert!(
        frame
            .descends
            .iter()
            .any(|item| item.path.as_ref() == path.as_slice())
    );
}

#[test]
fn floating_boxes_are_inert_and_popover_cards_explicitly_add_padding_and_occlusion() {
    use crate::display::{Layout, widget};
    use std::cell::RefCell;

    for card in [false, true] {
        let placements = Rc::new(RefCell::new(vec![]));
        let placed_boxes = placements.clone();
        let projection = Projection::new([crate::display::partial(move |_| {
            let rectangle = |id, width, height| {
                let placements = placed_boxes.clone();
                Layout::widget(Rc::new(move |_| {
                    let placements = placements.clone();
                    widget::leaf(
                        Extent {
                            width,
                            ascent: 0.0,
                            descent: height,
                        },
                        move |output: &mut widget::HoverContext<'_, EditingWorld, Hovered>,
                              placement| {
                            placements.borrow_mut().push((id, placement.rect));
                            output.claim(crate::display::widget::frame::Probe::exact(
                                placement,
                                Hovered::Tree(Hover::Entry(id)),
                            ));
                            output.handler().on_pointer_down(move |world, event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                )) && {
                                    world.text_clipboard.text = Some(id.to_string());
                                    true
                                }
                            });
                        },
                    )
                }))
            };
            let trigger = rectangle(0, 20.0, 20.0);
            let content = rectangle(1, 30.0, 40.0);
            let floating = if card {
                crate::display::popover(trigger, content)
            } else {
                crate::display::floating(trigger, content, |scale, anchor, extent| {
                    widget::popover::position(anchor, extent, 4.0 * scale)
                })
            };
            Some(crate::display::overlay([
                floating,
                rectangle(2, 100.0, 150.0),
            ]))
        })]);
        let doc = Document {
            root: Some(Value::record([])),
            cells: Cells::new(),
        };
        let mut world = editing_world(&doc, &core_libraries());
        let output = editing_frame_with_projection(&mut world, false, Some(&projection));
        let content = placements
            .borrow()
            .iter()
            .find(|(id, _)| *id == 1)
            .unwrap()
            .1;
        assert_eq!(
            content,
            if card {
                Rect::new(10.0, 34.0, 40.0, 74.0)
            } else {
                Rect::new(0.0, 24.0, 30.0, 64.0)
            }
        );
        let margin = Point::new(35.0, 25.0);
        assert_eq!(
            editing_frame_at(&mut world, false, Some(&projection), Some(margin))
                .claim
                .map(|(_, claim)| claim),
            Some(if card {
                Claim::Occludes
            } else {
                Claim::Direct(Hovered::Tree(Hover::Entry(2)))
            })
        );
        assert_eq!(
            editing_frame_at(&mut world, false, Some(&projection), Some(content.center()))
                .claim
                .map(|(_, claim)| claim),
            Some(Claim::Direct(Hovered::Tree(Hover::Entry(1))))
        );
        let handler = output.resolve_for_dispatch();
        for (point, expected) in [
            (margin, if card { None } else { Some("2") }),
            (content.center(), Some("1")),
        ] {
            world.text_clipboard.text = None;
            assert!(handler.dispatch_pointer_down(
                &mut world,
                &PointerButtonEvent {
                    button: Some(PointerButton::Primary),
                    pointer: PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse
                    },
                    state: PointerState {
                        position: (point.x, point.y).into(),
                        ..Default::default()
                    },
                }
            ));
            assert_eq!(world.text_clipboard.text.as_deref(), expected);
        }
    }
}

#[test]
fn native_leading_continuations_place_only_for_the_chosen_alternative() {
    use crate::display::widget;
    use std::cell::RefCell;
    let log = Rc::new(RefCell::new(Vec::new()));
    let projection_log = log.clone();
    let mut context = BenchContext::new();
    context.stack.projection = Projection::new([crate::display::partial(move |input| {
        input.value?;
        let alternative = |width, before_name, child_name| {
            let child_log = projection_log.clone();
            let before_log = projection_log.clone();
            widget::before(
                crate::display::Layout::widget(Rc::new(move |_| {
                    let log = child_log.clone();
                    widget::leaf(
                        Extent {
                            width,
                            ascent: 12.0,
                            descent: 3.0,
                        },
                        move |_, placement| {
                            log.borrow_mut().push((child_name, placement));
                        },
                    )
                })),
                Rc::new(move |_| {
                    let log = before_log.clone();
                    Box::new(move |_, placement| {
                        log.borrow_mut().push((before_name, placement));
                    })
                }),
            )
        };
        Some(crate::display::alternatives([
            alternative(100.0, "before wide", "wide"),
            alternative(20.0, "before narrow", "narrow"),
        ]))
    })]);
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    for (available, expected_width, names) in [
        (200.0, 100.0, ["before wide", "wide"]),
        (60.0, 20.0, ["before narrow", "narrow"]),
    ] {
        log.borrow_mut().clear();
        let (_, extent) = context.place(
            &doc,
            None,
            &Annotations::default(),
            available,
            None,
            None,
            None,
        );
        assert_eq!(
            extent,
            Extent {
                width: expected_width,
                ascent: 12.0,
                descent: 3.0
            }
        );
        let log = log.borrow();
        assert_eq!(log.iter().map(|(name, _)| *name).collect::<Vec<_>>(), names);
        assert_eq!(log[0].1, log[1].1);
        assert_eq!(log[0].1.rect.width(), expected_width);
    }
}

#[test]
fn stretching_widgets_receive_only_the_chosen_row_span() {
    use crate::display::widget;
    use std::cell::RefCell;
    let measured_spans = Rc::new(RefCell::new(Vec::new()));
    let placed_spans = Rc::new(RefCell::new(Vec::new()));
    let measured_log = measured_spans.clone();
    let placed_log = placed_spans.clone();
    let mut context = BenchContext::new();
    context.stack.projection = Projection::new([crate::display::partial(move |input| {
        input.value?;
        let measured_log = measured_log.clone();
        let placed_log = placed_log.clone();
        let side: widget::Widget<crate::Editor, Hovered> = Rc::new(move |_| {
            let placed_log = placed_log.clone();
            let extent = Extent {
                width: 5.0,
                ..Extent::default()
            };
            measured_log.borrow_mut().push(extent);
            measured::fill_height(widget::leaf(extent, move |_, placement| {
                placed_log.borrow_mut().push(placement);
            }))
        });
        let box_at = |width, ascent| {
            crate::display::Layout::widget(Rc::new(move |_| {
                widget::leaf(
                    Extent {
                        width,
                        ascent,
                        descent: 3.0,
                    },
                    |_, _| {},
                )
            }))
        };
        Some(crate::display::row(
            0.0,
            [
                crate::display::Layout::widget(side.clone()),
                crate::display::alternatives([box_at(100.0, 10.0), box_at(20.0, 40.0)]),
                crate::display::Layout::widget(side),
            ],
        ))
    })]);
    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    for (width, expected) in [
        (
            200.0,
            Extent {
                width: 100.0,
                ascent: 10.0,
                descent: 3.0,
            },
        ),
        (
            60.0,
            Extent {
                width: 20.0,
                ascent: 40.0,
                descent: 3.0,
            },
        ),
    ] {
        measured_spans.borrow_mut().clear();
        placed_spans.borrow_mut().clear();
        let (_, extent) =
            context.place(&doc, None, &Annotations::default(), width, None, None, None);
        assert_eq!(
            &*measured_spans.borrow(),
            &[Extent {
                width: 5.0,
                ..Extent::default()
            }; 2]
        );
        assert_eq!(
            extent,
            Extent {
                width: expected.width + 10.0,
                ..expected
            }
        );
        let placements = placed_spans.borrow();
        assert_eq!(placements.len(), 2);
        assert_eq!(placements[0].rect.height(), expected.height());
        assert_eq!(
            placements[1].rect.x0 - placements[0].rect.x1,
            expected.width
        );
    }
}

#[test]
fn decorative_and_pending_slots_share_the_active_query_frame() {
    let mut context = BenchContext::new();
    context.stack.projection = Projection::new([crate::display::partial(|input| {
        input.value?;
        Some(crate::display::slot())
    })]);
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    let decorative = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let selected = pending_value(&crate::test_root(), Vec::new());
    for (scale, size) in [(1.0, 14.0), (1.0, 22.0), (2.0, 14.0), (2.0, 22.0)] {
        context.styles = crate::styles::editor(crate::styles::Theme::Light.palette(), scale);
        context.styles.label.size = size;
        let frames = [
            (&decorative, None),
            (&empty, None),
            (&empty, Some(&selected)),
        ]
        .map(|(doc, selection)| {
            context.place(
                doc,
                selection,
                &Annotations::default(),
                600.0,
                None,
                None,
                None,
            )
        });
        let [
            (decorative, decorative_extent),
            (inactive, inactive_extent),
            (active, active_extent),
        ] = frames;
        assert_eq!(decorative_extent, inactive_extent);
        assert_eq!(inactive_extent, active_extent);
        assert_eq!(inactive_extent.width, 1.5 * f64::from(size) * scale);
        let outline = |bench: &Bench, width| {
            bench
                .list
                .0
                .iter()
                .find_map(|command| match command {
                    DrawCmd::Stroke {
                        shape: Shape::RoundedRect(rect),
                        style,
                        transform,
                        ..
                    } if style.width == width => Some((*rect, *transform)),
                    _ => None,
                })
                .expect("text frame outline")
        };
        assert_eq!(outline(&decorative, scale), outline(&inactive, scale));
        assert_eq!(outline(&inactive, scale), outline(&active, 2.5 * scale));
    }
}

#[test]
fn cell_parentheses_leave_a_gap_beside_empty_frames() {
    use kurbo::Shape as _;

    let doc = Document {
        root: Some(Value::from(new_cell_id())),
        cells: Cells::new(),
    };
    let selected = pending_value(
        &crate::test_root(),
        vec![Step::Follow(gid::Resolution::Document)],
    );
    let mut context = BenchContext::new();
    for scale in [1.0, 2.0] {
        context.styles = crate::styles::editor(crate::styles::Theme::Light.palette(), scale);
        for selection in [None, Some(&selected)] {
            let (bench, _) = context.place(
                &doc,
                selection,
                &Annotations::default(),
                600.0,
                None,
                None,
                None,
            );
            let frame = bench
                .list
                .0
                .iter()
                .find_map(|command| match command {
                    DrawCmd::Stroke {
                        shape: Shape::RoundedRect(rect),
                        style,
                        transform,
                        ..
                    } => Some(transform.transform_rect_bbox(rect.rect().inset(style.width / 2.0))),
                    _ => None,
                })
                .unwrap();
            let delimiters: Vec<_> = bench
                .list
                .0
                .iter()
                .filter_map(|command| match command {
                    DrawCmd::Fill {
                        shape: Shape::Path(path),
                        transform,
                        ..
                    } => Some(transform.transform_rect_bbox(path.bounding_box())),
                    _ => None,
                })
                .collect();
            let [left, right] = delimiters.as_slice() else {
                panic!("two cell parentheses");
            };
            assert!(left.x1 < frame.x0 && frame.x1 < right.x0);
        }
    }
}

#[test]
fn secondary_marks_only_the_same_definition_in_other_occurrences() {
    let stack = crate::stack::load();
    let cell = name::vocabulary::NAME;
    let mut cells = Cells::new();
    cells.set_value(cell, stack.libraries.first_value(cell).unwrap().clone());
    let root = Value::list([Value::from(cell), Value::from(cell)]);
    let positions: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
    let doc = Document {
        root: Some(root),
        cells,
    };
    let path = |index: usize, source| {
        vec![
            Step::Element(positions[index].clone()),
            Step::Follow(source),
            Step::Key(name::vocabulary::NAME),
        ]
    };
    for source in [
        gid::Resolution::Document,
        gid::Resolution::Library(name::ID),
    ] {
        let selected = Selection::edge(&crate::test_root(), path(0, source));
        let (bench, _) = place(&doc, Some(&selected), 900.0);
        let target = bench
            .descends
            .iter()
            .find(|descend| descend.path.as_ref() == path(1, gid::Resolution::Document))
            .unwrap();
        let marks: Vec<_> = bench
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(rect),
                    brush,
                    ..
                } if *brush
                    == Brush::from(
                        crate::styles::Theme::Light
                            .palette()
                            .accent
                            .with_alpha(0.10),
                    ) =>
                {
                    Some(*rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            marks,
            if source == gid::Resolution::Document {
                vec![highlight_outline(1.0, target.rect)]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn primary_and_related_highlights_share_geometry_without_overlapping() {
    let cell = new_cell_id();
    let definition = Value::list([text::value("same"), text::value("same")]);
    let position = definition.as_list().unwrap().keys().next().unwrap().clone();
    let mut cells = Cells::new();
    cells.set_value(cell, definition);
    let root = Value::list([Value::from(cell), Value::from(cell), text::value("same")]);
    let positions: Vec<_> = root.as_list().unwrap().keys().cloned().collect();
    let doc = Document {
        root: Some(root),
        cells,
    };
    let paths = [0, 1].map(|index| {
        vec![
            Step::Element(positions[index].clone()),
            Step::Follow(gid::Resolution::Document),
            Step::Element(position.clone()),
        ]
    });
    let mut context = BenchContext::new();
    let selected = Selection::edge(&crate::test_root(), paths[0].clone());
    let blue = |alpha| {
        Brush::from(
            crate::styles::Theme::Light
                .palette()
                .accent
                .with_alpha(alpha),
        )
    };
    let fills = |bench: &Bench, alpha| {
        bench
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(rect),
                    brush,
                    ..
                } if *brush == blue(alpha) => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let strokes = |bench: &Bench, alpha| {
        bench
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Stroke {
                    shape: Shape::RoundedRect(rect),
                    brush,
                    ..
                } if *brush == blue(alpha) => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for scale in [1.0, 2.0] {
        context.styles = crate::styles::editor(crate::styles::Theme::Light.palette(), scale);
        let annotations = Annotations::default();
        let (unmarked, _) = context.place(&doc, None, &annotations, 1200.0, None, None, None);
        let rects = paths.each_ref().map(|path| {
            unmarked
                .descends
                .iter()
                .find(|descend| descend.path.as_ref() == path.as_slice())
                .unwrap()
                .rect
        });
        let outlines = rects.map(|rect| {
            RoundedRect::from_rect(rect.inflate(2.0 * scale, 2.0 * scale), 4.0 * scale)
        });
        let (hovered, _) = context.place(
            &doc,
            None,
            &annotations,
            1200.0,
            Some(rects[0].center()),
            None,
            None,
        );
        assert_eq!(
            hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(Rc::from(
                paths[0].clone()
            )))))
        );
        assert_eq!(fills(&hovered, 0.08), vec![outlines[0]]);
        assert_eq!(fills(&hovered, 0.05), vec![outlines[1]]);
        assert_eq!(strokes(&hovered, 0.55), vec![outlines[0]]);
        assert!(strokes(&hovered, 0.25).is_empty());
        for pointer in [None, Some(rects[0].center()), Some(rects[1].center())] {
            let (selected, _) = context.place(
                &doc,
                Some(&selected),
                &annotations,
                1200.0,
                pointer,
                None,
                None,
            );
            assert_eq!(fills(&selected, 0.22), vec![outlines[0]]);
            assert_eq!(strokes(&selected, 1.0), vec![outlines[0]]);
            assert_eq!(fills(&selected, 0.10), vec![outlines[1]]);
            assert_eq!(strokes(&selected, 0.55), vec![outlines[1]]);
            assert!(fills(&selected, 0.08).is_empty());
            assert!(fills(&selected, 0.05).is_empty());
        }
    }
}

#[test]
fn sample_text_line_claims_its_own_hover() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/sample.gid"
    )))
    .expect("the sample parses");
    let path = vec![
        Step::Key(sample_vocabulary::STYLE),
        Step::Follow(gid::Resolution::Document),
        Step::Key(sample_vocabulary::COLOR),
    ];
    let (bench, _) = place(&doc, None, 900.0);
    let rect = bench
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &path)
        .expect("color descend")
        .rect;
    let (bench, _) = place_with_pointer(&doc, None, 900.0, Some(rect.center()));
    assert_eq!(
        bench.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(Rc::from(path)))))
    );
}

#[test]
fn custom_match_projection_uses_the_editor_fold() {
    let arm = Value::record([
        (
            crate::libraries::control::vocabulary::PATTERN,
            Value::from(vec![1]),
        ),
        (grap::vocabulary::EXPRESSION, Value::from(vec![2])),
    ]);
    let match_expression = grap::call(
        Value::from(crate::libraries::control::vocabulary::MATCH),
        [
            (
                crate::libraries::control::vocabulary::VALUE,
                Value::from(vec![1]),
            ),
            (
                crate::libraries::control::vocabulary::CASES,
                Value::list([arm]),
            ),
        ],
    );
    let doc = Document {
        root: Some(match_expression),
        cells: Cells::new(),
    };
    let (expanded, expanded_extent) = place(&doc, None, 900.0);
    let mut annotations = Annotations::default();
    crate::annotations::set_collapsed(&mut annotations, &[], false, true);
    let (folded, folded_extent) =
        place_with_annotations(&doc, None, &annotations, 900.0, None, None, None);

    assert!(folded.descends.len() < expanded.descends.len());
    assert!(folded_extent.width < expanded_extent.width);
}

#[test]
fn named_fields_display_alphabetically_before_unnamed_fields() {
    let alpha = CellId::from_u128(0xf1);
    let beta = CellId::from_u128(0x01);
    let unnamed_low = CellId::from_u128(0x02);
    let unnamed_high = CellId::from_u128(0xe1);
    let mut cells = Cells::new();
    cells.set_value(alpha, name::record("alpha", []));
    cells.set_value(beta, name::record("beta", []));
    let doc = Document {
        root: Some(Value::record([
            (alpha, Value::from(vec![1])),
            (beta, Value::from(vec![2])),
            (unnamed_low, Value::from(vec![3])),
            (unnamed_high, Value::from(vec![4])),
        ])),
        cells,
    };
    let (bench, _) = place(&doc, None, 320.0);
    let y = |field| {
        bench
            .descends
            .iter()
            .filter(|descend| descend.path.as_ref() == [Step::Key(field)])
            .map(|descend| descend.rect.y0)
            .reduce(f64::min)
            .expect("field descend")
    };
    assert!(y(alpha) < y(beta));
    assert!(y(beta) < y(unnamed_low));
    assert!(y(unnamed_low) < y(unnamed_high));
}

#[test]
fn expression_children_are_real() {
    let (doc, binders) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/grap-demo.gid"
    )))
    .expect("the Grap demo parses");
    let label = binders["inert_data"];
    let position = doc
        .root
        .as_ref()
        .and_then(Value::as_list)
        .and_then(|entries| {
            entries.iter().find_map(|(position, value)| {
                value
                    .as_record()
                    .is_some_and(|entry| entry.contains_key(&label))
                    .then(|| position.clone())
            })
        })
        .expect("inert-data demo entry");
    let record = vec![Step::Element(position), Step::Key(label)];
    let mut result = record.clone();
    result.push(Step::Key(grap::vocabulary::EVALUATE));
    let mut source_note = result.clone();
    source_note.push(Step::Key(binders["note"]));
    let (bench, _) = place(&doc, None, 560.0);
    assert!(
        bench
            .descends
            .iter()
            .any(|descend| descend.path.as_ref() == &record)
    );
    assert!(
        bench
            .descends
            .iter()
            .any(|descend| descend.path.as_ref() == &result)
    );
    assert!(
        bench
            .descends
            .iter()
            .any(|descend| descend.path.as_ref() == &source_note)
    );
}

/// The keyboard walk against real settled geometry: down visits
/// rows in screen order — never climbing back up — and up
/// retraces the same stops exactly.
#[test]
fn the_row_walk_descends_the_sample_projection_in_screen_order() {
    use ui_events::keyboard::{KeyState, Modifiers};
    let doc = sample_document();
    let (bench, _) = place(&doc, None, 560.0);
    let line = 14.0;
    let press = |named: NamedKey| KeyboardEvent {
        key: Key::Named(named),
        state: KeyState::Down,
        modifiers: Modifiers::empty(),
        ..Default::default()
    };
    let rect_of = |path: &Path| {
        bench
            .descends
            .iter()
            .find(|descend| descend.path.as_ref() == path)
            .expect("walk stops on placed descends")
            .rect
    };
    let select = |path: &[Step]| crate::selection::bare_edge(&crate::test_root(), path.to_vec());
    let mut selection: Option<Selection> = None;
    let mut walk: Vec<Path> = Vec::new();
    while walk.len() < 200 {
        match step_selection(
            crate::modifiers::native(),
            &bench.descends,
            None,
            selection.as_ref(),
            line,
            &press(NamedKey::ArrowDown),
        ) {
            Some(target) => {
                selection = Some(select(&target.path));
                walk.push(target.path.to_vec());
            }
            None => break,
        }
    }
    assert!(walk.len() >= 5 && walk.len() < 200, "walked {}", walk.len());
    assert!(
        walk.iter().any(|path| path.len() >= 2),
        "walk enters open blocks"
    );
    for pair in walk.windows(2) {
        assert!(
            rect_of(&pair[1]).y0 >= rect_of(&pair[0]).y0,
            "down never climbs: {:?} -> {:?}",
            pair[0],
            pair[1]
        );
    }
    for expect in walk.iter().rev().skip(1) {
        let up = step_selection(
            crate::modifiers::native(),
            &bench.descends,
            None,
            selection.as_ref(),
            line,
            &press(NamedKey::ArrowUp),
        )
        .expect("up retraces the walk");
        assert_eq!(up.path.as_ref(), expect);
        selection = Some(select(&up.path));
    }
    // A projected simple-name field is the cell's editable head.
    let head = bench
        .descends
        .iter()
        .map(|descend| descend.path.to_vec())
        .find(|path| {
            matches!(
                path.last(),
                Some(Step::Key(label))
                    if *label == name::vocabulary::NAME
            )
        })
        .expect("the sample has a cell head");
    let cell = head[..head.len() - 2].to_vec();
    assert_eq!(
        step_selection(
            crate::modifiers::native(),
            &bench.descends,
            None,
            Some(&select(&cell)),
            line,
            &press(NamedKey::ArrowRight),
        )
        .map(|target| target.path.to_vec()),
        Some(head)
    );
}

#[test]
fn placement_claims_the_hover_innermost_last() {
    let doc = sample_document();
    let (bench, _) = place(&doc, None, 560.0);
    let library = crate::stack::load().libraries;
    let sources = Sources {
        doc: &doc,
        libraries: &library,
    };
    // Over a string leaf every containing claim reports in
    // placement order. The innermost reports last — the string
    // itself, not its containers — and replaces the earlier
    // candidates in the real pass resolver.
    let string = bench
        .descends
        .iter()
        .find(|descend| {
            sources
                .resolve_path(&descend.path)
                .is_some_and(|value| text::read(value).is_some())
                && projected_name_owner(&descend.path).is_none()
        })
        .expect("the sample has a string leaf");
    let string_rect = string.rect;
    let string_path = string.path.clone();
    let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(string_rect.center()));
    assert!(matches!(
        &bench.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if *path == string_path
    ));
    let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(Point::new(-10.0, -10.0)));
    assert!(bench.hit.is_none());

    let center = string_rect.center();
    let clipped = place_with_inputs(
        &doc,
        None,
        560.0,
        Some(center),
        Some(Rect::new(
            string_rect.x0,
            string_rect.y0,
            center.x - 1.0,
            string_rect.y1,
        )),
    )
    .0;
    assert!(clipped.hit.is_none());
}

#[test]
fn hovering_a_field_label_targets_its_value() {
    let key = crate::test_values::label("title");
    let doc = Document {
        root: Some(Value::record([(key, text::value("hi"))])),
        cells: Cells::new(),
    };
    let (cold, _) = place(&doc, None, 400.0);
    let value = cold
        .descends
        .iter()
        .find(|descend| descend.path.last() == Some(&Step::Key(key)))
        .expect("the field value");
    let y = value.rect.y0 + value.rect.height() / 2.0;
    let mut x = value.rect.x0;
    let mut found = None;
    while x > 0.0 {
        x -= 2.0;
        let (bench, _) = place_with_pointer(&doc, None, 400.0, Some(Point::new(x, y)));
        if matches!(
            &bench.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
                if path.last() == Some(&Step::Key(key))
        ) {
            found = Some(bench);
            break;
        }
    }
    let bench = found.expect("a label claim left of the value");
    assert!(
        bench.list.0.iter().any(is_hover_wash),
        "the label's hover wash should paint"
    );
}

fn is_hover_wash(cmd: &DrawCmd) -> bool {
    match cmd {
        DrawCmd::Fill {
            brush: Brush::Solid(color),
            ..
        } => (color.components[3] - 0.08).abs() < 1e-5,
        DrawCmd::Clip { children, .. } => children.iter().any(is_hover_wash),
        _ => false,
    }
}

/// Two flat elements and two block rows, deterministically: the
/// short list stays a literal, the long strings force the block.
fn gap_document() -> Document {
    Document {
        root: Some(Value::record([
            (
                crate::test_values::label("tags"),
                Value::list([crate::test_values::text("a"), crate::test_values::text("b")]),
            ),
            (
                crate::test_values::label("body"),
                Value::list([
                    crate::test_values::text(
                        "a long enough string that the flat literal cannot fit",
                    ),
                    crate::test_values::text(
                        "and another beside it overflowing any width we render",
                    ),
                ]),
            ),
        ])),
        cells: Cells::new(),
    }
}

/// The two descends under `field`, in the given axis order.
fn elements_of(bench: &Bench, field: &str, by_y: bool) -> (Descend<World>, Descend<World>) {
    let mut found: Vec<&Descend<World>> = bench
        .descends
        .iter()
        .filter(|descend| descend.path.len() == 2 && descend.path.first() == Some(&key(field)))
        .collect();
    found.sort_by(|a, b| {
        let (a, b) = if by_y {
            (a.rect.y0, b.rect.y0)
        } else {
            (a.rect.x0, b.rect.x0)
        };
        a.total_cmp(&b)
    });
    assert_eq!(found.len(), 2);
    (found[0].clone(), found[1].clone())
}

#[test]
fn flat_separators_claim_the_insert_between() {
    let doc = gap_document();
    // Keep the surrounding record and this list in their preferred
    // flat forms; the block-layout behavior is covered below.
    let width = 900.0;
    let (bench, _) = place(&doc, None, width);
    let (first, second) = elements_of(&bench, "tags", false);
    let mid = Point::new(
        (first.rect.x1 + second.rect.x0) / 2.0,
        first.rect.center().y,
    );
    let (bench, _) = place_with_pointer(&doc, None, width, Some(mid));
    assert!(matches!(
        &bench.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
            if path.as_ref() == insertion_between(&first.path, &second.path)
    ));
}

fn insertion_between(left: &[Step], right: &[Step]) -> Path {
    let (Step::Element(low), parent) = left.split_last().unwrap() else {
        panic!("list item")
    };
    let (Step::Element(high), other) = right.split_last().unwrap() else {
        panic!("list item")
    };
    assert_eq!(parent, other);
    parent
        .iter()
        .cloned()
        .chain([Step::Element(
            gid::position::between(Some(low), Some(high)).unwrap(),
        )])
        .collect()
}

#[test]
fn list_presentation_callbacks_need_no_insertion_wiring() {
    use crate::display as d;
    use std::cell::Cell;

    for count in [0, 2] {
        for boundary in 0_usize..=count {
            let doc = Document {
                root: Some(Value::list((0..count).map(|i| f64::value(i as f64)))),
                cells: Cells::new(),
            };
            let positions: Vec<_> = doc
                .root
                .as_ref()
                .unwrap()
                .as_list()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            let position = gid::position::between(
                boundary.checked_sub(1).and_then(|i| positions.get(i)),
                positions.get(boundary),
            )
            .unwrap();
            let path = vec![Step::Element(position.clone())];
            let mut world = crate::test_editor(doc);
            let visits = Rc::new(Cell::new(0));
            let child = d::partial({
                let visits = visits.clone();
                move |input| {
                    f64::read(input.value?)?;
                    visits.set(visits.get() + 1);
                    Some(d::text("value"))
                }
            });
            let projection = world
                .stack
                .projection
                .clone()
                .with_entry(d::runtime_partial(move |input| {
                    let items = d::structure::list_items(input, Some(child.clone()))?;
                    let horizontal = d::structure::list_with(
                        input,
                        &items,
                        |_, item| d::row(0.0, [d::dim("item: "), item]),
                        |_, _| Some(|_| d::dim(" | ")),
                    );
                    let vertical = d::structure::list_with(
                        input,
                        &items,
                        |_, item| item,
                        |_, _| Some(|hover| d::widget::list::insertion_gap(4.0, hover)),
                    );
                    Some(d::alternatives([
                        d::row(0.0, horizontal),
                        d::col(0, 0.0, vertical),
                    ]))
                }));
            let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
            assert_eq!(visits.get(), count, "alternatives share projected children");
            let target = Hovered::Tree(Hover::Value(Rc::from(path.clone())));
            let rect = frame
                .descends
                .iter()
                .find(|d| d.path.is_empty())
                .unwrap()
                .rect;
            let point = (0..rect.width().ceil() as usize)
                .map(|x| Point::new(rect.x0 + x as f64 + 0.5, rect.center().y))
                .find(|point| {
                    matches!(frame.hover_geometry.probe(Some(*point), None, 0.0),
                    Some((_, Claim::Direct(ref hover))) if *hover == target)
                })
                .expect("the plain separator gets its insertion target from the list combinator");
            let before = world.model.doc.clone();
            let mut input = placed::DispatchContext::new(Some(crate::test_root()), Some(target));
            assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
                &mut world,
                &PointerButtonEvent {
                    button: Some(PointerButton::Primary),
                    pointer: PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse
                    },
                    state: PointerState {
                        position: (point.x, point.y).into(),
                        ..Default::default()
                    },
                },
                &mut input,
            ));
            assert!(Rc::ptr_eq(&before, &world.model.doc));
            assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
            assert!(
                editing_frame_with_projection(&mut world, false, Some(&projection))
                    .completion
                    .is_some()
            );
            assert!(world.commit_completion(f64::value(9.0), None, None));
            assert_eq!(world.sources().resolve_path(&path), Some(&f64::value(9.0)));
        }
    }
}

#[test]
fn flat_separators_beside_pending_are_inert_but_other_separators_still_insert() {
    use crate::display as d;

    let doc = Document {
        root: Some(Value::list([
            f64::value(1.0),
            f64::value(2.0),
            f64::value(3.0),
        ])),
        cells: Cells::new(),
    };
    let paths: Vec<Path> = doc
        .root
        .as_ref()
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .map(|position| vec![Step::Element(position.clone())])
        .collect();
    let mut world = crate::test_editor(doc);
    // This fixture's viewport follows its content height. Leave room below
    // the list so the completion popup does not cover the trailing comma.
    let projection = world
        .stack
        .projection
        .clone()
        .with_entry(d::runtime_partial(|input| {
            Some(d::padding(
                (0.0, 0.0, 0.0, 400.0).into(),
                d::structure::list_layout(input, None)?,
            ))
        }));
    let project =
        |world: &mut crate::Editor| editing_frame_with_projection(world, false, Some(&projection));
    let between = |frame: &placed::HoverOutput<crate::Editor>, left: &[Step], right: &[Step]| {
        let rect = |path: &[Step]| {
            frame
                .descends
                .iter()
                .find(|d| d.path.as_ref() == path)
                .unwrap()
                .rect
        };
        let (left, right) = (rect(left), rect(right));
        Point::new((left.x1 + right.x0) / 2.0, left.center().y)
    };
    let click = |world: &mut crate::Editor,
                 frame: placed::HoverOutput<crate::Editor>,
                 point: Point,
                 target: &[Step]| {
        let mut input = placed::DispatchContext::new(
            Some(crate::test_root()),
            Some(Hovered::Tree(Hover::Value(Rc::from(target)))),
        );
        frame.resolve_for_dispatch().dispatch_pointer_down_with(
            world,
            &PointerButtonEvent {
                button: Some(PointerButton::Primary),
                pointer: PointerInfo {
                    pointer_id: Some(PointerId::PRIMARY),
                    persistent_device_id: None,
                    pointer_type: PointerType::Mouse,
                },
                state: PointerState {
                    position: (point.x, point.y).into(),
                    ..Default::default()
                },
            },
            &mut input,
        )
    };
    let frame = project(&mut world);
    let point = between(&frame, &paths[1], &paths[2]);
    assert!(click(
        &mut world,
        frame,
        point,
        &insertion_between(&paths[1], &paths[2])
    ));
    let pending = world.model.selection.as_ref().unwrap().path().to_vec();
    world.model.selection = Some(crate::selection::pending_with_query(
        &crate::test_root(),
        pending.clone(),
        "x",
    ));
    for (left, right) in [(&paths[1], &pending), (&pending, &paths[2])] {
        let frame = project(&mut world);
        let point = between(&frame, left, right);
        let target = insertion_between(left, right);
        assert!(!matches!(
            frame.hover_geometry.probe(Some(point), None, 0.0),
            Some((_, Claim::Direct(Hovered::Tree(Hover::Value(path))))) if path.as_ref() == target
        ));
        let payload = world.model.selection.as_ref().unwrap().payload();
        // Neither comma installs an action, even if given its old target.
        assert!(!click(&mut world, frame, point, &target));
        assert_eq!(world.model.selection.as_ref().unwrap().payload(), payload);
    }

    let frame = project(&mut world);
    let point = between(&frame, &paths[0], &paths[1]);
    assert!(matches!(
        frame.hover_geometry.probe(Some(point), None, 0.0),
        Some((_, Claim::Direct(Hovered::Tree(Hover::Value(ref path)))))
            if path.as_ref() == insertion_between(&paths[0], &paths[1])
    ));
    assert!(click(
        &mut world,
        frame,
        point,
        &insertion_between(&paths[0], &paths[1])
    ));
    assert_ne!(world.model.selection.as_ref().unwrap().path(), pending);
}

#[test]
fn bracketed_vertical_list_gaps_open_pending_at_every_boundary() {
    for boundary in 0_usize..=2 {
        let doc = Document {
            root: Some(Value::list([
                crate::test_values::text("a long enough string that the flat literal cannot fit"),
                crate::test_values::text("and another beside it overflowing any width we render"),
            ])),
            cells: Cells::new(),
        };
        let positions: Vec<_> = doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let path = vec![Step::Element(
            gid::position::between(
                boundary.checked_sub(1).and_then(|i| positions.get(i)),
                positions.get(boundary),
            )
            .unwrap(),
        )];
        let mut world = crate::test_editor(doc);
        let frame = editing_frame(&mut world, false);
        let rect = |path: &[Step]| {
            frame
                .descends
                .iter()
                .find(|d| d.path.as_ref() == path)
                .unwrap()
                .rect
        };
        let items: Vec<_> = positions
            .iter()
            .map(|p| rect(&[Step::Element(p.clone())]))
            .collect();
        assert!(
            items[1].y0 > items[0].y1,
            "the standard list chose its vertical alternative"
        );
        let list = rect(&[]);
        let top = boundary.checked_sub(1).map_or(list.y0, |i| items[i].y1);
        let bottom = items.get(boundary).map_or(list.y1, |r| r.y0);
        let point = Point::new(items[0].center().x, (top + bottom) / 2.0);
        let target = Hovered::Tree(Hover::Value(Rc::from(path.clone())));
        assert!(matches!(frame.hover_geometry.probe(Some(point), None, 0.0),
            Some((_, Claim::Direct(ref hover))) if *hover == target));
        let before = world.model.doc.clone();
        let mut input = placed::DispatchContext::new(Some(crate::test_root()), Some(target));
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &PointerButtonEvent {
                button: Some(PointerButton::Primary),
                pointer: PointerInfo {
                    pointer_id: Some(PointerId::PRIMARY),
                    persistent_device_id: None,
                    pointer_type: PointerType::Mouse,
                },
                state: PointerState {
                    position: (point.x, point.y).into(),
                    ..Default::default()
                },
            },
            &mut input,
        ));
        assert!(Rc::ptr_eq(&before, &world.model.doc));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
        let pending_frame = editing_frame(&mut world, false);
        assert!(pending_frame.completion.is_some());
        let Step::Element(inserted) = &path[0] else {
            unreachable!()
        };
        let pending = pending_frame
            .descends
            .iter()
            .find(|d| d.path.as_ref() == path)
            .unwrap()
            .rect;
        for (neighbor, before) in [
            (boundary.checked_sub(1).and_then(|i| positions.get(i)), true),
            (positions.get(boundary), false),
        ] {
            if let Some(neighbor) = neighbor {
                let neighbor_rect = pending_frame
                    .descends
                    .iter()
                    .find(|d| d.path.as_ref() == [Step::Element(neighbor.clone())])
                    .unwrap()
                    .rect;
                let (low, high, y) = if before {
                    (neighbor, inserted, (neighbor_rect.y1 + pending.y0) / 2.0)
                } else {
                    (inserted, neighbor, (pending.y1 + neighbor_rect.y0) / 2.0)
                };
                let adjacent = vec![Step::Element(
                    gid::position::between(Some(low), Some(high)).unwrap(),
                )];
                assert!(!matches!(
                    pending_frame.hover_geometry.probe(Some(Point::new(pending.center().x, y)), None, 0.0),
                    Some((_, Claim::Direct(Hovered::Tree(Hover::Value(ref p))))) if p.as_ref() == adjacent
                ));
            }
        }
        assert!(world.commit_completion(f64::value(9.0), None, None));
        assert_eq!(world.sources().resolve_path(&path), Some(&f64::value(9.0)));
    }
}

#[test]
fn block_gaps_claim_insertions_and_brackets_still_claim_the_list() {
    let doc = gap_document();
    let (bench, _) = place(&doc, None, 560.0);
    let (upper, lower) = elements_of(&bench, "body", true);
    let parent = vec![key("body")];
    let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
    let (gap, _) = place_with_pointer(
        &doc,
        None,
        560.0,
        Some(Point::new(upper.rect.center().x, gap_y)),
    );
    assert!(matches!(
        &gap.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
            if path.as_ref() == insertion_between(&upper.path, &lower.path)
    ));
    // Just inside the bracket's absorbed gap, the bracket claims
    // the container outright — the widened handle.
    let list = bench
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &parent)
        .expect("the list has a landmark");
    let (claimed, _) = place_with_pointer(
        &doc,
        None,
        560.0,
        Some(Point::new(list.rect.x0 + 1.0, gap_y)),
    );
    assert!(matches!(
        &claimed.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if path.as_ref() == &parent
    ));
}

#[test]
fn cell_interiors_are_air_and_parentheses_are_handles() {
    let upper_key = crate::test_values::label("a");
    let lower_key = crate::test_values::label("b");
    let cell = gid::new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(upper_key, name::record("a", []));
    cells.set_value(lower_key, name::record("b", []));
    cells.set_value(
        cell,
        Value::record([
            (upper_key, crate::test_values::text("one")),
            (lower_key, crate::test_values::text("two")),
        ]),
    );
    let doc = Document {
        root: Some(Value::Cell(cell)),
        cells,
    };
    let width = 100.0;
    let (bench, _) = place(&doc, None, width);
    let field = |key| {
        bench
            .descends
            .iter()
            .find(|descend| {
                descend.path.as_ref() == [Step::Follow(gid::Resolution::Document), Step::Key(key)]
            })
            .expect("the cell's record field has a landmark")
    };
    let mut fields = [field(upper_key), field(lower_key)];
    fields.sort_by(|left, right| left.rect.y0.total_cmp(&right.rect.y0));
    let [upper, lower] = fields;
    assert!(
        upper.rect.y1 < lower.rect.y0,
        "upper {:?}, lower {:?}",
        upper.rect,
        lower.rect,
    );
    let label_x = upper.rect.x0 - 7.0;
    let (label, _) = place_with_pointer(
        &doc,
        None,
        width,
        Some(Point::new(label_x, upper.rect.center().y)),
    );
    assert!(matches!(
        &label.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
            if path.as_ref() == upper.path.as_ref()
    ));
    let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
    let (air, _) = place_with_pointer(&doc, None, width, Some(Point::new(label_x, gap_y)));
    assert!(air.hit.is_none());

    let cell_rect = bench
        .descends
        .iter()
        .find(|descend| descend.path.is_empty())
        .expect("the root cell has a landmark")
        .rect;
    let (paren, _) = place_with_pointer(
        &doc,
        None,
        width,
        Some(Point::new(cell_rect.x0 + 1.0, gap_y)),
    );
    assert!(matches!(
        &paren.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if path.is_empty()
    ));
}
