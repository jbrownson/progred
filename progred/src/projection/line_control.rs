//! Apply handler-owned line conversions to document and selection state.

use super::*;

/// Apply this handler's conversion and report whether it opens an undo step.
pub(crate) fn commit(
    doc: &mut Rc<gid::Document>,
    libraries: &crate::libraries::Libraries,
    selection: &mut Selection,
    update: &crate::display::LineUpdate,
) -> bool {
    let sources = Sources { doc, libraries };
    let next = selection
        .value_edit()
        .filter(|_| writable_at(&sources, selection.path()))
        .and_then(|editor| {
            let current = sources.resolve_path(selection.path());
            update(&sources, editor.text(), current).filter(|next| current != Some(next))
        });
    if next.is_some_and(|next| crate::selection::set_value(doc, libraries, selection.path(), next))
    {
        let first = !selection.recorded();
        selection.preserve_recorded(true);
        first
    } else {
        false
    }
}

pub(crate) fn edit(
    doc: &mut Rc<gid::Document>,
    libraries: &crate::libraries::Libraries,
    selection: &mut Selection,
    line: &crate::display::LineEdit,
    operation: impl FnOnce(&mut LineEditState) -> bool,
) -> (bool, bool) {
    let state = selection.edit_line_mut(&line.text);
    let before = state.text().to_owned();
    let handled = operation(state);
    let changed = before != state.text();
    let record = handled && changed && commit(doc, libraries, selection, &line.update);
    (handled, record)
}
