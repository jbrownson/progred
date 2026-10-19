//! Committing a picker's choice: the edit it makes, prepared against the
//! document so that nothing is installed unless all of it succeeds.

use crate::editing::Session;
use crate::site::Continuation;
use crate::sources::Sources;
use gid::Value;

/// Commits `value`, with `definition` for a new cell it names, into the
/// pending selection, then interprets `on_commit` against the result. False,
/// changing nothing, when nothing is pending or a step declines.
pub(crate) fn commit(
    session: &mut Session<'_>,
    value: Value,
    definition: Option<Value>,
    on_commit: Option<Continuation>,
) -> bool {
    let before = session.model.snapshot();
    let model = &mut *session.model;
    let libraries = session.libraries;
    if let Some(selection) = &model.selection
        && let Some(view) = model.workspace.view_mut(selection.root())
        && let Some(prepared) = prepare(
            &Sources {
                doc: &model.doc,
                libraries,
            },
            selection,
            &view.annotations,
            value,
            definition,
            on_commit.as_ref(),
        )
    {
        let (root, scope) = (selection.root().clone(), selection.scope().clone());
        if prepared.document_changed {
            model.doc = prepared.document;
            model.history.record(before);
        }
        crate::site::install_scoped(
            prepared.effects,
            &Sources {
                doc: &model.doc,
                libraries,
            },
            &root,
            scope,
            &prepared.path,
            &mut view.annotations,
            &mut model.selection,
        );
        true
    } else {
        false
    }
}

pub(crate) struct Prepared {
    pub document: std::rc::Rc<gid::Document>,
    pub document_changed: bool,
    pub path: gid::Path,
    pub effects: crate::site::PendingChanges,
}

/// Prepare the insertion and interpret its continuation against the
/// resulting document. The caller installs everything only on success.
pub(crate) fn prepare(
    sources: &Sources,
    selection: &crate::selection::Selection,
    annotations: &crate::annotations::Annotations,
    value: Value,
    definition: Option<Value>,
    on_commit: Option<&Continuation>,
) -> Option<Prepared> {
    use crate::selection::Stage;
    selection.writable(sources).then_some(())?;
    let mut document = std::rc::Rc::new(sources.doc.clone());
    let mut path = selection.path().to_vec();
    let document_changed = match selection.stage(sources) {
        Stage::Pending => {
            let defined = definition.zip(value.as_cell());
            crate::location::set_value(
                &mut document,
                sources.libraries,
                &selection.location()?,
                value,
            )
            .then_some(())?;
            if let Some((definition, cell)) = defined {
                std::rc::Rc::make_mut(&mut document)
                    .cells
                    .set_value(cell, definition);
            }
            true
        }
        Stage::Label => {
            let label = value.as_cell()?;
            path.push(gid::Step::Key(label));
            if selection
                .scope()
                .view(*sources)
                .resolve_path(&path)
                .is_some()
            {
                false
            } else {
                let changed = definition.is_some();
                if let Some(value) = definition {
                    std::rc::Rc::make_mut(&mut document)
                        .cells
                        .set_value(label, value);
                }
                changed
            }
        }
        Stage::Edge => return None,
    };
    let annotation = annotations.at(&path).cloned();
    let selection_scope = selection.scope();
    let selection = Some((selection.path().to_vec(), selection.payload()));
    let mut effects = crate::site::PendingChanges {
        annotation,
        annotation_changed: false,
        selection,
        selection_changed: false,
    };
    if let Some(function) = on_commit {
        function(
            &selection_scope.view(Sources {
                doc: &document,
                libraries: sources.libraries,
            }),
            &path,
            &mut effects,
        )
        .then_some(())?;
    }
    Some(Prepared {
        document,
        document_changed,
        path,
        effects,
    })
}

#[cfg(test)]
mod tests;
