//! Optional code origins as ordinary GID annotations, not execution history.

use crate::lower::{OriginId, OriginNode};
use crate::{Expression, path};
use gid::{CellId, Value};

pub mod vocabulary {
    use gid::CellId;

    pub const BODY_ORIGIN: CellId = CellId::from_u128(0x215831fe58bde294e79cccbc74212318);
    pub const INPUT: CellId = CellId::from_u128(0x9a87743644af3453a0415dab056a0c5d);
    pub const ROOT: CellId = CellId::from_u128(0x7548280579760b782d1f1ce75b5394e9);
    pub const CELL: CellId = CellId::from_u128(0xad9b456873ec85947b96bab375b3e28e);
    pub const PATH: CellId = CellId::from_u128(0x7da5a4ab1d994cc1f9df7af5668b6c85);
}

/// Where code was written, as a path from where it starts: the input an
/// evaluation was handed, which only its host can place; the host's
/// document root; or a cell's definition. Generated code has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    Input(Vec<gid::Step>),
    Root(Vec<gid::Step>),
    Cell { cell: CellId, path: Vec<gid::Step> },
}

pub fn value(origin: &SourceOrigin) -> Value {
    use vocabulary::*;
    match origin {
        SourceOrigin::Input(steps) => Value::record([(INPUT, path::value(steps))]),
        SourceOrigin::Root(steps) => Value::record([(ROOT, path::value(steps))]),
        SourceOrigin::Cell { cell, path: steps } => {
            Value::record([(CELL, (*cell).into()), (PATH, path::value(steps))])
        }
    }
}

pub fn read(value: &Value) -> Option<SourceOrigin> {
    use vocabulary::*;
    let fields = value.as_record()?;
    match (fields.get(&INPUT), fields.get(&ROOT), fields.get(&CELL)) {
        (Some(steps), None, None) => path::read(steps).map(SourceOrigin::Input),
        (None, Some(steps), None) => path::read(steps).map(SourceOrigin::Root),
        (None, None, Some(cell)) => Some(SourceOrigin::Cell {
            cell: cell.as_cell()?,
            path: path::read(fields.get(&PATH)?)?,
        }),
        _ => None,
    }
}

pub(crate) fn origin(expression: &Expression) -> Option<SourceOrigin> {
    let (start, steps) = route(expression.0.origin.as_ref()?);
    let mut origin = start.clone();
    let path = match &mut origin {
        SourceOrigin::Input(path) | SourceOrigin::Root(path) => path,
        SourceOrigin::Cell { path, .. } => path,
    };
    path.extend(steps);
    Some(origin)
}

/// Where `origin`'s chain starts, and the steps from there down to `origin`.
fn route(origin: &OriginId) -> (&SourceOrigin, Vec<gid::Step>) {
    match origin.0.as_ref() {
        OriginNode::Start(start) => (start, Vec::new()),
        OriginNode::Child { parent, step } => {
            let (start, mut path) = route(parent);
            path.push(step.clone());
            (start, path)
        }
    }
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
