//! Progred's document-aware line control: Puri composition and current-handler edits.

use super::*;

/// A selected line interprets missing state as its current spelling
/// with the caret at the end. Input materializes that same default;
/// projection never writes it back merely for being selected.
pub(super) fn view<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    line: progred_display::LineEdit,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let writable = !cx.source.transient() && writable_at(&cx.sources, path);
    let selected = cx.selection.filter(|selection| {
        writable && selection.path() == path && selection.stage(&cx.sources) == Stage::Edge
    });
    let default = selected
        .filter(|selection| selection.edit().is_none())
        .map(|selection| selection.initial_line(&line.text));
    let editing = selected.and_then(Selection::edit).or(default.as_ref());
    let active = selected.is_some();
    let content = match editing {
        Some(editing) => {
            let edit = hooks.edit_line.clone();
            let edit_path = path.to_vec();
            let edit_line = line.clone();
            render::line_edit(
                tcx,
                cx.styles,
                &line,
                Some(editing),
                move |ctx, operation| edit(ctx, &edit_path, &edit_line, operation),
            )
        }
        None => render::line_edit(tcx, cx.styles, &line, None, |_, _| false),
    };

    if !writable {
        return content;
    }

    let path: SharedPath = Rc::from(path);
    let select_path = path.clone();
    let select_line = line.clone();
    let select = hooks.select.clone();
    let edit = hooks.edit_line.clone();
    let select: crate::navigate::Select<C> = Rc::new(move |ctx, direction| {
        select(ctx, select_path.to_vec());
        if direction == Some(crate::navigate::Direction::Left) {
            edit(ctx, &select_path, &select_line, &|edit| {
                edit.state.cursor_to_start();
                true
            });
        }
        true
    });
    let content = before(content, move |p, _| p.select_landmark(select));
    let presentation = cx.styles.line_presentation(&line);
    let scale = cx.styles.scale;
    let select = hooks.select.clone();
    let edit = hooks.edit_line.clone();
    before(content, move |p, placement| {
        hover_claim(p, placement, Hover::Value(path.clone()));
        let path = path.clone();
        let line = line.clone();
        let presentation = presentation.clone();
        let select = select.clone();
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            is_primary_contact(event)
                && !crate::modifiers::pick(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    if !active {
                        select(ctx, path.to_vec());
                    }
                    edit(ctx, &path, &line, &|edit| {
                        edit.state.pointer_down(
                            &presentation,
                            edit.fonts,
                            edit.layouts,
                            scale as f32,
                            LineEditPointerDown {
                                point: Point::new(
                                    event.state.position.x - placement.rect.x0,
                                    event.state.position.y - placement.rect.y0,
                                ),
                                shift: event.state.modifiers.shift(),
                                count: event.state.count.max(1),
                            },
                        );
                        true
                    }) || !active
                }
        });
    })
}

/// Apply this handler's conversion and report whether it opens an undo step.
pub(crate) fn commit(
    doc: &mut Rc<gid::Document>,
    libraries: &progred_libraries::Libraries,
    selection: &mut Selection,
    update: &progred_display::LineUpdate,
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
    libraries: &progred_libraries::Libraries,
    selection: &mut Selection,
    line: &progred_display::LineEdit,
    operation: impl FnOnce(&mut LineEditState) -> bool,
) -> (bool, bool) {
    let state = selection.edit_line_mut(&line.text);
    let before = state.text().to_owned();
    let handled = operation(state);
    let changed = before != state.text();
    let record = handled && changed && commit(doc, libraries, selection, &line.update);
    (handled, record)
}
