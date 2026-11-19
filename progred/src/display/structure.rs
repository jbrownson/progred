//! Structural projection combinators. Child overrides apply at each
//! immediate child; recursion is a choice made by that projection.

use super::host::{Layout, Partial, ProjectionInput};
use super::*;
use puri::handler::HasHandler;
use ui_events::keyboard::{Key, NamedKey};

/// A list between brackets, one item to a line where they don't fit on one,
/// folding as Raw's lists do.
pub fn list<W: Host>(child: Option<Partial<W>>) -> Partial<W> {
    runtime_partial(move |input| {
        let positions = input.value?.list_positions()?;
        let empty = positions.is_empty();
        Some(crate::display::fold::folding_unless(
            empty,
            Delim::Bracket,
            list_layout_at(input, positions, child.clone()),
        ))
    })
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
    let separated = list_with(input, &items, item, |before, after| {
        list_gap(gap, before, after)
    });
    Some(authoring(if items.is_empty() {
        selectable_bracket(Delim::Bracket, row(0.0, separated))
    } else {
        col(1, 0.0, separated)
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
    let child = child.map(|child| crate::display::first(input, child));
    positions
        .into_iter()
        .map(|position| {
            let layout = shared(descend(
                [Step::Element(position.clone())],
                child.clone(),
                None,
            ));
            (position, layout)
        })
        .collect()
}

/// One interleaved presentation of `list_items` for this input. The caller
/// supplies item decoration and separator paint; this function owns insertion
/// eligibility and interaction. Arrange the returned sequence with row/col/etc.
/// Each place an element can go, before the first, between two, after the
/// last, or in an empty list, is an insertion point, which a click on its
/// separator or Tab opens, where there's no separator drawn, as at the ends
/// of a list on one line, by Tab alone. One beside an element being added
/// is that element's place, so it isn't another.
pub fn list_with<W: Host, S>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    items: &[(gid::Position, Layout<W>)],
    item: impl Fn(gid::Position, Layout<W>) -> Layout<W>,
    separator: impl Fn(Option<&gid::Position>, Option<&gid::Position>) -> Option<S>,
) -> Vec<Layout<W>>
where
    S: FnOnce(Option<W::Hover>) -> Layout<W>,
{
    let (items, last) = list_parts(input, items, item, separator);
    // Sized for every gap and item, so the list fills it without regrowing.
    let mut separated = Vec::with_capacity(items.len() * 2 + 1);
    for (gap, item) in items {
        separated.extend(gap);
        separated.push(item);
    }
    separated.extend(last);
    separated
}

/// [`list_with`]'s presentation as each item with the gap before it, and
/// the gap after the last.
fn list_parts<W: Host, S>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    items: &[(gid::Position, Layout<W>)],
    item: impl Fn(gid::Position, Layout<W>) -> Layout<W>,
    separator: impl Fn(Option<&gid::Position>, Option<&gid::Position>) -> Option<S>,
) -> ListParts<W>
where
    S: FnOnce(Option<W::Hover>) -> Layout<W>,
{
    let gap = |before: Option<&gid::Position>, after: Option<&gid::Position>| {
        let beside_pending = matches!(
            &input.pending,
            Some(Pending::Child(Step::Element(position)))
                if before == Some(position) || after == Some(position)
        );
        let site = (input.writable && !beside_pending)
            .then(|| gid::position::between(before, after))
            .flatten()
            .map(|position| input.site().below(&[Step::Element(position)]));
        let content = separator(before, after)
            .map(|render| render(site.as_ref().map(|site| W::Hover::from(site.hover()))));
        match (content, site) {
            (Some(content), Some(site)) => Some(widget::fill_width(insertion_point(
                selectable(content, &site),
                &site,
            ))),
            (Some(content), None) => Some(widget::fill_width(content)),
            (None, Some(site)) => Some(insertion_point(row(0.0, []), &site)),
            (None, None) => None,
        }
    };
    let parts = items
        .iter()
        .enumerate()
        .map(|(index, (position, layout))| {
            let before = index.checked_sub(1).map(|i| &items[i].0);
            (
                gap(before, Some(position)),
                item(position.clone(), layout.clone()),
            )
        })
        .collect();
    (parts, gap(items.last().map(|(p, _)| p), None))
}

/// Each item with the gap before it, if any, and the gap after the last.
type ListParts<W> = (Vec<(Option<Layout<W>>, Layout<W>)>, Option<Layout<W>>);

/// `child` as the place `site` names, where something can be added: Tab
/// stops there, and arriving opens it to fill in.
pub(crate) fn insertion_point<W: Host>(child: Layout<W>, site: &crate::editing::Site) -> Layout<W> {
    insertion(child, widget::navigation::arrival(site.clone()))
}

/// `child` as a place where something can be added, which Tab stops at and
/// arrives at through `select`.
pub(crate) fn insertion<W: Host>(
    child: Layout<W>,
    select: widget::navigation::Select<W>,
) -> Layout<W> {
    widget::around(
        child,
        Rc::new(move |_| {
            let select = select.clone();
            Box::new(move |child| widget::navigation::tab_stop(child, None, select))
        }),
    )
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

pub fn list_layout_at<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    positions: Vec<gid::Position>,
    child: Option<Partial<W>>,
) -> Layout<W> {
    let items = list_items_at(input, positions, child);
    let horizontal = comma_separated(input, &items);
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

/// `items` with a comma between each two, which a new item can be added at.
fn comma_separated<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    items: &[(gid::Position, Layout<W>)],
) -> Vec<Layout<W>> {
    list_with(input, items, |_, item| item, comma)
}

/// The comma between two items, highlighted where hovering it adds one.
fn comma<W: Host>(
    before: Option<&gid::Position>,
    after: Option<&gid::Position>,
) -> Option<impl FnOnce(Option<W::Hover>) -> Layout<W> + use<W>> {
    (before.is_some() && after.is_some()).then_some(|hover: Option<W::Hover>| {
        let comma = dim(", ");
        match hover {
            Some(hover) => hover_highlight(comma, hover),
            None => comma,
        }
    })
}

/// A list between brackets whose items flow: on one line where they fit,
/// and taking further lines as a paragraph's words do where they don't, each
/// item keeping its comma. For many small things of a kind, which read
/// across, as a function's parameters do; [`list`] puts one to a line. It
/// folds as Raw's lists do.
pub fn list_flowing<W: Host>(child: Option<Partial<W>>) -> Partial<W> {
    runtime_partial(move |input| {
        let positions = input.value?.list_positions()?;
        let empty = positions.is_empty();
        let items = list_items_at(input, positions, child.clone());
        let (items, last) = list_parts(
            input,
            &items,
            |_, item| item,
            |before, after| comma(before, after),
        );
        // Each item flows with what follows it, its comma or the place after
        // the last, and the first with the place before it too.
        let mut groups: Vec<Vec<Layout<W>>> = Vec::new();
        for (index, (gap, item)) in items.into_iter().enumerate() {
            match groups.last_mut() {
                Some(previous) => previous.extend(gap),
                None => groups.push(gap.into_iter().collect()),
            }
            if index > 0 {
                groups.push(Vec::new());
            }
            groups.last_mut().expect("a group").push(item);
        }
        match groups.last_mut() {
            Some(previous) => previous.extend(last),
            None => groups.push(last.into_iter().collect()),
        }
        let items = groups.into_iter().map(|parts| row(0.0, parts));
        Some(crate::display::fold::folding_unless(
            empty,
            Delim::Bracket,
            authoring(selectable_bracket(
                Delim::Bracket,
                flow(0.0, 2.0, items.collect::<Vec<_>>()),
            )),
        ))
    })
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
    parts: Vec<CellId>,
    child: Rc<dyn Fn(CellId) -> Option<Partial<W>>>,
    label: Rc<dyn Fn(CellId) -> Option<Layout<W>>>,
    shown_empty: bool,
}

impl<W: Host> Default for Fields<W> {
    fn default() -> Self {
        Self {
            leading: Vec::new(),
            slots: Vec::new(),
            parts: Vec::new(),
            child: Rc::new(|_| None),
            label: Rc::new(|_| None),
            shown_empty: false,
        }
    }
}

impl<W: Host> Fields<W> {
    /// `keys` first, in this order; the others after them, named ones by name and then the rest by id.
    pub(crate) fn leading(mut self, keys: impl IntoIterator<Item = CellId>) -> Self {
        self.leading = keys.into_iter().collect();
        self
    }

    /// `keys` drawn even where the record lacks them, as slots to fill.
    pub(crate) fn slots(mut self, keys: impl IntoIterator<Item = CellId>) -> Self {
        self.slots = keys.into_iter().collect();
        self
    }

    /// `keys`' values drawn as parts of the record ([`crate::display::part`]):
    /// no stop of their own, their names selecting nothing.
    pub(crate) fn parts(mut self, keys: impl IntoIterator<Item = CellId>) -> Self {
        self.parts = keys.into_iter().collect();
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

    /// The whole record, its fields between braces, folding as Raw's records
    /// do.
    pub(crate) fn record(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    ) -> Option<Layout<W>> {
        let labels = input.value?.record_keys()?;
        let empty = labels.is_empty();
        Some(crate::display::fold::folding_unless(
            empty,
            Delim::Brace,
            self.record_of(input, labels),
        ))
    }

    /// The whole record, its fields between braces, without the fold, for a
    /// view that folds it itself, as Raw does.
    pub(crate) fn record_of(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        labels: Vec<CellId>,
    ) -> Layout<W> {
        let keys = self.keys(labels, input, |_| false);
        authoring(record_braces(
            keys.into_iter().map(|key| self.field(input, key)),
            pending_field(input),
            adding_field(input),
        ))
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
        self.form_drawing(input, &[], queried, draw)
    }

    /// [`Fields::form`] for a record of `kind`, as its `isa` says, declining
    /// one of another kind. The form draws what kind it is, so the `isa`
    /// doesn't follow it.
    pub(crate) fn kind_form<const N: usize>(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        kind: CellId,
        queried: [CellId; N],
        draw: impl FnOnce([Option<::grap::RuntimeValue>; N]) -> Option<Layout<W>>,
    ) -> Option<Layout<W>> {
        (input.value?.kind() == Some(kind)).then_some(())?;
        self.form_drawing(input, &[::grap::isa::ISA], queried, draw)
    }

    /// A form `draw` makes of the `queried` fields' values, followed by the
    /// fields neither queried nor among `drawn`, which it draws too.
    fn form_drawing<const N: usize>(
        self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        drawn: &[CellId],
        queried: [CellId; N],
        draw: impl FnOnce([Option<::grap::RuntimeValue>; N]) -> Option<Layout<W>>,
    ) -> Option<Layout<W>> {
        let fields = input.value?;
        let content = draw(queried.map(|key| fields.field(key)))?;
        Some(self.after(
            input,
            |key| drawn.contains(&key) || queried.contains(&key),
            content,
        ))
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
        let keys = self.keys(input.value?.record_keys()?, input, drawn);
        let pending = pending_field(input);
        (self.shown_empty || !keys.is_empty() || pending.is_some()).then(|| {
            record_braces(
                keys.into_iter().map(|key| self.field(input, key)),
                pending,
                None,
            )
        })
    }

    /// The keys to draw but those `drawn` covers: the record's `labels`, its
    /// slots, and one being added, in order.
    fn keys(
        &self,
        labels: Vec<CellId>,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        drawn: impl Fn(CellId) -> bool,
    ) -> Vec<CellId> {
        let mut keys = labels;
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
        // The leading keys in their order, then named fields by name, then
        // the others by id; each key's name is read once.
        keys.sort_by_cached_key(|key| {
            let lead = self
                .leading
                .iter()
                .position(|leader| leader == key)
                .unwrap_or(self.leading.len());
            let name = input
                .env
                .name(*key)
                .map(|name| name.spelling().into_owned());
            (lead, name.is_none(), name, *key)
        });
        keys
    }

    fn field(
        &self,
        input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
        key: CellId,
    ) -> RecordField<W, W::Hover> {
        let name = (self.label)(key).unwrap_or_else(|| field_name(input.env, key));
        let part = self.parts.contains(&key);
        // A missing field's name selects it only where it's a slot to fill,
        // and a part's selects nothing.
        let selects = !part
            && (self.slots.contains(&key)
                || input.value.is_some_and(|value| value.contains_field(key)));
        let child = (self.child)(key).map(|child| crate::display::first(input, child));
        RecordField {
            label: field_label(input, key, name, selects),
            value: if part {
                crate::display::part([Step::Key(key)], child, None)
            } else {
                descend([Step::Key(key)], child, None)
            },
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

/// [`Fields::kind_form`], the fields read as usual.
pub(crate) fn kind_form<W: Host, const N: usize>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    kind: CellId,
    queried: [CellId; N],
    draw: impl FnOnce([Option<::grap::RuntimeValue>; N]) -> Option<Layout<W>>,
) -> Option<Layout<W>> {
    Fields::default().kind_form(input, kind, queried, draw)
}

/// A field's name, or its short id where it has none.
pub(crate) fn field_name<W: Host>(env: &dyn Env, key: CellId) -> Layout<W> {
    match env.name(key) {
        Some(name) => crate::display::named(&name, Face::Label),
        None => faced(crate::identity::short_id(key), Face::Id),
    }
}

/// The label of the field `key` here, read as `name`, then its colon, as
/// [`label`] draws it: a click selects the field where `selects`.
pub(crate) fn field_label<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    key: CellId,
    name: Layout<W>,
    selects: bool,
) -> Layout<W> {
    let site = input.site().below(&[Step::Key(key)]);
    let hover = site.label_hover();
    label(site, hover, key, row(0.0, [name, dim(":")]), selects)
}

/// `name`, which labels what's at `site` as the cell `cell`, as a field's
/// name does its field and a record's kind the record. It hovers as `hover`,
/// apart from what it labels, so a Cmd+click picks `cell`, not what's
/// labelled, or, with no slot open, acts as a click; a click selects `site`
/// where `selects`.
pub(crate) fn label<W: Host>(
    site: crate::editing::Site,
    hover: crate::hover::Hover,
    cell: CellId,
    name: Layout<W>,
    selects: bool,
) -> Layout<W> {
    let target = W::Hover::from(hover);
    let hover = target.clone();
    let select = selects.then(|| widget::navigation::selecting(site));
    let label = ::grap::RuntimeValue::from(Value::from(cell));
    let pressed = widget::before(
        name,
        Rc::new(move |_| {
            let target = target.clone();
            let select = select.clone();
            let label = label.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    let (click, otherwise) = (select.clone(), select.clone());
                    let label = label.clone();
                    output.on_press(
                        target,
                        move |world: &mut W, _| {
                            click
                                .as_ref()
                                .is_some_and(|select| select(&mut world.session()))
                        },
                        move |world: &mut W, _| {
                            widget::interaction::pick(world, &label)
                                || otherwise
                                    .as_ref()
                                    .is_some_and(|select| select(&mut world.session()))
                        },
                    );
                }
            })
        }),
    );
    on_hover(pressed, hover)
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

/// Where a field can be added to the record here, which Tab stops at and
/// arriving opens for its label: none while one is being added, which is
/// that place, nor in a record of a kind, whose kind lists its fields.
fn adding_field<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let kinded = input.value.and_then(|value| value.kind()).is_some();
    (input.writable && !kinded && !matches!(input.pending, Some(Pending::Field))).then(|| {
        let site = input.site();
        insertion(
            row(0.0, []),
            Rc::new(move |world: &mut W, _| {
                site.open(&mut world.session())
                    .select_with(crate::selection::payload::label(""));
                true
            }),
        )
    })
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
#[path = "tests/structure.rs"]
mod tests;
