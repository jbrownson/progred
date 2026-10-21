//! Structural projection combinators. Child overrides apply at each
//! immediate child; recursion is a choice made by that projection.

use super::host::{Layout, Partial, ProjectionInput};
use super::*;
use puri::handler::HasHandler;
use ui_events::keyboard::{Key, NamedKey};

pub fn list<W: Host>(child: Option<Partial<W>>) -> Partial<W> {
    runtime_partial(move |input| list_layout(input, child.clone()))
}

/// An editable list without brackets or commas; items retain the standard
/// occurrence paths, child projections, and pending insertion behavior.
pub fn list_column<W: Host>(gap: f64, child: Option<Partial<W>>) -> Partial<W> {
    runtime_partial(move |input| list_column_with(input, gap, child.clone(), |_, entry| entry))
}

/// Compose content beside each projected item, outside that item's value
/// boundary. The list still owns element paths and pending insertion slots.
pub(crate) fn list_column_with<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    gap: f64,
    child: Option<Partial<W>>,
    item: impl Fn(gid::Position, Layout<W>) -> Layout<W>,
) -> Option<Layout<W>> {
    let items = list_items(input, child)?;
    Some(authoring(if items.is_empty() {
        selectable_bracket(Delim::Bracket, row(0.0, []))
    } else {
        col(
            1,
            0.0,
            list_with(input, &items, item, |before, after| {
                list_gap(gap, before, after)
            }),
        )
    }))
}

/// Prepare element paths, including a pending insertion, and share their
/// projection and measurement across any presentations the caller builds.
pub fn list_items<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    child: Option<Partial<W>>,
) -> Option<Vec<(gid::Position, Layout<W>)>> {
    Some(list_items_at(input, input.value?.list_positions()?, child))
}

fn list_items_at<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    mut positions: Vec<gid::Position>,
    child: Option<Partial<W>>,
) -> Vec<(gid::Position, Layout<W>)> {
    if let Some(Pending::Child(Step::Element(position))) = &input.pending
        && !positions.contains(position)
    {
        positions.push(position.clone());
        positions.sort();
    }
    let child = child.map(|p| compose_partials([p, input.default_projection.clone()]));
    positions
        .into_iter()
        .map(|position| {
            let layout = shared(descend(
                [Step::Element(position.clone())],
                Projected::by(child.clone(), None),
            ));
            (position, layout)
        })
        .collect()
}

/// One interleaved presentation of `list_items` for this input. The caller
/// supplies item decoration and separator paint; this function owns insertion
/// eligibility and interaction. Arrange the returned sequence with row/col/etc.
pub fn list_with<W: Host, S>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    items: &[(gid::Position, Layout<W>)],
    item: impl Fn(gid::Position, Layout<W>) -> Layout<W>,
    separator: impl Fn(Option<&gid::Position>, Option<&gid::Position>) -> Option<S>,
) -> Vec<Layout<W>>
where
    S: FnOnce(Option<W::Hover>) -> Layout<W>,
{
    let gap = |before: Option<&gid::Position>, after: Option<&gid::Position>| {
        separator(before, after).map(|render| {
            let beside_pending = matches!(
                &input.pending,
                Some(Pending::Child(Step::Element(position)))
                    if before == Some(position) || after == Some(position)
            );
            let site = (input.writable && !beside_pending)
                .then(|| gid::position::between(before, after))
                .flatten()
                .map(|position| input.site().below(&[Step::Element(position)]));
            let content = render(site.as_ref().map(|site| W::Hover::from(site.hover())));
            widget::fill_width(match site {
                Some(site) => activatable(content, &site),
                None => content,
            })
        })
    };
    // Sized for every gap and item, so the list fills it without regrowing.
    let mut separated = Vec::with_capacity(items.len() * 2 + 1);
    separated.extend((0..=items.len()).flat_map(|index| {
        let before = index
            .checked_sub(1)
            .and_then(|i| items.get(i))
            .map(|(p, _)| p);
        let after = items.get(index);
        gap(before, after.map(|(p, _)| p))
            .into_iter()
            .chain(after.map(|(position, layout)| item(position.clone(), layout.clone())))
    }));
    separated
}

fn list_gap<W: Host>(
    gap: f64,
    before: Option<&gid::Position>,
    after: Option<&gid::Position>,
) -> Option<impl FnOnce(Option<W::Hover>) -> Layout<W> + use<W>> {
    let height = if before.is_some() && after.is_some() {
        gap
    } else {
        gap / 2.0
    };
    (before.is_some() || after.is_some())
        .then_some(move |hover| widget::list::insertion_gap(height, hover))
}

