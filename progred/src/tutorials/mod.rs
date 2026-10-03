//! The website's tutorials, followed step by step. Each embedded editor opens
//! as its page configures it, and the reader's input reaches it the way the
//! browser delivers it: pointer motion and buttons through Winit, keys as the
//! page hands them over, a painted frame after each.

use crate::EditorRunner;
use crate::frame::Hovered;
use gid::{CellId, Position, Step, Value};
use kurbo::{Point, Rect, Size};
use puri::draw::{DrawCmd, DrawList, Shape};
use puri::hover::Claim;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::PointerEvent;
use ui_events_winit::WindowEventTranslation;
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceId, ElementState, MouseButton, WindowEvent};
use winit::keyboard::ModifiersState;

mod peel;
mod story;
mod tutorial;

/// The tutorial column's frame less its border, tall enough that no lesson
/// scrolls.
const VIEWPORT: Size = Size::new(704.0, 1400.0);
const SCALE: f64 = 1.0;
const FOLLOW: Step = Step::Follow(gid::Resolution::Document);

fn website(path: &str) -> String {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../website/public")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// The editor address a page embeds in its section `id`, and the tasks the
/// page numbers for it.
fn section(page: &str, id: &str) -> (String, Vec<String>) {
    let html = website(page);
    let start = html
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("{page} has no #{id}"));
    let section = &html[start..];
    let section = &section[..section.find("</section>").unwrap_or(section.len())];
    let tag = &section[section
        .find("<iframe")
        .unwrap_or_else(|| panic!("#{id} embeds no editor"))..];
    let tag = &tag[..tag.find('>').unwrap()];
    let src = [" src=\"", " data-src=\""]
        .iter()
        .find_map(|attribute| {
            let value = &tag[tag.find(attribute)? + attribute.len()..];
            Some(value[..value.find('"')?].replace("&amp;", "&"))
        })
        .expect("the editor's address");
    let tasks = section
        .split("data-task=\"")
        .skip(1)
        .map(|rest| rest[..rest.find('"').unwrap()].to_owned())
        .collect();
    (src, tasks)
}

/// An editor embedded in a tutorial page.
pub(super) struct Embed {
    runner: EditorRunner,
    names: crate::gid_text::Binders,
    /// The page's choice of libraries and slots, which peeling draws from.
    libraries: Option<String>,
    slots: Option<String>,
    painted: DrawList,
    held: ModifiersState,
    tasks: Vec<String>,
    done: usize,
}

impl Embed {
    pub(super) fn open(page: &str, id: &str) -> Self {
        let (src, tasks) = section(page, id);
        let (_, query) = src.split_once('?').expect("the editor's options");
        let option = |name: &str| {
            query
                .split('&')
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                .map(str::to_owned)
        };
        let document = option("document").expect("a document");
        let (doc, names) = crate::gid_text::parse(&website(
            document
                .strip_prefix("../")
                .expect("a document beside the editor"),
        ))
        .unwrap();
        let libraries = option("libraries");
        let slots = option("tutorial-slots");
        let mut stack = crate::web_embed::libraries(libraries.as_deref()).unwrap();
        stack.projection =
            crate::web_embed::tutorial_slots(slots.as_deref(), stack.projection, &stack.libraries)
                .unwrap();
        let mut editor = crate::test_editor_with_stack(doc, stack);
        editor.command_modifier = puri::keyboard::CommandModifier::Control;
        editor.drawn_menu = option("menu").as_deref() != Some("hidden");
        let mut runner = EditorRunner::new(editor);
        runner.refresh_frame(SCALE, VIEWPORT);
        let mut embed = Self {
            runner,
            names,
            libraries,
            slots,
            painted: DrawList::new(),
            held: ModifiersState::empty(),
            tasks,
            done: 0,
        };
        embed.present();
        embed
    }

    /// Begin the page's next numbered task, which must be `id`.
    pub(super) fn task(&mut self, id: &str) {
        assert_eq!(
            self.tasks.get(self.done).map(String::as_str),
            Some(id),
            "the page's tasks are {:?}",
            self.tasks
        );
        self.done += 1;
    }

    pub(super) fn id(&self, name: &str) -> CellId {
        *self
            .names
            .get(name)
            .unwrap_or_else(|| panic!("the document names no {name:?}"))
    }

    pub(super) fn key(&self, name: &str) -> Step {
        Step::Key(self.id(name))
    }

    /// A path by the document's names, `follow` stepping into a cell.
    pub(super) fn path(&self, steps: &[&str]) -> Vec<Step> {
        steps
            .iter()
            .map(|step| match *step {
                "follow" => FOLLOW,
                name => self.key(name),
            })
            .collect()
    }

