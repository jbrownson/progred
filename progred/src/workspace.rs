//! Editor-owned views over one document. The document stays in the
//! center; panes form independently sized columns on either side.
//! A root convention may declare the main value the document view shows,
//! pane membership, side, order, and a presentation function; view mode,
//! annotations, scroll, and live sizing remain process state.

use crate::annotations::Annotations;
use crate::display::widget::view::{Root, Target};
use crate::location::Location;
use crate::occurrence;
use gid::{CellId, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};

/// The root's workspace declarations, which this module interprets: its
/// panes, on the left or the right, and its own libraries. The workspace and
/// presentation libraries name them.
pub mod vocabulary {
    use gid::CellId;

    /// The value the document view shows in place of the whole root, so the
    /// workspace's own fields stay out of sight.
    pub const MAIN: CellId = CellId::from_u128(0xf146d8a1d3174d9ab6136b4334768a2f);
    pub const PANES: CellId = CellId::from_u128(0xf30d400a4321a4d44d1628a8adc5a84d);
    pub const LEFT: CellId = CellId::from_u128(0xdc3a1b9a7fb4bc348760160e3b365bca);
    pub const RIGHT: CellId = CellId::from_u128(0xf13a5c1c4471c00178575a0e876768f8);
    pub const LIBRARIES: CellId = CellId::from_u128(0xbc197ad66fea59a9bd97c4191c7845b2);
    /// The keys a document library owns: it draws records carrying them.
    pub const KEYS: CellId = CellId::from_u128(0xbb30d215605f5599e31f6274641ca89f);
    /// A projection function: the one a document library draws its records
    /// with, and the one presentation and layout take.
    pub const PROJECTION: CellId = CellId::from_u128(0x873503e2e37a1722a0dd21399be9ee7f);
}

const DEFAULT_SIDE_WIDTH: f64 = 1.0 / 3.0;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub struct View {
    pub root: Root,
    pub shown: Shown,
    /// Sparse projection-local state. Two views over the same GID
    /// location deliberately do not share folds or other annotations.
    pub annotations: Annotations,
    /// Logical pixels, independent of monitor scale.
    pub scroll: Vec2,
}

impl View {
    fn new(root: Root) -> Self {
        Self {
            root,
            shown: Shown::ALL,
            annotations: Annotations::default(),
            scroll: Vec2::ZERO,
        }
    }
}

pub struct Pane {
    pub view: View,
    /// Requested fraction of its column. Layout may temporarily
    /// override it to satisfy minimums without rewriting the request.
    pub height: f64,
}

#[derive(Default)]
pub struct Column {
    pub panes: Vec<Pane>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Divider {
    Columns(Side),
    Panes {
        side: Side,
        before: Root,
        after: Root,
    },
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
}

#[derive(Default)]
pub struct Geometry<'a> {
    pub views: Vec<ViewPlacement<'a>>,
    pub dividers: Vec<DividerPlacement>,
}

impl<'a> Geometry<'a> {
    fn with_view(mut self, view: &'a View, rect: Rect) -> Self {
        self.views.push(ViewPlacement { view, rect });
        self
    }

    fn with_divider(mut self, divider: Divider, rect: Rect) -> Self {
        self.dividers.push(DividerPlacement { divider, rect });
        self
    }

    fn extend(mut self, other: Self) -> Self {
        self.views.extend(other.views);
        self.dividers.extend(other.dividers);
        self
    }
}

/// What a strip of the workspace shows: the document, or a side column.
#[derive(Clone, Copy)]
enum Area {
    Document,
    Column(Side),
}

pub struct Workspace {
    pub document: View,
    pub left: Column,
    pub right: Column,
    /// Fractions of the full usable width; the document receives the
    /// remainder. Empty columns retain their requested width.
    pub left_width: f64,
    pub right_width: f64,
    dragging: Option<Drag>,
}

pub(crate) struct Folds(Vec<(Root, Annotations)>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub side: Side,
    pub path: Path,
}

/// The path to `root`'s main value: where it declares one, or would, holding
/// only a workspace's own fields, as after the value it had was deleted.
pub fn main_path(root: Option<&Value>) -> Option<Path> {
    let fields = root?.as_record()?;
    let workspace = |key: &CellId| [vocabulary::PANES, vocabulary::LIBRARIES].contains(key);
    (fields.get(&vocabulary::MAIN).is_some()
        || fields.get(&vocabulary::PANES).is_some() && fields.iter().all(|(key, _)| workspace(key)))
    .then(|| vec![Step::Key(vocabulary::MAIN)])
}

