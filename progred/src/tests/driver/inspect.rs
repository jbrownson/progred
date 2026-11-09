//! What tests read back from the driver's frames: what is drawn where and
//! what it holds, the conveniences that click, type and select by document
//! path, input-method text, and the window's focus and appearance and the
//! page's libraries.

use super::{Driver, keystroke_for};
use crate::display::test_support::Landmarks;
use crate::frame::Hovered;
use crate::occurrence::Step as Occurs;
use gid::{CellId, Position, Step, Value};
use kurbo::{Point, Rect, Size};
use puri::draw::{DrawCmd, Shape};
use ui_events::keyboard::{Key, Modifiers, NamedKey};
use winit::keyboard::ModifiersState;

impl Driver {
    /// The window tests drive unless they need another.
    pub(crate) const VIEWPORT: Size = Size::new(900.0, 600.0);

    /// An editor of a document whose root is `root`, in a window of
    /// [`Self::VIEWPORT`].
    pub(crate) fn of(root: Value) -> Self {
        Self::new(crate::editor_of(root), Self::VIEWPORT)
    }
}

impl<P> Driver<P> {
    pub(crate) fn click_at(&mut self, point: Point) {
        self.clicks_at(point, 1);
    }

    fn double_click_at(&mut self, point: Point) {
        self.clicks_at(point, 2);
    }

    /// `rest` within item `index` of the list at `list`.
    pub(crate) fn element(&self, list: &[Step], index: usize, rest: &[Step]) -> Vec<Step> {
        [
            list,
            &[Step::Element(self.positions(list)[index].clone())],
            rest,
        ]
        .concat()
    }

    pub(crate) fn value(&self, path: &[Step]) -> Value {
        self.editor()
            .sources()
            .resolve_path(path)
            .cloned()
            .unwrap_or_else(|| panic!("nothing at {path:?}"))
    }

    pub(crate) fn number(&self, path: &[Step]) -> Option<f64> {
        crate::libraries::f64::read(&self.value(path))
    }

    pub(crate) fn positions(&self, list: &[Step]) -> Vec<Position> {
        self.value(list)
            .as_list()
            .unwrap_or_else(|| panic!("no list at {list:?}"))
            .keys()
            .cloned()
            .collect()
    }

    /// What the evaluation or render at `path` computes.
    pub(crate) fn result(&self, path: &[Step]) -> Value {
        self.computed(path, ::grap::vocabulary::EXPRESSION)
    }

    fn computed(&self, path: &[Step], key: CellId) -> Value {
        let value = self.value(path);
        let expression = value
            .as_record()
            .and_then(|fields| fields.get(&key))
            .unwrap_or_else(|| panic!("nothing to compute at {path:?}"));
        let evaluation =
            ::grap::Evaluator::new(&self.editor().sources(), crate::sources::DEFAULT_FUEL)
                .evaluate(expression);
        assert_eq!(evaluation.halt, None);
        evaluation.result.to_value()
    }

    /// The window gains or loses focus; whether that changed anything.
    pub(crate) fn focus(&mut self, focused: bool) -> bool {
        let changed = self
            .runner
            .focus_changed(focused, self.scale(), self.viewport);
        self.present();
        changed
    }

    /// The system's appearance becomes `palette`; whether that changed
    /// anything.
    pub(crate) fn palette(&mut self, palette: crate::display::widget::style::Palette) -> bool {
        let changed = self
            .runner
            .palette_changed(palette, self.scale(), self.viewport);
        self.present();
        changed
    }

    /// The platform's input method composes or commits text; whether the
    /// editor took it.
    pub(crate) fn ime(&mut self, event: puri::handler::ImeEvent) -> bool {
        self.runner.flush_pending_continuous();
        let handled = self.runner.ime_event(&event, self.scale(), self.viewport);
        self.present();
        handled
    }

    /// The page swaps the libraries the editor draws and reads with.
    pub(crate) fn stack(&mut self, stack: crate::stack::Stack) {
        self.runner
            .stack_changed(stack, self.scale(), self.viewport);
        self.present();
    }

    /// Select what's drawn at `path`, as arriving there with the keyboard
    /// would.
    pub(crate) fn select<S: Clone + Into<Occurs> + std::fmt::Debug>(&mut self, path: &[S]) {
        let select = self
            .landmark(path)
            .unwrap_or_else(|| panic!("nothing drawn at {path:?}"))
            .select
            .clone();
        assert!(select(&mut self.runner.editor, None), "{path:?} refused");
        self.present();
    }

