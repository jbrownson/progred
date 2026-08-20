/// Headless visual bench: the sample document through the real
/// projection, written as an SVG — the qlmanage trick for the whole
/// editor frame, no window needed. `cargo test -p progred svg_bench`
/// writes target/raw_projection.svg.
use super::*;
use progred_libraries::{name, text};
use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::fmt::Write as _;
use vello::kurbo::{BezPath, Shape as KurboShape};

type World = ();

struct Bench {
    list: DrawList,
    descends: Vec<Descend>,
    /// What the probe answered for the pass's pointer input.
    hit: Option<Claim<Hovered>>,
}

/// Probe with the pointer, then render and unpack the placed frame.
fn settle(placed: Placed<World, Bench>, pointer: Option<Point>) -> Bench {
    let hit = pointer.and_then(|point| placed.probe(point));
    let hovered = match &hit {
        Some(Claim::Names(hover)) => Some(hover.clone()),
        _ => None,
    };
    let Placed {
        descends, renders, ..
    } = placed;
    let mut bench = Bench {
        list: DrawList::new(),
        descends,
        hit,
    };
    let ink = crate::placed::Ink {
        hovered: hovered.as_ref(),
        hovered_value: None,
    };
    for render in renders {
        render(&mut bench, ink);
    }
    bench
}

impl Canvas for Bench {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }
    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        self.list.stroke(shape, style, brush, transform);
    }
    fn glyph_run(&mut self, run: GlyphRun) {
        self.list.glyph_run(run);
    }
    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let _ = (shape.into(), transform);
        content(self);
    }
}

fn css(brush: &Brush) -> String {
    match brush {
        Brush::Solid(color) => {
            let [r, g, b, a] = color.components;
            format!(
                "rgba({},{},{},{:.3})",
                (r * 255.0).round(),
                (g * 255.0).round(),
                (b * 255.0).round(),
                a
            )
        }
        _ => "magenta".to_string(),
    }
}

struct BezPen {
    path: BezPath,
    offset: Point,
}

