//! Real projection and dispatch fixtures, without a window.

use super::*;
use kurbo::Stroke;
use peniko::ImageData;
use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use puri::hover::Claim;

type World = crate::Editor;

/// The keyboard-navigation fixture, opened from disk rather than the
/// Examples menu.
const NAVIGATION_EXAMPLE: &str = include_str!("../../../../../examples/navigation.gid");

#[derive(Clone, Copy, Default)]
struct FrameTimes {
    prepare: std::time::Duration,
    choices: std::time::Duration,
    placement: std::time::Duration,
    hover: std::time::Duration,
    paint: std::time::Duration,
}

struct Bench {
    handler: puri::handler::Handler<World, crate::placed::DispatchContext<World>>,
    list: DrawList,
    descends: Vec<Descend<World>>,
    /// What the probe answered for the pass's pointer input.
    hit: Option<Claim<Hovered>>,
    times: FrameTimes,
    frame_elapsed: std::time::Duration,
}

#[test]
fn plain_reads_text_numbers_and_absences_but_keeps_calls_records() {
    use crate::libraries::{absent, f64};
    use crate::workspace::Projection;
    let (words, number, sum, missing) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let doc = Document {
        root: Some(Value::record([
            (words, text::value("hello")),
            (number, f64::value(3.0)),
            (sum, grap::call(f64::vocabulary::SUM.into(), [])),
            (
                missing,
                absent::with_reason(absent::vocabulary::NO_ALTERNATIVE),
            ),
        ])),
        cells: Cells::new(),
    };
    // How many glyphs each field draws, and whether the call's function
    // field is drawn as a field of its own.
    let drawn = |projection| {
        let mut editor = crate::test_editor_with_stack(doc.clone(), crate::stack::load());
        editor.model.workspace.document.projection = projection;
        let driver = crate::driver::Driver::new(editor, kurbo::Size::new(900.0, 600.0));
        let glyphs = |field| {
            let rect = driver.rect(&[Step::Key(field)]);
            crate::display::test_support::painted(&driver.painted().0)
                .filter_map(|command| match command {
                    DrawCmd::GlyphRun(run) => Some(run),
                    _ => None,
                })
                .flat_map(|run| {
                    run.glyphs
                        .iter()
                        .map(|glyph| run.transform * Point::new(glyph.x.into(), glyph.y.into()))
                        .collect::<Vec<_>>()
                })
                .filter(|glyph| rect.contains(*glyph))
                .count()
        };
        (
            glyphs(words),
            glyphs(number),
            driver.drawn(&[Step::Key(sum), Step::Key(grap::vocabulary::FUNCTION)]),
            driver.drawn(&[Step::Key(missing), Step::Key(absent::vocabulary::ABSENT)]),
        )
    };
    let (words_plain, number_plain, function_plain, reason_plain) = drawn(Projection::Plain);
    let (words_raw, number_raw, _, reason_raw) = drawn(Projection::Raw);
    // Plain reads "hello" and 3; Raw spells out their bytes.
    assert!(words_plain >= "hello".len(), "{words_plain}");
    assert!(words_plain < words_raw, "{words_plain} < {words_raw}");
    assert!(number_plain < number_raw, "{number_plain} < {number_raw}");
    assert!(function_plain, "a call stays a record");
    // Plain reads an absent value as its reason; Raw draws the reason's field.
    assert!(!reason_plain, "Plain draws the reason as the value itself");
    assert!(reason_raw, "Raw draws the reason as a field");
}