/// Where a view showing `target` starts drawing. A pane draws under its
/// declaration, through a jump to what it shows, so its occurrences stay
/// apart from the document view's, even where that draws the declaration
/// too. With `interpreted`, as while the editor library is on, a pane whose
/// entry is a location shows that place, and a workspace record starts at
/// its main value, so the workspace's own fields stay out of sight and out of
/// reach. Otherwise a pane shows its entry.
pub fn root_path(
    sources: &crate::sources::Sources<'_>,
    target: &Target,
    interpreted: bool,
) -> occurrence::Path {
    let (mut path, shown) = match target {
        Target::Document => (Vec::new(), Location::root(Vec::new())),
        Target::Pane { path: declaration } => {
            let shown = interpreted
                .then(|| {
                    sources
                        .resolve_path(declaration)
                        .and_then(crate::location::read)
                })
                .flatten()
                .unwrap_or_else(|| Location::root(declaration.clone()));
            let mut path = occurrence::of(declaration);
            path.push(occurrence::Step::jump(shown.clone()));
            (path, shown)
        }
    };
    if let Some(main) = main_path(sources.read(&shown)).filter(|_| interpreted) {
        path.extend(occurrence::of(&main));
    }
    path
}

/// The view `path` is drawn in. A pane's paths start with its declaration
/// and the jump to what it shows ([`root_path`]); every other path is the
/// document view's.
pub fn view_of(path: &[occurrence::Step]) -> Root {
    use occurrence::Step::{Element, Jump, Key};
    match path {
        [
            Key(vocabulary::PANES),
            Key(column),
            Element(position),
            Jump(_),
            ..,
        ] if [vocabulary::LEFT, vocabulary::RIGHT].contains(column) => Root::pane(vec![
            Step::Key(vocabulary::PANES),
            Step::Key(*column),
            Step::Element(position.clone()),
        ]),
        _ => Root::document(),
    }
}

/// Read pane declarations only from the document root. No recursive
/// search means an identical record deeper in user data stays data.
pub fn declarations(root: Option<&Value>) -> Vec<Declaration> {
    let panes = root
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::PANES))
        .and_then(Value::as_record);
    [
        (vocabulary::LEFT, Side::Left),
        (vocabulary::RIGHT, Side::Right),
    ]
    .into_iter()
    .flat_map(|(field, side)| {
        panes
            .and_then(|panes| panes.get(&field))
            .and_then(Value::as_list)
            .into_iter()
            .flat_map(move |declarations| {
                declarations.keys().map(move |position| Declaration {
                    side,
                    path: pane_path(side, position.clone()),
                })
            })
    })
    .collect()
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

fn column_key(side: Side) -> CellId {
    match side {
        Side::Left => vocabulary::LEFT,
        Side::Right => vocabulary::RIGHT,
    }
}

fn pane_path(side: Side, position: gid::Position) -> Path {
    vec![
        Step::Key(vocabulary::PANES),
        Step::Key(column_key(side)),
        Step::Element(position),
    ]
}

/// `root` as a record to add the workspace's fields to: as it is if it
/// already has them, and otherwise wrapped as a new workspace's main value,
/// which moves every path into it under `main`. Whether it was wrapped.
pub fn workspace_root(root: &Value) -> (Value, bool) {
    match root.as_record() {
        Some(fields)
            if fields.get(&vocabulary::PANES).is_some()
                || fields.get(&vocabulary::MAIN).is_some() =>
        {
            (root.clone(), false)
        }
        _ => (Value::record([(vocabulary::MAIN, root.clone())]), true),
    }
}

pub fn can_open(root: Option<&Value>) -> bool {
    let root = root.map(|root| workspace_root(root).0);
    root.as_ref()
        .and_then(Value::as_record)
        .is_some_and(|fields| {
            fields.get(&vocabulary::PANES).is_none_or(|panes| {
                panes.as_record().is_some_and(|panes| {
                    [vocabulary::LEFT, vocabulary::RIGHT].iter().all(|side| {
                        panes
                            .get(side)
                            .is_none_or(|value| value.as_list().is_some())
                    })
                })
            })
        })
}

pub fn append(root: &Value, side: Side, value: Value) -> Option<(Value, Path)> {
    let fields = root.as_record()?;
    let panes = match fields.get(&vocabulary::PANES) {
        Some(value) => value.as_record()?.clone(),
        None => gid::Record::new(),
    };
    let column = match panes.get(&column_key(side)) {
        Some(value) => value.as_list()?.clone(),
        None => gid::List::new(),
    };
    let position = gid::position::between(column.keys().next_back(), None)?;
    Some((
        Value::Record(fields.update(
            vocabulary::PANES,
            Value::Record(panes.update(
                column_key(side),
                Value::List(column.update(position.clone(), value)),
            )),
        )),
        pane_path(side, position),
    ))
}

