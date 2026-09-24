//! Optional code origins as ordinary GID annotations, not execution history.

use crate::{Expression, OriginNode, OriginRoot, SourceOrigin, path};
use gid::Value;

pub mod vocabulary {
    use gid::CellId;

    pub const BODY_ORIGIN: CellId = CellId::from_u128(0x215831fe58bde294e79cccbc74212318);
    pub const INPUT: CellId = CellId::from_u128(0x9a87743644af3453a0415dab056a0c5d);
    pub const STORED: CellId = CellId::from_u128(0x7548280579760b782d1f1ce75b5394e9);
    pub const CELL: CellId = CellId::from_u128(0xad9b456873ec85947b96bab375b3e28e);
    pub const PATH: CellId = CellId::from_u128(0x7da5a4ab1d994cc1f9df7af5668b6c85);
    pub const SOURCE: CellId = CellId::from_u128(0xb6e1a63b61cda48d634807e0ff298a68);
}

pub fn value(origin: &SourceOrigin) -> Value {
    use vocabulary::*;
    match origin {
        SourceOrigin::Input(steps) => Value::record([(INPUT, path::value(steps))]),
        SourceOrigin::Stored(steps) => Value::record([(STORED, path::value(steps))]),
        SourceOrigin::Cell {
            cell,
            source,
            path: steps,
        } => Value::record([
            (CELL, (*cell).into()),
            (SOURCE, path::resolution_value(*source)),
            (PATH, path::value(steps)),
        ]),
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
            source: path::read_resolution(fields.get(&SOURCE)?)?,
            path: path::read(fields.get(&PATH)?)?,
        }),
        _ => None,
    }
}

pub(crate) fn origin(expression: &Expression) -> Option<SourceOrigin> {
    let mut origin = expression.0.origin.as_ref()?;
    let mut path = Vec::new();
    loop {
        match origin.0.as_ref() {
            OriginNode::Root(root) => {
                path.reverse();
                return Some(match root {
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
                    OriginRoot::Cell { cell, source } => SourceOrigin::Cell {
                        cell: *cell,
                        source: *source,
                        path,
                    },
                });
            }
            OriginNode::Child { parent, step } => {
                path.push(step.clone());
                origin = parent;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{Resolution, Step, new_cell_id};

    #[test]
    fn origins_round_trip_with_open_metadata_and_stable_paths() {
        let path = vec![
            Step::Key(new_cell_id()),
            Step::Element(gid::position::between(None, None).unwrap()),
        ];
        for origin in [
            SourceOrigin::Input(path.clone()),
            SourceOrigin::Stored(path.clone()),
            SourceOrigin::Cell {
                cell: new_cell_id(),
                source: Resolution::Document,
                path: path.clone(),
            },
            SourceOrigin::Cell {
                cell: new_cell_id(),
                source: Resolution::Library(new_cell_id()),
                path,
            },
        ] {
            let encoded = value(&origin);
            assert_eq!(read(&encoded), Some(origin.clone()));
            let enriched = Value::Record(
                encoded
                    .as_record()
                    .unwrap()
                    .update(new_cell_id(), Value::record([])),
            );
            assert_eq!(read(&enriched), Some(origin));
        }
        assert_eq!(read(&Value::record([])), None);
        assert_eq!(
            read(&Value::record([
                (vocabulary::INPUT, Value::list([])),
                (vocabulary::STORED, Value::list([]))
            ])),
            None
        );
    }
}
