//! The assigned-size pane contract, reusing ordinary projection lowering.

use super::Projection;
use crate::libraries::presentation;
use crate::location::Location;
use crate::sources::Sources;
use gid::{Path, Step, Value};
use kurbo::Size;

/// A sized value a pane fills itself with: the value, and the cells
/// followed from the pane's place to reach it.
pub(crate) struct Entry<'a> {
    pub value: &'a Value,
    pub path: Path,
}

/// The sized value at `place`, past any cells it names.
pub(crate) fn entry<'a>(sources: Sources<'a>, place: &Location) -> Option<Entry<'a>> {
    let mut value = sources.read(place)?;
    let mut path = Vec::new();
    let mut seen = Vec::new();
    while let Some(cell) = value.as_cell() {
        if seen.contains(&cell) {
            return None;
        }
        seen.push(cell);
        let definition = sources.resolve(cell)?;
        path.push(Step::Follow(definition.resolution));
        value = definition.value;
    }
    presentation::viewport(value).map(|_| Entry { value, path })
}

pub(crate) fn projection(ambient: &Projection, size: Size) -> Projection {
    ambient
        .clone()
        .with_entry(crate::display::partial(move |input| {
            presentation::viewport_display(input, size.width, size.height)
        }))
}

#[cfg(test)]
mod tests;
