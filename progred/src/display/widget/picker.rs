//! A hole and its picker: a missing value, or a new field's label, that while
//! selected takes a query and lists offers for it on the floating card.
//! Projections draw one where they want a hole, with their own offers or
//! Raw's.

use crate::display::widget::offers::{HoleOffers, Offers};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::placed::{self, HoverPass, before, decorate, on_key};
use crate::projection::Cx;
use crate::render::text;
use crate::selection::{Selection, Stage};
use gid::Step;
use kurbo::Insets;
use measured::{Measured, min_width, pad};
use puri::Point;
use puri::edit::{LineEditDescription, LineEditPointerDown, LineEditPresentation, LineEditState};
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use puri_widgets::text_frame;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

/// What a hole lists for a query, and how it chooses.
#[derive(Clone)]
pub struct Picker {
    /// A missing value, or a new field's label.
    pub kind: CompletionKind,
    pub offers: HoleOffers,
    /// Typed into an empty query, each chooses its offer at once, as Raw's
    /// brackets make its constructors. Elsewhere a delimiter is text.
    pub keys: Vec<(&'static str, Completion)>,
}

impl Picker {
    pub fn new(kind: CompletionKind, offers: HoleOffers) -> Self {
        Self {
            kind,
            offers,
            keys: Vec::new(),
        }
    }

