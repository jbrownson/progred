//! Editor-owned views over one document, laid out in rows and columns of
//! parts. The editor's own row holds the document and the panes it opened
//! beside it; a document whose root is a row or column splits its part the
//! same way, each of the parts it splits into a pane. View mode,
//! annotations, scroll, and each part's share remain process state.

use crate::annotations::Annotations;
use crate::display::widget::view::{Area, ViewId};
use crate::occurrence;
use gid::{CellId, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};
use std::collections::HashMap;

/// What this module interprets at the document's root: the row or column it
/// splits into, and its own libraries. The editor and presentation
/// libraries name them.
pub mod vocabulary {
    use gid::CellId;

    /// The kinds that split the space they're given among their parts.
    pub mod kind {
        use gid::CellId;

        /// Parts side by side.
        pub const ROW: CellId = CellId::from_u128(0xf93143d539ec4ab2865709b80a7f395d);
        /// Parts one above another.
        pub const COLUMN: CellId = CellId::from_u128(0xab69ac8c77c94425865d437f9d780643);
    }

    /// A row's or column's parts, in order.
    pub const PARTS: CellId = CellId::from_u128(0x4abd6fa4c21d4ba19bfc7c2893b52d69);
    pub const LIBRARIES: CellId = CellId::from_u128(0xbc197ad66fea59a9bd97c4191c7845b2);
    /// The keys a document library owns: it draws records carrying them.
    pub const KEYS: CellId = CellId::from_u128(0xbb30d215605f5599e31f6274641ca89f);
    /// A projection function: the one a document library draws its records
    /// with, and the one presentation and layout take.
    pub const PROJECTION: CellId = CellId::from_u128(0x873503e2e37a1722a0dd21399be9ee7f);
}

/// The libraries an area shows: their projections, their offers and, for
/// the name library, names. Those it leaves off keep their definitions
/// loaded, so code still runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Every library but these.
    AllBut(Vec<CellId>),
    /// Only these, whatever else is loaded.
    Only(Vec<CellId>),
}

impl Shown {
    /// Every library.
    pub const ALL: Self = Self::AllBut(Vec::new());
    /// No library: GID's own kinds as they are.
    pub const RAW: Self = Self::Only(Vec::new());

    /// Whether `library` shows.
    pub fn shows(&self, library: CellId) -> bool {
        match self {
            Self::AllBut(hidden) => !hidden.contains(&library),
            Self::Only(shown) => shown.contains(&library),
        }
    }

    /// Switches `library` off, or back on.
    fn toggle(&mut self, library: CellId) {
        let (Self::AllBut(libraries) | Self::Only(libraries)) = self;
        match libraries.iter().position(|id| *id == library) {
            Some(index) => {
                libraries.remove(index);
            }
            None => libraries.push(library),
        }
    }
}

impl Default for Shown {
    fn default() -> Self {
        Self::ALL
    }
}

/// Which end of the editor's row a pane opens at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Which way a row or column lays out its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Side by side, as a row's.
    Across,
    /// One above another, as a column's.
    Down,
}

/// A row: `parts` side by side.
pub fn row(parts: impl IntoIterator<Item = Value>) -> Value {
    ::grap::isa::record(
        vocabulary::kind::ROW,
        [(vocabulary::PARTS, Value::list(parts))],
    )
}

/// A column: `parts` one above another.
#[cfg(test)]
pub fn column(parts: impl IntoIterator<Item = Value>) -> Value {
    ::grap::isa::record(
        vocabulary::kind::COLUMN,
        [(vocabulary::PARTS, Value::list(parts))],
    )
}

/// The parts `value` splits into and which way, where it's a row or column
/// with any. One with none has nothing to split into, so it's data.
pub fn split(value: &Value) -> Option<(Direction, &gid::List)> {
    let fields = value.as_record()?;
    let direction = match ::grap::isa::of(fields)? {
        vocabulary::kind::ROW => Direction::Across,
        vocabulary::kind::COLUMN => Direction::Down,
        _ => return None,
    };
    let parts = fields.get(&vocabulary::PARTS)?.as_list()?;
    (!parts.is_empty()).then_some((direction, parts))
}

