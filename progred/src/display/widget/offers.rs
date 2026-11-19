use crate::display::{
    Completion, CompletionKind, CompletionProvider, CompletionRequest, CompletionText, Face, Host,
    Offered,
};
use crate::identity::name_or_short_id;
use gid::{CellId, Value};
use std::{ops::Range, rc::Rc};

pub struct Entry<C> {
    /// Whether it makes something new, which reads `new` just before it,
    /// apart from what's matched.
    pub new: bool,
    pub display: String,
    /// Where in `display` its subscripts are.
    pub subscripts: Vec<Range<usize>>,
    /// What kind of thing it puts in: the existing cell's kind, as its
    /// definition says, or what something new is, unless it's listed as
    /// that already.
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub face: Face,
    pub source: Option<CellId>,
    pub activate: Rc<dyn Fn(&mut C)>,
}

impl<C> Clone for Entry<C> {
    fn clone(&self) -> Self {
        Self {
            new: self.new,
            display: self.display.clone(),
            subscripts: self.subscripts.clone(),
            kind: self.kind.clone(),
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
    /// The entry the keyboard has chosen.
    pub chosen: usize,
}

impl<C> Offers<C> {
    /// The cell the chosen entry refers to, which the frame marks as the
    /// hovered entry's is.
    pub fn chosen_source(&self) -> Option<CellId> {
        self.entries.get(self.chosen)?.source
    }
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
    // A new field's label can't be one the record already has.
    let taken = (request.kind == CompletionKind::Field)
        .then(|| request.value().and_then(Value::as_record))
        .flatten();
    crate::filter::rank_with_aliases(
        offers
            .into_iter()
            .filter(|offer| {
                fits(request, offer)
                    && !match &offer.offered {
                        Offered::Value(value) => value.as_cell(),
                        _ => None,
                    }
                    .is_some_and(|cell| taken.is_some_and(|fields| fields.get(&cell).is_some()))
            })
            .map(|offer| (text(request, &offer.display), offer))
            .collect(),
        |((display, _), _)| display,
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

/// Whether `offer` fits the hole: labels can only be cells, an existing
/// one or a new one, so an offer of another value, or of something made
/// around a cell, is not one.
fn fits(request: &CompletionRequest<'_>, offer: &Completion) -> bool {
    request.kind == CompletionKind::Value
        || match &offer.offered {
            Offered::Value(value) => value.as_cell().is_some(),
            Offered::Use(_) => false,
            Offered::New(_) => true,
        }
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

/// `offer` listed as `display`, subscripted where its name is.
fn listed<W: Host>(
    request: &CompletionRequest<'_>,
    (display, subscripts): (String, Vec<Range<usize>>),
    offer: Completion,
) -> Entry<W> {
    let activate = offer.activate;
    let source = offer.offered.existing();
    let kind = match &offer.offered {
        Offered::New(what) => what
            .as_ref()
            .map(|what| text(request, what).0)
            .filter(|what| *what != display),
        _ => source
            .and_then(|cell| (request.resolve)(cell))
            .and_then(|definition| ::grap::isa::of(definition.value.as_record()?))
            .map(|kind| text(request, &CompletionText::Name(kind)).0),
    };
    Entry {
        new: matches!(offer.offered, Offered::New(_)),
        display,
        subscripts,
        kind,
        detail: offer.detail.as_ref().map(|detail| text(request, detail).0),
        matches: Vec::new(),
        face: offer.face,
        source,
        activate: Rc::new(move |world: &mut W| activate(&mut world.session())),
    }
}

/// `text` spelled, and where in that its subscripts are.
fn text(request: &CompletionRequest<'_>, text: &CompletionText) -> (String, Vec<Range<usize>>) {
    match text {
        CompletionText::Literal(text) => (text.clone(), Vec::new()),
        CompletionText::Named(name) => (name.spelling().into_owned(), name.subscripts()),
        CompletionText::Name(cell) => match (request.name)(*cell) {
            Some(name) => (name.spelling().into_owned(), name.subscripts()),
            None => (name_or_short_id(None::<&str>, *cell), Vec::new()),
        },
    }
}

#[cfg(test)]
#[path = "tests/offers.rs"]
mod tests;
