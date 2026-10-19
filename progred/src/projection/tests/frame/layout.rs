use super::*;
use crate::clipboard::Clipboard;
use crate::display::test_support::primary_press;
use crate::driver::Driver;
use puri::edit::TextClipboard;
use puri::handler::test_support::DispatchShorthands;
use ui_events::keyboard::Modifiers;

#[test]
fn runtime_render_preserves_native_closures_through_evaluation() {
    use crate::display as d;
    use std::cell::Cell;
    let mut world = crate::editor_of(Value::record([]));
    let sources = crate::sources::Sources {
        doc: &world.model.doc,
        libraries: &world.stack.libraries,
    };
    let closure = ::grap::evaluate_runtime_at(
        &::grap::RuntimeValue::from(::grap::lambda([], Value::record([]))),
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
                    d::test_support::text("retained callback").measure(context, build)
                }))
            })
        }
    });
    world.stack.projection = Projection::new([d::runtime_partial(move |_| {
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
        let _frame = editing_frame(&mut world);
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
                d::test_support::text("retained closure")
            })
        }
    });
    let projection = Projection::new([d::runtime_partial(move |_| {
        let children = d::compose_partials([
            child.clone(),
            d::structure::list(None),
            d::test_support::record_partial(|_| None),
        ]);
        Some(d::at_with_projection(
            [Step::Key(result)],
            value.clone(),
            Some(children.clone()),
            Some(children),
        ))
    })]);
    let mut world = crate::editor_of(Value::record([]));
    world.stack.projection = projection;
    let frame = editing_frame(&mut world);
    assert_eq!(visits.get(), 1);
    let path = vec![Step::Key(result), Step::Key(field), Step::Element(position)];
    assert!(frame.landmarks.landmark(&path.as_slice()).is_some());
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
                        move |output: &mut widget::HoverContext<'_, crate::Editor, Hovered>,
                              placement| {
                            placements.borrow_mut().push((id, placement.rect));
                            output.claim(puri::hover::Probe::exact(
                                placement,
                                Hovered::Tree(Hover::Entry(id)),
                            ));
                            output.handler().on_pointer_down(move |world, event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                )) && {
                                    world.text_clipboard.set_text(&id.to_string());
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
        let mut world = crate::test_editor(doc.clone());
        world.stack.projection = projection;
        let output = editing_frame(&mut world);
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
            framed(&mut world, Some(margin))
                .claim
                .map(|(_, claim)| claim),
            Some(if card {
                Claim::Occludes
            } else {
                Claim::Direct(Hovered::Tree(Hover::Entry(2)))
            })
        );
        assert_eq!(
            framed(&mut world, Some(content.center()))
                .claim
                .map(|(_, claim)| claim),
            Some(Claim::Direct(Hovered::Tree(Hover::Entry(1))))
        );
        let handler = output.resolve_for_dispatch();
        for (point, expected) in [
            (margin, if card { None } else { Some("2") }),
            (content.center(), Some("1")),
        ] {
            world.text_clipboard = Clipboard::default();
            assert!(
                handler
                    .dispatch_pointer_down(&mut world, &primary_press(point, Modifiers::empty()))
            );
            assert_eq!(world.text_clipboard.get_text().as_deref(), expected);
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
        let (_, extent) = context.frame(BenchFrame {
            ..BenchFrame::of(&doc, available)
        });
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
        let (_, extent) = context.frame(BenchFrame {
            ..BenchFrame::of(&doc, width)
        });
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
    let empty = Document::default();
    let decorative = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let selected = pending_value(&crate::test_root(), Default::default(), Vec::new());
    for (scale, size) in [(1.0, 14.0), (1.0, 22.0), (2.0, 14.0), (2.0, 22.0)] {
        context.styles = crate::display::widget::style::editor(
            crate::display::widget::style::Theme::Light.palette(),
            scale,
        );
        context.styles.label.size = size;
        let frames = [
            (&decorative, None),
            (&empty, None),
            (&empty, Some(&selected)),
        ]
        .map(|(doc, selection)| {
            context.frame(BenchFrame {
                selection: selection,
                ..BenchFrame::of(doc, 600.0)
            })
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
        Default::default(),
        vec![Step::Follow(gid::Resolution::Document)],
    );
    let mut context = BenchContext::new();
    for scale in [1.0, 2.0] {
        context.styles = crate::display::widget::style::editor(
            crate::display::widget::style::Theme::Light.palette(),
            scale,
        );
        for selection in [None, Some(&selected)] {
            let (bench, _) = context.frame(BenchFrame {
                selection: selection,
                ..BenchFrame::of(&doc, 600.0)
            });
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
    cells.set_value(
        cell,
        crate::test_support::first_value(&stack.libraries, cell)
            .unwrap()
            .clone(),
    );
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
        let selected = Selection::edge(&crate::test_root(), Default::default(), path(0, source));
        let (bench, _) = place(&doc, Some(&selected), 900.0);
        let target = bench
            .landmarks
            .landmark(&path(1, gid::Resolution::Document))
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
                        crate::display::widget::style::Theme::Light
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
    let selected = Selection::edge(&crate::test_root(), Default::default(), paths[0].clone());
    let blue = |alpha| {
        Brush::from(
            crate::display::widget::style::Theme::Light
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
        context.styles = crate::display::widget::style::editor(
            crate::display::widget::style::Theme::Light.palette(),
            scale,
        );
        let annotations = Annotations::default();
        let (unmarked, _) = context.frame(BenchFrame {
            annotations: Some(&annotations),
            ..BenchFrame::of(&doc, 1200.0)
        });
        let rects = paths
            .each_ref()
            .map(|path| unmarked.landmarks.landmark(&path.as_slice()).unwrap().rect);
        let outlines = rects.map(|rect| {
            RoundedRect::from_rect(rect.inflate(2.0 * scale, 2.0 * scale), 4.0 * scale)
        });
        let (hovered, _) = context.frame(BenchFrame {
            annotations: Some(&annotations),
            pointer: Some(rects[0].center()),
            ..BenchFrame::of(&doc, 1200.0)
        });
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
            let (selected, _) = context.frame(BenchFrame {
                selection: Some(&selected),
                annotations: Some(&annotations),
                pointer: pointer,
                ..BenchFrame::of(&doc, 1200.0)
            });
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
    let doc = sample_document();
    let path = vec![
        key("style"),
        Step::Follow(gid::Resolution::Document),
        key("color"),
    ];
    let (bench, _) = place(&doc, None, 900.0);
    let rect = bench
        .landmarks
        .landmark(&path)
        .expect("color landmark")
        .rect;
    let (bench, _) = place_with(BenchFrame {
        pointer: Some(rect.center()),
        ..BenchFrame::of(&doc, 900.0)
    });
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
    crate::annotations::set_folded(&mut annotations, &[], false, true);
    let (folded, folded_extent) = place_with(BenchFrame {
        annotations: Some(&annotations),
        ..BenchFrame::of(&doc, 900.0)
    });

    assert!(folded.landmarks.len() < expanded.landmarks.len());
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
            .landmarks
            .iter()
            .filter(|landmark| landmark.path.as_ref() == [Step::Key(field)])
            .map(|landmark| landmark.rect.y0)
            .reduce(f64::min)
            .expect("field landmark")
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
    assert!(bench.landmarks.landmark(&record).is_some());
    assert!(bench.landmarks.landmark(&result).is_some());
    assert!(bench.landmarks.landmark(&source_note).is_some());
}

/// The chosen presentation supplies routing, independently of font metrics.
#[test]
fn list_navigation_follows_the_selected_layout_alternative() {
    use puri::handler::NavigationDirection;
    let list = Value::list([
        text::value("first"),
        text::value("second"),
        text::value("third"),
    ]);
    let paths: Vec<_> = positions(&list)
        .into_iter()
        .map(|position| vec![Step::Element(position)])
        .collect();
    let doc = Document {
        root: Some(list),
        cells: Cells::new(),
    };
    for (width, along, across) in [
        (
            1400.0,
            NavigationDirection::Right,
            NavigationDirection::Down,
        ),
        (100.0, NavigationDirection::Down, NavigationDirection::Right),
    ] {
        let selection = make_selection(paths[0].clone());
        let (bench, _) = place(&doc, Some(&selection), width);
        let mut world = crate::test_editor(doc.clone());
        world.model.selection = Some(selection);
        let mut input = crate::frame::DispatchContext {
            landmarks: bench.landmarks.clone().into(),
            ..Default::default()
        };
        assert_eq!(
            bench
                .handler
                .dispatch(
                    &mut world,
                    puri::handler::Event::Navigate(across),
                    &mut input
                )
                .handled(),
            across == NavigationDirection::Right,
            "vertical rows offer horizontal wrapping without changing their primary flow"
        );
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            paths[if across == NavigationDirection::Right {
                1
            } else {
                0
            }]
        );
        world.model.selection = Some(make_selection(paths[0].clone()));
        assert!(
            bench
                .handler
                .dispatch(
                    &mut world,
                    puri::handler::Event::Navigate(along),
                    &mut input
                )
                .handled()
        );
        assert_eq!(world.model.selection.as_ref().unwrap().path(), paths[1]);
    }
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
        .landmarks
        .iter()
        .find(|landmark| {
            sources
                .resolve_path(&landmark.path)
                .is_some_and(|value| text::read(value).is_some())
                && !matches!(landmark.path.last(), Some(Step::Key(label)) if *label == name::vocabulary::NAME)
        })
        .expect("the sample has a string leaf");
    let string_rect = string.rect;
    let string_path = string.path.clone();
    let (bench, _) = place_with(BenchFrame {
        pointer: Some(string_rect.center()),
        ..BenchFrame::of(&doc, 560.0)
    });
    assert!(matches!(
        &bench.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if *path == string_path
    ));
    let (bench, _) = place_with(BenchFrame {
        pointer: Some(Point::new(-10.0, -10.0)),
        ..BenchFrame::of(&doc, 560.0)
    });
    assert!(bench.hit.is_none());

    let center = string_rect.center();
    let clipped = place_with(BenchFrame {
        pointer: Some(center),
        viewport: Some(Rect::new(
            string_rect.x0,
            string_rect.y0,
            center.x - 1.0,
            string_rect.y1,
        )),
        ..BenchFrame::of(&doc, 560.0)
    })
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
        .landmarks
        .iter()
        .find(|landmark| landmark.path.last() == Some(&Step::Key(key)))
        .expect("the field value");
    let y = value.rect.y0 + value.rect.height() / 2.0;
    let mut x = value.rect.x0;
    let mut found = None;
    while x > 0.0 {
        x -= 2.0;
        let (bench, _) = place_with(BenchFrame {
            pointer: Some(Point::new(x, y)),
            ..BenchFrame::of(&doc, 400.0)
        });
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
        crate::display::test_support::painted(&bench.list.0).any(is_hover_wash),
        "the label's hover wash should paint"
    );
}

fn is_hover_wash(cmd: &DrawCmd) -> bool {
    matches!(
        cmd,
        DrawCmd::Fill {
            brush: Brush::Solid(color),
            ..
        } if (color.components[3] - 0.08).abs() < 1e-5
    )
}

/// Two flat elements and two block rows, deterministically: the
/// short list stays a literal, the long strings force the block.
fn gap_document() -> Document {
    Document {
        root: Some(Value::record([
            (
                crate::test_values::label("tags"),
                Value::list([
                    crate::libraries::text::value("a"),
                    crate::libraries::text::value("b"),
                ]),
            ),
            (
                crate::test_values::label("body"),
                Value::list([
                    crate::libraries::text::value(
                        "a long enough string that the flat literal cannot fit",
                    ),
                    crate::libraries::text::value(
                        "and another beside it overflowing any width we render",
                    ),
                ]),
            ),
        ])),
        cells: Cells::new(),
    }
}

/// The two landmarks under `field`, in the given axis order.
fn elements_of(bench: &Bench, field: &str, by_y: bool) -> (Landmark<World>, Landmark<World>) {
    let mut found: Vec<&Landmark<World>> = bench
        .landmarks
        .iter()
        .filter(|landmark| landmark.path.len() == 2 && landmark.path.first() == Some(&key(field)))
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
    let (bench, _) = place_with(BenchFrame {
        pointer: Some(mid),
        ..BenchFrame::of(&doc, width)
    });
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
                    Some(d::test_support::text("value"))
                }
            });
            world.stack.projection = world
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
            editing_frame(&mut world);
            assert_eq!(visits.get(), count, "alternatives share projected children");
            let mut driver = Driver::new(world, Driver::VIEWPORT);
            let target = Hovered::Tree(Hover::Value(Rc::from(path.clone())));
            let rect = driver.rect(&[]);
            let point = (0..rect.width().ceil() as usize)
                .map(|x| Point::new(rect.x0 + x as f64 + 0.5, rect.center().y))
                .find(|point| driver.hover_at(*point).as_ref() == Some(&target))
                .expect("the plain separator gets its insertion target from the list combinator");
            let before = driver.editor().model.doc.clone();
            driver.click_at(point);
            assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
            assert_eq!(driver.selection(), path);
            assert!(driver.picker_entries().is_some());
            assert!(driver.paste(&f64::value(9.0)));
            assert_eq!(driver.value(&path), f64::value(9.0));
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
    world.stack.projection = world
        .stack
        .projection
        .clone()
        .with_entry(d::runtime_partial(|input| {
            Some(d::padding(
                (0.0, 0.0, 0.0, 400.0).into(),
                d::structure::list_layout(input, None)?,
            ))
        }));
    let between = |frame: &crate::projection::test_support::HoverOutput<crate::Editor>,
                   left: &[Step],
                   right: &[Step]| {
        let rect = |path: &[Step]| frame.landmarks.landmark(path).unwrap().rect;
        let (left, right) = (rect(left), rect(right));
        Point::new((left.x1 + right.x0) / 2.0, left.center().y)
    };
    let click = |world: &mut crate::Editor,
                 frame: crate::projection::test_support::HoverOutput<crate::Editor>,
                 point: Point,
                 target: &[Step]| {
        let mut input = crate::frame::DispatchContext::new(
            Some(crate::test_root()),
            Some(Hovered::Tree(Hover::Value(Rc::from(target)))),
        );
        frame.resolve_for_dispatch().dispatch_pointer_down_with(
            world,
            &primary_press(point, Modifiers::empty()),
            &mut input,
        )
    };
    let frame = editing_frame(&mut world);
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
        Default::default(),
        pending.clone(),
        "x",
    ));
    for (left, right) in [(&paths[1], &pending), (&pending, &paths[2])] {
        let frame = editing_frame(&mut world);
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

    let frame = editing_frame(&mut world);
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
                crate::libraries::text::value(
                    "a long enough string that the flat literal cannot fit",
                ),
                crate::libraries::text::value(
                    "and another beside it overflowing any width we render",
                ),
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
        // A window whose content is 500 points wide, too narrow for the
        // flat literal.
        let mut driver = Driver::new(
            crate::test_editor(doc),
            kurbo::Size::new(524.0, Driver::VIEWPORT.height),
        );
        let items: Vec<_> = positions
            .iter()
            .map(|p| driver.rect(&[Step::Element(p.clone())]))
            .collect();
        assert!(
            items[1].y0 > items[0].y1,
            "the standard list chose its vertical alternative"
        );
        let list = driver.rect(&[]);
        let top = boundary.checked_sub(1).map_or(list.y0, |i| items[i].y1);
        let bottom = items.get(boundary).map_or(list.y1, |r| r.y0);
        let point = Point::new(items[0].center().x, (top + bottom) / 2.0);
        assert_eq!(
            driver.hover_at(point),
            Some(Hovered::Tree(Hover::Value(Rc::from(path.clone()))))
        );
        let before = driver.editor().model.doc.clone();
        driver.click_at(point);
        assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
        assert_eq!(driver.selection(), path);
        assert!(driver.picker_entries().is_some());
        let Step::Element(inserted) = &path[0] else {
            unreachable!()
        };
        let pending = driver.rect(&path);
        for (neighbor, before) in [
            (boundary.checked_sub(1).and_then(|i| positions.get(i)), true),
            (positions.get(boundary), false),
        ] {
            if let Some(neighbor) = neighbor {
                let neighbor_rect = driver.rect(&[Step::Element(neighbor.clone())]);
                let (low, high, y) = if before {
                    (neighbor, inserted, (neighbor_rect.y1 + pending.y0) / 2.0)
                } else {
                    (inserted, neighbor, (pending.y1 + neighbor_rect.y0) / 2.0)
                };
                let adjacent = vec![Step::Element(
                    gid::position::between(Some(low), Some(high)).unwrap(),
                )];
                assert_ne!(
                    driver.hover_at(Point::new(pending.center().x, y)),
                    Some(Hovered::Tree(Hover::Value(Rc::from(adjacent))))
                );
            }
        }
        assert!(driver.paste(&f64::value(9.0)));
        assert_eq!(driver.value(&path), f64::value(9.0));
    }
}