/// Where the part at `position` of the row or column at `at` is.
fn part_path(at: &[Step], position: &gid::Position) -> Path {
    at.iter()
        .cloned()
        .chain([
            Step::Key(vocabulary::PARTS),
            Step::Element(position.clone()),
        ])
        .collect()
}

/// Every part `value` splits into below `at`, each followed by its own
/// parts where it splits again, and whether it does.
fn parts_of(value: &Value, at: &[Step]) -> Vec<(Path, bool)> {
    split(value)
        .into_iter()
        .flat_map(|(_, parts)| parts.iter())
        .flat_map(|(position, part)| {
            let path = part_path(at, position);
            let inner = parts_of(part, &path);
            std::iter::once((path, !inner.is_empty())).chain(inner)
        })
        .collect()
}

/// Where the document's panes are: the parts its `root` splits into, where
/// it's a row or column, in order, a part that splits again giving its own.
/// Only the root splits, so a row deeper in the document stays data.
#[cfg(any(test, feature = "cam-profile"))]
pub fn panes(root: Option<&Value>) -> Vec<Path> {
    root.into_iter()
        .flat_map(|root| parts_of(root, &[]))
        .filter_map(|(path, splits)| (!splits).then_some(path))
        .collect()
}

/// The part at `path` below `value`, along keys and elements.
fn part_at<'v>(value: &'v Value, path: &[Step]) -> Option<&'v Value> {
    path.iter().try_fold(value, |value, step| match step {
        Step::Key(key) => value.as_record()?.get(key),
        Step::Element(position) => value.as_list()?.get(position),
        Step::Follow(_) => None,
    })
}

/// `root` with the part at `path` moved one place along its row or column,
/// and where it went: left or right in a row, up or down in a column.
pub fn move_part(root: &Value, path: &[Step], direction: Move) -> Option<(Value, Path)> {
    let [
        container @ ..,
        Step::Key(vocabulary::PARTS),
        Step::Element(position),
    ] = path
    else {
        return None;
    };
    let (along, parts) = split(part_at(root, container)?)?;
    let forward = match (along, direction) {
        (Direction::Across, Move::Right) | (Direction::Down, Move::Down) => true,
        (Direction::Across, Move::Left) | (Direction::Down, Move::Up) => false,
        _ => return None,
    };
    let index = parts.keys().position(|key| key == position)?;
    let destination = if forward {
        (index + 1 < parts.len()).then_some(index + 1)?
    } else {
        index.checked_sub(1)?
    };
    let value = parts.get(position)?.clone();
    let remaining = parts.without(position);
    let keys: Vec<_> = remaining.keys().collect();
    let position = gid::position::between(
        destination
            .checked_sub(1)
            .and_then(|index| keys.get(index).copied()),
        keys.get(destination).copied(),
    )?;
    let list = container
        .iter()
        .cloned()
        .chain([Step::Key(vocabulary::PARTS)])
        .collect::<Path>();
    Some((
        crate::spine::set(
            Some(root),
            &list,
            Value::List(remaining.update(position.clone(), value)),
        )?,
        part_path(container, &position),
    ))
}

/// Whether `path`, in a view, reaches `part`'s path in it, or is below it.
fn under(part: &[Step], path: &[occurrence::Step]) -> bool {
    path.len() >= part.len()
        && part
            .iter()
            .zip(path)
            .all(|(step, drawn)| drawn.data().as_ref() == Some(step))
}

pub struct View {
    pub area: Area,
    pub shown: Shown,
    /// Sparse projection-local state. Two views over the same GID
    /// location deliberately do not share folds or other annotations.
    pub annotations: Annotations,
    /// Logical pixels, independent of monitor scale.
    pub scroll: Vec2,
}

