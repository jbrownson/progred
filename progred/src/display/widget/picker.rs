//! A hole and its picker: a missing value, or a new field's label, that while
//! selected takes a query and lists offers for it on the floating card.
//! Projections draw one where they want a hole, with their own offers or
//! Raw's.

use crate::display::host::Layout;
use crate::display::widget::HoverPass;
use crate::display::widget::offers::{Entry, HoleOffers, Offers};
use crate::display::widget::placed::{before, decorate, on_key, text, text_edit};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};
use crate::display::{Host, Toolkit};
use crate::hover::Hover;
use crate::location::Location;
use crate::selection::{Selection, Stage};
use kurbo::{Insets, Point};
use measured::{Measured, min_width, pad};
use puri::edit::{EditOperation, LineEditDescription, LineEditPresentation, LineEditState};
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use puri_widgets::text_frame;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

/// What a hole lists for a query, and how it chooses.
pub struct Picker<W> {
    /// A missing value, or a new field's label.
    pub kind: CompletionKind,
    pub offers: HoleOffers<W>,
    /// Typed into an empty query, each chooses its offer at once, as Raw's
    /// brackets make its constructors. Elsewhere a delimiter is text.
    pub keys: Vec<(&'static str, Completion)>,
}

impl<W> Clone for Picker<W> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            offers: self.offers.clone(),
            keys: self.keys.clone(),
        }
    }
}

impl<W> Picker<W> {
    pub fn new(kind: CompletionKind, offers: HoleOffers<W>) -> Self {
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
pub fn picker<W: Host>(picker: Picker<W>) -> Layout<W> {
    crate::display::Layout::widget(Rc::new(move |context| {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Completion);
        hole(context.toolkit, context.text, &picker)
    }))
}

/// The hole where `toolkit` is drawn, as [`picker`] draws it.
pub(crate) fn hole<W: Host>(
    toolkit: &dyn Toolkit,
    tcx: &mut TextCtx,
    picker: &Picker<W>,
) -> Measured<HoverPass<W, W::Hover>> {
    match picker.kind {
        CompletionKind::Value => pending_view(toolkit, tcx, picker),
        CompletionKind::Field => toolkit
            .label_query()
            .map(|query| label_query(toolkit, tcx, query, picker))
            .unwrap_or_else(|| text(tcx, "…", &toolkit.styles().dim)),
    }
}

/// A missing value with ordinary selection and navigation behavior.
/// The active selection replaces its empty frame with a completion query.
fn pending_view<W: Host>(
    toolkit: &dyn Toolkit,
    tcx: &mut TextCtx,
    picker: &Picker<W>,
) -> Measured<HoverPass<W, W::Hover>> {
    let writable = toolkit.writable();
    let selected = toolkit.selection().filter(|current| {
        writable
            && *toolkit.path() == current.path()
            && current.stage(&toolkit.sources()) == Stage::Pending
    });
    let default = selected
        .filter(|current| current.edit().is_none())
        .map(Selection::initial_query);
    let engaged = selected.and_then(Selection::edit).or(default.as_ref());
    let content = match engaged {
        Some(query) => query_content(toolkit, tcx, query, picker),
        None => crate::display::widget::empty(tcx, toolkit.styles()),
    };
    // Selection draws the same outline as the inactive frame.
    pending_target(toolkit, content)
}

fn pending_target<W: Host>(
    toolkit: &dyn Toolkit,
    child: Measured<HoverPass<W, W::Hover>>,
) -> Measured<HoverPass<W, W::Hover>> {
    let path = toolkit.path().clone();
    let scale = toolkit.styles().scale;
    let palette = toolkit.styles().palette;
    let selected = toolkit.selected();
    let writable = toolkit.writable();
    let select = crate::display::widget::navigation::selecting(toolkit.site());
    let stop = path.clone();
    let child = crate::display::widget::before_place(child, move |_, output| {
        if writable {
            output.tab_stop(stop.clone());
        }
        output.navigation_stop(stop, selected);
    });
    let child = crate::display::widget::navigation::landmark(
        child,
        path.clone(),
        crate::display::widget::navigation::arrival(toolkit.site()),
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
                hover.hovered.as_ref().and_then(W::tree),
                Some(Hover::Value(hovered)) if hovered == &highlight_path
            ) {
                crate::display::widget::style::hover_highlight(palette, scale, cv, outline);
            }
        });
        p.claim(placement, W::Hover::from(Hover::Value(path.clone())));
        let select = select.clone();
        p.activate(W::Hover::from(Hover::Value(path.clone())), move |world| {
            select(&mut world.session())
        });
    })
}