#[test]
fn an_absent_value_reads_as_its_reason_then_its_fields_all_on_red() {
    use crate::libraries::{absent, f64};
    let washes = |root: Value| {
        let doc = Document {
            root: Some(root),
            cells: Cells::new(),
        };
        let error = peniko::Brush::from(crate::styles::Theme::Light.palette().error);
        place(&doc, None, 600.0)
            .0
            .list
            .0
            .into_iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(wash),
                    brush,
                    ..
                } if brush == error => Some(wash.rect()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Each cause of a failed alternative is washed too, inside its failure's.
    let failure = washes(absent::from_causes([
        absent::with_reason(absent::vocabulary::DECLINED),
        ::grap::absent::with_detail(
            ::grap::absent::MISSING_ARGUMENT,
            absent::vocabulary::VALUE,
            f64::value(3.0),
        ),
    ]));
    assert_eq!(failure.len(), 3, "{failure:?}");
    for cause in &failure[1..] {
        assert_eq!(failure[0].union(*cause), failure[0], "{failure:?}");
    }
    assert!(washes(Value::record([(new_cell_id(), f64::value(3.0))])).is_empty());
}

fn click_at(world: &mut crate::Editor, point: Point) -> Option<Hovered> {
    let frame = editing_frame(world, false);
    let Some((_, Claim::Direct(target))) = frame.hover_geometry.probe(Some(point), None, 0.0)
    else {
        return None;
    };
    let mut input =
        crate::placed::DispatchContext::new(Some(crate::test_root()), Some(target.clone()));
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
    );
    Some(target)
}

/// Paint and unpack the output of the completed hover pass.
fn settle(mut placed: crate::placed::HoverOutput<World>) -> Bench {
    let binding = std::time::Instant::now();
    #[cfg(feature = "layout-profile")]
    let profile = crate::display::profile::enter(crate::display::profile::Kind::Hover);
    let hit = placed.claim.take();
    let hit = hit.map(|(_, claim)| claim);
    let hovered = match &hit {
        Some(Claim::Direct(hover)) => Some(hover.clone()),
        _ => None,
    };
    let landmark = match &hovered {
        Some(Hovered::Tree(Hover::Value(path))) => {
            placed.descends.iter().find(|descend| descend.path == *path)
        }
        _ => None,
    };
    let hovered_secondary = match &hovered {
        Some(Hovered::Tree(hover)) => hover_secondary(landmark, placed.completion.as_ref(), hover),
        _ => None,
    };
    let selected_secondary =
        crate::display::widget::navigation::selected_secondary(&placed.descends);
    let frame = placed.bind(crate::placed::Attribution {
        hovered,
        hovered_secondary,
        selected_secondary,
        hovered_trace: None,
    });
    let hover = binding.elapsed();
    #[cfg(feature = "layout-profile")]
    drop(profile);
    let mut bench = Bench {
        handler: frame.handler.unwrap_or_default(),
        list: DrawList::new(),
        descends: frame.descends,
        hit,
        times: FrameTimes {
            hover,
            ..Default::default()
        },
        frame_elapsed: std::time::Duration::ZERO,
    };
    puri::frame::render(frame.renders, &mut bench);
    bench
}

impl puri::draw::CanvasSink for Bench {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        self.list.mesh(scene, transform);
    }
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.list.image(image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }
    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.list.stroke(shape, style, brush, transform);
    }
    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.list.glyph_run(run);
    }
    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        let _ = (shape, transform);
        content(self);
    }
}

/// Configure the editor's projection slots as a tutorial page does.
fn tutorial_slots(world: &mut crate::Editor, slots: &[CellId]) {
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(CellId::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
        &world.stack.libraries,
    )
    .unwrap();
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
    place_with_annotations(
        doc,
        selection,
        &Annotations::default(),
        width,
        pointer,
        viewport,
        None,
    )
}

fn place_with_annotations(
    doc: &Document,
    selection: Option<&Selection>,
    annotations: &Annotations,
    width: f64,
    pointer: Option<Point>,
    viewport: Option<Rect>,
    root: Option<&[Step]>,
) -> (Bench, Extent) {
    BenchContext::new().place(doc, selection, annotations, width, pointer, viewport, root)
}

struct BenchContext {
    computations: crate::computations::Computations,
    stack: crate::stack::Stack<World>,
    styles: crate::styles::Styles,
    fonts: parley::FontContext,
    layouts: parley::LayoutContext<Brush>,
    cache: puri::text::TextCache,
}

