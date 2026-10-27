//! Editor-owned views over one document, laid out in rows and columns. The
//! editor's layout holds the document's view and the views it opened beside
//! it; a view whose root value is a row or column splits its part of the
//! window the same way, each of the parts it splits into a pane. The layout,
//! each pane's libraries shown, annotations and scroll, and each part's
//! share remain process state.

use crate::annotations::Annotations;
use crate::display::widget::view::Area;
use crate::location::{Location, Start};
use crate::occurrence;
use crate::sources::Sources;
use gid::{CellId, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

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

/// The editor library, which names this vocabulary and the rest of the
/// editor's. While an area shows it, a root row or column splits the area
/// and a part that's a location shows that place.
pub const EDITOR_LIBRARY: CellId = CellId::from_u128(0x6fe840ea1fa7bff4153b107f7d5683dc);

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
/// Only the root splits the window; a row deeper in the document is drawn
/// inside it.
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
    let index = parts.keys().position(|key| key == position)?;
    let destination = if forward(along, direction)? {
        (index + 1 < parts.len()).then_some(index + 1)?
    } else {
        index.checked_sub(1)?
    };
    let value = parts.get(position)?.clone();
    let remaining = parts.with(position.clone(), None);
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
            Value::List(remaining.with(position.clone(), value)),
        )?,
        part_path(container, &position),
    ))
}

/// Whether moving `direction` goes forward along a row or column going
/// `along`: right in a row, down in a column. None across it.
fn forward(along: Direction, direction: Move) -> Option<bool> {
    match (along, direction) {
        (Direction::Across, Move::Right) | (Direction::Down, Move::Down) => Some(true),
        (Direction::Across, Move::Left) | (Direction::Down, Move::Up) => Some(false),
        _ => None,
    }
}

/// Whether `path`, below a view's root, reaches `part`'s path there, or is
/// below it.
fn under(part: &[Step], path: &[occurrence::Step]) -> bool {
    path.len() >= part.len()
        && part
            .iter()
            .zip(path)
            .all(|(step, drawn)| drawn.data().as_ref() == Some(step))
}

/// What a view shows: a place, which it draws and edits where it's stored,
/// or a copy of a computed value, which nothing stores.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Place(Location<'static>),
    Copy(Value),
}

/// A view the editor lays out, showing its entry. A view is known by its
/// address, not by what it shows, so two views of one place are two views,
/// each with its own panes.
#[derive(Debug)]
pub struct View {
    pub entry: Entry,
}

impl View {
    /// A view of the document's root.
    pub fn of_root() -> Self {
        Self {
            entry: Entry::Place(Location::root(Vec::new())),
        }
    }

    /// Where the view starts drawing: at a place from the document's root,
    /// along the place's own steps, as the document's view does; at one
    /// inside a cell's definition, from a jump there; and at a copy, as
    /// computed.
    pub fn root_path(&self) -> occurrence::Path {
        match &self.entry {
            Entry::Place(place) => match place.start {
                Start::Root => occurrence::of(&place.path),
                Start::Cell { .. } => vec![occurrence::Step::jump(place.clone())],
            },
            Entry::Copy(_) => vec![occurrence::Step::Computed],
        }
    }

    /// Where the value `part` below the view's root is stored: from its
    /// place, along the part's steps. None in a copy.
    pub fn place(&self, part: &[Step]) -> Option<Location<'static>> {
        match &self.entry {
            Entry::Place(place) => Some(Location {
                start: place.start,
                path: [&place.path[..], part].concat().into(),
            }),
            Entry::Copy(_) => None,
        }
    }

    /// The value `part` below the view's root, as stored or copied, before
    /// any location it holds is followed.
    pub fn value<'a>(&'a self, sources: &Sources<'a>, part: &[Step]) -> Option<&'a Value> {
        match &self.entry {
            Entry::Place(_) => sources.read(&self.place(part)?),
            Entry::Copy(value) => part_at(value, part),
        }
    }
}

