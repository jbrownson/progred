//! Editor-owned views over one document. The document stays in the
//! center; panes form independently sized columns on either side.
//! A root convention may declare pane membership, side, order, and a
//! presentation function; view mode, annotations, scroll, and live
//! sizing remain process state.

use crate::annotations::Annotations;
pub use crate::libraries::workspace::vocabulary;
use gid::{CellId, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};
#[cfg(test)]
use std::rc::Rc;

const DEFAULT_SIDE_WIDTH: f64 = 1.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Standard,
    /// Raw's structure with names, text, bytes and numbers reading as
    /// themselves.
    Plain,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub use crate::display::widget::view::{Root, Target};

pub struct View {
    pub root: Root,
    pub projection: Projection,
    /// Libraries whose projections this area leaves off. Their
    /// definitions stay loaded, so code still runs and keys keep names.
    pub hidden: Vec<CellId>,
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
            projection: Projection::Standard,
            hidden: Vec::new(),
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

pub struct Geometry<'a> {
    pub views: Vec<ViewPlacement<'a>>,
    pub dividers: Vec<DividerPlacement>,
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

/// Read pane declarations only from the document root. No recursive
/// search means an identical record deeper in user data stays data.
pub fn declarations(root: Option<&Value>) -> Vec<Declaration> {
    let Some(panes) = root
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::PANES))
        .and_then(Value::as_record)
    else {
        return Vec::new();
    };
    [
        (vocabulary::LEFT, Side::Left),
        (vocabulary::RIGHT, Side::Right),
    ]
    .into_iter()
    .flat_map(|(field, side)| {
        panes
            .get(&field)
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
    let Some(declared) = sources
        .root()
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::LIBRARIES))
        .and_then(Value::as_list)
    else {
        return Vec::new();
    };
    declared
        .values()
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
                projection: fields
                    .get(&crate::libraries::presentation::vocabulary::PROJECTION)?
                    .clone()
                    .into(),
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

