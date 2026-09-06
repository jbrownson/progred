//! Editor-owned views over one document. The document stays in the
//! center; panes form independently sized columns on either side.
//! A root convention may declare pane membership, side, order, and a
//! presentation function; view mode, annotations, scroll, and live
//! sizing remain process state.

use crate::annotations::Annotations;
use gid::{CellId, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};
pub use progred_libraries::workspace::vocabulary;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

const DEFAULT_SIDE_WIDTH: f64 = 1.0 / 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Standard,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Document,
    Pane { path: Path },
}

/// A view's transient identity and immutable data root. Equality is
/// allocation identity: two views of the same value remain distinct.
#[derive(Clone)]
pub struct Root(Rc<Target>);

impl Root {
    pub fn document() -> Self {
        Self(Rc::new(Target::Document))
    }

    pub fn pane(path: Path) -> Self {
        Self(Rc::new(Target::Pane { path }))
    }

    pub fn target(&self) -> &Target {
        &self.0
    }
}

impl PartialEq for Root {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Root {}

impl Hash for Root {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Rc::as_ptr(&self.0).hash(state);
    }
}

impl std::fmt::Debug for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Root")
            .field(&(Rc::as_ptr(&self.0) as usize))
            .field(self.target())
            .finish()
    }
}

pub struct View {
    pub root: Root,
    pub projection: Projection,
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
pub struct ViewPlacement {
    pub root: Root,
    pub rect: Rect,
}

#[derive(Clone)]
pub struct DividerPlacement {
    pub divider: Divider,
    /// The visible one-pixel rule. Its input target grows at placement.
    pub rect: Rect,
}

pub struct Geometry {
    pub views: Vec<ViewPlacement>,
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

