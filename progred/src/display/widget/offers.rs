use crate::display::{
    Completion, CompletionKind, CompletionProvider, CompletionRequest, CompletionText, Face, Host,
};
use crate::identity::name_or_short_id;
use gid::{CellId, Value};
use std::{ops::Range, rc::Rc};

pub struct Entry<C> {
    pub display: String,
    pub detail: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub face: Face,
    pub source: Option<CellId>,
    pub activate: Rc<dyn Fn(&mut C)>,
}

impl<C> Clone for Entry<C> {
    fn clone(&self) -> Self {
        Self {
            display: self.display.clone(),
            detail: self.detail.clone(),
            matches: self.matches.clone(),
            face: self.face,
            source: self.source,
            activate: self.activate.clone(),
        }
    }
}

/// Retained in the placed frame for attribution to the exact visible offers.
pub struct Offers<C> {
    pub entries: Rc<[Entry<C>]>,
}

/// A hole's list for a query, in the order shown, with the spans that matched.
pub type HoleOffers<W> = Rc<dyn Fn(&CompletionRequest<'_>) -> Vec<Entry<W>>>;

/// A projection's own vocabulary for its holes, and nothing else, narrowed to
/// the query and ranked by how well it matches.
pub fn ranked<W: Host>(provider: CompletionProvider) -> HoleOffers<W> {
    Rc::new(move |request| rank(provider(request).unwrap_or_default(), request))
}

/// `offers` that match the query, best first, named as the document names
/// them before matching. A new field's label is a cell the record doesn't
/// have yet.
pub(crate) fn rank<W: Host>(
    offers: Vec<Completion>,
    request: &CompletionRequest<'_>,
) -> Vec<Entry<W>> {
    crate::filter::rank_with_aliases(
        offers
            .into_iter()
            .filter(|offer| {
                fits(request, offer)
                    && (request.kind != CompletionKind::Field
                        || !offer
                            .preview
                            .as_ref()
                            .and_then(Value::as_cell)
                            .is_some_and(|cell| {
                                request
                                    .value()
                                    .and_then(Value::as_record)
                                    .is_some_and(|fields| fields.contains_key(&cell))
                            }))
            })
            .map(|offer| (text(request, &offer.display), offer))
            .collect(),
        |(display, _)| display,
        |(_, completion)| &completion.aliases,
        request.query,
    )
    .into_iter()
    .map(|ranked| {
        let (display, completion) = ranked.item;
        Entry {
            matches: ranked.matches,
            ..listed(request, display, completion)
        }
    })
    .collect()
}

/// Whether `offer` fits the hole: labels can only be cells, so an offer
/// advertising another value is not one.
fn fits(request: &CompletionRequest<'_>, offer: &Completion) -> bool {
    request.kind == CompletionKind::Value
        || offer
            .preview
            .as_ref()
            .is_none_or(|value| value.as_cell().is_some())
}

/// `offer` as listed, if it fits the hole.
pub(crate) fn fitting<W: Host>(
    request: &CompletionRequest<'_>,
    offer: Completion,
) -> Option<Entry<W>> {
    fits(request, &offer).then(|| entry(request, offer))
}

/// `offer` as listed, unmatched, choosing it through the session the
/// dispatching world lends.
pub(crate) fn entry<W: Host>(request: &CompletionRequest<'_>, offer: Completion) -> Entry<W> {
    listed(request, text(request, &offer.display), offer)
}

/// `offer` listed as `display`.
fn listed<W: Host>(
    request: &CompletionRequest<'_>,
    display: String,
    offer: Completion,
) -> Entry<W> {
    let activate = offer.activate;
    Entry {
        display,
        detail: offer.detail.as_ref().map(|detail| text(request, detail)),
        matches: Vec::new(),
        face: offer.face,
        source: offer.preview.as_ref().and_then(Value::as_cell),
        activate: Rc::new(move |world: &mut W| activate(&mut world.session())),
    }
}

fn text(request: &CompletionRequest<'_>, text: &CompletionText) -> String {
    match text {
        CompletionText::Literal(text) => text.clone(),
        CompletionText::Name(cell) => name_or_short_id((request.name)(*cell), *cell),
    }
}

#[cfg(test)]
mod tests;