impl View {
    fn new(area: Area) -> Self {
        Self {
            area,
            shown: Shown::ALL,
            annotations: Annotations::default(),
            scroll: Vec2::ZERO,
        }
    }
}

/// A pane the editor opened beside the document, which the document
/// doesn't keep.
pub struct Opened {
    pub id: CellId,
    pub view: View,
    /// What it shows, as a pane's entry: a location, or a copy of a
    /// computed value, which has none.
    pub entry: Value,
}

/// A part of the editor's row.
enum Part {
    Document,
    Opened(Opened),
}

/// What takes a share of a row or column: the document's part or an opened
/// pane in the editor's row, or the part of the document's at a path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Document,
    Opened(CellId),
    Part(Path),
}

/// The rule between two neighbouring parts, which drags to move share from
/// one to the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divider {
    /// The way the row or column it divides goes.
    pub direction: Direction,
    pub before: Slot,
    pub after: Slot,
}

#[derive(Clone)]
pub struct ViewPlacement<'a> {
    pub view: &'a View,
    pub rect: Rect,
}

#[derive(Clone)]
pub struct DividerPlacement {
    pub divider: Divider,
    /// The visible one-pixel rule. Its input target grows at placement.
    pub rect: Rect,
    /// The share a pixel of dragging moves from one side to the other.
    pub rate: f64,
}

#[derive(Default)]
pub struct Geometry<'a> {
    pub views: Vec<ViewPlacement<'a>>,
    pub dividers: Vec<DividerPlacement>,
}

impl<'a> Geometry<'a> {
    fn view(view: &'a View, rect: Rect) -> Self {
        Self {
            views: vec![ViewPlacement { view, rect }],
            dividers: Vec::new(),
        }
    }

    fn extend(mut self, other: Self) -> Self {
        self.views.extend(other.views);
        self.dividers.extend(other.dividers);
        self
    }
}

pub struct Workspace {
    pub document: View,
    /// The editor's row: the document's part among the panes opened beside
    /// it, in order.
    row: Vec<Part>,
    /// The views of the parts the document's root splits into.
    panes: Vec<View>,
    /// Each part's share of its row or column, beside its neighbours': one
    /// where unset.
    shares: HashMap<Slot, f64>,
    dragging: Option<Drag>,
}

pub(crate) struct Folds(Vec<(Area, Annotations)>);

#[derive(Clone)]
struct Drag {
    divider: Divider,
    start: Vec2,
    before: f64,
    after: f64,
    rate: f64,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            document: View::new(Area::whole(ViewId::Document)),
            row: vec![Part::Document],
            panes: Vec::new(),
            shares: HashMap::new(),
            dragging: None,
        }
    }
}

impl Workspace {
    /// The panes the editor opened, in the row's order.
    pub fn opened(&self) -> impl Iterator<Item = &Opened> {
        self.row.iter().filter_map(|part| match part {
            Part::Opened(opened) => Some(opened),
            Part::Document => None,
        })
    }

    /// Every view: the document's, the opened panes' and the document's
    /// panes'.
    fn views(&self) -> impl Iterator<Item = &View> {
        std::iter::once(&self.document)
            .chain(self.opened().map(|opened| &opened.view))
            .chain(&self.panes)
    }

    fn views_mut(&mut self) -> impl Iterator<Item = &mut View> {
        std::iter::once(&mut self.document)
            .chain(self.row.iter_mut().filter_map(|part| match part {
                Part::Opened(opened) => Some(&mut opened.view),
                Part::Document => None,
            }))
            .chain(&mut self.panes)
    }

    pub(crate) fn folds(&self) -> Folds {
        Folds(
            self.views()
                .map(|view| (view.area.clone(), view.annotations.clone()))
                .collect(),
        )
    }

    pub(crate) fn restore_folds(&mut self, saved: Folds) {
        for view in self.views_mut() {
            if let Some((_, annotations)) = saved.0.iter().find(|(area, _)| *area == view.area) {
                view.annotations
                    .restore_field(crate::annotations::FOLD, annotations);
            }
        }
        self.dragging = None;
    }

