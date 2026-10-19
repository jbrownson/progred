//! The assigned-size pane contract, reusing ordinary projection lowering.

use super::Projection;
use crate::libraries::presentation;
use crate::sources::Sources;
use gid::{Path, Step, Value};
use kurbo::Size;

pub(crate) struct Entry<'a> {
    pub value: &'a Value,
    pub path: Path,
}

pub(crate) fn entry<'a>(sources: Sources<'a>, path: &[Step]) -> Option<Entry<'a>> {
    let mut value = sources.resolve_path(path)?;
    let mut path = path.to_vec();
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
