//! Editor-owned views over one document. The document stays in the
//! center; panes form independently sized columns on either side.
//! A root convention may declare pane membership, side, order, and a
//! presentation function; view mode, annotations, scroll, and live
//! sizing remain process state.

use crate::annotations::Annotations;
use gid::{CellId, Cells, Path, Step, Value};
use kurbo::{Rect, Size, Vec2};
use progred_libraries::{Library, name, presentation};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

const DEFAULT_SIDE_WIDTH: f64 = 1.0 / 3.0;

pub const ID: CellId = CellId::from_u128(0x7c295d8a64d3e257dc2c3932e43def74);

pub mod vocabulary {
    use gid::CellId;

    pub const PANES: CellId = CellId::from_u128(0xf30d400a4321a4d44d1628a8adc5a84d);
    pub const LEFT: CellId = CellId::from_u128(0xdc3a1b9a7fb4bc348760160e3b365bca);
    pub const RIGHT: CellId = CellId::from_u128(0xf13a5c1c4471c00178575a0e876768f8);
}

/// The editor-owned vocabulary contributed to the ordinary source
/// environment. Pane behavior remains root-sensitive host behavior;
/// these cells only give its document fields readable names.
pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::PANES, "panes"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        "workspace",
        progred_libraries::Definitions::from_parts(cells, Default::default()),
        vec![],
    )
    .with_root_completions([progred_display::Completion::new(
        "workspace",
        Value::record([(
            vocabulary::PANES,
            Value::record([
                (vocabulary::LEFT, Value::list([])),
                (vocabulary::RIGHT, Value::list([])),
            ]),
        )]),
    )
    .with_aliases(["panes"])
    .with_detail("workspace library")])
    .with_root_field_completions([progred_display::Completion::new(
        "panes",
        Value::from(vocabulary::PANES),
    )
    .with_detail("workspace library")])
}

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
    /// `anchor` is the current document path through which ordinary
    /// editing reaches this cell. The Rc root distinguishes duplicate
    /// views of that occurrence.
    Cell {
        cell: CellId,
        anchor: Path,
    },
    /// A pane declared by an occurrence beneath the document root.
    /// Its source path is both its editable root and its durable
    /// identity within this document; the surrounding [`Root`] keeps
    /// the view's process-local identity stable while it remains.
    Declared {
        value_path: Path,
        projection_path: Path,
    },
}

/// A view's transient identity and immutable data root. Equality is
/// allocation identity: two panes over the same cell remain distinct.
#[derive(Clone)]
pub struct Root(Rc<Target>);

impl Root {
    pub fn document() -> Self {
        Self(Rc::new(Target::Document))
    }

    pub fn cell(cell: CellId, anchor: Path) -> Self {
        Self(Rc::new(Target::Cell { cell, anchor }))
    }

    fn declared(value_path: Path, projection_path: Path) -> Self {
        Self(Rc::new(Target::Declared {
            value_path,
            projection_path,
        }))
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub side: Side,
    pub record_path: Path,
    pub value_path: Path,
    /// Resolves to an optional ordinary Grap callable applied to the
    /// pane's source value. An absent result falls through to the
    /// ordinary projection of that source.
    pub projection_path: Path,
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
                declarations.iter().filter_map(move |(position, value)| {
                    value.as_record()?.get(&presentation::vocabulary::VALUE)?;
                    let parent = vec![
                        Step::Key(vocabulary::PANES),
                        Step::Key(field),
                        Step::Element(position.clone()),
                    ];
                    Some(Declaration {
                        side,
                        record_path: parent.clone(),
                        value_path: parent
                            .iter()
                            .cloned()
                            .chain([Step::Key(presentation::vocabulary::VALUE)])
                            .collect(),
                        projection_path: parent
                            .into_iter()
                            .chain([Step::Key(presentation::vocabulary::PROJECTION)])
                            .collect(),
                    })
                })
            })
    })
    .collect()
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
        Self {
            document: View::new(Root::document()),
            left: Column::default(),
            right: Column::default(),
            left_width: DEFAULT_SIDE_WIDTH,
            right_width: DEFAULT_SIDE_WIDTH,
            dragging: None,
        }
    }
}

impl Workspace {
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

