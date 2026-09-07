//! Structural projection combinators. Child overrides apply at each
//! immediate child; recursion is a choice made by that projection.

use super::*;

pub fn list<World: 'static, Hover: Clone + 'static>(
    child: Option<Partial<World, Hover>>,
) -> Partial<World, Hover> {
    partial(move |input| list_layout(input, child.clone()))
}

pub fn list_layout<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
    child: Option<Partial<World, Hover>>,
) -> Option<Layout<World, Hover>> {
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
    let children = positions
        .iter()
        .map(|position| {
            shared(descend(
                Step::Element(position.clone()),
                child.clone(),
                None,
            ))
        })
        .collect::<Vec<_>>();
    let mut flat = Vec::new();
    for (index, layout) in children.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).and_then(|i| positions.get(i)) {
            let separator = dim(", ");
            flat.push(
                match input
                    .writable
                    .then(|| input.targets.insert_after(previous.clone()))
                    .flatten()
                {
                    Some((hover, action)) => activatable(separator, hover, action),
                    None => separator,
                },
            );
        }
        flat.push(layout.clone());
    }
    Some(selectable_bracket(
        Delim::Bracket,
        alternatives([row(0.0, flat), col(0, 4.0, children)]),
    ))
}

pub fn record<World: 'static, Hover: Clone + 'static>(
    child: impl Fn(CellId) -> Option<Partial<World, Hover>> + 'static,
) -> Partial<World, Hover> {
    partial(move |input| record_layout(input, &child))
}

pub fn record_layout<World: 'static, Hover: Clone + 'static>(
    input: &ProjectionInput<'_, World, Hover>,
    child: impl Fn(CellId) -> Option<Partial<World, Hover>>,
) -> Option<Layout<World, Hover>> {
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
    let fields = keys.into_iter().map(|key| {
        let label = match input.env.name(key) {
            Some(name) => faced(name, Face::Label),
            None => {
                let hex = key.simple().to_string();
                faced(format!("…{}", &hex[hex.len() - 5..]), Face::Id)
            }
        };
        let target = input.targets.at([Step::Key(key)]);
        let head = row(0.0, [label, dim(":")]);
        RecordField {
            label: if fields.contains_key(&key) {
                activatable(head, target.hover, target.select)
            } else {
                pickable(head, target.hover, key.into())
            },
            value: descend(
                Step::Key(key),
                child(key).map(|p| compose_partials([p, input.default_projection.clone()])),
                None,
            ),
        }
    });
    let pending = matches!(input.pending, Some(Pending::Field)).then(|| {
        block_hover(on_click(
            row(
                0.0,
                [completion(CompletionKind::Field, None), dim(": "), slot()],
            ),
            Rc::new(|_| true),
        ))
    });
    Some(record_heads(fields, pending))
}