pub fn move_value(root: &Value, path: &[Step], direction: Move) -> Option<(Value, Path)> {
    let (side, position) = match path {
        [
            Step::Key(vocabulary::PANES),
            Step::Key(side),
            Step::Element(position),
        ] => match *side {
            vocabulary::LEFT => Some((Side::Left, position)),
            vocabulary::RIGHT => Some((Side::Right, position)),
            _ => None,
        },
        _ => None,
    }?;
    let column = root
        .as_record()?
        .get(&vocabulary::PANES)?
        .as_record()?
        .get(&column_key(side))?
        .as_list()?;
    let value = column.get(position)?.clone();
    match direction {
        Move::Left if side == Side::Right => {
            append(&crate::spine::without(root, path)?, Side::Left, value)
        }
        Move::Right if side == Side::Left => {
            append(&crate::spine::without(root, path)?, Side::Right, value)
        }
        Move::Up | Move::Down => {
            let index = column.keys().position(|key| key == position)?;
            let destination = match direction {
                Move::Up => index.checked_sub(1)?,
                _ => (index + 1 < column.len()).then_some(index + 1)?,
            };
            let remaining = column.without(position);
            let keys: Vec<_> = remaining.keys().collect();
            let position = gid::position::between(
                destination
                    .checked_sub(1)
                    .and_then(|index| keys.get(index).copied()),
                keys.get(destination).copied(),
            )?;
            Some((
                crate::spine::set(
                    Some(root),
                    &path[..2],
                    Value::List(remaining.update(position.clone(), value)),
                )?,
                pane_path(side, position),
            ))
        }
        _ => None,
    }
}

#[derive(Clone)]
struct Drag {
    divider: Divider,
    start: Vec2,
    before: f64,
    after: f64,
}

impl Default for Workspace {
    fn default() -> Self {
        let mut document = View::new(Root::document());
        crate::annotations::set_folded(
            &mut document.annotations,
            &[crate::occurrence::Step::Key(vocabulary::PANES)],
            false,
            true,
        );
        Self {
            document,
            left: Column::default(),
            right: Column::default(),
            left_width: DEFAULT_SIDE_WIDTH,
            right_width: DEFAULT_SIDE_WIDTH,
            dragging: None,
        }
    }
}

impl Workspace {
    /// The document's view, then each pane's, the left column's first.
    fn views(&self) -> impl Iterator<Item = &View> {
        std::iter::once(&self.document).chain(
            self.left
                .panes
                .iter()
                .chain(&self.right.panes)
                .map(|pane| &pane.view),
        )
    }

    fn views_mut(&mut self) -> impl Iterator<Item = &mut View> {
        std::iter::once(&mut self.document).chain(
            self.left
                .panes
                .iter_mut()
                .chain(&mut self.right.panes)
                .map(|pane| &mut pane.view),
        )
    }

    pub(crate) fn folds(&self) -> Folds {
        Folds(
            self.views()
                .map(|view| (view.root.clone(), view.annotations.clone()))
                .collect(),
        )
    }

    pub(crate) fn restore_folds(&mut self, saved: Folds) {
        for view in self.views_mut() {
            if let Some((_, annotations)) = saved.0.iter().find(|(root, _)| *root == view.root) {
                view.annotations
                    .restore_field(crate::annotations::FOLD, annotations);
            }
        }
        self.dragging = None;
    }

    pub fn document_root(&self) -> &Root {
        &self.document.root
    }

    pub fn view(&self, root: &Root) -> Option<&View> {
        self.views().find(|view| view.root == *root)
    }

    pub fn view_mut(&mut self, root: &Root) -> Option<&mut View> {
        self.views_mut().find(|view| view.root == *root)
    }

    /// The view `path` is drawn in.
    pub fn view_at(&self, path: &[occurrence::Step]) -> Option<&View> {
        self.view(&view_of(path))
    }

    pub fn view_at_mut(&mut self, path: &[occurrence::Step]) -> Option<&mut View> {
        self.view_mut(&view_of(path))
    }

    pub fn selected_or_document(&self, selected: Option<&Root>) -> &View {
        selected
            .and_then(|root| self.view(root))
            .unwrap_or(&self.document)
    }