    /// The structure the last copy put on the clipboard.
    pub(crate) fn copied(&mut self) -> Option<Value> {
        self.runner.editor.text_clipboard.structure()
    }

    /// Puts beside the last copy the picture the shell puts there once the
    /// copy's key is handled, `render` making its PNG.
    pub(crate) fn picture_copy(
        &mut self,
        render: impl FnOnce(&crate::picture::Picture) -> Result<Vec<u8>, String>,
    ) {
        self.runner
            .editor
            .picture_copy(self.scale(), self.viewport, render);
    }

    /// The picture the last copy put on the clipboard, as a PNG.
    pub(crate) fn copied_picture(&self) -> Option<&[u8]> {
        let crate::clipboard::Clipboard::Own { picture, .. } = &self.editor().text_clipboard;
        picture.as_deref()
    }

    /// Copy `value` from elsewhere and paste it with the command key's V;
    /// whether the editor took it.
    pub(crate) fn paste(&mut self, value: &Value) -> bool {
        self.runner.editor.text_clipboard.copy(
            crate::clipboard::spelling(value).0,
            Some(value),
            None,
        );
        self.keystroke(Key::Character("v".into()), self.command())
    }

    /// The command key's Z; whether there was anything to undo.
    pub(crate) fn undo(&mut self) -> bool {
        self.keystroke(Key::Character("z".into()), self.command())
    }

    /// The command key's Shift-Z; whether there was anything to redo.
    pub(crate) fn redo(&mut self) -> bool {
        self.keystroke(
            Key::Character("z".into()),
            self.command() | Modifiers::SHIFT,
        )
    }

    /// The editor's command key, as a key press holds it.
    fn command(&self) -> Modifiers {
        crate::test_support::command_keys(self.editor().command_modifier)
    }

    /// The text of the line the selection edits, if it edits one.
    pub(crate) fn line(&self) -> Option<&str> {
        Some(self.editor().model.selection.as_ref()?.edit()?.text())
    }

    /// The edited line's anchor and focus, as byte offsets.
    pub(crate) fn line_selection(&self) -> Option<(usize, usize)> {
        Some(
            self.editor()
                .model
                .selection
                .as_ref()?
                .edit()?
                .selection_offsets(),
        )
    }

    /// What the open picker offers, in order; `None` when none is open.
    pub(crate) fn picker_entries(&mut self) -> Option<Vec<String>> {
        Some(
            self.offers()?
                .into_iter()
                .map(|(display, _)| display)
                .collect(),
        )
    }

    /// What the open picker says beside its entry `display`.
    pub(crate) fn picker_detail(&mut self, display: &str) -> Option<String> {
        self.offers()?
            .into_iter()
            .find(|(shown, _)| shown == display)
            .and_then(|(_, detail)| detail)
    }

    /// The open picker's entries and the details beside them. They're read
    /// from the document view framed again, the shown frame keeping no
    /// offers, so a picker open in a pane isn't found.
    fn offers(&mut self) -> Option<Vec<(String, Option<String>)>> {
        let frame = crate::frame::test_support::editing_frame(&mut self.runner.editor);
        frame.completion.map(|offers| {
            offers
                .entries
                .iter()
                .map(|entry| (entry.display.clone(), entry.detail.clone()))
                .collect()
        })
    }

    /// Click the open picker's entry `display`.
    pub(crate) fn choose(&mut self, display: &str) {
        let entries = self.picker_entries().expect("an open picker");
        let index = entries
            .iter()
            .position(|entry| entry == display)
            .unwrap_or_else(|| panic!("no {display:?} among {entries:?}"));
        self.click_at(self.locate(Hovered::Tree(crate::hover::Hover::Entry(index))));
    }

    pub(crate) fn selection(&self) -> crate::occurrence::Path {
        self.editor()
            .model
            .selection
            .as_ref()
            .expect("a selection")
            .path()
            .to_vec()
    }

    /// Whether the selection selects a value, authors one, or authors a
    /// field's label.
    pub(crate) fn stage(&self) -> Option<crate::selection::Stage> {
        let editor = self.editor();
        Some(editor.model.selection.as_ref()?.stage(&editor.sources()))
    }