#[test]
fn block_gaps_claim_insertions_and_brackets_still_claim_the_list() {
    let doc = gap_document();
    let (bench, _) = place(&doc, None, 560.0);
    let (upper, lower) = elements_of(&bench, "body", true);
    let parent = vec![key("body")];
    let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
    let (gap, _) = place_with(BenchFrame {
        pointer: Some(Point::new(upper.rect.center().x, gap_y)),
        ..BenchFrame::of(&doc, 560.0)
    });
    assert!(matches!(
        &gap.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
            if path.as_ref() == insertion_between(&upper.path, &lower.path)
    ));
    // Just inside the bracket's absorbed gap, the bracket claims
    // the container outright — the widened handle.
    let list = bench
        .landmarks
        .landmark(&parent)
        .expect("the list has a landmark");
    let (claimed, _) = place_with(BenchFrame {
        pointer: Some(Point::new(list.rect.x0 + 1.0, gap_y)),
        ..BenchFrame::of(&doc, 560.0)
    });
    assert!(matches!(
        &claimed.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if path.as_ref() == parent
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
            (upper_key, crate::libraries::text::value("one")),
            (lower_key, crate::libraries::text::value("two")),
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
            .landmarks
            .iter()
            .find(|landmark| {
                landmark.path.as_ref() == [Step::Follow(gid::Resolution::Document), Step::Key(key)]
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
    let (label, _) = place_with(BenchFrame {
        pointer: Some(Point::new(label_x, upper.rect.center().y)),
        ..BenchFrame::of(&doc, width)
    });
    assert!(matches!(
        &label.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path))))
            if path.as_ref() == upper.path.as_ref()
    ));
    let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
    let (air, _) = place_with(BenchFrame {
        pointer: Some(Point::new(label_x, gap_y)),
        ..BenchFrame::of(&doc, width)
    });
    assert!(air.hit.is_none());

    let cell_rect = bench
        .landmarks
        .iter()
        .find(|landmark| landmark.path.is_empty())
        .expect("the root cell has a landmark")
        .rect;
    let (paren, _) = place_with(BenchFrame {
        pointer: Some(Point::new(cell_rect.x0 + 1.0, gap_y)),
        ..BenchFrame::of(&doc, width)
    });
    assert!(matches!(
        &paren.hit,
        Some(Claim::Direct(Hovered::Tree(Hover::Value(path)))) if path.is_empty()
    ));
}
