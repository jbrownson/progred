//! Adapt completion offers and pending queries to the editor’s floating card.

use super::{
    Cx, SharedPath, Source, atom_content, edit_presentation, face_style, hover_block, hover_claim,
    hover_highlight, placeholder_box, primary_highlight, primary_highlight_stroke, tree_hovered,
};
use crate::completion::{Entry, Offers, completion_entries_with, constructor_entries};
use crate::display::widget::completion::border as completion_border;
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::placed::{self, HoverPass, before, decorate, on_key};
use crate::render::text;
use crate::selection::{Selection, Stage};
use crate::styles::Styles;
use gid::Path;
use kurbo::{Insets, Size};
use measured::{Extent, Measured, min_width, pad};
use puri::edit::{LineEditPointerDown, LineEditState};
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use puri::{Placement, Point};
use puri_widgets::text_frame;
use std::rc::Rc;
use ui_events::keyboard::Key;

pub(crate) fn control(
    context: &mut crate::display::widget::Context<'_, '_, crate::Editor, Hovered>,
    kind: crate::display::CompletionKind,
    provider: Option<&crate::display::CompletionProvider>,
) -> Measured<HoverPass<crate::Editor>> {
    match kind {
        crate::display::CompletionKind::Value => pending_view(
            context.inputs,
            context.text,
            context.path.to_vec(),
            provider,
        ),
        crate::display::CompletionKind::Field => context
            .inputs
            .pending_edge_under(context.path)
            .map(|(query, _)| {
                label_query(context.inputs, context.text, context.path, query, provider)
            })
            .unwrap_or_else(|| text(context.text, "…", &context.inputs.styles.dim)),
    }
}

/// A missing value with ordinary selection and navigation behavior.
/// The active selection replaces its empty frame with a completion query.
pub(super) fn pending_view(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    completions: Option<&crate::display::CompletionProvider>,
) -> Measured<HoverPass<crate::Editor>> {
    let writable = !cx.source.transient() && crate::selection::writable_at(&cx.sources, &path);
    let selected = cx.selection.filter(|current| {
        writable
            && current.path() == path.as_slice()
            && current.stage(&cx.sources) == Stage::Pending
    });
    let default = selected
        .filter(|current| current.edit().is_none())
        .map(Selection::initial_query);
    let engaged = selected.and_then(Selection::edit).or(default.as_ref());
    let content = placeholder(cx, tcx, &path, engaged, false, completions);
    // Selection draws the same outline as the inactive frame.
    pending_target(cx, path, content)
}

fn pending_target(
    cx: &Cx,
    path: Path,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    let (path, transient): (SharedPath, bool) = match cx.source {
        Source::Transient { owner } if owner != path.as_slice() => return child,
        Source::Transient { owner } => (Rc::from(owner), true),
        Source::Stored => (Rc::from(path), false),
    };
    let scale = cx.styles.scale;
    let selected = cx.selected(path.as_ref());
    let root = cx.view.clone();
    let child = if transient {
        child
    } else {
        let target = path.clone();
        let root = root.clone();
        crate::display::widget::navigation::landmark(
            child,
            path.clone(),
            Rc::new(move |ctx, _| {
                crate::editing::select(ctx, &root, &target);
                true
            }),
        )
    };
    before(child, move |p, placement| {
        let outline = text_frame::outline(scale, placement.rect);
        let highlight_path = path.clone();
        p.render(move |cv, hover| {
            if selected {
                primary_highlight(scale, cv, outline);
            } else if matches!(
                tree_hovered(hover),
                Some(Hover::Value(hovered)) if hovered == &highlight_path
            ) {
                hover_highlight(cv, outline);
            }
        });
        if !transient {
            hover_claim(p, placement, Hover::Value(path.clone()));
        }
        let target = path.clone();
        let root = root.clone();
        p.activate(Hovered::Tree(Hover::Value(target.clone())), move |ctx| {
            crate::editing::select(ctx, &root, &target);
            true
        });
    })
}