impl OutlinePen for BezPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path
            .move_to((self.offset.x + x as f64, self.offset.y - y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path
            .line_to((self.offset.x + x as f64, self.offset.y - y as f64));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(
            (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
            (self.offset.x + x as f64, self.offset.y - y as f64),
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.curve_to(
            (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
            (self.offset.x + cx1 as f64, self.offset.y - cy1 as f64),
            (self.offset.x + x as f64, self.offset.y - y as f64),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

fn svg_shape(shape: &Shape, transform: Affine) -> String {
    let mut path = match shape {
        Shape::Rect(rect) => rect.to_path(0.05),
        Shape::RoundedRect(rect) => rect.to_path(0.05),
        Shape::Circle(circle) => circle.to_path(0.05),
        Shape::Line(line) => {
            let mut p = BezPath::new();
            p.move_to(line.p0);
            p.line_to(line.p1);
            p
        }
        Shape::Path(path) => path.clone(),
    };
    path.apply_affine(transform);
    path.to_svg()
}

fn write_cmds(out: &mut String, cmds: &[DrawCmd]) {
    for cmd in cmds {
        match cmd {
                DrawCmd::Fill {
                    shape,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="{}"/>"#,
                    svg_shape(shape, *transform),
                    css(brush)
                )
                .unwrap(),
                DrawCmd::Stroke {
                    shape,
                    style,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_shape(shape, *transform),
                    css(brush),
                    style.width
                )
                .unwrap(),
                DrawCmd::GlyphRun(run) => {
                    let Ok(font_ref) = FontRef::from_index(run.font.data.as_ref(), run.font.index)
                    else {
                        continue;
                    };
                    let outlines = font_ref.outline_glyphs();
                    let coords: Vec<NormalizedCoord> = run
                        .normalized_coords
                        .iter()
                        .map(|bits| NormalizedCoord::from_bits(*bits))
                        .collect();
                    let size = Size::new(run.size);
                    let mut path = BezPath::new();
                    for glyph in &run.glyphs {
                        let mut pen = BezPen {
                            path: std::mem::take(&mut path),
                            offset: run.transform * Point::new(glyph.x as f64, glyph.y as f64),
                        };
                        if let Some(outline) = outlines.get(GlyphId::new(glyph.id)) {
                            let settings = DrawSettings::unhinted(size, LocationRef::new(&coords));
                            let _ = outline.draw(settings, &mut pen);
                        }
                        path = pen.path;
                    }
                    writeln!(out, r#"<path d="{}" fill="{}"/>"#, path.to_svg(), css(&run.brush))
                        .unwrap();
                }
                DrawCmd::Clip { children, .. } => write_cmds(out, children),
            }
    }
}

fn place_with_pointer(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    pointer: Option<Point>,
) -> (Bench, Extent) {
    place_with_inputs(doc, selection, width, pointer, None)
}

fn place_with_inputs(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    pointer: Option<Point>,
    viewport: Option<Rect>,
) -> (Bench, Extent) {
    let stack = crate::stack::load::<World>();
    let sources = Sources {
        doc,
        library: &stack.library,
    };
    let styles = crate::styles::editor(1.0);
    let collapse = Annotations::default();
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let hooks = Hooks::<World> {
        select: Rc::new(|_, _, _| {}),
        toggle: Rc::new(|_, _| {}),
        rename: Rc::new(|_, _, _| {}),
        edit: Rc::new(|_| None),
        pick: Rc::new(|_, _| false),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_| false),
        apply: Rc::new(|_, _, _| false),
    };
    // Timed as the layout perf canary: a projection is a
    // per-keystroke cost, and the fallback-heavy narrow widths
    // are where accidental exponentials have surfaced twice.
    // Numbers only, no assert (user call) — read them when the
    // bench runs; single-digit milliseconds is healthy.
    let start = std::time::Instant::now();
    let node = project::<World, Bench>(
        ProjectDescription {
            sources,
            selection,
            graph_node: None,
            annotations: &collapse,
            raw: false,
            styles: &styles,
            width: width - 48.0,
            projection: Some(&stack.projection),
            foreign: &stack.foreign,
        },
        &mut tcx,
        hooks,
    );
    let elapsed = start.elapsed();
    eprintln!("project at {width:.0}px: {elapsed:.1?}");
    let extent = node.extent;
    let rect = node.extent.rect_at(Point::new(24.0, 24.0));
    let placed = measured::place(
        node,
        match viewport {
            Some(clip_rect) => Placement::new(rect, clip_rect),
            None => Placement::root(rect),
        },
    );
    (settle(placed, pointer), extent)
}

fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
    place_with_pointer(doc, selection, width, None)
}

fn render(doc: &Document, selection: Option<&Selection>, width: f64, out_path: &str) {
    let (bench, extent) = place(doc, selection, width);
    let (width, height) = (width.max(extent.width + 48.0), extent.height() + 48.0);
    let mut out = String::new();
    writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        )
        .unwrap();
    writeln!(
        out,
        r##"<rect width="{width:.0}" height="{height:.0}" fill="#FFFFFF"/>"##
    )
    .unwrap();
    write_cmds(&mut out, &bench.list.0);
    writeln!(out, "</svg>").unwrap();
    std::fs::write(out_path, out).unwrap();
}

#[test]
fn svg_bench_renders_the_sample_projection() {
    let doc = sample_document();
    render(&doc, None, 900.0, "../target/raw_projection.svg");
    render(&doc, None, 560.0, "../target/raw_projection_narrow.svg");
    // The deep-fallback regime: hugging fails at most levels, so
    // this render is also the canary against layout cost blowing
    // up when width is scarce.
    render(&doc, None, 320.0, "../target/raw_projection_tight.svg");
}

#[test]
fn svg_bench_renders_the_grap_demo() {
    let (doc, _) = crate::gid_text::parse(include_str!("../../../grap-demo.gid"))
        .expect("the Grap demo parses");
    render(&doc, None, 900.0, "../target/grap_demo.svg");
    render(&doc, None, 560.0, "../target/grap_demo_narrow.svg");
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
            .filter(|descend| descend.path == [Step::Key(field)])
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
    let (doc, binders) = crate::gid_text::parse(include_str!("../../../grap-demo.gid"))
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
    result.push(Step::Key(grap::vocabulary::GRAP));
    let mut source_note = result.clone();
    source_note.push(Step::Key(binders["note"]));
    let (bench, _) = place(&doc, None, 560.0);
    assert!(bench.descends.iter().any(|descend| descend.path == record));
    assert!(bench.descends.iter().any(|descend| descend.path == result));
    assert!(
        bench
            .descends
            .iter()
            .any(|descend| descend.path == source_note)
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
            .find(|descend| &descend.path == path)
            .expect("walk stops on placed descends")
            .rect
    };
    let select = |path: &Path| crate::selection::bare_edge(path.clone());
    let mut selection: Option<Selection> = None;
    let mut walk: Vec<Path> = Vec::new();
    while walk.len() < 200 {
        match step_selection(
            &bench.descends,
            selection.as_ref(),
            line,
            &press(NamedKey::ArrowDown),
        ) {
            Some(path) => {
                selection = Some(select(&path));
                walk.push(path);
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
            selection.as_ref(),
            line,
            &press(NamedKey::ArrowUp),
        )
        .expect("up retraces the walk");
        assert_eq!(&up, expect);
        selection = Some(select(&up));
    }
    // A projected simple-name field is the cell's editable head.
    let head = bench
        .descends
        .iter()
        .map(|descend| descend.path.clone())
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
            Some(&select(&cell)),
            line,
            &press(NamedKey::ArrowRight),
        ),
        Some(head)
    );
}

fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

#[test]
fn placement_claims_the_hover_innermost_last() {
    let doc = sample_document();
    let (bench, _) = place(&doc, None, 560.0);
    let library = crate::stack::load::<()>().library;
    let sources = Sources {
        doc: &doc,
        library: &library,
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
                .resolve(&descend.path)
                .is_some_and(|value| text::read(value).is_some())
                && projected_name_owner(&descend.path).is_none()
        })
        .expect("the sample has a string leaf");
    let string_rect = string.rect;
    let string_path = string.path.clone();
    let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(string_rect.center()));
    assert!(matches!(
        &bench.hit,
        Some(Claim::Names(Hovered::Tree(Hover::Value(path)))) if *path == string_path
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
fn hovering_a_field_label_paints_the_hover_wash() {
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
            Some(Claim::Names(Hovered::Tree(Hover::Label(path))))
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
fn elements_of(bench: &Bench, field: &str, by_y: bool) -> (Descend, Descend) {
    let mut found: Vec<&Descend> = bench
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
    let clone = |d: &Descend| Descend {
        path: d.path.clone(),
        rect: d.rect,
    };
    (clone(found[0]), clone(found[1]))
}

#[test]
fn flat_separators_claim_the_insert_between() {
    let doc = gap_document();
    let (bench, _) = place(&doc, None, 560.0);
    let (first, second) = elements_of(&bench, "tags", false);
    let mid = Point::new(
        (first.rect.x1 + second.rect.x0) / 2.0,
        first.rect.center().y,
    );
    let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(mid));
    assert!(matches!(
        &bench.hit,
        Some(Claim::Names(Hovered::Tree(Hover::Insert(path)))) if *path == first.path
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
    let styles = crate::styles::editor(1.0);
    let list = bench
        .descends
        .iter()
        .find(|descend| descend.path == parent)
        .expect("the list has a landmark");
    let (claimed, _) = place_with_pointer(
        &doc,
        None,
        560.0,
        Some(Point::new(
            list.rect.x0 + delim_advance(&styles, Delim::Bracket) + 1.0,
            gap_y,
        )),
    );
    assert!(matches!(
        &claimed.hit,
        Some(Claim::Names(Hovered::Tree(Hover::Value(path)))) if *path == parent
    ));
}

#[test]
fn popup_rows_claim_their_entries_and_the_card_occludes() {
    let popup = Popup {
        anchor: Rect::new(0.0, 0.0, 10.0, 10.0),
        entries: vec![
            Entry {
                display: "\"x\"".to_string(),
                detail: None,
                matches: Vec::new(),
                id: false,
                action: EntryAction::Value(crate::test_values::text("x")),
            },
            Entry {
                display: "new list".to_string(),
                detail: None,
                matches: Vec::new(),
                id: false,
                action: EntryAction::NewList,
            },
        ],
    };
    let place_card = |pointer| {
        let mut fonts = parley::FontContext::new();
        let mut layouts = parley::LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let card = popup_view::<World, Bench>(
            &mut tcx,
            &crate::styles::editor(1.0),
            &popup.entries,
            0,
            |_, _| {},
        );
        let extent = card.extent;
        let placed = measured::place_top_left(card, Point::ZERO);
        (settle(placed, Some(pointer)), extent)
    };
    // The card's own padding claims-and-clears: an overlay's
    // pointer never falls through to what sits beneath it.
    let (padding, extent) = place_card(Point::new(1.0, 1.0));
    assert_eq!(padding.hit, Some(Claim::Occludes));
    // Scanning down the card crosses both rows, each claiming its
    // index — an address into the live entries, never a snapshot.
    let winners: Vec<Hover> = (0..extent.height() as usize)
        .filter_map(|y| {
            let (bench, _) = place_card(Point::new(extent.width / 2.0, y as f64 + 0.5));
            match bench.hit {
                Some(Claim::Names(Hovered::Tree(hover))) => Some(hover),
                _ => None,
            }
        })
        .collect();
    assert!(winners.contains(&Hover::Entry(0)));
    assert!(winners.contains(&Hover::Entry(1)));
}

#[test]
fn svg_bench_renders_the_placeholder_notation() {
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    render(&empty, None, 320.0, "../target/raw_placeholder_root.svg");
    // The engaged twin: same slot, same rect, selection blue.
    render(
        &empty,
        Some(&pending_value(Vec::new())),
        320.0,
        "../target/raw_placeholder_engaged.svg",
    );
    let cells = Cells::new();
    let bare = new_cell_id();
    render(
        &Document {
            root: Some(Value::from(bare)),
            cells,
        },
        None,
        320.0,
        "../target/raw_placeholder_cell.svg",
    );
    // The commit transition pair: the same spelling typed in the
    // slot and committed as the string — glyphs should not move.
    render(
        &empty,
        Some(&crate::selection::pending_with_query(Vec::new(), "\"asdf\"")),
        320.0,
        "../target/raw_placeholder_typed.svg",
    );
    render(
        &Document {
            root: Some(crate::test_values::text("asdf")),
            cells: Cells::new(),
        },
        Some(&crate::selection::bare_edge(Vec::new())),
        320.0,
        "../target/raw_placeholder_committed.svg",
    );
    // The empty string under its write-through editor: quotes
    // stay snug, no slot minimum applies to string literals.
    let empty_string = Document {
        root: Some(crate::test_values::text("")),
        cells: Cells::new(),
    };
    let stack = crate::stack::load::<World>();
    let sel = Selection::edge(
        &Sources {
            doc: &empty_string,
            library: &stack.library,
        },
        &stack.projection,
        Vec::new(),
    );
    render(
        &empty_string,
        Some(&sel),
        320.0,
        "../target/raw_empty_string_editing.svg",
    );
}

// The re-opened label: the tags field's query seeded with its
// quoted spelling, ringed in place, the value staying put.
#[test]
fn svg_bench_renders_a_label_rename() {
    let doc = sample_document();
    let library = crate::stack::load::<()>().library;
    let path = vec![
        Step::Key(crate::test_values::label("shape")),
        Step::Follow,
        Step::Key(crate::test_values::label("tags")),
    ];
    let rename = pending_rename(
        &Sources {
            doc: &doc,
            library: &library,
        },
        &path,
    )
    .unwrap();
    render(&doc, Some(&rename), 560.0, "../target/raw_label_rename.svg");
}

#[test]
fn svg_bench_renders_a_pending_edge() {
    let doc = sample_document();
    let library = crate::stack::load::<()>().library;
    let edge = pending_edge(
        &Sources {
            doc: &doc,
            library: &library,
        },
        vec![Step::Key(crate::test_values::label("shape"))],
    )
    .unwrap();
    assert_eq!(edge.stage(), crate::selection::Stage::Label);
    let typing = edge.with_query("na");
    render(&doc, Some(&typing), 560.0, "../target/raw_pending_edge.svg");
}
