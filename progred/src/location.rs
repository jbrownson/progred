//! Where stored data lives. A location starts at the document's root or at a
//! cell's definition and descends by ordinary steps from there. Following a
//! cell starts a new location at its definition, however the cell was reached,
//! so whether something can be edited depends on where it's stored, not on
//! the route to it. A computed value isn't stored, so it has no location of
//! its own, but a cell inside one still starts one. Every edit writes where
//! its location is stored.

use crate::library::Libraries;
use crate::sources::Sources;
use crate::spine;
use gid::{CellId, Document, Resolution, Step, Value};
use std::borrow::Cow;
use std::rc::Rc;

/// A location as ordinary GID data, as a pane declares the place it shows:
/// `{in: cell, follow: resolution, steps: […]}` inside a cell's definition, or
/// `{steps: […]}` from the document's root. The steps and resolution use the
/// path convention's encoding. The editor library names these cells.
pub mod vocabulary {
    use gid::CellId;

    pub const IN: CellId = CellId::from_u128(0x46f7f059b8b648c7a2a973d7deb5c914);
    pub const STEPS: CellId = CellId::from_u128(0xcb41b29467c842858d8e59860308cf73);
    /// The reason a place that can't be reached is absent.
    pub const NOT_FOUND: CellId = CellId::from_u128(0x84f655992c58a9436f12ceac317021d5);
}

/// `location` as data.
pub fn value(location: &Location) -> Value {
    let steps = (vocabulary::STEPS, ::grap::path::value(&location.path));
    match location.start {
        Start::Root => Value::record([steps]),
        Start::Cell { cell, resolution } => Value::record([
            (vocabulary::IN, cell.into()),
            (
                ::grap::path::vocabulary::FOLLOW,
                ::grap::path::resolution_value(resolution),
            ),
            steps,
        ]),
    }
}

/// `location` as an absent: the place, which can't be reached.
pub fn not_found(location: &Location) -> Value {
    let described = value(location);
    let fields = described.as_record().expect("a location is a record");
    Value::record(
        fields
            .iter()
            .map(|(key, value)| (*key, value.clone()))
            .chain([(::grap::absent::ABSENT, vocabulary::NOT_FOUND.into())]),
    )
}

/// The location `value` describes, when it's one.
pub fn read(value: &Value) -> Option<Location<'static>> {
    let fields = value.as_record()?;
    let path = ::grap::path::read(fields.get(&vocabulary::STEPS)?)?;
    let start = match fields.get(&vocabulary::IN) {
        None => Start::Root,
        Some(cell) => Start::Cell {
            cell: cell.as_cell()?,
            resolution: ::grap::path::read_resolution(
                fields.get(&::grap::path::vocabulary::FOLLOW)?,
            )?,
        },
    };
    Some(Location {
        start,
        path: Cow::Owned(path),
    })
}

/// Where a location starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Start {
    /// The document's root value.
    Root,
    /// The definition `resolution` gives `cell`.
    Cell {
        cell: CellId,
        resolution: Resolution,
    },
}

/// A start and the steps from it. The steps can still follow cells, as a
/// route through drawn data may; [`Location::stored`] restarts the location
/// at the last cell it follows.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location<'a> {
    pub start: Start,
    pub path: Cow<'a, [Step]>,
}

impl<'a> Location<'a> {
    pub fn root(path: impl Into<Cow<'a, [Step]>>) -> Self {
        Self {
            start: Start::Root,
            path: path.into(),
        }
    }

    pub fn in_cell(cell: CellId, resolution: Resolution, path: impl Into<Cow<'a, [Step]>>) -> Self {
        Self {
            start: Start::Cell { cell, resolution },
            path: path.into(),
        }
    }