impl PartialEq for View {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for View {}

impl Hash for View {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

/// How the editor divides its window: one view filling it, or a row or
/// column of layouts, each with its share beside its neighbours'.
pub enum Layout {
    View(Rc<View>),
    Split(Direction, Vec<(Layout, f64)>),
}

impl Layout {
    /// The views it lays out, in reading order.
    fn views(&self) -> Box<dyn Iterator<Item = &Rc<View>> + '_> {
        match self {
            Self::View(view) => Box::new(std::iter::once(view)),
            Self::Split(_, children) => {
                Box::new(children.iter().flat_map(|(child, _)| child.views()))
            }
        }
    }

    /// The layout at `way`, the place of each on the way to it in its row
    /// or column.
    fn at(&self, way: &[usize]) -> Option<&Layout> {
        way.iter().try_fold(self, |layout, index| match layout {
            Self::Split(_, children) => children.get(*index).map(|(child, _)| child),
            Self::View(_) => None,
        })
    }

    fn at_mut(&mut self, way: &[usize]) -> Option<&mut Layout> {
        way.iter().try_fold(self, |layout, index| match layout {
            Self::Split(_, children) => children.get_mut(*index).map(|(child, _)| child),
            Self::View(_) => None,
        })
    }

    /// The way to `view`.
    fn way_to(&self, view: &Rc<View>) -> Option<Vec<usize>> {
        match self {
            Self::View(shown) => (shown == view).then(Vec::new),
            Self::Split(_, children) => {
                children.iter().enumerate().find_map(|(index, (child, _))| {
                    let mut way = child.way_to(view)?;
                    way.insert(0, index);
                    Some(way)
                })
            }
        }
    }

    /// Removes `view`. A row or column it leaves with one layout gives way to
    /// that one.
    fn remove(&mut self, view: &Rc<View>) -> bool {
        let Self::Split(_, children) = self else {
            return false;
        };
        let removed = match children
            .iter()
            .position(|(child, _)| matches!(child, Self::View(shown) if shown == view))
        {
            Some(index) => {
                children.remove(index);
                true
            }
            None => children.iter_mut().any(|(child, _)| child.remove(view)),
        };
        if children.len() == 1
            && let Some((only, _)) = children.pop()
        {
            *self = only;
        }
        removed
    }
}

/// An area of the window that draws one value of a view, and its state.
pub struct Pane {
    pub area: Area,
    pub shown: Shown,
    /// Sparse projection-local state. Two panes over the same GID
    /// location deliberately do not share folds or other annotations.
    pub annotations: Annotations,
    /// Logical pixels, independent of monitor scale.
    pub scroll: Vec2,
}

impl Pane {
    fn new(area: Area) -> Self {
        Self {
            area,
            shown: Shown::ALL,
            annotations: Annotations::default(),
            scroll: Vec2::ZERO,
        }
    }
}

/// What takes a share of a row or column: a layout in the editor's, at its
/// way there, or the part of a view's root value an area draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Layout(Vec<usize>),
    Part(Area),
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
pub struct PanePlacement<'a> {
    pub pane: &'a Pane,
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
    pub panes: Vec<PanePlacement<'a>>,
    pub dividers: Vec<DividerPlacement>,
}

impl<'a> Geometry<'a> {
    fn pane(pane: &'a Pane, rect: Rect) -> Self {
        Self {
            panes: vec![PanePlacement { pane, rect }],
            dividers: Vec::new(),
        }
    }

    fn extend(mut self, other: Self) -> Self {
        self.panes.extend(other.panes);
        self.dividers.extend(other.dividers);
        self
    }
}

pub struct Workspace {
    /// The document's view, which shows its root. It can't be closed.
    pub document: Rc<View>,
    /// How the editor divides its window among its views.
    layout: Layout,
    /// What's drawn: each view's whole area, and each part of one whose
    /// root value splits, in reading order.
    panes: Vec<Pane>,
    /// Each part's share of its row or column, beside its neighbours': one
    /// where unset.
    shares: HashMap<Area, f64>,
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
        Self::new(Rc::new(View::of_root()))
    }
}

impl Workspace {
    /// A workspace laying out only `document`, the document's view.
    pub fn new(document: Rc<View>) -> Self {
        Self {
            layout: Layout::View(document.clone()),
            panes: vec![Pane::new(Area::whole(document.clone()))],
            document,
            shares: HashMap::new(),
            dragging: None,
        }
    }