fn placeholder(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    engaged: Option<&LineEditState>,
    labels: bool,
    completions: Option<&crate::display::CompletionProvider>,
) -> Measured<HoverPass<crate::Editor>> {
    match engaged {
        Some(query) => query_content(cx, tcx, path, query, labels, completions),
        None => placeholder_box(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus an ordinary floating
/// card. Serves both pending stages — a value and a new field's label
/// (`labels` narrows the offers there).
fn query_content(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    query: &LineEditState,
    labels: bool,
    completions: Option<&crate::display::CompletionProvider>,
) -> Measured<HoverPass<crate::Editor>> {
    // The card and keyboard commit must answer from one list.
    let everything = cx.selection.is_some_and(Selection::completion_everything);
    let value_at = |path: &[gid::Step]| cx.sources.resolve_path(path);
    let resolve = |cell| cx.sources.definition(cell);
    let request = crate::display::CompletionRequest {
        query: query.text(),
        kind: if labels {
            crate::display::CompletionKind::Field
        } else {
            crate::display::CompletionKind::Value
        },
        scope: if everything {
            crate::display::CompletionScope::Everything
        } else {
            crate::display::CompletionScope::Suggested
        },
        path,
        value_at: &value_at,
        resolve: &resolve,
    };
    let (entries, everything) =
        completion_entries_with(&cx.sources, cx.raw, &request, cx.completions, completions);
    let fallback = text(tcx, "…", &cx.styles.dim);
    let presentation = edit_presentation(&cx.styles.label);
    let content = atom_content(
        Some(query),
        fallback,
        presentation.clone(),
        None,
        tcx,
        cx.styles,
    );
    // Preserve the empty frame's width while the query is short.
    let content = min_width(
        text_frame::empty_width(cx.styles.label.size, cx.styles.scale),
        content,
    );
    let offers = Offers {
        entries: entries.clone(),
    };
    let scale = cx.styles.scale;
    let trigger = before(content, move |p, placement| {
        let rect = placement.rect;
        *p.completion() = Some(offers);
        // Clicks in the query place the caret, straight through the
        // edit hook — the selection transition is never involved, so
        // clicking what you are typing can't discard it.
        hover_block(p, placement);
        p.handler().on_pointer_down(move |ctx, event| {
            is_primary_contact(event)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && crate::editing::edit_query(ctx, &|edit| {
                    edit.state.pointer_down(
                        &presentation,
                        edit.fonts,
                        edit.layouts,
                        scale as f32,
                        LineEditPointerDown {
                            point: Point::new(
                                event.state.position.x - rect.x0,
                                event.state.position.y - rect.y0,
                            ),
                            shift: event.state.modifiers.shift(),
                            count: event.state.count.max(1),
                        },
                    );
                    true
                })
        });
    });
    let choice = cx.selection.map(Selection::choice).unwrap_or(0);
    let scroll = cx
        .selection
        .map(Selection::completion_scroll)
        .unwrap_or(0.0);
    let root = cx.view.clone();
    let card = completion_card(
        tcx,
        cx.styles,
        &entries,
        choice,
        scroll,
        everything,
        move |world, scroll, choice, everything| {
            crate::editing::completion_view(world, &root, scroll, choice, everything)
        },
    );
    let card = if query.text().is_empty() && !query.is_composing() {
        let constructors = constructor_entries(request.kind);
        on_key(card, move |world, event| {
            if event.state.is_down()
                && !(event.modifiers.ctrl() || event.modifiers.meta())
                && let Key::Character(key) = &event.key
                && let Some((_, entry)) = constructors.iter().find(|(shortcut, _)| *shortcut == key)
            {
                (entry.activate)(world);
                true
            } else {
                false
            }
        })
    } else {
        card
    };
    placed::floating(trigger, card, move |placement, extent| {
        completion_placement(placement, extent, scale)
    })
}

pub(super) fn completion_placement(
    placement: Placement,
    extent: Extent,
    scale: f64,
) -> Option<Placement> {
    (!placement.clipped_out()).then(|| {
        let ring_outset = primary_highlight_stroke(scale).width / 2.0;
        let anchor = text_frame::outline(scale, placement.rect)
            .rect()
            .inflate(ring_outset, ring_outset);
        let border_outset = completion_border(scale).width / 2.0;
        let outer = crate::display::widget::popover::rect(
            anchor,
            Size::new(
                extent.width + 2.0 * border_outset,
                extent.height() + 2.0 * border_outset,
            ),
            placement.clip_rect,
            0.0,
        );
        Placement::new(
            outer.inflate(-border_outset, -border_outset),
            placement.clip_rect,
        )
    })
}

pub(super) fn completion_card<C: 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry<C>],
    choice: usize,
    scroll: f64,
    everything: bool,
    set_view: impl Fn(&mut C, f64, usize, bool) + 'static,
) -> Measured<HoverPass<C>> {
    let entries = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| crate::display::widget::completion::Entry {
            display: &entry.display,
            detail: entry.detail.as_deref(),
            matches: &entry.matches,
            style: face_style(styles, entry.face),
            target: Hovered::Tree(Hover::Entry(index)),
            activate: entry.activate.clone(),
        })
        .collect::<Vec<_>>();
    crate::display::widget::completion::card(
        tcx,
        styles,
        &entries,
        Hovered::Tree(Hover::MoreCompletions),
        crate::display::widget::completion::State {
            choice,
            scroll,
            everything,
        },
        move |world, state| set_view(world, state.scroll, state.choice, state.everything),
        crate::modifiers::command,
    )
}

/// The new-field label stage engaged, its query wearing the primary
/// ring explicitly: a pending edge has no path of its own for
/// [`descend`] to mark, and the ring spans the QUERY frame alone, the
/// way a value pending's does. Clicks inside belong to the query's
/// own caret target; clicks beside fall through like any pending's.
pub(super) fn label_query(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    query: &LineEditState,
    completions: Option<&crate::display::CompletionProvider>,
) -> Measured<HoverPass<crate::Editor>> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, path, Some(query), true, completions);
    let ringed = decorate(content, move |p, rect| {
        primary_highlight(scale, p, text_frame::outline(scale, rect));
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
}