    /// `rest` within item `index` of the list at `list`.
    pub(super) fn element(&self, list: &[Step], index: usize, rest: &[Step]) -> Vec<Step> {
        [
            list,
            &[Step::Element(self.positions(list)[index].clone())],
            rest,
        ]
        .concat()
    }

    // Reading what's there.

    pub(super) fn editor(&self) -> &crate::Editor {
        &self.runner.editor
    }

    pub(super) fn value(&self, path: &[Step]) -> Value {
        self.editor()
            .sources()
            .resolve_path(path)
            .cloned()
            .unwrap_or_else(|| panic!("nothing at {path:?}"))
    }

    pub(super) fn cell(&self, name: &str) -> Value {
        self.editor()
            .model
            .doc
            .cells
            .value(self.id(name))
            .cloned()
            .unwrap_or_else(|| panic!("{name} is empty"))
    }

    pub(super) fn number(&self, path: &[Step]) -> Option<f64> {
        crate::libraries::f64::read(&self.value(path))
    }

    pub(super) fn text(&self, path: &[Step]) -> Option<String> {
        crate::libraries::text::read(&self.value(path)).map(str::to_owned)
    }

    pub(super) fn positions(&self, list: &[Step]) -> Vec<Position> {
        self.value(list)
            .as_list()
            .unwrap_or_else(|| panic!("no list at {list:?}"))
            .keys()
            .cloned()
            .collect()
    }

    pub(super) fn texts(&self, list: &[Step]) -> Vec<String> {
        self.value(list)
            .as_list()
            .unwrap_or_else(|| panic!("no list at {list:?}"))
            .values()
            .map(|value| {
                crate::libraries::text::read(value)
                    .unwrap_or("?")
                    .to_owned()
            })
            .collect()
    }

    /// What the `{render: …}` at `path` asks to draw.
    pub(super) fn rendered(&self, path: &[Step]) -> Value {
        self.computed(path, crate::libraries::presentation::vocabulary::RENDER)
    }

    /// What the `{evaluate: …}` at `path` computes.
    pub(super) fn result(&self, path: &[Step]) -> Value {
        self.computed(path, ::grap::vocabulary::EVALUATE)
    }

    fn computed(&self, path: &[Step], key: CellId) -> Value {
        let value = self.value(path);
        let expression = value
            .as_record()
            .and_then(|fields| fields.get(&key))
            .unwrap_or_else(|| panic!("nothing to compute at {path:?}"));
        let evaluation =
            ::grap::evaluate_value(expression, &self.editor().sources(), ::grap::DEFAULT_FUEL);
        assert!(evaluation.completed);
        evaluation.result
    }

    pub(super) fn selection(&self) -> Vec<Step> {
        self.editor()
            .model
            .selection
            .as_ref()
            .expect("a selection")
            .path()
            .to_vec()
    }

    pub(super) fn source_selection(&self) -> Option<Vec<Step>> {
        self.editor()
            .model
            .selection
            .as_ref()?
            .source_path()
            .map(|path| path.to_vec())
    }

    pub(super) fn drawn(&self, path: &[Step]) -> bool {
        self.landmark(path).is_some()
    }

