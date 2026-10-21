//! Where something is drawn. An occurrence path names one drawn occurrence the
//! way a document path names stored data, and it says where that occurrence's
//! data is: its steps descend through data as a document path's do, a jump
//! restarts them at a location, and a computed value stops them until a jump
//! inside it. So the path alone tells two drawings of the same data apart, and
//! says where each reads and writes.

use crate::location::{Location, Start};
use crate::sources::Sources;
use gid::{CellId, Position, Resolution, Value};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// One step of an occurrence path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Into a record's field, as a document path steps.
    Key(CellId),
    /// Into a list's element.
    Element(Position),
    /// Through a reference to its cell's definition from `resolution`.
    Follow(Resolution),
    /// From here on, the data at `location`: where a projection jumped, or a
    /// cell followed inside a computed value, which has no stored route to it.
    Jump(Rc<Location<'static>>),
    /// From here on, a computed value, which isn't stored, until a jump
    /// inside it.
    Computed,
}

pub type Path = Vec<Step>;

/// Occurrence paths key every drawn occurrence's state, so they hash on each
/// frame: a jump hashes only where its location starts and how far it goes,
/// which equal jumps share, and leaves telling the rest apart to equality.
impl Hash for Step {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Key(cell) => cell.hash(state),
            Self::Element(position) => position.hash(state),
            Self::Follow(resolution) => resolution.hash(state),
            Self::Jump(location) => {
                location.start.hash(state);
                location.path.len().hash(state);
            }
            Self::Computed => {}
        }
    }
}

impl From<gid::Step> for Step {
    fn from(step: gid::Step) -> Self {
        match step {
            gid::Step::Key(cell) => Self::Key(cell),
            gid::Step::Element(position) => Self::Element(position),
            gid::Step::Follow(resolution) => Self::Follow(resolution),
        }
    }
}

impl From<&gid::Step> for Step {
    fn from(step: &gid::Step) -> Self {
        step.clone().into()
    }
}

/// In tests, an occurrence's data step equals the document step it is, so a
/// test can name a drawn place by its document path. Elsewhere the two kinds
/// of path stay apart.
#[cfg(test)]
impl PartialEq<gid::Step> for Step {
    fn eq(&self, other: &gid::Step) -> bool {
        self.data().as_ref() == Some(other)
    }
}

#[cfg(test)]
impl PartialEq<Step> for gid::Step {
    fn eq(&self, other: &Step) -> bool {
        other == self
    }
}

impl Step {
    /// The jump to `location`.
    pub fn jump(location: Location<'_>) -> Self {
        Self::Jump(Rc::new(location.into_owned()))
    }

    /// This step as a document path's, when it is one.
    pub fn data(&self) -> Option<gid::Step> {
        match self {
            Self::Key(cell) => Some(gid::Step::Key(*cell)),
            Self::Element(position) => Some(gid::Step::Element(position.clone())),
            Self::Follow(resolution) => Some(gid::Step::Follow(*resolution)),
            Self::Jump(_) | Self::Computed => None,
        }
    }
}

/// A document path's steps, as an occurrence's.
pub fn of(steps: &[gid::Step]) -> Path {
    steps.iter().map(Step::from).collect()
}

/// `path` with a document path's `steps` after it.
pub fn below(path: &[Step], steps: &[gid::Step]) -> Path {
    path.iter()
        .cloned()
        .chain(steps.iter().map(Step::from))
        .collect()
}

/// The document path `path` is, when it's only data steps.
#[cfg(test)]
pub fn document(path: &[Step]) -> Option<gid::Path> {
    path.iter().map(Step::data).collect()
}

/// Whether `steps` are all data steps, as a document path's are.
pub fn is_data(steps: &[Step]) -> bool {
    steps.iter().all(|step| step.data().is_some())
}

/// Whether what's at `path` is computed: the last jump or computed value on
/// the way there is a computed value.
pub fn computed(path: &[Step]) -> bool {
    matches!(
        path.iter()
            .rev()
            .find(|step| matches!(step, Step::Jump(_) | Step::Computed)),
        Some(Step::Computed)
    )
}