pub fn list_layout<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    child: Option<Partial<W>>,
) -> Option<Layout<W>> {
    Some(list_layout_at(input, input.value?.list_positions()?, child))
}

pub fn list_layout_at<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    positions: Vec<gid::Position>,
    child: Option<Partial<W>>,
) -> Layout<W> {
    let items = list_items_at(input, positions, child);
    let horizontal = list_with(
        input,
        &items,
        |_, item| item,
        |before, after| {
            (before.is_some() && after.is_some()).then_some(|hover| {
                let comma = dim(", ");
                match hover {
                    Some(hover) => hover_highlight(comma, hover),
                    None => comma,
                }
            })
        },
    );
    let vertical = list_with(
        input,
        &items,
        |_, item| item,
        |before, after| list_gap(4.0, before, after),
    );
    authoring(selectable_bracket(
        Delim::Bracket,
        alternatives([row(0.0, horizontal), col(1, 0.0, vertical)]),
    ))
}

/// How a record's fields read, whether drawn as a whole record
/// ([`Fields::record`]) or after what a projection draws of the record itself
/// ([`Fields::form`], [`Fields::after`]): which come first, which show as
/// slots to fill even where the record lacks them, and how a field's name and
/// value are drawn. A field being added is drawn among them, where it will
/// stay.
pub(crate) struct Fields<W: Host> {
    leading: Vec<CellId>,
    slots: Vec<CellId>,
    child: Rc<dyn Fn(CellId) -> Option<Partial<W>>>,
    label: Rc<dyn Fn(CellId) -> Option<Layout<W>>>,
    shown_empty: bool,
}

impl<W: Host> Default for Fields<W> {
    fn default() -> Self {
        Self {
            leading: Vec::new(),
            slots: Vec::new(),
            child: Rc::new(|_| None),
            label: Rc::new(|_| None),
            shown_empty: false,
        }
    }
}

impl<W: Host> Fields<W> {
    /// `keys` first, in this order; the others after them by [`field_order`].
    pub(crate) fn leading(mut self, keys: impl IntoIterator<Item = CellId>) -> Self {
        self.leading = keys.into_iter().collect();
        self
    }

    /// `keys` drawn even where the record lacks them, as slots to fill.
    pub(crate) fn slots(mut self, keys: impl IntoIterator<Item = CellId>) -> Self {
        self.slots = keys.into_iter().collect();
        self
    }

    /// A field's value drawn by the projection `child` gives for its key,
    /// where it gives one.
    pub(crate) fn child(mut self, child: impl Fn(CellId) -> Option<Partial<W>> + 'static) -> Self {
        self.child = Rc::new(child);
        self
    }

    /// A field's name read as `label` gives for its key, where it gives one.
    pub(crate) fn label(mut self, label: impl Fn(CellId) -> Option<Layout<W>> + 'static) -> Self {
        self.label = Rc::new(label);
        self
    }

    /// The fields after a form's own drawn even when there are none, as a
    /// call's arguments are.
    pub(crate) fn shown_empty(mut self) -> Self {
        self.shown_empty = true;
        self
    }

    /// The whole record, its fields between braces.
    pub(crate) fn record(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    ) -> Option<Layout<W>> {
        let keys = self.keys(input, |_| false)?;
        Some(authoring(record_braces(
            keys.into_iter().map(|key| self.field(input, key)),
            pending_field(input),
        )))
    }

    /// A form drawn from the `queried` fields, the others following it
    /// ([`Fields::after`]), so the form hides nothing. `draw` is given the
    /// queried fields' values, in their order, to bind by name. One of the
    /// queried being added is the form's to draw, or to leave to a plainer
    /// view.
    pub(crate) fn form<const N: usize>(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        queried: [CellId; N],
        draw: impl FnOnce([Option<::grap::RuntimeValue>; N]) -> Option<Layout<W>>,
    ) -> Option<Layout<W>> {
        let fields = input.value?;
        let content = draw(queried.map(|key| fields.field(key)))?;
        Some(self.after(input, |key| queried.contains(&key), content))
    }

    /// `content` followed by the fields it doesn't draw, as a call's function
    /// is by its arguments. Drawn as one piece, the value takes no new field
    /// from Enter: these views don't offer [`authoring`], which a rawer view
    /// of it does.
    pub(crate) fn after(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        drawn: impl Fn(CellId) -> bool,
        content: Layout<W>,
    ) -> Layout<W> {
        match self.rest(input, drawn) {
            Some(rest) => hug(content, rest, 0.0),
            None => content,
        }
    }

