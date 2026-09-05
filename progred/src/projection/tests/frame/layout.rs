use super::*;

#[test]
fn secondary_marks_only_the_same_definition_in_other_occurrences() {
    let stack = crate::stack::load::<World>();
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
    let sources = Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    for source in [
        gid::Resolution::Document,
        gid::Resolution::Library(name::ID),
    ] {
        let selected = Selection::edge(
            &crate::workspace::Root::document(),
            &sources,
            path(0, source),
        );
        let (bench, _) = place(&doc, Some(&selected), 900.0);
        let target = bench
            .descends
            .iter()
            .find(|descend| descend.path.as_ref() == path(1, source))
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
                } if *brush == Brush::from(Color::new([0.0, 0.48, 1.0, 0.10])) => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(
            marks,
            vec![RoundedRect::from_rect(target.rect.inset(3.0), 5.0)]
        );
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
            progred_libraries::control::vocabulary::PATTERN,
            Value::from(vec![1]),
        ),
        (grap::vocabulary::EXPRESSION, Value::from(vec![2])),
    ]);
    let match_expression = grap::call(
        Value::from(progred_libraries::control::vocabulary::MATCH),
        [
            (
                progred_libraries::control::vocabulary::VALUE,
                Value::from(vec![1]),
            ),
            (
                progred_libraries::control::vocabulary::CASES,
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
    let select = |path: &[Step]| {
        crate::selection::bare_edge(&crate::workspace::Root::document(), path.to_vec())
    };
    let mut selection: Option<Selection> = None;
    let mut walk: Vec<Path> = Vec::new();
    while walk.len() < 200 {
        match step_selection(
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
    let library = crate::stack::load::<()>().libraries;
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
        Some(Claim::Direct(Hovered::Tree(Hover::Insert(path)))) if *path == first.path
    ));
}

#[test]
fn block_gaps_are_unclaimed_air_and_brackets_widen() {
    let doc = gap_document();
    let (bench, _) = place(&doc, None, 560.0);
    let (upper, lower) = elements_of(&bench, "body", true);
    let parent = vec![key("body")];
    let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
    // Between the rows nothing claims: the gap is air, and air is
    // the SHELL's backstop — hold-or-clear by reach, never the
    // container outright.
    let (air, _) = place_with_pointer(
        &doc,
        None,
        560.0,
        Some(Point::new(upper.rect.center().x, gap_y)),
    );
    assert!(air.hit.is_none());
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