struct BenchFrame<'a> {
    doc: &'a Document,
    selection: Option<&'a Selection>,
    annotations: &'a Annotations,
    width: f64,
    origin: Point,
    pointer: Option<Point>,
    viewport: Option<Rect>,
    root: &'a [Step],
}

impl BenchContext {
    fn new() -> Self {
        Self {
            computations: crate::computations::Computations::default(),
            stack: crate::stack::load(),
            styles: crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0),
            fonts: crate::fonts::bundled_font_context(),
            layouts: parley::LayoutContext::new(),
            cache: puri::text::TextCache::default(),
        }
    }

    fn place(
        &mut self,
        doc: &Document,
        selection: Option<&Selection>,
        annotations: &Annotations,
        width: f64,
        pointer: Option<Point>,
        viewport: Option<Rect>,
        root: Option<&[Step]>,
    ) -> (Bench, Extent) {
        let (bench, extent) = self.frame(BenchFrame {
            doc,
            selection,
            annotations,
            width: width - 48.0,
            origin: Point::new(24.0, 24.0),
            pointer,
            viewport,
            root: root.unwrap_or(&[]),
        });
        eprintln!(
            "frame at {width:.0}px: {:.1?} (project {:.1?})",
            bench.frame_elapsed,
            bench.times.prepare + bench.times.choices,
        );
        (bench, extent)
    }

    fn frame(&mut self, input: BenchFrame<'_>) -> (Bench, Extent) {
        let BenchFrame {
            doc,
            selection,
            annotations,
            width,
            origin,
            pointer,
            viewport,
            root,
        } = input;
        let Self {
            computations,
            stack,
            styles,
            fonts,
            layouts,
            cache,
        } = self;
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        let sources = Sources {
            doc,
            libraries: &stack.libraries,
        };
        let mut tcx = TextCtx {
            fonts,
            layouts,
            scale: styles.scale as f32,
            cache,
        };

        // Test-only phase timings; normal frames contain no timers.
        let start = std::time::Instant::now();
        let root_path = root;
        let root = sources.resolve_path(root_path);
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
        let graph = prepare_project(
            ProjectDescription {
                focused: true,
                computations: Some(computations),
                view: &crate::test_root(),
                completions: Some(&stack.completions),
                sources,
                root,
                root_path,
                selection,
                source_selection: selection,
                annotations,
                raw: false,
                styles,
                width,
                projection: Some(&stack.projection),
            },
            &mut tcx,
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let prepare = start.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Choices);
        let node = resolve_choices(
            graph,
            width,
            std::env::var_os("PROGRED_LAYOUT_TRACE").is_some(),
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let choices = phase.elapsed();
        let phase = std::time::Instant::now();
        let extent = node.extent;
        let rect = node.extent.rect_at(origin);
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Placement);
        let placed = crate::display::widget::frame::place(
            node,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
            &crate::display::widget::HoverInput {
                pointer,
                reach_px: crate::frame::HOVER_REACH_POINTS * styles.scale,
                ..Default::default()
            },
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let placement = phase.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Paint);
        let mut settled = settle(placed);
        #[cfg(feature = "layout-profile")]
        drop(profile);
        settled.times = FrameTimes {
            prepare,
            choices,
            placement,
            hover: settled.times.hover,
            paint: phase.elapsed().saturating_sub(settled.times.hover),
        };
        settled.frame_elapsed = start.elapsed();
        (settled, extent)
    }
}

fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
    place_with_pointer(doc, selection, width, None)
}

mod completion;
#[cfg(not(target_arch = "wasm32"))]
mod compositor;
mod conject;
mod custom_navigation;
mod declarations;
mod document_libraries;
mod drawing;
mod fidget_shapes;
mod fidget_source;
mod focus;
mod folds;
mod interaction;
mod iop_tree_native;
mod layout;
mod logical_navigation;
mod navigation;
mod numbers;
mod outline;
mod profile;
mod svg;
mod tutorial;