    pub fn open_cell(&mut self, side: Side, cell: CellId, anchor: Path) -> Root {
        let root = Root::cell(cell, anchor);
        let column = self.column_mut(side);
        let count = column.panes.len() + 1;
        let retained = (count - 1) as f64 / count as f64;
        for pane in &mut column.panes {
            pane.height *= retained;
        }
        column.panes.push(Pane {
            view: View::new(root.clone()),
            height: 1.0 / count as f64,
        });
        root
    }

    /// Reconcile the document-declared panes with the live workspace.
    /// Declaration paths determine side and order; surviving panes
    /// retain their process identity, annotations, scroll, projection,
    /// and requested height. Manually opened session panes remain after
    /// the declared panes in their respective columns.
    pub fn sync_declared(&mut self, declarations: &[Declaration]) {
        let mut declared = Vec::new();
        let mut session_left = Vec::new();
        let mut session_right = Vec::new();
        for (side, pane) in std::mem::take(&mut self.left.panes)
            .into_iter()
            .map(|pane| (Side::Left, pane))
            .chain(
                std::mem::take(&mut self.right.panes)
                    .into_iter()
                    .map(|pane| (Side::Right, pane)),
            )
        {
            if matches!(pane.view.root.target(), Target::Declared { .. }) {
                declared.push(pane);
            } else {
                match side {
                    Side::Left => session_left.push(pane),
                    Side::Right => session_right.push(pane),
                }
            }
        }

        let mut left = Vec::new();
        let mut right = Vec::new();
        for declaration in declarations {
            let existing = declared.iter().position(|pane| {
                matches!(
                    pane.view.root.target(),
                    Target::Declared { value_path, .. }
                        if *value_path == declaration.value_path
                )
            });
            let pane = existing.map_or_else(
                || {
                    crate::annotations::set_collapsed(
                        &mut self.document.annotations,
                        &declaration.record_path,
                        false,
                        true,
                    );
                    Pane {
                        view: View::new(Root::declared(
                            declaration.value_path.clone(),
                            declaration.projection_path.clone(),
                        )),
                        height: 1.0,
                    }
                },
                |index| declared.remove(index),
            );
            match declaration.side {
                Side::Left => left.push(pane),
                Side::Right => right.push(pane),
            }
        }
        left.extend(session_left);
        right.extend(session_right);
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

    pub fn close(&mut self, root: &Root) -> bool {
        if matches!(root.target(), Target::Declared { .. }) {
            return false;
        }
        let Some(side) = self.side(root) else {
            return false;
        };
        let column = self.column_mut(side);
        let before = column.panes.len();
        column.panes.retain(|pane| pane.view.root != *root);
        if column.panes.len() == before {
            return false;
        }
        normalize(&mut column.panes);
        if self
            .dragging
            .as_ref()
            .is_some_and(|drag| drag.divider.names(root))
        {
            self.dragging = None;
        }
        true
    }

    pub fn can_move(&self, root: &Root, direction: Move) -> bool {
        if matches!(root.target(), Target::Declared { .. }) {
            return false;
        }
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

    pub fn move_pane(&mut self, root: &Root, direction: Move) -> bool {
        if !self.can_move(root, direction) {
            return false;
        }
        let side = self.side(root).expect("a movable pane has a side");
        match direction {
            Move::Up | Move::Down => {
                let column = self.column_mut(side);
                let index = column
                    .panes
                    .iter()
                    .position(|pane| pane.view.root == *root)
                    .expect("the pane is in its column");
                let other = if direction == Move::Up {
                    index - 1
                } else {
                    index + 1
                };
                column.panes.swap(index, other);
            }
            Move::Left | Move::Right => {
                let destination = if direction == Move::Left {
                    Side::Left
                } else {
                    Side::Right
                };
                let index = self
                    .column(side)
                    .panes
                    .iter()
                    .position(|pane| pane.view.root == *root)
                    .expect("the pane is in its column");
                let pane = self.column_mut(side).panes.remove(index);
                normalize(&mut self.column_mut(side).panes);
                self.column_mut(destination).panes.push(pane);
                normalize(&mut self.column_mut(destination).panes);
            }
        }
        true
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

    fn column(&self, side: Side) -> &Column {
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

impl Divider {
    fn names(&self, root: &Root) -> bool {
        matches!(self, Self::Panes { before, after, .. } if before == root || after == root)
    }
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

    #[test]
    fn roots_are_view_identity_not_cell_identity() {
        let cell = CellId::from_u128(1);
        let left = Root::cell(cell, Vec::new());
        let other = Root::cell(cell, Vec::new());
        assert_ne!(left, other);
        assert_eq!(left, left.clone());
    }

    #[test]
    fn duplicate_views_keep_independent_projection_state() {
        let mut workspace = Workspace::default();
        let cell = CellId::from_u128(1);
        let one = workspace.open_cell(Side::Left, cell, Vec::new());
        let other = workspace.open_cell(Side::Right, cell, Vec::new());

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
    fn columns_stack_move_close_and_keep_the_document() {
        let mut workspace = Workspace::default();
        let document = workspace.document.root.clone();
        let one = workspace.open_cell(Side::Left, CellId::from_u128(1), Vec::new());
        let two = workspace.open_cell(Side::Left, CellId::from_u128(2), Vec::new());
        assert_eq!(workspace.left.panes.len(), 2);
        assert!(workspace.move_pane(&two, Move::Up));
        assert_eq!(workspace.left.panes[0].view.root, two);
        assert!(workspace.move_pane(&two, Move::Right));
        assert_eq!(workspace.side(&two), Some(Side::Right));
        assert!(!workspace.close(&document));
        assert!(workspace.close(&one));
        assert!(workspace.view(&document).is_some());
    }

    #[test]
    fn root_pane_declarations_are_paths_to_their_contents() {
        let root = Value::record([(
            vocabulary::PANES,
            Value::record([
                (
                    vocabulary::LEFT,
                    Value::list([Value::record([
                        (
                            presentation::vocabulary::VALUE,
                            Value::from(CellId::from_u128(1)),
                        ),
                        (
                            presentation::vocabulary::PROJECTION,
                            Value::from(CellId::from_u128(9)),
                        ),
                    ])]),
                ),
                (
                    vocabulary::RIGHT,
                    Value::list([Value::record([(
                        presentation::vocabulary::VALUE,
                        Value::from(CellId::from_u128(2)),
                    )])]),
                ),
            ]),
        )]);
        let declarations = declarations(Some(&root));
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].side, Side::Left);
        assert_eq!(declarations[1].side, Side::Right);
        assert_eq!(
            declarations[0].projection_path.last(),
            Some(&Step::Key(presentation::vocabulary::PROJECTION))
        );
        assert_eq!(
            declarations[0].record_path.first(),
            Some(&Step::Key(vocabulary::PANES))
        );
        assert_eq!(
            declarations[0].value_path.first(),
            Some(&Step::Key(vocabulary::PANES))
        );
        assert_eq!(
            declarations[0].value_path.last(),
            Some(&Step::Key(presentation::vocabulary::VALUE))
        );
        assert_eq!(declarations[0].record_path, declarations[0].value_path[..3]);
    }

    #[test]
    fn declared_panes_reconcile_without_losing_view_state() {
        let one = Declaration {
            side: Side::Left,
            record_path: vec![Step::Key(CellId::from_u128(11))],
            value_path: vec![Step::Key(CellId::from_u128(1))],
            projection_path: vec![Step::Key(CellId::from_u128(9))],
        };
        let two = Declaration {
            side: Side::Left,
            record_path: vec![Step::Key(CellId::from_u128(12))],
            value_path: vec![Step::Key(CellId::from_u128(2))],
            projection_path: vec![Step::Key(CellId::from_u128(9))],
        };
        let mut workspace = Workspace::default();
        workspace.sync_declared(&[one.clone(), two.clone()]);
        assert!(crate::annotations::collapsed(
            &workspace.document.annotations,
            &one.record_path,
            false,
        ));
        crate::annotations::set_collapsed(
            &mut workspace.document.annotations,
            &one.record_path,
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
            &one.record_path,
            false,
        ));
        assert!(!workspace.can_move(&root, Move::Up));
        assert!(!workspace.close(&root));

        workspace.sync_declared(&[]);
        assert!(workspace.left.panes.is_empty());
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
        let upper = workspace.open_cell(Side::Left, CellId::from_u128(1), Vec::new());
        let lower = workspace.open_cell(Side::Left, CellId::from_u128(2), Vec::new());

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
        workspace.open_cell(Side::Left, CellId::from_u128(1), Vec::new());
        workspace.open_cell(Side::Left, CellId::from_u128(2), Vec::new());
        workspace.open_cell(Side::Right, CellId::from_u128(3), Vec::new());
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