    /// Show `shown` in the selected area.
    pub fn show(&mut self, selected: Option<&Root>, shown: Shown) {
        match selected.and_then(|root| self.view_mut(root)) {
            Some(view) => view.shown = shown,
            None => self.document.shown = shown,
        }
    }

    /// Switch one library off, or back on, in the selected area.
    pub fn toggle_library(&mut self, selected: Option<&Root>, library: CellId) {
        match selected.and_then(|root| self.view_mut(root)) {
            Some(view) => view.shown.toggle(library),
            None => self.document.shown.toggle(library),
        }
    }

    /// Reconcile pane occurrences with their transient view state.
    pub fn sync_declared(&mut self, declarations: &[Declaration]) {
        let mut declared: Vec<_> = std::mem::take(&mut self.left.panes)
            .into_iter()
            .chain(std::mem::take(&mut self.right.panes))
            .collect();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for declaration in declarations {
            let existing = declared.iter().position(|pane| {
                matches!(
                    pane.view.root.target(),
                    Target::Pane { path } if *path == declaration.path
                )
            });
            let pane = existing.map_or_else(
                || Pane {
                    view: View::new(Root::pane(declaration.path.clone())),
                    height: 1.0,
                },
                |index| declared.remove(index),
            );
            match declaration.side {
                Side::Left => left.push(pane),
                Side::Right => right.push(pane),
            }
        }
        normalize(&mut left);
        normalize(&mut right);
        self.left.panes = left;
        self.right.panes = right;
        if self
            .dragging
            .as_ref()
            .is_some_and(|drag| !self.divider_is_live(&drag.divider))
        {
            self.dragging = None;
        }
    }

    /// The side column holding the pane `root`, and its index there.
    fn pane(&self, root: &Root) -> Option<(Side, &Column, usize)> {
        [Side::Left, Side::Right].into_iter().find_map(|side| {
            let column = self.column(side);
            column
                .panes
                .iter()
                .position(|pane| pane.view.root == *root)
                .map(|index| (side, column, index))
        })
    }

    pub fn can_move(&self, root: &Root, direction: Move) -> bool {
        self.pane(root)
            .is_some_and(|(side, column, index)| match direction {
                Move::Up => index > 0,
                Move::Down => index + 1 < column.panes.len(),
                Move::Left => side == Side::Right,
                Move::Right => side == Side::Left,
            })
    }

    pub fn start_resize(&mut self, divider: Divider, start: Vec2) -> bool {
        let sizes = match &divider {
            Divider::Columns(Side::Left) => Some((self.left_width, 0.0)),
            Divider::Columns(Side::Right) => Some((self.right_width, 0.0)),
            Divider::Panes {
                side,
                before,
                after,
            } => {
                let height = |root: &Root| {
                    self.column(*side)
                        .panes
                        .iter()
                        .find(|pane| pane.view.root == *root)
                        .map(|pane| pane.height)
                };
                height(before).zip(height(after))
            }
        };
        if let Some((before, after)) = sizes {
            self.dragging = Some(Drag {
                divider,
                start,
                before,
                after,
            });
            true
        } else {
            false
        }
    }