    pub fn with_keys(self, keys: Vec<(&'static str, Completion)>) -> Self {
        Self { keys, ..self }
    }
}

/// A hole where this projection is drawn, choosing from `picker`'s offers.
pub fn picker(picker: Picker) -> crate::view::Layout {
    crate::display::Layout::widget(Rc::new(move |context| {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Completion);
        hole(context.inputs, context.text, context.path, &picker)
    }))
}

/// The hole at `path`, as [`picker`] draws it.
pub(crate) fn hole(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    picker: &Picker,
) -> Measured<HoverPass<crate::Editor>> {
    match picker.kind {
        CompletionKind::Value => pending_view(cx, tcx, path.to_vec(), picker),
        CompletionKind::Field => cx
            .pending_edge_under(path)
            .map(|query| label_query(cx, tcx, path, query, picker))
            .unwrap_or_else(|| text(tcx, "…", &cx.styles.dim)),
    }
}

/// A missing value with ordinary selection and navigation behavior.
/// The active selection replaces its empty frame with a completion query.
fn pending_view(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Vec<Step>,
    picker: &Picker,
) -> Measured<HoverPass<crate::Editor>> {
    let writable = cx.edits.writable(&cx.sources, &path);
    let selected = cx.selection.filter(|current| {
        writable
            && current.path() == path.as_slice()
            && current.stage(&cx.sources) == Stage::Pending
    });
    let default = selected
        .filter(|current| current.edit().is_none())
        .map(Selection::initial_query);
    let engaged = selected.and_then(Selection::edit).or(default.as_ref());
    let content = placeholder(cx, tcx, &path, engaged, picker);
    // Selection draws the same outline as the inactive frame.
    pending_target(cx, path, content)
}

fn pending_target(
    cx: &Cx,
    path: Vec<Step>,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    let path: Rc<[Step]> = Rc::from(path);
    let scale = cx.styles.scale;
    let palette = cx.styles.palette;
    let selected = cx.selected(path.as_ref());
    let select = crate::projection::select_handler(path.clone(), cx);
    let stop = path.clone();
    let child = crate::display::widget::before_place(child, move |_, output| {
        output.navigation_stop(stop, selected);
    });
    let child = crate::display::widget::navigation::landmark(
        child,
        path.clone(),
        crate::display::widget::navigation::arrival(path.clone(), cx),
        cx.edits.clone(),
        None,
        selected,
    );
    before(child, move |p, placement| {
        let outline = text_frame::outline(scale, placement.rect);
        let highlight_path = path.clone();
        p.render(move |cv, hover| {
            if selected {
                crate::display::widget::style::primary_highlight(palette, scale, cv, outline);
            } else if matches!(
                hover.hovered.as_ref(),
                Some(Hovered::Tree(Hover::Value(hovered))) if hovered == &highlight_path
            ) {
                crate::display::widget::style::hover_highlight(palette, scale, cv, outline);
            }
        });
        p.claim(placement, Hovered::Tree(Hover::Value(path.clone())));
        let select = select.clone();
        p.activate(Hovered::Tree(Hover::Value(path.clone())), move |ctx| {
            select(&mut ctx.session())
        });
    })
}

fn placeholder(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    engaged: Option<&LineEditState>,
    picker: &Picker,
) -> Measured<HoverPass<crate::Editor>> {
    match engaged {
        Some(query) => query_content(cx, tcx, path, query, picker),
        None => crate::display::widget::empty(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus an ordinary floating
/// card. Serves both pending stages — a value and a new field's label.
fn query_content(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    query: &LineEditState,
    picker: &Picker,
) -> Measured<HoverPass<crate::Editor>> {
    // This is only reached for an active writable picker. Keep the boundary
    // total if a caller supplies an unmapped occurrence anyway.
    let Some(location) = cx.edits.location(path) else {
        return crate::display::widget::empty(tcx, cx.styles);
    };
    // The card and keyboard commit must answer from one list, read from
    // where the hole's location starts.
    let start = location.start;
    let value_at = move |path: &[Step]| cx.sources.read_from(start, path);
    let resolve = |cell| cx.sources.definition(cell);
    let request = CompletionRequest {
        raw: cx.raw,
        query: query.text(),
        kind: picker.kind,
        scope: CompletionScope::Everything,
        path: &location.path,
        value_at: &value_at,
        resolve: &resolve,
        cells: &|| cx.sources.cells().collect(),
        name: &|cell| cx.sources.name(cell),
        libraries: cx.completions,
    };
    let entries = (picker.offers)(&request);
    let presentation =
        LineEditPresentation::new(cx.styles.label.size, cx.styles.label.brush.clone());
    let content = crate::render::text_edit(
        LineEditDescription {
            state: query,
            focused: cx.focused,
            presentation: presentation.clone(),
            style: &cx.styles.edit,
            placeholder: None,
        },
        tcx,
        crate::editing::edit_query,
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
        p.occlude(placement);
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
    let scroll_root = root.clone();
    let scroll_path = path.to_vec();
    let card = crate::display::widget::completion::card(
        tcx,
        cx.styles,
        &entries,
        crate::display::widget::completion::State { choice, scroll },
        move |world: &crate::Editor| {
            world
                .model
                .selection
                .as_ref()
                .filter(|selection| {
                    selection.root() == &scroll_root
                        && selection.path() == scroll_path
                        && selection.stage(&world.sources()) != Stage::Edge
                })
                .map(Selection::completion_scroll)
        },
        move |world, view| crate::editing::completion_view(world, &root, view.scroll, view.choice),
    );
    let card = if !picker.keys.is_empty() && query.text().is_empty() && !query.is_composing() {
        let keys = picker.keys.clone();
        on_key(card, move |world, event| {
            if event.state.is_down()
                && !(event.modifiers.ctrl() || event.modifiers.meta())
                && let Key::Character(key) = &event.key
                && let Some((_, offer)) = keys.iter().find(|(typed, _)| *typed == key.as_str())
            {
                (offer.activate)(&mut world.session());
                true
            } else {
                false
            }
        })
    } else {
        card
    };
    // Tab commits what's been typed, as Enter would, then moves on to the
    // next empty slot, unless the commit opened one of its own, such as a new
    // call's first argument. With no slot left it stays on what it committed;
    // the next Tab lets go. An empty query leaves Tab to the editor.
    let typed = (!query.text().is_empty())
        .then(|| entries.get(choice).map(|entry| entry.activate.clone()))
        .flatten();
    let card = before(card, move |p, _| {
        p.handler()
            .on_key_with(move |world: &mut crate::Editor, event, input| {
                let Some(activate) = typed.as_ref().filter(|_| {
                    event.state.is_down()
                        && event.key == Key::Named(NamedKey::Tab)
                        && !(event.modifiers.ctrl()
                            || event.modifiers.meta()
                            || event.modifiers.alt())
                }) else {
                    return false;
                };
                activate(world);
                if world
                    .model
                    .selection
                    .as_ref()
                    .is_some_and(|selected| selected.stage(&world.sources()) == Stage::Edge)
                {
                    crate::navigate::advance(world, input.geometry(scale), event.modifiers.shift());
                }
                true
            });
    });
    placed::floating(trigger, card, move |placement, extent| {
        crate::display::widget::completion::placement(placement, extent, scale)
    })
}

/// The new-field label stage engaged, its query wearing the primary
/// ring explicitly: a pending edge has no path of its own for the
/// engine's landmark to mark, and the ring spans the QUERY frame alone,
/// the way a value pending's does. Clicks inside belong to the query's
/// own caret target; clicks beside fall through like any pending's.
fn label_query(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    query: &LineEditState,
    picker: &Picker,
) -> Measured<HoverPass<crate::Editor>> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, path, Some(query), picker);
    let palette = cx.styles.palette;
    let ringed = decorate(content, move |p, rect| {
        crate::display::widget::style::primary_highlight(
            palette,
            scale,
            p,
            text_frame::outline(scale, rect),
        );
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
}
