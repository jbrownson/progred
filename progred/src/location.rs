//! Where stored data lives. A location starts at the document's root or at a
//! cell's definition and descends by ordinary steps from there. Following a
//! cell starts a new location at its definition, however the cell was reached,
//! so whether something can be edited depends on where it's stored, not on
//! the route to it. A computed value isn't stored, so it has no location of
//! its own, but a cell inside one still starts one.

use crate::sources::Sources;
use gid::{CellId, Resolution, Step};
use std::borrow::Cow;

/// Where a location starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The document's root value.
    Root,
    /// The definition `source` gives `cell`.
    Cell { cell: CellId, source: Resolution },
}

/// A start and the steps from it. The steps can still follow cells, as a
/// route a projection's conject names may; [`Location::stored`] restarts the
/// location at the last cell it follows.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    pub fn in_cell(cell: CellId, source: Resolution, path: impl Into<Cow<'a, [Step]>>) -> Self {
        Self {
            start: Start::Cell { cell, source },
            path: path.into(),
        }
    }

    pub fn into_owned(self) -> Location<'static> {
        Location {
            start: self.start,
            path: Cow::Owned(self.path.into_owned()),
        }
    }

    /// The location `steps` below this one.
    pub fn descendant(&self, steps: &[Step]) -> Location<'static> {
        Location {
            start: self.start,
            path: self.path.iter().chain(steps).cloned().collect(),
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
                source: Resolution::Document,
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
mod tests {
    use super::*;
    use crate::libraries::{Libraries, text};
    use gid::{Cells, Document, Value, new_cell_id};

    fn sources<'a>(doc: &'a Document, libraries: &'a Libraries) -> Sources<'a> {
        Sources { doc, libraries }
    }

    #[test]
    fn a_location_reads_from_its_start() {
        let (field, cell) = (new_cell_id(), new_cell_id());
        let mut cells = Cells::new();
        cells.set_value(cell, Value::record([(field, text::value("inside"))]));
        let doc = Document {
            root: Some(Value::record([(field, text::value("at the root"))])),
            cells,
        };
        let libraries = Libraries::default();
        let sources = sources(&doc, &libraries);
        let path = [Step::Key(field)];
        assert_eq!(
            sources.read(&Location::root(&path[..])),
            Some(&text::value("at the root"))
        );
        assert_eq!(
            sources.read(&Location::in_cell(cell, Resolution::Document, &path[..])),
            Some(&text::value("inside"))
        );
        assert_eq!(
            sources.read(&Location::in_cell(
                cell,
                Resolution::Library(cell),
                &path[..]
            )),
            None
        );
    }

    #[test]
    fn a_route_through_a_cell_is_stored_in_its_definition() {
        let (field, cell, inner) = (new_cell_id(), new_cell_id(), new_cell_id());
        let mut cells = Cells::new();
        cells.set_value(cell, Value::record([(field, Value::from(inner))]));
        let doc = Document {
            root: Some(Value::record([(field, Value::from(cell))])),
            cells,
        };
        let libraries = Libraries::default();
        let sources = sources(&doc, &libraries);
        let follow = || Step::Follow(Resolution::Document);
        let route = Location::root(vec![
            Step::Key(field),
            follow(),
            Step::Key(field),
            follow(),
            Step::Key(field),
        ]);
        assert_eq!(
            route.stored(&sources),
            Some(Location::in_cell(
                inner,
                Resolution::Document,
                vec![Step::Key(field)]
            ))
        );
        let from_cell =
            Location::in_cell(cell, Resolution::Document, vec![Step::Key(field), follow()]);
        assert_eq!(
            from_cell.stored(&sources),
            Some(Location::in_cell(inner, Resolution::Document, Vec::new()))
        );
        let spine = Location::root(vec![Step::Key(field)]);
        assert_eq!(spine.stored(&sources), Some(spine.clone()));
        let broken = Location::root(vec![Step::Key(new_cell_id()), follow()]);
        assert_eq!(broken.stored(&sources), None);
    }

    #[test]
    fn writes_land_where_the_document_owns_the_data() {
        let (field, cell) = (new_cell_id(), new_cell_id());
        let doc = Document {
            root: Some(Value::record([(field, Value::from(text::ID))])),
            cells: Cells::new(),
        };
        let stack = crate::stack::load();
        let sources = sources(&doc, &stack.libraries);
        assert!(writable(&sources, &Location::root(vec![Step::Key(field)])));
        assert!(writable(
            &sources,
            &Location::in_cell(cell, Resolution::Document, Vec::new())
        ));
        assert!(!writable(
            &sources,
            &Location::in_cell(text::ID, Resolution::Library(text::ID), Vec::new())
        ));
        assert!(!writable(
            &sources,
            &Location::root(vec![
                Step::Key(field),
                Step::Follow(Resolution::Library(text::ID))
            ])
        ));
        assert!(!writable(
            &sources,
            &Location::root(vec![
                Step::Key(new_cell_id()),
                Step::Follow(Resolution::Document)
            ])
        ));
    }
}