    /// The document's view's whole area.
    pub fn document_area(&self) -> Area {
        self.document.area.clone()
    }

    /// Where an arrow with nothing selected starts: the document's whole
    /// area, or its first part's where it splits.
    pub fn document_start(&self) -> Area {
        self.panes.first().unwrap_or(&self.document).area.clone()
    }

    /// The state of the area `area`.
    pub fn view(&self, area: &Area) -> Option<&View> {
        self.views().find(|view| view.area == *area)
    }

    pub fn view_mut(&mut self, area: &Area) -> Option<&mut View> {
        self.views_mut().find(|view| view.area == *area)
    }

    /// The area of `view` that draws `path`: the part of the document whose
    /// path leads it, where the document splits, else the view's whole area.
    pub fn area_of(&self, view: ViewId, path: &[occurrence::Step]) -> Area {
        match view {
            ViewId::Document => self
                .panes
                .iter()
                .map(|pane| &pane.area)
                .find(|area| under(area.path(), path))
                .unwrap_or(&self.document.area)
                .clone(),
            opened => Area::whole(opened),
        }
    }

    /// The state of the area that draws `path` in `view`.
    pub fn view_at(&self, view: ViewId, path: &[occurrence::Step]) -> Option<&View> {
        self.view(&self.area_of(view, path))
    }

    pub fn view_at_mut(&mut self, view: ViewId, path: &[occurrence::Step]) -> Option<&mut View> {
        let area = self.area_of(view, path);
        self.view_mut(&area)
    }

    pub fn selected_or_document(&self, selected: Option<&Area>) -> &View {
        selected
            .and_then(|area| self.view(area))
            .unwrap_or(&self.document)
    }

    /// Show `shown` in the selected area.
    pub fn show(&mut self, selected: Option<&Area>, shown: Shown) {
        match selected.and_then(|area| self.view_mut(area)) {
            Some(view) => view.shown = shown,
            None => self.document.shown = shown,
        }
    }

    /// Switch one library off, or back on, in the selected area.
    pub fn toggle_library(&mut self, selected: Option<&Area>, library: CellId) {
        match selected.and_then(|area| self.view_mut(area)) {
            Some(view) => view.shown.toggle(library),
            None => self.document.shown.toggle(library),
        }
    }

    /// Whether the document's part splits as its root does: while the
    /// document's view shows the editor library. Plain and Raw show the
    /// root whole.
    fn splits(&self) -> bool {
        self.document.shown.shows(crate::libraries::editor::ID)
    }