/// Where the data at `path` is: from the document's root, or from the last
/// jump's location, along the data steps after it. None for a computed value.
pub fn location(path: &[Step]) -> Option<Location<'static>> {
    let marker = path
        .iter()
        .rposition(|step| matches!(step, Step::Jump(_) | Step::Computed));
    let (mut location, rest) = match marker.map(|index| (&path[index], index)) {
        None => (Location::root(Vec::new()), path),
        Some((Step::Jump(location), index)) => ((**location).clone(), &path[index + 1..]),
        Some(_) => return None,
    };
    location.path.to_mut().extend(
        rest.iter()
            .map(|step| step.data().expect("only data steps follow the last jump")),
    );
    Some(location)
}

/// The value at `path`, where it's stored.
pub fn read<'a>(sources: &Sources<'a>, path: &[Step]) -> Option<&'a Value> {
    sources.read(&location(path)?)
}

/// Whether a write can land where `path`'s data is stored.
pub fn writable(sources: &Sources, path: &[Step]) -> bool {
    stored_start(sources, path).is_some_and(crate::location::owned)
}

/// Where the data at `path` is stored starts, as [`Location::stored`] finds
/// it, without building the location: the definition its last Follow enters,
/// else where its last jump's location is stored, else the root. None for a
/// computed value, or where the route to that Follow no longer reaches a cell.
fn stored_start(sources: &Sources, path: &[Step]) -> Option<Start> {
    let marker = path
        .iter()
        .rposition(|step| matches!(step, Step::Jump(_) | Step::Computed));
    let (from, rest) = match marker {
        None => (None, path),
        Some(index) => match &path[index] {
            Step::Jump(location) => (Some(&**location), &path[index + 1..]),
            _ => return None,
        },
    };
    match rest
        .iter()
        .rposition(|step| matches!(step, Step::Follow(_)))
    {
        Some(follow) => {
            let Step::Follow(resolution) = rest[follow] else {
                unreachable!("found as a Follow")
            };
            let start = from.map_or(Start::Root, |location| location.start);
            let before = from.map_or(&[][..], |location| &location.path);
            let cell = rest[..follow]
                .iter()
                .try_fold(sources.read_from(start, before)?, |value, step| {
                    sources.step(value, &step.data()?)
                })?
                .as_cell()?;
            Some(Start::Cell { cell, resolution })
        }
        None => match from {
            None => Some(Start::Root),
            Some(location) => location.stored(sources).map(|stored| stored.start),
        },
    }
}

/// An occurrence path as ordinary GID data, for Grap and the web embed: data
/// steps in the path convention's encoding, `{jump: location}` and
/// `{computed: {}}`. The editor library names these cells.
pub mod vocabulary {
    use gid::CellId;

    pub const JUMP: CellId = CellId::from_u128(0x03ac221c46af01eda93773d95edd2964);
    pub const COMPUTED: CellId = CellId::from_u128(0x8da525995c2c1283f642561a138397f0);
}

/// `path` as data.
pub fn value(path: &[Step]) -> Value {
    Value::list(path.iter().map(|step| match step {
        Step::Jump(location) => {
            Value::record([(vocabulary::JUMP, crate::location::value(location))])
        }
        Step::Computed => Value::record([(vocabulary::COMPUTED, Value::record([]))]),
        data => ::grap::path::step_value(&data.data().expect("a data step")),
    }))
}

/// The occurrence path `value` describes, when it's one.
pub fn from_value(value: &Value) -> Option<Path> {
    value
        .as_list()?
        .values()
        .map(|step| {
            let fields = step.as_record()?;
            match (
                fields.get(&vocabulary::JUMP),
                fields.get(&vocabulary::COMPUTED),
            ) {
                (Some(location), None) if fields.len() == 1 => {
                    crate::location::read(location).map(Step::jump)
                }
                (None, Some(_)) if fields.len() == 1 => Some(Step::Computed),
                _ => ::grap::path::read_step(step).map(Step::from),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