    /// The fields `drawn` doesn't cover between braces, to follow what is
    /// drawn of the record; `None` where there are none to show.
    pub(crate) fn rest(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        drawn: impl Fn(CellId) -> bool,
    ) -> Option<Layout<W>> {
        let keys = self.keys(input, drawn)?;
        let pending = pending_field(input);
        (self.shown_empty || !keys.is_empty() || pending.is_some())
            .then(|| record_braces(keys.into_iter().map(|key| self.field(input, key)), pending))
    }

    /// The keys to draw but those `drawn` covers: the record's, its slots,
    /// and one being added, in order.
    fn keys(
        &self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        drawn: impl Fn(CellId) -> bool,
    ) -> Option<Vec<CellId>> {
        let mut keys = input.value?.record_keys()?;
        let adding = match &input.pending {
            Some(Pending::Child(Step::Key(key))) => Some(*key),
            _ => None,
        };
        for key in self.slots.iter().copied().chain(adding) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys.retain(|key| !drawn(*key));
        let lead = |key: &CellId| {
            self.leading
                .iter()
                .position(|leader| leader == key)
                .unwrap_or(self.leading.len())
        };
        keys.sort_by(|left, right| {
            lead(left)
                .cmp(&lead(right))
                .then_with(|| field_order(input.env, left, right))
        });
        Some(keys)
    }

    fn field(
        &self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        key: CellId,
    ) -> RecordField<W, W::Hover> {
        let name = (self.label)(key).unwrap_or_else(|| field_label(input.env, key));
        // A missing field's name selects it only where it's a slot to fill.
        let selects =
            self.slots.contains(&key) || input.value.is_some_and(|value| value.contains_field(key));
        RecordField {
            label: record_label_reading(input, key, name, selects),
            value: descend(
                [Step::Key(key)],
                Projected::by(
                    (self.child)(key)
                        .map(|child| compose_partials([child, input.default_projection.clone()])),
                    None,
                ),
            ),
        }
    }
}

/// [`Fields::form`], the fields read as usual.
pub(crate) fn record_form<W: Host, const N: usize>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    queried: [CellId; N],
    draw: impl FnOnce([Option<::grap::RuntimeValue>; N]) -> Option<Layout<W>>,
) -> Option<Layout<W>> {
    Fields::default().form(input, queried, draw)
}

/// Named fields first, by name, then the others by id.
pub(crate) fn field_order(env: &dyn Env, left: &CellId, right: &CellId) -> Ordering {
    match (env.name(*left), env.name(*right)) {
        (Some(a), Some(b)) => a.spelling().cmp(&b.spelling()).then(left.cmp(right)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

/// A field's name, or its short id where it has none.
pub(crate) fn field_label<W: Host>(env: &dyn Env, key: CellId) -> Layout<W> {
    match env.name(key) {
        Some(name) => crate::display::named(&name, Face::Label),
        None => faced(crate::identity::short_id(key), Face::Id),
    }
}

/// The ordinary field-name presentation and interaction, independent of how
/// the enclosing record arranges its fields.
pub(crate) fn record_label<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    key: CellId,
) -> Layout<W> {
    let present = input.value.is_some_and(|value| value.contains_field(key));
    record_label_reading(input, key, field_label(input.env, key), present)
}

/// A field's label read as `name`, which a click on selects the field where
/// `selects`.
fn record_label_reading<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    key: CellId,
    name: Layout<W>,
    selects: bool,
) -> Layout<W> {
    let site = input.site().below(&[Step::Key(key)]);
    crate::display::field_label_at(row(0.0, [name, dim(":")]), &site, key, selects)
}

/// Enter's authoring for the container `child` draws: a field or element
/// beside one of its children, or with the command chord one within it
/// (see [`crate::editing::enter_pending`]). The children's own handlers come
/// first. A projection that doesn't offer this, such as a number's, takes
/// nothing in from Enter.
pub fn authoring<W: Host>(child: Layout<W>) -> Layout<W> {
    super::widget::around(
        child,
        Rc::new(|context| {
            let path = context.path().to_vec();
            let scale = context.toolkit.styles().scale;
            Box::new(move |child| {
                super::widget::before_place(child, move |_, output| {
                    let command = output.input.command_modifier;
                    output.handler().on_key_with(move |world, event, input| {
                        event.state.is_down()
                            && event.key == Key::Named(NamedKey::Enter)
                            && crate::editing::author_in(
                                &mut world.session(),
                                &path,
                                command,
                                input.geometry(scale),
                                event,
                            )
                    });
                })
            })
        }),
    )
}

pub(crate) fn pending_field<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    matches!(input.pending, Some(Pending::Field)).then(|| {
        block_hover(row(
            0.0,
            [
                picker(raw::picker(CompletionKind::Field)),
                dim(": "),
                slot(),
            ],
        ))
    })
}

#[cfg(test)]
mod tests;
