//! What tests read back from the driver's frames: what is drawn where and
//! what it holds, the conveniences that click, type and select by document
//! path, and the window's focus and appearance.

use super::{Driver, keystroke_for};
use crate::display::test_support::Landmarks;
use crate::frame::Hovered;
use gid::{CellId, Position, Step, Value};
use kurbo::{Point, Rect};
use puri::draw::{DrawCmd, Shape};
use puri::hover::Claim;
use std::rc::Rc;
use ui_events::keyboard::{Key, Modifiers, NamedKey};
use winit::keyboard::ModifiersState;

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

    /// What the `{render: …}` at `path` asks to draw.
    pub(crate) fn rendered(&self, path: &[Step]) -> Value {
        self.computed(path, crate::libraries::presentation::vocabulary::RENDER)
    }

    /// What the `{evaluate: …}` at `path` computes.
    pub(crate) fn result(&self, path: &[Step]) -> Value {
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
    pub(crate) fn palette(&mut self, palette: crate::styles::Palette) -> bool {
        let changed = self
            .runner
            .palette_changed(palette, self.scale(), self.viewport);
        self.present();
        changed
    }

    /// Select what's drawn at `path`, as arriving there with the keyboard
    /// would.
    pub(crate) fn select(&mut self, path: &[Step]) {
        let select = self
            .landmark(path)
            .unwrap_or_else(|| panic!("nothing drawn at {path:?}"))
            .select
            .clone();
        assert!(select(&mut self.runner.editor, None), "{path:?} refused");
        self.present();
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
        let frame = crate::projection::test_support::editing_frame(&mut self.runner.editor, false);
        frame.completion.map(|offers| {
            offers
                .entries
                .iter()
                .map(|entry| entry.display.clone())
                .collect()
        })
    }

    pub(crate) fn selection(&self) -> Vec<Step> {
        self.editor()
            .model
            .selection
            .as_ref()
            .expect("a selection")
            .path()
            .to_vec()
    }

    /// Where the selected data is stored.
    pub(crate) fn source_selection(&self) -> Option<crate::location::Location<'static>> {
        let sources = self.editor().sources();
        let location = self.editor().model.selection.as_ref()?.location()?;
        location
            .stored(&sources)
            .map(crate::location::Location::into_owned)
    }

    pub(crate) fn drawn(&self, path: &[Step]) -> bool {
        self.landmark(path).is_some()
    }

    fn landmark(&self, path: &[Step]) -> Option<&crate::navigate::Descend<crate::Editor>> {
        self.runner.frame.dispatch.descends.landmark(path)
    }

    /// Where `path` is drawn.
    pub(crate) fn rect(&self, path: &[Step]) -> Rect {
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
    pub(crate) fn text_at(&self, path: &[Step]) -> Point {
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

    /// Every landmark drawn under `prefix`, and where.
    pub(crate) fn drawn_under(&self, prefix: &[Step]) -> Vec<(Rc<[Step]>, Rect)> {
        self.runner
            .frame
            .dispatch
            .descends
            .iter()
            .filter(|landmark| landmark.path.starts_with(prefix))
            .map(|landmark| (landmark.path.clone(), landmark.rect))
            .collect()
    }

    /// The circles the last frame painted: where on screen, the circle in
    /// its drawing's own coordinates, and its paint.
    pub(crate) fn circles(&self) -> Vec<(Point, kurbo::Circle, puri::Brush)> {
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

    pub(crate) fn click(&mut self, path: &[Step]) {
        self.click_at(self.rect(path).center());
    }

    /// Double-click the text drawn at `path`, selecting its word.
    pub(crate) fn double_click(&mut self, path: &[Step]) {
        self.double_click_at(self.text_at(path));
    }

    /// Hold Ctrl and click.
    pub(crate) fn pick_at(&mut self, point: Point) {
        self.hold(ModifiersState::CONTROL);
        self.click_at(point);
        self.hold(ModifiersState::empty());
    }

    /// Click the swatch of the color at `color`, then somewhere new in the
    /// picker it opens.
    pub(crate) fn pick_color(&mut self, color: &[Step]) {
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
