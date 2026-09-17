//! Structural projection combinators. Child overrides apply at each
//! immediate child; recursion is a choice made by that projection.

use super::*;

pub fn list(
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> Partial<crate::Editor, crate::frame::Hovered> {
    partial(move |input| list_layout(input, child.clone()))
}

/// An editable list without brackets or commas; items retain the standard
/// occurrence paths, child projections, and pending insertion behavior.
pub fn list_column(
    gap: f64,
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> Partial<crate::Editor, crate::frame::Hovered> {
    partial(move |input| list_column_with(input, gap, child.clone(), |_, entry| entry))
}

/// Compose content beside each projected item, outside that item's value
/// boundary. The list still owns element paths and pending insertion slots.
pub(crate) fn list_column_with(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    gap: f64,
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
    item: impl Fn(
        gid::Position,
        Layout<crate::Editor, crate::frame::Hovered>,
    ) -> Layout<crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let items = list_items(input, child)?;
    Some(if items.is_empty() {
        selectable_bracket(Delim::Bracket, row(0.0, []))
    } else {
        col(
            1,
            0.0,
            list_with(input, &items, item, |before, after| {
                list_gap(gap, before, after)
            }),
        )
    })
}

/// Prepare element paths, including a pending insertion, and share their
/// projection and measurement across any presentations the caller builds.
pub fn list_items(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> Option<Vec<(gid::Position, Layout<crate::Editor, crate::frame::Hovered>)>> {
    let elements = input.value?.as_list()?;
    let mut positions = elements
        .iter()
        .map(|(position, _)| position.clone())
        .collect::<Vec<_>>();
    if let Some(Pending::Child(Step::Element(position))) = &input.pending
        && !positions.contains(position)
    {
        positions.push(position.clone());
        positions.sort();
    }
    let child = child.map(|p| compose_partials([p, input.default_projection.clone()]));
    Some(
        positions
            .into_iter()
            .map(|position| {
                let layout = shared(descend(
                    Step::Element(position.clone()),
                    child.clone(),
                    None,
                ));
                (position, layout)
            })
            .collect(),
    )
}

/// One interleaved presentation of `list_items` for this input. The caller
/// supplies item decoration and separator paint; this function owns insertion
/// eligibility and interaction. Arrange the returned sequence with row/col/etc.
pub fn list_with<S>(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    items: &[(gid::Position, Layout<crate::Editor, crate::frame::Hovered>)],
    item: impl Fn(
        gid::Position,
        Layout<crate::Editor, crate::frame::Hovered>,
    ) -> Layout<crate::Editor, crate::frame::Hovered>,
    separator: impl Fn(Option<&gid::Position>, Option<&gid::Position>) -> Option<S>,
) -> Vec<Layout<crate::Editor, crate::frame::Hovered>>
where
    S: FnOnce(Option<crate::frame::Hovered>) -> Layout<crate::Editor, crate::frame::Hovered>,
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
            let target = (input.writable && !beside_pending)
                .then(|| gid::position::between(before, after))
                .flatten()
                .map(|position| input.targets.at([Step::Element(position)]));
            let content = render(target.as_ref().map(|t| t.hover.clone()));
            let content = match target {
                Some(target) => activatable(content, target.hover, target.select),
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

fn list_gap(
    gap: f64,
    before: Option<&gid::Position>,
    after: Option<&gid::Position>,
) -> Option<
    impl FnOnce(Option<crate::frame::Hovered>) -> Layout<crate::Editor, crate::frame::Hovered> + use<>,
> {
    let height = if before.is_some() && after.is_some() {
        gap
    } else {
        gap / 2.0
    };
    (before.is_some() || after.is_some())
        .then_some(move |hover| widget::list::insertion_gap(height, hover))
}

pub fn list_layout(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let items = list_items(input, child)?;
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
    Some(selectable_bracket(
        Delim::Bracket,
        alternatives([row(0.0, horizontal), col(1, 0.0, vertical)]),
    ))
}

#[cfg(test)]
pub fn record(
    child: impl Fn(CellId) -> Option<Partial<crate::Editor, crate::frame::Hovered>> + 'static,
) -> Partial<crate::Editor, crate::frame::Hovered> {
    partial(move |input| record_layout(input, &child))
}

pub fn record_layout(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    child: impl Fn(CellId) -> Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let keys = record_keys(input)?;
    Some(record_heads(
        keys.into_iter()
            .map(|key| record_field(input, key, child(key))),
        pending_field(input),
    ))
}

pub(crate) fn record_keys(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Vec<CellId>> {
    let fields = input.value?.as_record()?;
    let mut keys = fields.keys().copied().collect::<Vec<_>>();
    if let Some(Pending::Child(Step::Key(key))) = &input.pending
        && !keys.contains(key)
    {
        keys.push(*key);
    }
    keys.sort_by(
        |left, right| match (input.env.name(*left), input.env.name(*right)) {
            (Some(a), Some(b)) => a.cmp(b).then(left.cmp(right)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => left.cmp(right),
        },
    );
    Some(keys)
}

pub(crate) fn record_field(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    key: CellId,
    child: Option<Partial<crate::Editor, crate::frame::Hovered>>,
) -> RecordField<crate::Editor, crate::frame::Hovered> {
    RecordField {
        label: record_label(input, key),
        value: descend(
            Step::Key(key),
            child.map(|p| compose_partials([p, input.default_projection.clone()])),
            None,
        ),
    }
}

/// The ordinary field-name presentation and interaction, independent of how
/// the enclosing record arranges its fields.
pub(crate) fn record_label(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
    key: CellId,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    let fields = input.value.and_then(Value::as_record);
    let label = match input.env.name(key) {
        Some(name) => faced(name, Face::Label),
        None => {
            let hex = key.simple().to_string();
            faced(format!("…{}", &hex[hex.len() - 5..]), Face::Id)
        }
    };
    let target = input.targets.at([Step::Key(key)]);
    let head = row(0.0, [label, dim(":")]);
    if fields.is_some_and(|fields| fields.contains_key(&key)) {
        activatable(head, target.hover, target.select)
    } else {
        pickable(head, target.hover, key.into())
    }
}

pub(crate) fn pending_field(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    matches!(input.pending, Some(Pending::Field)).then(|| {
        block_hover(on_click(
            row(
                0.0,
                [completion(CompletionKind::Field, None), dim(": "), slot()],
            ),
            Rc::new(|_| true),
        ))
    })
}