    /// What the area `area` shows as a pane, its entry: a document pane's
    /// part, or what an opened pane holds. None for the document's whole
    /// area.
    pub fn entry<'a>(
        &'a self,
        sources: &crate::sources::Sources<'a>,
        area: &Area,
    ) -> Option<&'a Value> {
        match area.view {
            ViewId::Document if area.path().is_empty() => None,
            ViewId::Document => sources.resolve_path(area.path()),
            ViewId::Opened(id) => self
                .opened()
                .find(|opened| opened.id == id)
                .map(|opened| &opened.entry),
        }
    }

    /// Where the area `area` starts drawing in its view. The document's whole
    /// area starts at the root, and a document pane at its part's path. An
    /// opened pane draws what it holds, which nothing stores, as computed.
    /// With `interpreted`, as while the editor library is on, a pane whose
    /// entry is a location shows that place instead: it jumps there.
    pub fn root_path(
        &self,
        sources: &crate::sources::Sources<'_>,
        area: &Area,
        interpreted: bool,
    ) -> occurrence::Path {
        let place = interpreted
            .then(|| self.entry(sources, area).and_then(crate::location::read))
            .flatten();
        let mut path = occurrence::of(area.path());
        match (place, area.view) {
            (Some(place), _) => path.push(occurrence::Step::jump(place)),
            (None, ViewId::Opened(_)) => path.push(occurrence::Step::Computed),
            (None, ViewId::Document) => {}
        }
        path
    }

    /// Reconcile the panes with the document's `root`: a view for each part
    /// it splits into, a surviving part's keeping its state.
    pub fn sync(&mut self, root: Option<&Value>) {
        let parts = match root {
            Some(root) if self.splits() => parts_of(root, &[]),
            _ => Vec::new(),
        };
        let mut previous = std::mem::take(&mut self.panes);
        self.panes = parts
            .iter()
            .filter(|(_, splits)| !splits)
            .map(|(path, _)| {
                let area = Area::part(ViewId::Document, path.clone());
                match previous.iter().position(|view| view.area == area) {
                    Some(index) => previous.swap_remove(index),
                    None => View::new(area),
                }
            })
            .collect();
        let opened: Vec<CellId> = self.opened().map(|opened| opened.id).collect();
        let live = |slot: &Slot| match slot {
            Slot::Document => true,
            Slot::Opened(id) => opened.contains(id),
            Slot::Part(path) => parts.iter().any(|(part, _)| part == path),
        };
        self.shares.retain(|slot, _| live(slot));
        if self
            .dragging
            .as_ref()
            .is_some_and(|drag| !live(&drag.divider.before) || !live(&drag.divider.after))
        {
            self.dragging = None;
        }
    }

    /// Opens `entry` as a new pane at `side`'s end of the editor's row, and
    /// returns its area.
    pub fn open(&mut self, side: Side, entry: Value) -> Area {
        let id = gid::new_cell_id();
        let area = Area::whole(ViewId::Opened(id));
        let part = Part::Opened(Opened {
            id,
            view: View::new(area.clone()),
            entry,
        });
        match side {
            Side::Left => self.row.insert(0, part),
            Side::Right => self.row.push(part),
        }
        area
    }

    /// Closes the opened pane whose area is `area`, if it is one.
    pub fn close(&mut self, area: &Area) -> bool {
        let before = self.row.len();
        self.row
            .retain(|part| !matches!(part, Part::Opened(opened) if opened.view.area == *area));
        self.row.len() < before
    }

    /// Moves the document pane `from`'s view and share to `to`, where its
    /// part went. Its folds, kept by path, start over.
    pub fn retarget(&mut self, from: &Area, to: Area) {
        if let Some(share) = self.shares.remove(&Slot::Part(from.path().to_vec())) {
            self.shares.insert(Slot::Part(to.path().to_vec()), share);
        }
        if let Some(view) = self.view_mut(from) {
            view.area = to;
            view.annotations = Annotations::default();
        }
    }

    /// Where the opened pane whose area is `area` is in the editor's row.
    fn place_in_row(&self, area: &Area) -> Option<usize> {
        self.row
            .iter()
            .position(|part| matches!(part, Part::Opened(opened) if opened.view.area == *area))
    }

    /// Whether the pane whose area is `area` can move that way: an opened
    /// pane along the editor's row, a document pane along its row or column
    /// in `document`.
    pub fn can_move(&self, area: &Area, direction: Move, document: Option<&Value>) -> bool {
        match area.view {
            ViewId::Document if area.path().is_empty() => false,
            ViewId::Opened(_) => self
                .place_in_row(area)
                .is_some_and(|index| match direction {
                    Move::Left => index > 0,
                    Move::Right => index + 1 < self.row.len(),
                    Move::Up | Move::Down => false,
                }),
            ViewId::Document => document
                .is_some_and(|document| move_part(document, area.path(), direction).is_some()),
        }
    }

    /// Moves the opened pane whose area is `area` one place along the
    /// editor's row.
    pub fn move_opened(&mut self, area: &Area, direction: Move) -> bool {
        let Some(index) = self.place_in_row(area) else {
            return false;
        };
        let other = match direction {
            Move::Left => index.checked_sub(1),
            Move::Right => (index + 1 < self.row.len()).then_some(index + 1),
            Move::Up | Move::Down => None,
        };
        other.is_some_and(|other| {
            self.row.swap(index, other);
            true
        })
    }

    fn share(&self, slot: &Slot) -> f64 {
        self.shares.get(slot).copied().unwrap_or(1.0)
    }

    #[cfg(test)]
    pub fn set_share(&mut self, slot: Slot, share: f64) {
        self.shares.insert(slot, share);
    }

    /// Starts dragging `divider` from `start`, moving `rate` share a pixel.
    pub fn start_resize(&mut self, divider: Divider, start: Vec2, rate: f64) -> bool {
        self.dragging = Some(Drag {
            before: self.share(&divider.before),
            after: self.share(&divider.after),
            divider,
            start,
            rate,
        });
        true
    }

    pub fn resize(&mut self, divider: &Divider, point: Vec2) -> bool {
        let Some(drag) = self
            .dragging
            .as_ref()
            .filter(|drag| drag.divider == *divider)
            .cloned()
        else {
            return false;
        };
        let travel = match divider.direction {
            Direction::Across => point.x - drag.start.x,
            Direction::Down => point.y - drag.start.y,
        };
        let pair = drag.before + drag.after;
        let first = (drag.before + travel * drag.rate).clamp(0.0, pair);
        self.shares.insert(divider.before.clone(), first);
        self.shares.insert(divider.after.clone(), pair - first);
        true
    }

    pub fn finish_resize(&mut self, divider: &Divider) -> bool {
        if self
            .dragging
            .as_ref()
            .is_some_and(|drag| drag.divider == *divider)
        {
            self.dragging = None;
            true
        } else {
            false
        }
    }

    pub fn cancel_resize(&mut self) -> bool {
        self.dragging.take().is_some()
    }

    /// The editor's row filling `size`: its parts side by side, the
    /// document's split as its `root` is, where it is.
    pub fn geometry(&self, root: Option<&Value>, size: Size, scale: f64) -> Geometry<'_> {
        let parts = self
            .row
            .iter()
            .map(|part| match part {
                Part::Document => (Slot::Document, part),
                Part::Opened(opened) => (Slot::Opened(opened.id), part),
            })
            .collect();
        self.lay_out(
            Direction::Across,
            parts,
            Rect::from_origin_size(kurbo::Point::ZERO, size),
            scale,
            |part, rect| match part {
                Part::Opened(opened) => Geometry::view(&opened.view, rect),
                Part::Document => {
                    match root.filter(|root| self.splits() && split(root).is_some()) {
                        Some(root) => self.split_geometry(root, &[], rect, scale),
                        None => Geometry::view(&self.document, rect),
                    }
                }
            },
        )
    }

    /// The row or column `value` at `at` in the document filling `rect`:
    /// each part that splits again split in turn, and each other a pane.
    fn split_geometry(&self, value: &Value, at: &[Step], rect: Rect, scale: f64) -> Geometry<'_> {
        let Some((direction, parts)) = split(value) else {
            return Geometry::default();
        };
        let parts = parts
            .iter()
            .map(|(position, part)| {
                let path = part_path(at, position);
                (Slot::Part(path.clone()), (path, part))
            })
            .collect();
        self.lay_out(direction, parts, rect, scale, |(path, part), rect| {
            if split(part).is_some() {
                self.split_geometry(part, &path, rect, scale)
            } else {
                let area = Area::part(ViewId::Document, path);
                self.panes
                    .iter()
                    .find(|view| view.area == area)
                    .map(|view| Geometry::view(view, rect))
                    .unwrap_or_default()
            }
        })
    }

    /// `parts` one after another along `direction` in `rect`, a rule apart,
    /// at their shares, any of them raised to a minimum where all fit, each
    /// filling its own with `fill`.
    fn lay_out<'a, T>(
        &'a self,
        direction: Direction,
        parts: Vec<(Slot, T)>,
        rect: Rect,
        scale: f64,
        fill: impl Fn(T, Rect) -> Geometry<'a>,
    ) -> Geometry<'a> {
        let rule = scale.max(1.0);
        let (start, length, minimum) = match direction {
            Direction::Across => (rect.x0, rect.width(), 180.0 * scale),
            Direction::Down => (rect.y0, rect.height(), 96.0 * scale),
        };
        let usable = (length - rule * parts.len().saturating_sub(1) as f64).max(0.0);
        let weights: Vec<f64> = parts.iter().map(|(slot, _)| self.share(slot)).collect();
        let total: f64 = weights.iter().sum();
        let rate = if usable > 0.0 { total / usable } else { 0.0 };
        let band = |from: f64, to: f64| match direction {
            Direction::Across => Rect::new(from, rect.y0, to, rect.y1),
            Direction::Down => Rect::new(rect.x0, from, rect.x1, to),
        };
        let slots: Vec<Slot> = parts.iter().map(|(slot, _)| slot.clone()).collect();
        let mut geometry = Geometry::default();
        let mut at = start;
        for (index, ((slot, part), length)) in parts
            .into_iter()
            .zip(allocate(&weights, usable, minimum))
            .enumerate()
        {
            geometry = geometry.extend(fill(part, band(at, at + length)));
            at += length;
            if let Some(next) = slots.get(index + 1) {
                geometry.dividers.push(DividerPlacement {
                    divider: Divider {
                        direction,
                        before: slot,
                        after: next.clone(),
                    },
                    rect: band(at, at + rule),
                    rate,
                });
                at += rule;
            }
        }
        geometry
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