    /// The views the editor lays out, in reading order.
    pub fn views(&self) -> impl Iterator<Item = &Rc<View>> {
        self.layout.views()
    }

    pub(crate) fn folds(&self) -> Folds {
        Folds(
            self.panes
                .iter()
                .map(|pane| (pane.area.clone(), pane.annotations.clone()))
                .collect(),
        )
    }

    pub(crate) fn restore_folds(&mut self, saved: Folds) {
        for pane in &mut self.panes {
            if let Some((_, annotations)) = saved.0.iter().find(|(area, _)| *area == pane.area) {
                pane.annotations
                    .restore_field(crate::annotations::FOLD, annotations);
            }
        }
        self.dragging = None;
    }

    /// The document's view's whole area.
    pub fn document_area(&self) -> Area {
        Area::whole(self.document.clone())
    }

    /// Where an arrow with nothing selected starts: the document's first
    /// part where its root splits, else its whole area.
    pub fn document_start(&self) -> Area {
        self.panes
            .iter()
            .map(|pane| &pane.area)
            .find(|area| area.view == self.document && !area.path().is_empty())
            .cloned()
            .unwrap_or_else(|| self.document_area())
    }

    /// The pane drawing the area `area`.
    pub fn pane(&self, area: &Area) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.area == *area)
    }

    pub fn pane_mut(&mut self, area: &Area) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.area == *area)
    }

    /// The area of `view` that draws `path`: the part whose path leads it,
    /// below the view's root, where the view's root value splits, else the
    /// view's whole area.
    pub fn area_of(&self, view: &Rc<View>, path: &[occurrence::Step]) -> Area {
        let root = view.root_path();
        path.strip_prefix(root.as_slice())
            .and_then(|below| {
                self.panes.iter().map(|pane| &pane.area).find(|area| {
                    area.view == *view && !area.path().is_empty() && under(area.path(), below)
                })
            })
            .cloned()
            .unwrap_or_else(|| Area::whole(view.clone()))
    }

    /// The pane drawing `path` in `view`.
    pub fn pane_at(&self, view: &Rc<View>, path: &[occurrence::Step]) -> Option<&Pane> {
        self.pane(&self.area_of(view, path))
    }

    pub fn pane_at_mut(&mut self, view: &Rc<View>, path: &[occurrence::Step]) -> Option<&mut Pane> {
        let area = self.area_of(view, path);
        self.pane_mut(&area)
    }

    /// The pane of the document's whole area, which there always is.
    pub fn document_pane(&self) -> &Pane {
        self.pane(&self.document_area())
            .expect("the document's view always has its pane")
    }

    pub fn document_pane_mut(&mut self) -> &mut Pane {
        let area = self.document_area();
        self.pane_mut(&area)
            .expect("the document's view always has its pane")
    }

    /// The selected area's pane, else the document's whole one.
    pub fn selected_or_document(&self, selected: Option<&Area>) -> &Pane {
        match selected.and_then(|area| self.pane(area)) {
            Some(pane) => pane,
            None => self.document_pane(),
        }
    }

    fn selected_or_document_mut(&mut self, selected: Option<&Area>) -> &mut Pane {
        match selected.filter(|area| self.pane(area).is_some()).cloned() {
            Some(area) => self.pane_mut(&area).expect("a pane found a moment ago"),
            None => self.document_pane_mut(),
        }
    }

    /// Show `shown` in the selected area.
    pub fn show(&mut self, selected: Option<&Area>, shown: Shown) {
        self.selected_or_document_mut(selected).shown = shown;
    }

    /// Switch one library off, or back on, in the selected area.
    pub fn toggle_library(&mut self, selected: Option<&Area>, library: CellId) {
        self.selected_or_document_mut(selected)
            .shown
            .toggle(library);
    }

    /// Reconcile the panes with what the views show: a whole area for each
    /// view, and, while its whole area shows the editor library, one for
    /// each part its root value splits into, a surviving area keeping its
    /// state. Plain and Raw show a view's root whole.
    pub fn sync(&mut self, sources: &Sources) {
        let mut previous = std::mem::take(&mut self.panes);
        let mut take = |area: Area| match previous.iter().position(|pane| pane.area == area) {
            Some(index) => previous.swap_remove(index),
            None => Pane::new(area),
        };
        let mut panes = Vec::new();
        let mut parts = Vec::new();
        for view in self.layout.views() {
            let whole = take(Area::whole(view.clone()));
            let splits = whole.shown.shows(EDITOR_LIBRARY);
            panes.push(whole);
            let Some(value) = view.value(sources, &[]).filter(|_| splits) else {
                continue;
            };
            for (path, again) in parts_of(value, &[]) {
                let area = Area::part(view.clone(), path);
                if !again {
                    panes.push(take(area.clone()));
                }
                parts.push(area);
            }
        }
        self.panes = panes;
        self.shares.retain(|area, _| parts.contains(area));
        let live = |slot: &Slot| match slot {
            Slot::Layout(_) => true,
            Slot::Part(area) => parts.contains(area),
        };
        if self
            .dragging
            .as_ref()
            .is_some_and(|drag| !live(&drag.divider.before) || !live(&drag.divider.after))
        {
            self.dragging = None;
        }
    }

    /// Opens a view of `entry` at `side`'s end of the editor's row, and
    /// returns it.
    pub fn open(&mut self, side: Side, entry: Entry) -> Rc<View> {
        let view = Rc::new(View { entry });
        let opened = (Layout::View(view.clone()), 1.0);
        let mut row = match std::mem::replace(&mut self.layout, Layout::View(view.clone())) {
            Layout::Split(Direction::Across, row) => row,
            other => vec![(other, 1.0)],
        };
        match side {
            Side::Left => row.insert(0, opened),
            Side::Right => row.push(opened),
        }
        self.layout = Layout::Split(Direction::Across, row);
        self.panes.push(Pane::new(Area::whole(view.clone())));
        self.dragging = None;
        view
    }

    /// Closes `view`, unless it's the document's.
    pub fn close(&mut self, view: &Rc<View>) -> bool {
        let closed = *view != self.document && self.layout.remove(view);
        if closed {
            self.panes.retain(|pane| pane.area.view != *view);
            self.dragging = None;
        }
        closed
    }

    /// Moves the part pane `from`'s state and share to `to`, where its part
    /// went. Its folds, kept by path, start over.
    pub fn retarget(&mut self, from: &Area, to: Area) {
        if let Some(share) = self.shares.remove(from) {
            self.shares.insert(to.clone(), share);
        }
        if let Some(pane) = self.pane_mut(from) {
            pane.area = to;
            pane.annotations = Annotations::default();
        }
    }

    /// Where `view` moving `direction` goes: the way to the row or column
    /// it's in, its place there, and its neighbour's that way.
    fn step(&self, view: &Rc<View>, direction: Move) -> Option<(Vec<usize>, usize, usize)> {
        let mut way = self.layout.way_to(view)?;
        let index = way.pop()?;
        let Layout::Split(along, children) = self.layout.at(&way)? else {
            return None;
        };
        let other = if forward(*along, direction)? {
            (index + 1 < children.len()).then_some(index + 1)?
        } else {
            index.checked_sub(1)?
        };
        Some((way, index, other))
    }

    /// Whether the pane at `area` can move that way: a view's whole area
    /// along the row or column of the editor's it's in, and a part along
    /// its own in the view's root value, where that can be written.
    pub fn can_move(&self, sources: &Sources, area: &Area, direction: Move) -> bool {
        if area.path().is_empty() {
            self.step(&area.view, direction).is_some()
        } else {
            area.view
                .value(sources, &[])
                .is_some_and(|value| move_part(value, area.path(), direction).is_some())
                && area.view.place(&[]).is_some_and(|place| {
                    place
                        .stored(sources)
                        .is_some_and(|stored| crate::location::owned(stored.start))
                })
        }
    }

    /// Moves `view` one place along the row or column of the editor's it's
    /// in.
    pub fn move_view(&mut self, view: &Rc<View>, direction: Move) -> bool {
        let Some((way, index, other)) = self.step(view, direction) else {
            return false;
        };
        let Some(Layout::Split(_, children)) = self.layout.at_mut(&way) else {
            return false;
        };
        children.swap(index, other);
        self.dragging = None;
        true
    }

    fn share(&self, slot: &Slot) -> f64 {
        match slot {
            Slot::Layout(way) => way
                .split_last()
                .and_then(|(index, parent)| match self.layout.at(parent)? {
                    Layout::Split(_, children) => children.get(*index).map(|(_, share)| *share),
                    Layout::View(_) => None,
                })
                .unwrap_or(1.0),
            Slot::Part(area) => self.shares.get(area).copied().unwrap_or(1.0),
        }
    }

    pub fn set_share(&mut self, slot: Slot, share: f64) {
        match slot {
            Slot::Layout(way) => {
                if let Some((index, parent)) = way.split_last()
                    && let Some(Layout::Split(_, children)) = self.layout.at_mut(parent)
                    && let Some(child) = children.get_mut(*index)
                {
                    child.1 = share;
                }
            }
            Slot::Part(area) => {
                self.shares.insert(area, share);
            }
        }
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
        self.set_share(divider.before.clone(), first);
        self.set_share(divider.after.clone(), pair - first);
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

    /// The editor's layout filling `size`: each view in its share, a view
    /// whose root value splits divided as that value is, in one walk.
    pub fn geometry(&self, sources: &Sources, size: Size, scale: f64) -> Geometry<'_> {
        self.layout_geometry(
            sources,
            &self.layout,
            &[],
            Rect::from_origin_size(kurbo::Point::ZERO, size),
            scale,
        )
    }

    fn layout_geometry(
        &self,
        sources: &Sources,
        layout: &Layout,
        way: &[usize],
        rect: Rect,
        scale: f64,
    ) -> Geometry<'_> {
        match layout {
            Layout::View(view) => {
                let whole = Area::whole(view.clone());
                let splits = self
                    .pane(&whole)
                    .is_some_and(|pane| pane.shown.shows(EDITOR_LIBRARY));
                match view
                    .value(sources, &[])
                    .filter(|value| splits && split(value).is_some())
                {
                    Some(value) => self.split_geometry(view, value, &[], rect, scale),
                    None => self.pane_geometry(&whole, rect),
                }
            }
            Layout::Split(direction, children) => {
                let children = children
                    .iter()
                    .enumerate()
                    .map(|(index, (child, _))| {
                        let way = [way, &[index]].concat();
                        (Slot::Layout(way.clone()), (way, child))
                    })
                    .collect();
                self.lay_out(*direction, children, rect, scale, |(way, child), rect| {
                    self.layout_geometry(sources, child, &way, rect, scale)
                })
            }
        }
    }

    /// The row or column `value` at `at` in `view` filling `rect`: each part
    /// that splits again split in turn, and each other a pane.
    fn split_geometry(
        &self,
        view: &Rc<View>,
        value: &Value,
        at: &[Step],
        rect: Rect,
        scale: f64,
    ) -> Geometry<'_> {
        let Some((direction, parts)) = split(value) else {
            return Geometry::default();
        };
        let parts = parts
            .iter()
            .map(|(position, part)| {
                let area = Area::part(view.clone(), part_path(at, position));
                (Slot::Part(area.clone()), (area, part))
            })
            .collect();
        self.lay_out(direction, parts, rect, scale, |(area, part), rect| {
            if split(part).is_some() {
                self.split_geometry(view, part, area.path(), rect, scale)
            } else {
                self.pane_geometry(&area, rect)
            }
        })
    }

    fn pane_geometry(&self, area: &Area, rect: Rect) -> Geometry<'_> {
        self.pane(area)
            .map(|pane| Geometry::pane(pane, rect))
            .unwrap_or_default()
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

/// Where the area `area` starts drawing in its view: where the view does,
/// for its whole area, and its part's path below that for a part. With
/// `interpreted`, as while the editor library is on, a part that's a
/// location shows that place instead: it jumps there.
pub fn root_path(sources: &Sources, area: &Area, interpreted: bool) -> occurrence::Path {
    let mut path = occurrence::below(&area.view.root_path(), area.path());
    let place = (interpreted && !area.path().is_empty())
        .then(|| {
            area.view
                .value(sources, area.path())
                .and_then(crate::location::read)
        })
        .flatten();
    if let Some(place) = place {
        path.push(occurrence::Step::jump(place));
    }
    path
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
#[path = "tests/workspace.rs"]
mod tests;
