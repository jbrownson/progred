//! Adapt completion offers and pending queries to the editor’s floating card.

use super::{
    Cx, Hooks, atom_content, edit_presentation, face_style, hover_block, hover_claim,
    placeholder_box, primary_highlight, source_target, tree_hovered,
};
use crate::completion::{Commit, Entry, Offers, completion_entries_with, constructor_entries};
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::placed::{self, Placed, before, decorate, leaf, on_key};
use crate::render::text;
use crate::selection::{Selection, Stage};
use crate::styles::Styles;
use gid::Path;
use kurbo::{Insets, Size};
use measured::{Extent, Measured, col, min_width, pad};
use puri::edit::{LineEditPointerDown, LineEditState};
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use puri::{Canvas, Color, Point, Stroke, Vec2};
use puri_widgets::panel::Panel;
use puri_widgets::text_frame;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

/// A missing value with ordinary selection and navigation behavior.
/// The active selection replaces its empty frame with a completion query.
pub(super) fn pending_view<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let engaged = cx
        .selection
        .filter(|current| current.stage() == Stage::Pending && current.path() == path.as_slice())
        .and_then(Selection::edit);
    let content = placeholder(cx, tcx, &path, engaged, false, completions, hooks);
    // Selection draws the same outline as the inactive frame.
    source_target(cx, path, None, hooks, content)
}

