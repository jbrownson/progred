//! Optional code origins as ordinary GID annotations, not execution history.

use crate::lower::{OriginId, OriginNode, OriginRoot};
use crate::{Expression, SourceOrigin, path};
use gid::Value;

pub mod vocabulary {
    use gid::CellId;

    pub const BODY_ORIGIN: CellId = CellId::from_u128(0x215831fe58bde294e79cccbc74212318);
    pub const INPUT: CellId = CellId::from_u128(0x9a87743644af3453a0415dab056a0c5d);
    pub const STORED: CellId = CellId::from_u128(0x7548280579760b782d1f1ce75b5394e9);
    pub const CELL: CellId = CellId::from_u128(0xad9b456873ec85947b96bab375b3e28e);
    pub const PATH: CellId = CellId::from_u128(0x7da5a4ab1d994cc1f9df7af5668b6c85);
}

pub fn value(origin: &SourceOrigin) -> Value {
    use vocabulary::*;
    match origin {
        SourceOrigin::Input(steps) => Value::record([(INPUT, path::value(steps))]),
        SourceOrigin::Stored(steps) => Value::record([(STORED, path::value(steps))]),
        SourceOrigin::Cell { cell, path: steps } => {
            Value::record([(CELL, (*cell).into()), (PATH, path::value(steps))])
        }
    }
}

pub fn read(value: &Value) -> Option<SourceOrigin> {
    use vocabulary::*;
    let fields = value.as_record()?;
    match (fields.get(&INPUT), fields.get(&STORED), fields.get(&CELL)) {
        (Some(steps), None, None) => path::read(steps).map(SourceOrigin::Input),
        (None, Some(steps), None) => path::read(steps).map(SourceOrigin::Stored),
        (None, None, Some(cell)) => Some(SourceOrigin::Cell {
            cell: cell.as_cell()?,
            path: path::read(fields.get(&PATH)?)?,
        }),
        _ => None,
    }
}

pub(crate) fn origin(expression: &Expression) -> Option<SourceOrigin> {
    let (root, path) = route(expression.0.origin.as_ref()?);
    Some(match root {
        OriginRoot::Input => SourceOrigin::Input(path),
        OriginRoot::Located(base) => {
            let mut base = base.clone();
            let prefix = match &mut base {
                SourceOrigin::Input(path) | SourceOrigin::Stored(path) => path,
                SourceOrigin::Cell { path, .. } => path,
            };
            prefix.extend(path);
            base
        }
        OriginRoot::Cell(cell) => SourceOrigin::Cell { cell: *cell, path },
    })
}

/// `origin`'s root and the steps from it down to `origin`.
fn route(origin: &OriginId) -> (&OriginRoot, Vec<gid::Step>) {
    match origin.0.as_ref() {
        OriginNode::Root(root) => (root, Vec::new()),
        OriginNode::Child { parent, step } => {
            let (root, mut path) = route(parent);
            path.push(step.clone());
            (root, path)
        }
    }
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