pub fn can_open(root: Option<&Value>) -> bool {
    root.and_then(Value::as_record).is_some_and(|fields| {
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
    let [Step::Key(panes), Step::Key(side), Step::Element(position)] = path else {
        return None;
    };
    (*panes == vocabulary::PANES).then_some(())?;
    let side = match *side {
        vocabulary::LEFT => Side::Left,
        vocabulary::RIGHT => Side::Right,
        _ => return None,
    };
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
        crate::annotations::set_collapsed(
            &mut document.annotations,
            &[Step::Key(vocabulary::PANES)],
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
    pub(crate) fn folds(&self) -> Folds {
        Folds(
            std::iter::once(&self.document)
                .chain(
                    self.left
                        .panes
                        .iter()
                        .chain(&self.right.panes)
                        .map(|pane| &pane.view),
                )
                .map(|view| (view.root.clone(), view.annotations.clone()))
                .collect(),
        )
    }

    pub(crate) fn restore_folds(&mut self, saved: Folds) {
        for view in std::iter::once(&mut self.document).chain(
            self.left
                .panes
                .iter_mut()
                .chain(&mut self.right.panes)
                .map(|pane| &mut pane.view),
        ) {
            if let Some((root, annotations)) = saved
                .0
                .iter()
                .find(|(root, _)| root.target() == view.root.target())
            {
                view.root = root.clone();
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
        if self.document.root == *root {
            return Some(&self.document);
        }
        self.left
            .panes
            .iter()
            .chain(&self.right.panes)
            .find(|pane| pane.view.root == *root)
            .map(|pane| &pane.view)
    }

    pub fn view_mut(&mut self, root: &Root) -> Option<&mut View> {
        if self.document.root == *root {
            return Some(&mut self.document);
        }
        self.left
            .panes
            .iter_mut()
            .chain(&mut self.right.panes)
            .find(|pane| pane.view.root == *root)
            .map(|pane| &mut pane.view)
    }

    pub fn selected_or_document(&self, selected: Option<&Root>) -> &View {
        selected
            .and_then(|root| self.view(root))
            .unwrap_or(&self.document)
    }

    /// Switch the selected area to `projection`, or back to the standard
    /// one if it's already showing it.
    pub fn toggle_projection(&mut self, selected: Option<&Root>, projection: Projection) {
        let toggle = |view: &mut View| {
            view.projection = if view.projection == projection {
                Projection::Standard
            } else {
                projection
            };
        };
        match selected.and_then(|root| self.view_mut(root)) {
            Some(view) => toggle(view),
            None => toggle(&mut self.document),
        }
    }

    /// Switch one library's projection off, or back on, in the selected
    /// area.
    pub fn toggle_library_projection(&mut self, selected: Option<&Root>, library: CellId) {
        fn toggle(view: &mut View, library: CellId) {
            match view.hidden.iter().position(|hidden| *hidden == library) {
                Some(index) => {
                    view.hidden.remove(index);
                }
                None => view.hidden.push(library),
            }
        }
        match selected.and_then(|root| self.view_mut(root)) {
            Some(view) => toggle(view, library),
            None => toggle(&mut self.document, library),
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

    #[cfg(test)]
    pub fn side(&self, root: &Root) -> Option<Side> {
        self.pane(root).map(|(side, ..)| side)
    }

    pub fn can_move(&self, root: &Root, direction: Move) -> bool {
        let Some((side, column, index)) = self.pane(root) else {
            return false;
        };
        match direction {
            Move::Up => index > 0,
            Move::Down => index + 1 < column.panes.len(),
            Move::Left => side == Side::Right,
            Move::Right => side == Side::Left,
        }
    }

    pub fn start_resize(&mut self, divider: Divider, start: Vec2) -> bool {
        let (before, after) = match &divider {
            Divider::Columns(Side::Left) => (self.left_width, 0.0),
            Divider::Columns(Side::Right) => (self.right_width, 0.0),
            Divider::Panes {
                side,
                before,
                after,
            } => {
                let column = self.column(*side);
                let Some(before) = column
                    .panes
                    .iter()
                    .find(|pane| pane.view.root == *before)
                    .map(|pane| pane.height)
                else {
                    return false;
                };
                let Some(after) = column
                    .panes
                    .iter()
                    .find(|pane| pane.view.root == *after)
                    .map(|pane| pane.height)
                else {
                    return false;
                };
                (before, after)
            }
        };
        self.dragging = Some(Drag {
            divider,
            start,
            before,
            after,
        });
        true
    }

    pub fn resize(&mut self, divider: &Divider, point: Vec2, size: Size) -> bool {
        let Some(drag) = self
            .dragging
            .as_ref()
            .filter(|drag| drag.divider == *divider)
            .cloned()
        else {
            return false;
        };
        match divider {
            Divider::Columns(Side::Left) => {
                self.left_width =
                    (drag.before + ratio(point.x - drag.start.x, size.width)).clamp(0.0, 1.0);
            }
            Divider::Columns(Side::Right) => {
                self.right_width =
                    (drag.before - ratio(point.x - drag.start.x, size.width)).clamp(0.0, 1.0);
            }
            Divider::Panes {
                side,
                before,
                after,
            } => {
                let column = self.column_mut(*side);
                let before_index = column
                    .panes
                    .iter()
                    .position(|pane| pane.view.root == *before);
                let after_index = column
                    .panes
                    .iter()
                    .position(|pane| pane.view.root == *after);
                let (Some(before_index), Some(after_index)) = (before_index, after_index) else {
                    return false;
                };
                if after_index != before_index + 1 {
                    return false;
                }
                let pair = drag.before + drag.after;
                let delta = ratio(point.y - drag.start.y, size.height);
                let before_height = (drag.before + delta).clamp(0.0, pair);
                column.panes[before_index].height = before_height;
                column.panes[after_index].height = pair - before_height;
            }
        }
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

    pub fn geometry(&self, size: Size, scale: f64) -> Geometry<'_> {
        let divider = scale.max(1.0);
        let left_present = !self.left.panes.is_empty();
        let right_present = !self.right.panes.is_empty();
        let column_count = 1 + usize::from(left_present) + usize::from(right_present);
        let usable_width = (size.width - divider * (column_count - 1) as f64).max(0.0);
        let document_weight = match (left_present, right_present) {
            (true, true) => (1.0 - self.left_width - self.right_width).max(0.0),
            (true, false) => (1.0 - self.left_width).max(0.0),
            (false, true) => (1.0 - self.right_width).max(0.0),
            (false, false) => 1.0,
        };
        let requested: Vec<f64> = [
            left_present.then_some(self.left_width),
            Some(document_weight),
            right_present.then_some(self.right_width),
        ]
        .into_iter()
        .flatten()
        .collect();
        let widths = allocate(&requested, usable_width, 180.0 * scale);
        let mut width_index = 0;
        let mut x = 0.0;
        let mut views = Vec::new();
        let mut dividers = Vec::new();

        if left_present {
            let width = widths[width_index];
            width_index += 1;
            column_geometry(
                &self.left,
                Side::Left,
                Rect::new(x, 0.0, x + width, size.height),
                divider,
                scale,
                &mut views,
                &mut dividers,
            );
            x += width;
            dividers.push(DividerPlacement {
                divider: Divider::Columns(Side::Left),
                rect: Rect::new(x, 0.0, x + divider, size.height),
            });
            x += divider;
        }

        let document_width = widths[width_index];
        width_index += 1;
        views.push(ViewPlacement {
            view: &self.document,
            rect: Rect::new(x, 0.0, x + document_width, size.height),
        });
        x += document_width;

        if right_present {
            dividers.push(DividerPlacement {
                divider: Divider::Columns(Side::Right),
                rect: Rect::new(x, 0.0, x + divider, size.height),
            });
            x += divider;
            let width = widths[width_index];
            column_geometry(
                &self.right,
                Side::Right,
                Rect::new(x, 0.0, x + width, size.height),
                divider,
                scale,
                &mut views,
                &mut dividers,
            );
        }

        Geometry { views, dividers }
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
            } => {
                let panes = &self.column(*side).panes;
                panes
                    .windows(2)
                    .any(|pair| pair[0].view.root == *before && pair[1].view.root == *after)
            }
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

fn ratio(value: f64, total: f64) -> f64 {
    if total > 0.0 { value / total } else { 0.0 }
}

fn normalize(panes: &mut [Pane]) {
    let total: f64 = panes.iter().map(|pane| pane.height.max(0.0)).sum();
    if panes.is_empty() {
        return;
    }
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

#[allow(clippy::too_many_arguments)]
fn column_geometry<'a>(
    column: &'a Column,
    side: Side,
    rect: Rect,
    divider: f64,
    scale: f64,
    views: &mut Vec<ViewPlacement<'a>>,
    dividers: &mut Vec<DividerPlacement>,
) {
    let usable = (rect.height() - divider * column.panes.len().saturating_sub(1) as f64).max(0.0);
    let weights: Vec<f64> = column.panes.iter().map(|pane| pane.height).collect();
    let heights = allocate(&weights, usable, 96.0 * scale);
    let mut y = rect.y0;
    for (index, (pane, height)) in column.panes.iter().zip(heights).enumerate() {
        views.push(ViewPlacement {
            view: &pane.view,
            rect: Rect::new(rect.x0, y, rect.x1, y + height),
        });
        y += height;
        if let Some(next) = column.panes.get(index + 1) {
            dividers.push(DividerPlacement {
                divider: Divider::Panes {
                    side,
                    before: pane.view.root.clone(),
                    after: next.view.root.clone(),
                },
                rect: Rect::new(rect.x0, y, rect.x1, y + divider),
            });
            y += divider;
        }
    }
}

#[cfg(test)]
mod tests;