    pub fn into_owned(self) -> Location<'static> {
        Location {
            start: self.start,
            path: Cow::Owned(self.path.into_owned()),
        }
    }

    /// The same location from where its data is stored: the definition its
    /// last Follow enters, or else its own start. None when the route to that
    /// Follow no longer reaches a cell.
    pub fn stored(&self, sources: &Sources) -> Option<Location<'_>> {
        match last_follow(&self.path) {
            Some((before, source, after)) => sources
                .read_from(self.start, before)?
                .as_cell()
                .map(|cell| Location::in_cell(cell, source, after)),
            None => Some(Location {
                start: self.start,
                path: Cow::Borrowed(&self.path),
            }),
        }
    }
}

/// Whether writes can land where `start` stores its data: the root, or one of
/// the document's own definitions. A library's definitions are read-only.
pub fn owned(start: Start) -> bool {
    matches!(
        start,
        Start::Root
            | Start::Cell {
                resolution: Resolution::Document,
                ..
            }
    )
}

/// Whether a write at `location` can land: where it's stored must be the
/// document's own.
pub fn writable(sources: &Sources, location: &Location) -> bool {
    location
        .stored(sources)
        .is_some_and(|stored| owned(stored.start))
}

/// Whether a value could be written at `location`, ignoring permissions: it
/// holds one, or its container does, as for a deleted field or element,
/// or it's a cell's definition, written or not.
pub fn reachable(sources: &Sources, location: &Location) -> bool {
    location
        .stored(sources)
        .is_some_and(|stored| match stored.path.split_last() {
            None => true,
            Some((_, container)) => sources.read_from(stored.start, container).is_some(),
        })
}

/// Writes `value` at `location`, where it's stored: the definition its last
/// Follow enters, or else its start. The single write every edit reduces
/// to: the steps from there are a value spine, rebuilt around the new leaf
/// through the lens. A bare cell takes its first value through the empty
/// spine.
pub fn set_value(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    location: &Location,
    value: Value,
) -> bool {
    let write = {
        let sources = Sources {
            doc: &*doc,
            libraries,
        };
        location
            .stored(&sources)
            .filter(|stored| owned(stored.start))
            .and_then(|stored| {
                spine::set(sources.read_from(stored.start, &[]), &stored.path, value)
                    .map(|rebuilt| (stored.start, rebuilt))
            })
    };
    write
        .map(|(start, rebuilt)| store(doc, start, Some(rebuilt)))
        .is_some()
}

/// Deletes the value at `location`, where it's stored. Within a value it
/// rebuilds the owner without it — unlinking; anything the dropped value
/// linked stays in the table for the orphan pool. At a definition's own
/// start it removes the cell's value: bare again, the symmetric partner of
/// authoring a value into one. At the root's own start it empties the
/// document; locations that no longer resolve decline.
pub fn delete_edge(doc: &mut Rc<Document>, libraries: &Libraries, location: &Location) -> bool {
    let deletion = {
        let sources = Sources {
            doc: &*doc,
            libraries,
        };
        location
            .stored(&sources)
            .filter(|stored| owned(stored.start))
            .and_then(|stored| {
                let owner = sources.read_from(stored.start, &[])?;
                if stored.path.is_empty() {
                    Some((stored.start, None))
                } else {
                    spine::without(owner, &stored.path).map(|rebuilt| (stored.start, Some(rebuilt)))
                }
            })
    };
    deletion
        .map(|(start, value)| store(doc, start, value))
        .is_some()
}

/// Replace, or with None remove, the value stored at `start`.
fn store(doc: &mut Rc<Document>, start: Start, value: Option<Value>) {
    let doc = Rc::make_mut(doc);
    match (start, value) {
        (Start::Root, value) => doc.root = value,
        (Start::Cell { cell, .. }, Some(value)) => doc.cells.set_value(cell, value),
        (Start::Cell { cell, .. }, None) => doc.cells.clear_value(cell),
    }
}

/// The path's last Follow, between the steps before and after it.
fn last_follow(path: &[Step]) -> Option<(&[Step], Resolution, &[Step])> {
    let (before, rest) = path.split_at(
        path.iter()
            .rposition(|step| matches!(step, Step::Follow(_)))?,
    );
    match rest.split_first()? {
        (Step::Follow(source), after) => Some((before, *source, after)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