/// The libraries the document declares at its root, in order. Each entry, or
/// the cell it names, is a record of the `keys` the library owns and the
/// `projection` that draws a record carrying them, a function of `value`.
pub(crate) fn libraries(
    sources: &crate::sources::Sources<'_>,
) -> Vec<crate::display::DocumentLibrary> {
    sources
        .root()
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::LIBRARIES))
        .and_then(Value::as_list)
        .into_iter()
        .flat_map(|declared| declared.values())
        .filter_map(|entry| {
            let entry = match entry.as_cell() {
                Some(cell) => sources.resolve(cell)?.value,
                None => entry,
            };
            let fields = entry.as_record()?;
            Some(crate::display::DocumentLibrary {
                keys: fields
                    .get(&vocabulary::KEYS)?
                    .as_list()?
                    .values()
                    .filter_map(Value::as_cell)
                    .collect(),
                projection: fields.get(&vocabulary::PROJECTION)?.clone().into(),
            })
        })
        .collect()
}

/// Allocate requested weights while temporarily enforcing a minimum.
/// If all minima cannot fit, equal division is the only honest answer.
fn allocate(weights: &[f64], total: f64, minimum: f64) -> Vec<f64> {
    if weights.is_empty() {
        return Vec::new();
    }
    if total <= minimum * weights.len() as f64 {
        return vec![total / weights.len() as f64; weights.len()];
    }
    let weights: Vec<f64> = weights.iter().map(|weight| weight.max(0.0)).collect();
    let mut result = vec![0.0; weights.len()];
    let mut free: Vec<usize> = (0..weights.len()).collect();
    let mut remaining = total;
    loop {
        let sum: f64 = free.iter().map(|index| weights[*index]).sum();
        let equal = sum <= f64::EPSILON;
        let undersized: Vec<usize> = free
            .iter()
            .copied()
            .filter(|index| {
                let share = if equal {
                    remaining / free.len() as f64
                } else {
                    remaining * weights[*index] / sum
                };
                share < minimum
            })
            .collect();
        if undersized.is_empty() {
            let free_count = free.len();
            for index in free {
                result[index] = if equal {
                    remaining / free_count as f64
                } else {
                    remaining * weights[index] / sum
                };
            }
            break;
        }
        for index in undersized {
            result[index] = minimum;
            remaining -= minimum;
            free.retain(|candidate| *candidate != index);
        }
    }
    result
}

#[cfg(test)]
mod tests;