/// A focused completion query: the editor plus an ordinary floating
/// card. Serves both pending stages — a value and a new field's label.
fn query_content<W: Host>(
    toolkit: &dyn Toolkit,
    tcx: &mut TextCtx,
    query: &LineEditState,
    picker: &Picker<W>,
) -> Measured<HoverPass<W, W::Hover>> {
    // This is only reached for an active writable picker. Keep the boundary
    // total if a caller supplies an unmapped occurrence anyway.
    match toolkit.location() {
        Some(location) => located_query(toolkit, tcx, &location, query, picker),
        None => crate::display::widget::empty(tcx, toolkit.styles()),
    }
}

/// [`query_content`] for a hole stored at `location`.
fn located_query<W: Host>(
    toolkit: &dyn Toolkit,
    tcx: &mut TextCtx,
    location: &Location<'_>,
    query: &LineEditState,
    picker: &Picker<W>,
) -> Measured<HoverPass<W, W::Hover>> {
    let styles = toolkit.styles();
    // The card and keyboard commit must answer from one list, read from
    // where the hole's location starts.
    let start = location.start;
    let sources = toolkit.sources();
    let value_at = move |path: &[gid::Step]| sources.read_from(start, path);
    let resolve = |cell| sources.resolve(cell);
    let request = CompletionRequest {
        query: query.text(),
        kind: picker.kind,
        scope: CompletionScope::Everything,
        path: &location.path,
        value_at: &value_at,
        resolve: &resolve,
        cells: &|| sources.cells(),
        name: &|cell| toolkit.name(cell),
        libraries: toolkit.completions(),
    };
    let entries: Rc<[Entry<W>]> = (picker.offers)(&request).into();
    let presentation = LineEditPresentation::new(styles.label.size, styles.label.brush.clone());
    let content = text_edit(
        LineEditDescription {
            state: query,
            focused: toolkit.focused(),
            presentation: presentation.clone(),
            style: &styles.edit,
        },
        tcx,
        edit_query::<W>,
    );
    // Preserve the empty frame's width while the query is short.
    let content = min_width(
        text_frame::empty_width(styles.label.size, styles.scale),
        content,
    );
    let offers = Offers {
        entries: entries.clone(),
    };
    let scale = styles.scale;
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
                && edit_query(ctx, &|edit| {
                    edit.state.pointer_down(
                        &presentation,
                        edit.fonts,
                        edit.layouts,
                        scale as f32,
                        event,
                        rect.origin(),
                    );
                    true
                })
        });
    });
    let choice = toolkit.selection().map(Selection::choice).unwrap_or(0);
    let scroll = toolkit
        .selection()
        .map(Selection::completion_scroll)
        .unwrap_or(0.0);
    let site = toolkit.site();
    let scroll_site = site.clone();
    let card = crate::display::widget::completion::card(
        tcx,
        styles,
        &entries,
        crate::display::widget::completion::State { choice, scroll },
        move |world: &mut W| {
            world
                .session()
                .pending()
                .filter(|picking| picking.is_at(&scroll_site))
                .map(|picking| picking.scroll())
        },
        move |world: &mut W, view| {
            if let Some(mut picking) = world
                .session()
                .pending()
                .filter(|picking| picking.is_at(&site))
            {
                picking.set_view(view.scroll, view.choice);
            }
        },
    );
    let card =
        if !picker.keys.is_empty() && query.text().is_empty() && query.preedit_parts().is_none() {
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
        p.handler().on_key_with(move |world: &mut W, event, input| {
            if event.state.is_down()
                && event.key == Key::Named(NamedKey::Tab)
                && crate::modifiers::at_most_shift(&event.modifiers)
                && let Some(activate) = typed.as_ref()
            {
                activate(world);
                let committed =
                    {
                        let session = world.session();
                        session.model.selection.as_ref().is_some_and(|selected| {
                            selected.stage(&session.sources()) == Stage::Edge
                        })
                    };
                if committed {
                    crate::display::widget::navigation::advance(
                        world,
                        input.geometry(scale),
                        event.modifiers.shift(),
                    );
                }
                true
            } else {
                false
            }
        });
    });
    super::container::floating(trigger, card, move |placement, extent| {
        crate::display::widget::completion::placement(placement, extent, scale)
    })
}

/// Edits the open picker's query. A retained handler can outlive it, and
/// then declines.
fn edit_query<W: Host>(world: &mut W, operation: &EditOperation<'_>) -> bool {
    world
        .session()
        .pending()
        .is_some_and(|mut picking| picking.edit_query(operation))
}

/// The new-field label stage engaged, its query wearing the primary
/// ring explicitly: a pending edge has no path of its own for the
/// engine's landmark to mark, and the ring spans the QUERY frame alone,
/// the way a value pending's does. Clicks inside belong to the query's
/// own caret target; clicks beside fall through like any pending's.
fn label_query<W: Host>(
    toolkit: &dyn Toolkit,
    tcx: &mut TextCtx,
    query: &LineEditState,
    picker: &Picker<W>,
) -> Measured<HoverPass<W, W::Hover>> {
    let scale = toolkit.styles().scale;
    let content = query_content(toolkit, tcx, query, picker);
    let palette = toolkit.styles().palette;
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
