//! Structural projection combinators. Child overrides apply at each
//! immediate child; recursion is a choice made by that projection.

use super::host::{Layout, Partial, ProjectionInput};
use super::projection::group;
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
    Some(authoring(group(if items.is_empty() {
        selectable_bracket(Delim::Bracket, row(0.0, []))
    } else {
        col(
            1,
            0.0,
            list_with(input, &items, item, |before, after| {
                list_gap(gap, before, after)
            }),
        )
    })))
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
pub fn list_with<W: Host, S>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    items: &[(gid::Position, Layout<W>)],
    item: impl Fn(gid::Position, Layout<W>) -> Layout<W>,
    separator: impl Fn(Option<&gid::Position>, Option<&gid::Position>) -> Option<S>,
) -> Vec<Layout<W>>
where
    S: FnOnce(Option<W::Hover>) -> Layout<W>,
{
    let mut separated = Vec::with_capacity(items.len() * 2 + 1);
    for index in 0..=items.len() {
        let before = index
            .checked_sub(1)
            .and_then(|i| items.get(i))
            .map(|(p, _)| p);
        let after = items.get(index).map(|(p, _)| p);
        if let Some(render) = separator(before, after) {
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
            let content = match site {
                Some(site) => activatable(content, &site),
                None => content,
            };
            separated.push(widget::fill_width(content));
        }
        if let Some((position, layout)) = items.get(index) {
            separated.push(item(position.clone(), layout.clone()));
        }
    }
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
        group(alternatives([row(0.0, horizontal), col(1, 0.0, vertical)])),
    ))
}

#[cfg(test)]
pub fn record<W: Host>(child: impl Fn(CellId) -> Option<Partial<W>> + 'static) -> Partial<W> {
    runtime_partial(move |input| record_layout(input, &child))
}

pub fn record_layout<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    child: impl Fn(CellId) -> Option<Partial<W>>,
) -> Option<Layout<W>> {
    Some(record_layout_at(input, input.value?.record_keys()?, child))
}

pub fn record_layout_at<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    keys: Vec<CellId>,
    child: impl Fn(CellId) -> Option<Partial<W>>,
) -> Layout<W> {
    authoring(record_heads(
        ordered_keys(input, keys)
            .into_iter()
            .map(|key| record_field(input, key, child(key))),
        pending_field(input),
    ))
}

pub(crate) fn record_keys<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Vec<CellId>> {
    Some(ordered_keys(input, input.value?.record_keys()?))
}

/// Display order, including a pending new field.
fn ordered_keys<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    mut keys: Vec<CellId>,
) -> Vec<CellId> {
    if let Some(Pending::Child(Step::Key(key))) = &input.pending
        && !keys.contains(key)
    {
        keys.push(*key);
    }
    keys.sort_by(|left, right| field_order(input.env, left, right));
    keys
}

/// Named fields first, by name, then the others by id.
pub(crate) fn field_order(env: &dyn Env, left: &CellId, right: &CellId) -> Ordering {
    match (env.name(*left), env.name(*right)) {
        (Some(a), Some(b)) => a.cmp(b).then(left.cmp(right)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

/// A field's name, or its short id where it has none.
pub(crate) fn field_label<W: Host>(env: &dyn Env, key: CellId) -> Layout<W> {
    match env.name(key) {
        Some(name) => faced(name, Face::Label),
        None => faced(crate::identity::short_id(key), Face::Id),
    }
}

pub(crate) fn record_field<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    key: CellId,
    child: Option<Partial<W>>,
) -> RecordField<W, W::Hover> {
    RecordField {
        label: record_label(input, key),
        value: descend(
            [Step::Key(key)],
            child.map(|p| compose_partials([p, input.default_projection.clone()])),
            None,
        ),
    }
}

/// The ordinary field-name presentation and interaction, independent of how
/// the enclosing record arranges its fields.
pub(crate) fn record_label<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    key: CellId,
) -> Layout<W> {
    let site = input.site().below(&[Step::Key(key)]);
    let head = row(0.0, [field_label(input.env, key), dim(":")]);
    if input.value.is_some_and(|value| value.contains_field(key)) {
        activatable(head, &site)
    } else {
        pickable(head, W::Hover::from(site.hover()), Value::from(key))
    }
}

/// The fields a projection leaves undrawn, and one being added, as a
/// record fragment to follow what it does draw.
pub(crate) fn extra_fields<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    drawn: impl Fn(CellId) -> bool,
) -> Option<Layout<W>> {
    let fields = record_keys(input)?
        .into_iter()
        .filter(|key| !drawn(*key))
        .map(|key| record_field(input, key, None))
        .collect::<Vec<_>>();
    let pending = pending_field(input);
    (!fields.is_empty() || pending.is_some()).then(|| record_fragment(fields, pending))
}

/// `content` followed by its [`extra_fields`], as a call's head is by its
/// arguments. Drawn as one piece, the value takes no new field from Enter:
/// these views don't offer [`authoring`], which a rawer view of it does.
pub(crate) fn with_extra_fields<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    drawn: impl Fn(CellId) -> bool,
    content: Layout<W>,
) -> Layout<W> {
    match extra_fields(input, drawn) {
        Some(extras) => super::projection::group_hug(content, extras, 0.0, 20.0),
        None => content,
    }
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
            let root = context.toolkit.view().clone();
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
                                &root,
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