    pub fn toggle_projection(&mut self, selected: Option<&Root>) {
        let root = selected
            .filter(|root| self.view(root).is_some())
            .cloned()
            .unwrap_or_else(|| self.document.root.clone());
        let view = self.view_mut(&root).expect("a live workspace root");
        view.projection = match view.projection {
            Projection::Standard => Projection::Raw,
            Projection::Raw => Projection::Standard,
        };
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

    pub fn side(&self, root: &Root) -> Option<Side> {
        self.left
            .panes
            .iter()
            .any(|pane| pane.view.root == *root)
            .then_some(Side::Left)
            .or_else(|| {
                self.right
                    .panes
                    .iter()
                    .any(|pane| pane.view.root == *root)
                    .then_some(Side::Right)
            })
    }

    pub fn can_move(&self, root: &Root, direction: Move) -> bool {
        let Some(side) = self.side(root) else {
            return false;
        };
        let column = self.column(side);
        let index = column
            .panes
            .iter()
            .position(|pane| pane.view.root == *root)
            .expect("the pane is in its column");
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

    pub fn geometry(&self, size: Size, scale: f64) -> Geometry {
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
            root: self.document.root.clone(),
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
fn column_geometry(
    column: &Column,
    side: Side,
    rect: Rect,
    divider: f64,
    scale: f64,
    views: &mut Vec<ViewPlacement>,
    dividers: &mut Vec<DividerPlacement>,
) {
    let usable = (rect.height() - divider * column.panes.len().saturating_sub(1) as f64).max(0.0);
    let weights: Vec<f64> = column.panes.iter().map(|pane| pane.height).collect();
    let heights = allocate(&weights, usable, 96.0 * scale);
    let mut y = rect.y0;
    for (index, (pane, height)) in column.panes.iter().zip(heights).enumerate() {
        views.push(ViewPlacement {
            root: pane.view.root.clone(),
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
mod tests {
    use super::*;
    use gid::Cells;

    fn add_pane(workspace: &mut Workspace, side: Side) -> Root {
        let column = workspace.column_mut(side);
        let position = gid::position::between(
            column
                .panes
                .last()
                .and_then(|pane| match pane.view.root.target() {
                    Target::Pane { path } => match path.last() {
                        Some(Step::Element(position)) => Some(position),
                        _ => None,
                    },
                    _ => None,
                }),
            None,
        )
        .unwrap();
        let root = Root::pane(pane_path(side, position));
        column.panes.push(Pane {
            view: View::new(root.clone()),
            height: 1.0,
        });
        for pane in &mut column.panes {
            pane.height = 1.0;
        }
        normalize(&mut column.panes);
        root
    }

    #[test]
    fn roots_are_view_identity_not_cell_identity() {
        let left = Root::pane(Vec::new());
        let other = Root::pane(Vec::new());
        assert_ne!(left, other);
        assert_eq!(left, left.clone());
    }

    #[test]
    fn duplicate_views_keep_independent_projection_state() {
        let mut workspace = Workspace::default();
        let one = add_pane(&mut workspace, Side::Left);
        let other = add_pane(&mut workspace, Side::Right);

        crate::annotations::set_collapsed(
            &mut workspace.view_mut(&one).unwrap().annotations,
            &[],
            false,
            true,
        );

        assert!(crate::annotations::collapsed(
            &workspace.view(&one).unwrap().annotations,
            &[],
            false,
        ));
        assert!(!crate::annotations::collapsed(
            &workspace.view(&other).unwrap().annotations,
            &[],
            false,
        ));
        assert!(!crate::annotations::collapsed(
            &workspace.document.annotations,
            &[],
            false,
        ));
    }

    #[test]
    fn panes_are_values_and_their_edits_are_document_edits() {
        let cell = CellId::from_u128(1);
        let values = [
            Value::from(cell),
            Value::from(b"blob".to_vec()),
            Value::list([]),
            Value::record([]),
        ];
        let root = values.iter().fold(Value::record([]), |root, value| {
            append(&root, Side::Left, value.clone()).unwrap().0
        });
        let locations = declarations(Some(&root));
        assert_eq!(locations.len(), values.len());
        for (location, value) in locations.iter().zip(&values) {
            assert_eq!(crate::spine::get(&root, &location.path), Some(value));
            assert!(matches!(location.path.last(), Some(Step::Element(_))));
        }
        let (moved, path) = move_value(&root, &locations[1].path, Move::Up).unwrap();
        assert_eq!(declarations(Some(&moved))[0].path, path);
        assert_eq!(crate::spine::get(&moved, &path), Some(&values[1]));
        assert_eq!(
            crate::spine::get(&moved, &locations[0].path),
            Some(&values[0])
        );
        let (moved, path) = move_value(&moved, &path, Move::Right).unwrap();
        assert_eq!(declarations(Some(&moved)).last().unwrap().side, Side::Right);
        let mut doc = gid::Document {
            root: Some(moved),
            cells: Cells::new(),
        };
        doc.cells
            .set_value(cell, Value::from(b"definition".to_vec()));
        let mut doc = Rc::new(doc);
        let before = doc.clone();
        assert!(crate::selection::delete_edge(
            &mut doc,
            &Default::default(),
            &path
        ));
        assert_eq!(declarations(doc.root.as_ref()).len(), 3);
        assert_eq!(doc.cells.value(cell), before.cells.value(cell));
        let mut history = crate::history::History::default();
        history.record(before.clone());
        let restored = history.undo(doc).unwrap();
        assert_eq!(restored.root, before.root);
    }

    #[test]
    fn opening_panes_preserves_malformed_containers_and_nonrecord_roots() {
        let value = Value::from(b"data".to_vec());
        for root in [
            value.clone(),
            Value::record([(vocabulary::PANES, value.clone())]),
        ] {
            assert!(!can_open(Some(&root)));
            assert!(append(&root, Side::Left, value.clone()).is_none());
        }
    }

    #[test]
    fn declared_panes_reconcile_without_losing_view_state() {
        let one = Declaration {
            side: Side::Left,
            path: vec![Step::Key(CellId::from_u128(1))],
        };
        let two = Declaration {
            side: Side::Left,
            path: vec![Step::Key(CellId::from_u128(2))],
        };
        let mut workspace = Workspace::default();
        let panes_path = [Step::Key(vocabulary::PANES)];
        workspace.sync_declared(&[one.clone(), two.clone()]);
        assert!(crate::annotations::collapsed(
            &workspace.document.annotations,
            &panes_path,
            false,
        ));
        assert!(workspace.document.annotations.at(&one.path).is_none());
        assert!(workspace.document.annotations.at(&two.path).is_none());
        crate::annotations::set_collapsed(
            &mut workspace.document.annotations,
            &panes_path,
            false,
            false,
        );
        let root = workspace.left.panes[0].view.root.clone();
        workspace.left.panes[0].view.scroll = Vec2::new(4.0, 9.0);
        workspace.left.panes[0].view.projection = Projection::Raw;

        workspace.sync_declared(&[two, one.clone()]);
        let moved = &workspace.left.panes[1].view;
        assert_eq!(moved.root, root);
        assert_eq!(moved.scroll, Vec2::new(4.0, 9.0));
        assert_eq!(moved.projection, Projection::Raw);
        assert!(!crate::annotations::collapsed(
            &workspace.document.annotations,
            &panes_path,
            false,
        ));
        assert!(workspace.can_move(&root, Move::Up));

        workspace.sync_declared(&[]);
        assert!(workspace.left.panes.is_empty());
        workspace.sync_declared(&[one]);
        assert!(!crate::annotations::collapsed(
            &workspace.document.annotations,
            &panes_path,
            false,
        ));
    }

    #[test]
    fn minimums_override_without_rewriting_requested_percentages() {
        let sizes = allocate(&[0.05, 0.95], 1_000.0, 100.0);
        assert_eq!(sizes, [100.0, 900.0]);
        assert_eq!(allocate(&[0.1, 0.9], 100.0, 80.0), [50.0, 50.0]);
    }

    #[test]
    fn divider_drags_update_requests_without_baking_in_minimums() {
        let mut workspace = Workspace::default();
        let upper = add_pane(&mut workspace, Side::Left);
        let lower = add_pane(&mut workspace, Side::Left);

        let columns = Divider::Columns(Side::Left);
        workspace.left_width = 0.05;
        workspace.start_resize(columns.clone(), Vec2::new(180.0, 0.0));
        assert!(workspace.resize(&columns, Vec2::new(180.0, 0.0), Size::new(1_000.0, 600.0),));
        assert_eq!(workspace.left_width, 0.05);
        let geometry = workspace.geometry(Size::new(1_000.0, 600.0), 1.0);
        let left_width = geometry
            .views
            .iter()
            .find(|placed| placed.root == upper)
            .unwrap()
            .rect
            .width();
        assert_eq!(left_width, 180.0);
        assert_eq!(workspace.left_width, 0.05);

        workspace.finish_resize(&columns);
        let panes = Divider::Panes {
            side: Side::Left,
            before: upper,
            after: lower,
        };
        workspace.start_resize(panes.clone(), Vec2::new(0.0, 300.0));
        assert!(workspace.resize(&panes, Vec2::new(0.0, 450.0), Size::new(1_000.0, 600.0),));
        assert_eq!(workspace.left.panes[0].height, 0.75);
        assert_eq!(workspace.left.panes[1].height, 0.25);
    }

    #[test]
    fn geometry_keeps_document_full_height_and_stacks_each_side() {
        let mut workspace = Workspace::default();
        let document = workspace.document.root.clone();
        add_pane(&mut workspace, Side::Left);
        add_pane(&mut workspace, Side::Left);
        add_pane(&mut workspace, Side::Right);
        let geometry = workspace.geometry(Size::new(900.0, 600.0), 1.0);
        let document = geometry
            .views
            .iter()
            .find(|placed| placed.root == document)
            .unwrap();
        assert_eq!((document.rect.y0, document.rect.y1), (0.0, 600.0));
        let left: Vec<_> = geometry
            .views
            .iter()
            .filter(|placed| workspace.side(&placed.root) == Some(Side::Left))
            .collect();
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].rect.height(), left[1].rect.height());
        assert_eq!(geometry.dividers.len(), 3);
    }
}