    fn landmark(&self, path: &[Step]) -> Option<&crate::navigate::Descend<crate::Editor>> {
        self.runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|landmark| landmark.path.as_ref() == path)
    }

    /// Where `path` is drawn.
    pub(super) fn rect(&self, path: &[Step]) -> Rect {
        self.landmark(path)
            .map(|landmark| landmark.rect)
            .unwrap_or_else(|| {
                panic!(
                    "nothing drawn at {path:?}; drawn: {:#?}",
                    self.runner
                        .frame
                        .dispatch
                        .descends
                        .iter()
                        .map(|landmark| landmark.path.clone())
                        .collect::<Vec<_>>()
                )
            })
    }

    /// The middle of the text drawn at `path`: its largest run of glyphs,
    /// not a subscript or a quote mark beside it.
    pub(super) fn text_at(&self, path: &[Step]) -> Point {
        fn runs<'a>(commands: &'a [DrawCmd], into: &mut Vec<&'a puri::draw::GlyphRun>) {
            for command in commands {
                match command {
                    DrawCmd::GlyphRun(run) => into.push(run),
                    DrawCmd::Clip { children, .. } => runs(children, into),
                    _ => {}
                }
            }
        }
        let rect = self.rect(path);
        let mut found = Vec::new();
        runs(&self.painted.0, &mut found);
        found
            .into_iter()
            .filter_map(|run| {
                let glyphs = run
                    .glyphs
                    .iter()
                    .map(|glyph| run.transform * Point::new(glyph.x.into(), glyph.y.into()))
                    .collect::<Vec<_>>();
                (!glyphs.is_empty() && glyphs.iter().all(|glyph| rect.contains(*glyph)))
                    .then_some((run.size, glyphs))
            })
            .max_by(|(size, glyphs), (other_size, others)| {
                size.total_cmp(other_size)
                    .then(glyphs.len().cmp(&others.len()))
            })
            .map(|(size, glyphs)| {
                let span = glyphs[glyphs.len() - 1].x + f64::from(size) * 0.5 - glyphs[0].x;
                Point::new(glyphs[0].x + span / 2.0, rect.center().y)
            })
            .unwrap_or_else(|| panic!("no text drawn at {path:?}"))
    }

    /// Every landmark drawn under `prefix`, and where.
    pub(super) fn drawn_under(&self, prefix: &[Step]) -> Vec<(Rc<[Step]>, Rect)> {
        self.runner
            .frame
            .dispatch
            .descends
            .iter()
            .filter(|landmark| landmark.path.starts_with(prefix))
            .map(|landmark| (landmark.path.clone(), landmark.rect))
            .collect()
    }

    /// Every drawn occurrence that shares the selection's identity.
    pub(super) fn lit(&self) -> Vec<Rc<[Step]>> {
        let descends = &self.runner.frame.dispatch.descends;
        let Some(selected) = crate::display::widget::navigation::selected_secondary(descends)
        else {
            return Vec::new();
        };
        descends
            .iter()
            .filter(|landmark| landmark.secondary.as_ref() == Some(&selected))
            .map(|landmark| landmark.path.clone())
            .collect()
    }

    /// The circles the last frame painted: where on screen, the circle in
    /// its drawing's own coordinates, and its paint.
    pub(super) fn circles(&self) -> Vec<(Point, kurbo::Circle, puri::Brush)> {
        fn circles(commands: &[DrawCmd], into: &mut Vec<(Point, kurbo::Circle, puri::Brush)>) {
            for command in commands {
                match command {
                    DrawCmd::Fill {
                        shape: Shape::Circle(circle),
                        brush,
                        transform,
                    } => into.push((*transform * circle.center, *circle, brush.clone())),
                    DrawCmd::Clip { children, .. } => circles(children, into),
                    _ => {}
                }
            }
        }
        let mut found = Vec::new();
        circles(&self.painted.0, &mut found);
        found
    }

    /// The images the last frame painted, as their pixels.
    pub(super) fn images(&self) -> Vec<Vec<u8>> {
        fn images(commands: &[DrawCmd], into: &mut Vec<Vec<u8>>) {
            for command in commands {
                match command {
                    DrawCmd::Image { image, .. } => into.push(image.data.data().to_vec()),
                    DrawCmd::Clip { children, .. } => images(children, into),
                    _ => {}
                }
            }
        }
        let mut found = Vec::new();
        images(&self.painted.0, &mut found);
        found
    }

    /// The slider drawn in `region`, which blocks the pointer from what's
    /// beneath it.
    pub(super) fn slider(&self, region: Rect) -> Rect {
        self.blocking(
            region.center().x,
            region.y0..region.y1,
            region.x0..region.x1,
        )
        .expect("a slider")
    }

    /// The card a swatch at `swatch` opened, which blocks the pointer from
    /// what's beneath it.
    pub(super) fn picker(&self, swatch: Rect) -> Rect {
        self.blocking(
            swatch.x0 + 30.0,
            swatch.y1..swatch.y1 + 400.0,
            swatch.x0 - 40.0..swatch.x0 + 400.0,
        )
        .expect("a picker below the swatch")
    }

    /// The extent of what blocks the pointer along the line `x` within
    /// `rows`, and across at its middle within `columns`.
    fn blocking(
        &self,
        x: f64,
        rows: std::ops::Range<f64>,
        columns: std::ops::Range<f64>,
    ) -> Option<Rect> {
        let blocked = |hover: &Hovered| matches!(hover, Hovered::Blocked);
        let column = self.find(Rect::new(x, rows.start, x + 1.0, rows.end), blocked)?;
        let y = column.center().y;
        let row = self.find(Rect::new(columns.start, y, columns.end, y + 1.0), blocked)?;
        Some(Rect::new(row.x0, column.y0, row.x1, column.y1))
    }

    /// What the pointer would land on at `point`.
    fn hover_at(&self, point: Point) -> Option<Hovered> {
        match self
            .runner
            .frame
            .dispatch
            .hover_geometry
            .probe(Some(point), None, 0.0)?
        {
            (_, Claim::Direct(hover) | Claim::Extended(hover)) => Some(hover),
            (_, Claim::Occludes) => Some(Hovered::Blocked),
        }
    }

    /// The bounds of every point in `region` the pointer would land on as
    /// `wanted`, sampled every pixel.
    fn find(&self, region: Rect, wanted: impl Fn(&Hovered) -> bool) -> Option<Rect> {
        let mut found: Option<Rect> = None;
        let mut y = region.y0.floor();
        while y < region.y1 {
            let mut x = region.x0.floor();
            while x < region.x1 {
                let point = Point::new(x, y);
                if self.hover_at(point).as_ref().is_some_and(&wanted) {
                    let dot = Rect::from_points(point, point);
                    found = Some(found.map_or(dot, |rect| rect.union(dot)));
                }
                x += 1.0;
            }
            y += 1.0;
        }
        found
    }

    /// Switch off the first `depth` of peel.js's layers, as its slider does:
    /// those libraries stop drawing, and names go with the last.
    pub(super) fn peel(&mut self, depth: usize) {
        let layers = website("peel.js")
            .split("libraries: [")
            .skip(1)
            .map(|rest| {
                rest[..rest.find(']').unwrap()]
                    .split(',')
                    .map(|id| id.trim().trim_matches('"').to_owned())
                    .filter(|id| !id.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let drawing = layers[depth..].iter().flatten().collect::<Vec<_>>();
        let libraries = self.libraries.as_deref().expect("the page's libraries");
        let projections = libraries
            .split(',')
            .filter(|id| drawing.iter().any(|drawn| drawn.as_str() == *id))
            .collect::<Vec<_>>()
            .join(",");
        let stack = crate::web_embed::peeled(
            Some(libraries),
            &projections,
            self.slots.as_deref(),
            depth < layers.len(),
        )
        .unwrap();
        self.runner.stack_changed(stack, SCALE, VIEWPORT);
        self.present();
    }

    pub(super) fn layers() -> usize {
        website("peel.js").matches("libraries: [").count()
    }

    // Input, as the browser delivers it.

    fn present(&mut self) {
        for _ in 0..8 {
            self.runner
                .flush_before_window_event(&WindowEvent::RedrawRequested);
            let paint = self.runner.prepare_paint(SCALE, VIEWPORT);
            self.painted = DrawList::new();
            puri::frame::render(paint.renders, &mut self.painted);
            if !self.runner.frame_presented() {
                return;
            }
        }
        panic!("every presented frame asked for another");
    }

    fn window_event(&mut self, event: WindowEvent) {
        let pointer = self.translate(&event);
        self.dispatch(&event, pointer);
    }

    /// What the window's reducer makes of `event`, stamped with the moment it
    /// arrives.
    fn translate(&mut self, event: &WindowEvent) -> Option<PointerEvent> {
        match crate::translate_window_event(&mut self.runner.editor.reducer, SCALE, event) {
            Some(WindowEventTranslation::Pointer(pointer)) => Some(pointer),
            _ => None,
        }
    }

    fn dispatch(&mut self, event: &WindowEvent, pointer: Option<PointerEvent>) {
        self.runner.flush_before_window_event(event);
        if let WindowEvent::ModifiersChanged(modifiers) = event {
            self.runner.modifiers_changed(
                ui_events_winit::keyboard::from_winit_modifier_state(modifiers.state()),
                SCALE,
                VIEWPORT,
            );
        }
        if let Some(pointer) = pointer {
            self.runner.pointer_event(&pointer, SCALE, VIEWPORT);
        }
        self.present();
    }

    pub(super) fn hold(&mut self, modifiers: ModifiersState) {
        self.held = modifiers;
        self.window_event(WindowEvent::ModifiersChanged(modifiers.into()));
    }

    pub(super) fn move_to(&mut self, point: Point) {
        self.window_event(WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(point.x, point.y),
        });
    }

    fn button(&mut self, state: ElementState) {
        self.window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        });
    }

    /// A moment passes: the next press starts a new click rather than
    /// continuing the last one.
    fn pause(&mut self) {
        self.runner.editor.reducer = Default::default();
        crate::translate_window_event(
            &mut self.runner.editor.reducer,
            SCALE,
            &WindowEvent::ModifiersChanged(self.held.into()),
        );
    }

    pub(super) fn click_at(&mut self, point: Point) {
        self.clicks_at(point, 1);
    }

    fn double_click_at(&mut self, point: Point) {
        self.clicks_at(point, 2);
    }

    fn clicks_at(&mut self, point: Point, count: usize) {
        self.pause();
        self.move_to(point);
        // A double click's presses arrive together: however long the editor
        // takes with the first, the second still lands within the interval.
        let presses = (0..count)
            .flat_map(|_| [ElementState::Pressed, ElementState::Released])
            .map(|state| {
                let event = WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    state,
                    button: MouseButton::Left,
                };
                let pointer = self.translate(&event);
                (event, pointer)
            })
            .collect::<Vec<_>>();
        for (event, pointer) in presses {
            self.dispatch(&event, pointer);
        }
    }

    pub(super) fn click(&mut self, path: &[Step]) {
        self.click_at(self.rect(path).center());
    }

    /// Double-click the text drawn at `path`, selecting its word.
    pub(super) fn double_click(&mut self, path: &[Step]) {
        self.double_click_at(self.text_at(path));
    }

    /// Hold Ctrl and click.
    pub(super) fn pick_at(&mut self, point: Point) {
        self.hold(ModifiersState::CONTROL);
        self.click_at(point);
        self.hold(ModifiersState::empty());
    }

    pub(super) fn press_at(&mut self, point: Point) {
        self.pause();
        self.move_to(point);
        self.button(ElementState::Pressed);
    }

    pub(super) fn release(&mut self) {
        self.button(ElementState::Released);
    }

    pub(super) fn drag(&mut self, from: Point, to: &[Point]) {
        self.press_at(from);
        for point in to {
            self.move_to(*point);
        }
        self.release();
    }

    /// Click the swatch of the color at `color`, then somewhere new in the
    /// picker it opens.
    pub(super) fn pick_color(&mut self, color: &[Step]) {
        let swatch = self.rect(color);
        self.click_at(Point::new(swatch.x0 + 7.0, swatch.center().y));
        let picker = self.picker(swatch);
        self.click_at(Point::new(
            picker.x0 + picker.width() * 0.8,
            picker.y0 + picker.height() * 0.2,
        ));
    }

    /// Click the separator between a list's items `first` and `first + 1`.
    pub(super) fn click_between(&mut self, list: &[Step], first: usize) {
        let positions = self.positions(list);
        let [before, after] = [first, first + 1]
            .map(|index| self.rect(&[list, &[Step::Element(positions[index].clone())]].concat()));
        self.click_at(if (after.y0 - before.y0).abs() < 2.0 {
            Point::new((before.x1 + after.x0) / 2.0, before.center().y)
        } else {
            Point::new(before.x0 + 4.0, (before.y1 + after.y0) / 2.0)
        });
    }

    /// One key, down and up, as the page forwards it; whether the editor
    /// took the press.
    pub(super) fn keystroke(&mut self, key: Key, modifiers: Modifiers) -> bool {
        self.runner.flush_pending_continuous();
        if self.runner.editor.modifiers != modifiers {
            self.runner.modifiers_changed(modifiers, SCALE, VIEWPORT);
        }
        let event = |state| KeyboardEvent {
            state,
            key: key.clone(),
            modifiers,
            ..Default::default()
        };
        let handled = self
            .runner
            .keyboard_event(&event(KeyState::Down), SCALE, VIEWPORT);
        self.runner
            .keyboard_event(&event(KeyState::Up), SCALE, VIEWPORT);
        if !modifiers.is_empty() {
            self.runner
                .modifiers_changed(Modifiers::empty(), SCALE, VIEWPORT);
        }
        self.present();
        handled
    }

    pub(super) fn press(&mut self, key: NamedKey) {
        let named = Key::Named(key);
        assert!(
            self.keystroke(named.clone(), Modifiers::empty()),
            "{named:?} did nothing at {:?}",
            self.editor()
                .model
                .selection
                .as_ref()
                .map(|s| s.path().to_vec())
        );
    }

    /// Ctrl and a letter.
    pub(super) fn command(&mut self, letter: &str) {
        assert!(self.keystroke(Key::Character(letter.into()), Modifiers::CONTROL));
    }

    pub(super) fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            let shifted =
                character.is_ascii_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(character);
            assert!(
                self.keystroke(
                    Key::Character(character.to_string().into()),
                    if shifted {
                        Modifiers::SHIFT
                    } else {
                        Modifiers::empty()
                    },
                ),
                "typing {character:?} did nothing at {:?}",
                self.editor()
                    .model
                    .selection
                    .as_ref()
                    .map(|s| s.path().to_vec())
            );
        }
    }
}

impl Drop for Embed {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert_eq!(
                self.done,
                self.tasks.len(),
                "untried tasks: {:?}",
                &self.tasks[self.done..]
            );
        }
    }
}