    pub fn resize(&mut self, divider: &Divider, point: Vec2, size: Size) -> bool {
        match self
            .dragging
            .as_ref()
            .filter(|drag| drag.divider == *divider)
            .cloned()
        {
            None => false,
            Some(drag) => match divider {
                Divider::Columns(Side::Left) => {
                    self.left_width =
                        (drag.before + ratio(point.x - drag.start.x, size.width)).clamp(0.0, 1.0);
                    true
                }
                Divider::Columns(Side::Right) => {
                    self.right_width =
                        (drag.before - ratio(point.x - drag.start.x, size.width)).clamp(0.0, 1.0);
                    true
                }
                Divider::Panes {
                    side,
                    before,
                    after,
                } => {
                    let panes = &mut self.column_mut(*side).panes;
                    match divided(panes, before, after).and_then(|at| panes.get_mut(at..=at + 1)) {
                        Some([first, second]) => {
                            let pair = drag.before + drag.after;
                            let delta = ratio(point.y - drag.start.y, size.height);
                            first.height = (drag.before + delta).clamp(0.0, pair);
                            second.height = pair - first.height;
                            true
                        }
                        _ => false,
                    }
                }
            },
        }
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

    /// The columns side by side, a rule apart: the left, the document and
    /// the right. A side column takes its requested fraction and the
    /// document the rest, any of them raised to a minimum where all fit.
    pub fn geometry(&self, size: Size, scale: f64) -> Geometry<'_> {
        let rule = scale.max(1.0);
        let column = |side: Side, width: f64| {
            (!self.column(side).panes.is_empty()).then_some((Area::Column(side), width))
        };
        let (left, right) = (
            column(Side::Left, self.left_width),
            column(Side::Right, self.right_width),
        );
        let document_weight = left
            .into_iter()
            .chain(right)
            .fold(1.0, |rest, (_, width)| rest - width)
            .max(0.0);
        let areas: Vec<_> = [left, Some((Area::Document, document_weight)), right]
            .into_iter()
            .flatten()
            .collect();
        let usable_width = (size.width - rule * (areas.len() - 1) as f64).max(0.0);
        let weights: Vec<f64> = areas.iter().map(|(_, weight)| *weight).collect();
        let strip = |x: f64, width: f64| Rect::new(x, 0.0, x + width, size.height);
        areas
            .into_iter()
            .zip(allocate(&weights, usable_width, 180.0 * scale))
            .fold(
                (0.0, Geometry::default()),
                |(x, geometry), ((area, _), width)| match area {
                    Area::Document => (
                        x + width,
                        geometry.with_view(&self.document, strip(x, width)),
                    ),
                    Area::Column(Side::Left) => (
                        x + width + rule,
                        geometry
                            .extend(column_geometry(
                                &self.left,
                                Side::Left,
                                strip(x, width),
                                rule,
                                scale,
                            ))
                            .with_divider(Divider::Columns(Side::Left), strip(x + width, rule)),
                    ),
                    Area::Column(Side::Right) => (
                        x + rule + width,
                        geometry
                            .with_divider(Divider::Columns(Side::Right), strip(x, rule))
                            .extend(column_geometry(
                                &self.right,
                                Side::Right,
                                strip(x + rule, width),
                                rule,
                                scale,
                            )),
                    ),
                },
            )
            .1
    }

    pub fn column(&self, side: Side) -> &Column {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    fn column_mut(&mut self, side: Side) -> &mut Column {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    fn divider_is_live(&self, divider: &Divider) -> bool {
        match divider {
            Divider::Columns(Side::Left) => !self.left.panes.is_empty(),
            Divider::Columns(Side::Right) => !self.right.panes.is_empty(),
            Divider::Panes {
                side,
                before,
                after,
            } => divided(&self.column(*side).panes, before, after).is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

/// Where in `panes` a rule divides `before` from `after`, one right after
/// the other: the index of `before`.
fn divided(panes: &[Pane], before: &Root, after: &Root) -> Option<usize> {
    panes.windows(2).position(|pair| {
        matches!(pair, [first, second]
            if first.view.root == *before && second.view.root == *after)
    })
}

fn ratio(value: f64, total: f64) -> f64 {
    if total > 0.0 { value / total } else { 0.0 }
}

fn normalize(panes: &mut [Pane]) {
    let total: f64 = panes.iter().map(|pane| pane.height.max(0.0)).sum();
    if total > 0.0 {
        for pane in panes {
            pane.height = pane.height.max(0.0) / total;
        }
    } else {
        let equal = 1.0 / panes.len() as f64;
        for pane in panes {
            pane.height = equal;
        }
    }
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

/// The column's panes stacked in `rect`, a rule apart, at their requested
/// shares of its height, any of them raised to a minimum where all fit.
fn column_geometry(column: &Column, side: Side, rect: Rect, rule: f64, scale: f64) -> Geometry<'_> {
    let usable = (rect.height() - rule * column.panes.len().saturating_sub(1) as f64).max(0.0);
    let weights: Vec<f64> = column.panes.iter().map(|pane| pane.height).collect();
    let band = |y: f64, height: f64| Rect::new(rect.x0, y, rect.x1, y + height);
    let successors = column.panes.iter().skip(1).map(Some).chain([None]);
    column
        .panes
        .iter()
        .zip(allocate(&weights, usable, 96.0 * scale))
        .zip(successors)
        .fold(
            (rect.y0, Geometry::default()),
            |(y, geometry), ((pane, height), next)| {
                let geometry = geometry.with_view(&pane.view, band(y, height));
                match next {
                    Some(next) => (
                        y + height + rule,
                        geometry.with_divider(
                            Divider::Panes {
                                side,
                                before: pane.view.root.clone(),
                                after: next.view.root.clone(),
                            },
                            band(y + height, rule),
                        ),
                    ),
                    None => (y + height, geometry),
                }
            },
        )
        .1
}

#[cfg(test)]
mod tests;