    /// Where the selected data is stored.
    pub(crate) fn source_selection(&self) -> Option<crate::location::Location<'static>> {
        let sources = self.editor().sources();
        let location = self.editor().model.selection.as_ref()?.location()?;
        location
            .stored(&sources)
            .map(crate::location::Location::into_owned)
    }

    pub(crate) fn drawn<S: Clone + Into<Occurs>>(&self, path: &[S]) -> bool {
        self.landmark(path).is_some()
    }

    /// Whether the last frame draws `path` in `view`, of the views that may
    /// draw the same path.
    pub(crate) fn drawn_in<S: Clone + Into<Occurs>>(
        &self,
        view: &std::rc::Rc<crate::workspace::View>,
        path: &[S],
    ) -> bool {
        let path = occurrence(path);
        self.runner
            .frame
            .dispatch
            .landmarks
            .iter()
            .any(|landmark| landmark.path.view() == view && landmark.path == path)
    }

    /// What the last frame draws at `path`, if anything.
    pub(crate) fn landmark<S: Clone + Into<Occurs>>(
        &self,
        path: &[S],
    ) -> Option<&crate::display::widget::navigation::Landmark<crate::Editor>> {
        self.runner
            .frame
            .dispatch
            .landmarks
            .landmark(&occurrence(path))
    }

    /// What the last frame's occurrence at `path` shares with the others the
    /// frame marks beside it when it's selected or hovered.
    pub(crate) fn shared<S: Clone + Into<Occurs>>(
        &self,
        path: &[S],
    ) -> Option<crate::display::widget::source::Secondary> {
        let path = occurrence(path);
        self.runner
            .frame
            .dispatch
            .landmarks
            .iter()
            .filter(|landmark| landmark.path == path)
            .find_map(|landmark| landmark.secondary.clone())
    }

    /// Where `path` is drawn.
    pub(crate) fn rect<S: Clone + Into<Occurs> + std::fmt::Debug>(&self, path: &[S]) -> Rect {
        self.landmark(path)
            .map(|landmark| landmark.rect)
            .unwrap_or_else(|| {
                panic!(
                    "nothing drawn at {path:?}; drawn: {:#?}",
                    self.runner
                        .frame
                        .dispatch
                        .landmarks
                        .iter()
                        .map(|landmark| landmark.path.clone())
                        .collect::<Vec<_>>()
                )
            })
    }

    /// The middle of the text drawn at `path`: its largest run of glyphs,
    /// not a subscript or a quote mark beside it.
    pub(crate) fn text_at<S: Clone + Into<Occurs> + std::fmt::Debug>(&self, path: &[S]) -> Point {
        let rect = self.rect(path);
        crate::display::test_support::painted(&self.painted.0)
            .filter_map(|command| match command {
                DrawCmd::GlyphRun(run) => Some(run),
                _ => None,
            })
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

    /// The glyph runs drawn wholly within `path`'s box, each as its glyphs
    /// and paint: what's spelled there, and in which face.
    pub(crate) fn glyphs_at<S: Clone + Into<Occurs> + std::fmt::Debug>(
        &self,
        path: &[S],
    ) -> Vec<(Vec<u32>, peniko::Brush)> {
        let rect = self.rect(path);
        crate::display::test_support::painted(&self.painted.0)
            .filter_map(|command| match command {
                DrawCmd::GlyphRun(run) => Some(run),
                _ => None,
            })
            .filter(|run| {
                !run.glyphs.is_empty()
                    && run.glyphs.iter().all(|glyph| {
                        rect.contains(run.transform * Point::new(glyph.x.into(), glyph.y.into()))
                    })
            })
            .map(|run| {
                (
                    run.glyphs.iter().map(|glyph| glyph.id).collect(),
                    run.brush.clone(),
                )
            })
            .collect()
    }

    /// Every landmark drawn under `prefix`, and where.
    pub(crate) fn drawn_under<S: Clone + Into<Occurs>>(
        &self,
        prefix: &[S],
    ) -> Vec<(crate::occurrence::Trail, Rect)> {
        let prefix = occurrence(prefix);
        self.runner
            .frame
            .dispatch
            .landmarks
            .iter()
            .filter(|landmark| landmark.path.starts_with(&prefix))
            .map(|landmark| (landmark.path.clone(), landmark.rect))
            .collect()
    }

    /// The circles the last frame painted: where on screen, the circle in
    /// its drawing's own coordinates, and its paint.
    pub(crate) fn circles(&self) -> Vec<(Point, kurbo::Circle, peniko::Brush)> {
        crate::display::test_support::painted(&self.painted.0)
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::Circle(circle),
                    brush,
                    transform,
                } => Some((*transform * circle.center, *circle, brush.clone())),
                _ => None,
            })
            .collect()
    }

    /// The images the last frame painted, as their pixels.
    pub(crate) fn images(&self) -> Vec<Vec<u8>> {
        crate::display::test_support::painted(&self.painted.0)
            .filter_map(|command| match command {
                DrawCmd::Image { image, .. } => Some(image.data.data().to_vec()),
                _ => None,
            })
            .collect()
    }

    /// The slider drawn in `region`, which blocks the pointer from what's
    /// beneath it.
    pub(crate) fn slider(&self, region: Rect) -> Rect {
        self.blocking(
            region.center().x,
            region.y0..region.y1,
            region.x0..region.x1,
        )
        .expect("a slider")
    }

    /// The card a swatch at `swatch` opened, which blocks the pointer from
    /// what's beneath it.
    pub(crate) fn picker(&self, swatch: Rect) -> Rect {
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
    pub(crate) fn hover_at(&self, point: Point) -> Option<Hovered> {
        crate::frame::test_support::hover_at(&self.runner.frame.dispatch, point)
    }

    /// The first point, scanning the window on a 4-pixel grid, where the
    /// pointer would land on `wanted`.
    pub(crate) fn locate(&self, wanted: Hovered) -> Point {
        (2..self.viewport.height as usize)
            .step_by(4)
            .flat_map(|y| {
                (2..self.viewport.width as usize)
                    .step_by(4)
                    .map(move |x| (x, y))
            })
            .map(|(x, y)| Point::new(x as f64, y as f64))
            .find(|point| self.hover_at(*point).as_ref() == Some(&wanted))
            .unwrap_or_else(|| panic!("nothing in the window hovers as {wanted:?}"))
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

    pub(crate) fn click<S: Clone + Into<Occurs> + std::fmt::Debug>(&mut self, path: &[S]) {
        self.click_at(self.rect(path).center());
    }

    /// Double-click the text drawn at `path`, selecting its word.
    pub(crate) fn double_click<S: Clone + Into<Occurs> + std::fmt::Debug>(&mut self, path: &[S]) {
        self.double_click_at(self.text_at(path));
    }

    /// Hold the editor's command key, as picking and scrubbing do.
    pub(crate) fn hold_command(&mut self) {
        self.hold(match self.editor().command_modifier {
            puri::keyboard::CommandModifier::Meta => ModifiersState::SUPER,
            puri::keyboard::CommandModifier::Control => ModifiersState::CONTROL,
        });
    }

    /// Click at `point` holding the editor's command key, as picking does.
    pub(crate) fn pick_at(&mut self, point: Point) {
        self.hold_command();
        self.click_at(point);
        self.hold(ModifiersState::empty());
    }

    /// Click the swatch of the color at `color`, then somewhere new in the
    /// picker it opens.
    pub(crate) fn pick_color<S: Clone + Into<Occurs> + std::fmt::Debug>(&mut self, color: &[S]) {
        let swatch = self.rect(color);
        self.click_at(Point::new(swatch.x0 + 7.0, swatch.center().y));
        let picker = self.picker(swatch);
        self.click_at(Point::new(
            picker.x0 + picker.width() * 0.8,
            picker.y0 + picker.height() * 0.2,
        ));
    }

    /// Click the separator between a list's items `first` and `first + 1`.
    pub(crate) fn click_between(&mut self, list: &[Step], first: usize) {
        let positions = self.positions(list);
        let [before, after] = [first, first + 1]
            .map(|index| self.rect(&[list, &[Step::Element(positions[index].clone())]].concat()));
        self.click_at(if (after.y0 - before.y0).abs() < 2.0 {
            Point::new((before.x1 + after.x0) / 2.0, before.center().y)
        } else {
            Point::new(before.x0 + 4.0, (before.y1 + after.y0) / 2.0)
        });
    }

    pub(crate) fn press(&mut self, key: NamedKey) {
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

    pub(crate) fn type_text(&mut self, text: &str) {
        for character in text.chars() {
            let (key, modifiers) = keystroke_for(character);
            assert!(
                self.keystroke(key, modifiers),
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

/// An occurrence path from either kind of step: the paths tests name are
/// mostly document paths, which an occurrence path of the same steps is.
fn occurrence<S: Clone + Into<Occurs>>(path: &[S]) -> crate::occurrence::Path {
    path.iter().cloned().map(Into::into).collect()
}