fn placeholder<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    engaged: Option<&LineEditState>,
    labels: bool,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    match engaged {
        Some(query) => query_content(cx, tcx, path, query, labels, completions, hooks),
        None => placeholder_box(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus an ordinary floating
/// card. Serves both pending stages — a value and a new field's label
/// (`labels` narrows the offers there).
fn query_content<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    query: &LineEditState,
    labels: bool,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    // The card and keyboard commit must answer from one list.
    let everything = cx.selection.is_some_and(Selection::completion_everything);
    let commit = if labels {
        Commit::Label(hooks.commit_label.clone())
    } else {
        Commit::Value(hooks.commit_value.clone())
    };
    let value_at = |path: &[gid::Step]| cx.sources.resolve_path(path);
    let request = progred_display::CompletionRequest {
        query: query.text(),
        kind: if labels {
            progred_display::CompletionKind::Field
        } else {
            progred_display::CompletionKind::Value
        },
        scope: if everything {
            progred_display::CompletionScope::Everything
        } else {
            progred_display::CompletionScope::Suggested
        },
        path,
        value_at: &value_at,
    };
    let (entries, everything) = completion_entries_with(
        &cx.sources,
        cx.raw,
        &commit,
        &request,
        hooks.completions.as_ref(),
        completions,
    );
    let fallback = text(tcx, "…", &cx.styles.dim);
    let presentation = edit_presentation(&cx.styles.label);
    let content = atom_content(
        Some(query),
        fallback,
        presentation.clone(),
        None,
        tcx,
        cx.styles,
        hooks,
    );
    // Preserve the empty frame's width while the query is short.
    let content = min_width(
        text_frame::empty_width(cx.styles.label.size, cx.styles.scale),
        content,
    );
    let edit = hooks.edit.clone();
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
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            is_primary_contact(event)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && edit(ctx).is_some_and(|edit| {
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
    let set_completion_view = hooks.set_completion_view.clone();
    let card = completion_card(
        tcx,
        cx.styles,
        &entries,
        choice,
        scroll,
        everything,
        move |world, scroll, choice, everything| {
            set_completion_view(world, scroll, choice, everything)
        },
    );
    let card = if query.text().is_empty() && !query.is_composing() {
        let constructors = constructor_entries(&commit);
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
    placed::popover(trigger, card, 4.0 * scale)
}

/// Completion offers and expansion share row navigation. Each row's
/// callback handles both clicks and Enter. The raised card swallows
/// other clicks so nothing lands on content underneath.
pub(super) fn completion_card<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry<C>],
    choice: usize,
    scroll: f64,
    everything: bool,
    set_view: impl Fn(&mut C, f64, usize, bool) + 'static,
) -> Measured<Placed<C, Cv>> {
    let scale = styles.scale;
    let widget_entries = entries
        .iter()
        .map(|entry| puri_widgets::completion::Entry {
            display: &entry.display,
            detail: entry.detail.as_deref(),
            matches: &entry.matches,
            style: face_style(styles, entry.face),
        })
        .collect::<Vec<_>>();
    let widget = puri_widgets::completion::Completion::new(
        tcx,
        &widget_entries,
        !everything,
        puri_widgets::completion::Style {
            detail: &styles.id,
            more: &styles.dim,
            scale,
            chosen: Color::new([0.0, 0.48, 1.0, 0.14]),
            hovered: Color::new([0.0, 0.48, 1.0, 0.08]),
        },
    );
    let set_view = Rc::new(set_view);
    let expand: Rc<dyn Fn(&mut C)> = {
        let set_view = set_view.clone();
        Rc::new(move |world| {
            set_view(world, scroll, choice, true);
        })
    };
    let items = widget
        .rows
        .into_iter()
        .zip(entries)
        .enumerate()
        .map(|(index, (row, entry))| (row, Hover::Entry(index), entry.activate.clone()))
        .chain(
            widget
                .more
                .map(|row| (row, Hover::MoreCompletions, expand.clone())),
        )
        .collect::<Vec<_>>();
    let count = items.len();
    let choice = choice.min(count.saturating_sub(1));
    let activate = items.get(choice).map(|(_, _, activate)| activate.clone());
    let rows = items
        .into_iter()
        .enumerate()
        .map(|(index, (row, hover, activate))| {
            completion_row(row, hover, index == choice, move |world| activate(world))
        })
        .collect::<Vec<_>>();
    let gap = 2.0 * scale;
    let row_spans = completion_row_spans(&rows, gap, scale);
    let content = col(0, gap, rows);
    let viewport_height = completion_viewport_height(&row_spans);
    let maximum = (content.extent.height() / scale - viewport_height).max(0.0);
    let scroll = scroll.clamp(0.0, maximum);
    let viewport_extent = Extent {
        width: content.extent.width,
        ascent: content.extent.ascent.min(viewport_height * scale),
        descent: (viewport_height * scale - content.extent.ascent).max(0.0),
    };
    let scroll_view = set_view.clone();
    let scrolled = placed::scrolled_at(
        content,
        Vec2::new(0.0, scroll * scale),
        None,
        move |world, event| {
            let (next, outcome) = crate::frame::scroll_offset(
                Vec2::new(0.0, scroll),
                event,
                scale,
                Size::new(viewport_extent.width, viewport_extent.height()),
                Vec2::new(0.0, maximum),
            );
            if next.y != scroll {
                scroll_view(world, next.y, choice, everything);
            }
            outcome
        },
    );
    let viewport = measured::overlay(
        leaf(viewport_extent, |_, _| {}),
        scrolled,
        move |placement, _, _| Some(placement),
    );
    let card = pad(Insets::uniform(4.0 * scale), viewport);
    let card = on_key(card, move |world, event| {
        if event.state.is_down() {
            match event.key {
                Key::Named(NamedKey::Enter) => activate.as_ref().is_some_and(|activate| {
                    activate(world);
                    true
                }),
                Key::Named(key @ (NamedKey::Tab | NamedKey::ArrowDown))
                    if !everything
                        && !crate::modifiers::command(&event.modifiers)
                        && (key == NamedKey::Tab || choice == count.saturating_sub(1)) =>
                {
                    expand(world);
                    true
                }
                Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown))
                    if !crate::modifiers::command(&event.modifiers) =>
                {
                    let next = match direction {
                        NamedKey::ArrowUp => choice.saturating_sub(1),
                        _ => choice.saturating_add(1).min(count.saturating_sub(1)),
                    };
                    set_view(
                        world,
                        reveal_completion(scroll, next, &row_spans, viewport_height)
                            .clamp(0.0, maximum),
                        next,
                        everything,
                    );
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    });
    let panel = Panel {
        fill: Some(Color::WHITE.into()),
        border: Some((
            Stroke::new(scale),
            Color::new([0.75, 0.77, 0.81, 1.0]).into(),
        )),
        radius: 6.0 * scale,
    };
    before(card, move |p, placement| {
        panel.place(p, placement);
        hover_block(p, placement);
    })
}

fn completion_row_spans<C, Cv>(
    rows: &[Measured<Placed<C, Cv>>],
    gap: f64,
    scale: f64,
) -> Vec<(f64, f64)> {
    rows.iter()
        .scan(0.0, |top, row| {
            let span = (*top, *top + row.extent.height() / scale);
            *top = span.1 + gap / scale;
            Some(span)
        })
        .collect()
}

fn completion_viewport_height(spans: &[(f64, f64)]) -> f64 {
    const VISIBLE_ROWS: usize = 8;
    spans
        .get(
            VISIBLE_ROWS
                .saturating_sub(1)
                .min(spans.len().saturating_sub(1)),
        )
        .map_or(0.0, |(_, bottom)| *bottom)
}

fn reveal_completion(
    scroll: f64,
    choice: usize,
    spans: &[(f64, f64)],
    viewport_height: f64,
) -> f64 {
    match spans.get(choice) {
        Some((top, _)) if *top < scroll => *top,
        Some((_, bottom)) if *bottom > scroll + viewport_height => *bottom - viewport_height,
        _ => scroll,
    }
}

fn completion_row<C: 'static, Cv: Canvas + 'static>(
    row: puri_widgets::completion::Row,
    hover: Hover,
    chosen: bool,
    activate: impl Fn(&mut C) + Clone + 'static,
) -> Measured<Placed<C, Cv>> {
    leaf(
        placed::metrics_extent(row.metrics()),
        move |p, placement| {
            hover_claim(p, placement, hover.clone());
            let target = Hovered::Tree(hover.clone());
            let accept = move |world: &mut C| {
                activate(world);
                true
            };
            p.activate(target.clone(), accept.clone());
            p.pick(target, accept);
            p.ink(move |canvas, ink| {
                row.draw(canvas, placement, chosen, tree_hovered(ink) == Some(&hover));
            });
        },
    )
}

/// The new-field label stage engaged, its query wearing the primary
/// ring explicitly: a pending edge has no path of its own for
/// [`descend`] to mark, and the ring spans the QUERY frame alone, the
/// way a value pending's does. Clicks inside belong to the query's
/// own caret target; clicks beside fall through like any pending's.
pub(super) fn label_query<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[gid::Step],
    query: &LineEditState,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, path, Some(query), true, completions, hooks);
    let ringed = decorate(content, move |p, rect| {
        primary_highlight(scale, p, rect);
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
}
