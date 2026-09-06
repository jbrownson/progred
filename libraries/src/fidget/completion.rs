use super::{f32, node, vocabulary::*};
use gid::{CellId, Step, Value};
use progred_display::{Completion, CompletionKind, CompletionRequest, CompletionScope};

const SHAPES: &[(CellId, &str)] = &[
    (UNION, "union"),
    (DIFFERENCE, "difference"),
    (INTERSECTION, "intersection"),
    (TRANSLATE, "translate"),
    (SUM, "sum"),
    (SUBTRACT, "subtract"),
    (MULTIPLY, "multiply"),
    (DIVIDE, "divide"),
    (MIN, "min"),
    (MAX, "max"),
    (NEGATE, "negate"),
    (ABS, "abs"),
    (SQRT, "sqrt"),
    (SQUARE, "square"),
];

fn parameters(marker: CellId) -> Option<&'static [(CellId, &'static str)]> {
    match marker {
        SPHERE | CIRCLE => Some(&[(RADIUS, "radius")]),
        TRANSLATE => Some(&[
            (FIELD, "field"),
            (DELTA_X, "x"),
            (DELTA_Y, "y"),
            (DELTA_Z, "z"),
        ]),
        SUM | SUBTRACT | MULTIPLY | DIVIDE | MIN | MAX | UNION | DIFFERENCE | INTERSECTION => {
            Some(&[(LEFT, "left"), (RIGHT, "right")])
        }
        NEGATE | ABS | SQRT | SQUARE => Some(&[(OPERAND, "operand")]),
        _ => None,
    }
}

enum Slot {
    Expression,
    Field,
    Parameters(CellId),
    Number,
    Axis,
}

fn argument(marker: CellId, field: CellId) -> Option<Slot> {
    parameters(marker)?
        .iter()
        .any(|(id, _)| *id == field)
        .then(|| {
            if matches!(field, LEFT | RIGHT | OPERAND | FIELD) {
                Slot::Field
            } else {
                Slot::Number
            }
        })
}

fn called_shape(value: &Value) -> Option<CellId> {
    let function = value
        .as_record()?
        .get(&grap_runtime::vocabulary::FUNCTION)?
        .as_cell()?;
    parameters(function).map(|_| function)
}

fn context(request: &CompletionRequest<'_>) -> Option<Slot> {
    match request.kind {
        CompletionKind::Field => request.value().and_then(called_shape).map(Slot::Parameters),
        CompletionKind::Value => request
            .path
            .iter()
            .enumerate()
            .rev()
            .find(|(_, step)| !matches!(step, Step::Follow(_)))
            .and_then(|(index, step)| match step {
                Step::Key(field) => (request.value_at)(&request.path[..index])
                    .and_then(called_shape)
                    .and_then(|marker| argument(marker, *field))
                    .map(|slot| match slot {
                        Slot::Field => Slot::Expression,
                        slot => slot,
                    }),
                _ => None,
            }),
    }
    .or_else(|| slot(request.path))
}

fn slot(path: &[Step]) -> Option<Slot> {
    let mut steps = path
        .iter()
        .rev()
        .filter(|step| !matches!(step, Step::Follow(_)));
    match steps.next()? {
        Step::Key(FIDGET) => Some(Slot::Expression),
        Step::Key(AXIS) => Some(Slot::Axis),
        Step::Key(marker) if SHAPES.iter().any(|(id, _)| id == marker) => {
            Some(Slot::Parameters(*marker))
        }
        Step::Key(field) => match steps.next()? {
            Step::Key(marker) if SHAPES.iter().any(|(id, _)| id == marker) => {
                argument(*marker, *field)
            }
            _ => None,
        },
        Step::Element(_) if matches!(steps.next(), Some(Step::Key(FIDGET))) => {
            Some(Slot::Expression)
        }
        _ => None,
    }
}

fn labels(fields: &[(CellId, &str)]) -> Vec<Completion> {
    fields
        .iter()
        .map(|(cell, name)| Completion::new(*name, (*cell).into()).with_detail("fidget library"))
        .collect()
}

fn fields() -> Vec<Completion> {
    SHAPES
        .iter()
        .map(|(marker, name)| {
            let path = std::iter::once(Step::Key(*marker))
                .chain(
                    parameters(*marker)
                        .and_then(|fields| fields.first())
                        .map(|(field, _)| Step::Key(*field)),
                )
                .collect::<Vec<_>>();
            Completion::new(*name, node(*marker, Value::record([])))
                .with_detail("fidget library")
                .on_commit(crate::selection::pending_at(&path))
        })
        .chain([(X, "x"), (Y, "y"), (Z, "z")].map(|(axis, name)| {
            Completion::new(name, node(AXIS, axis.into())).with_detail("fidget library")
        }))
        .collect()
}

fn shape_calls() -> impl Iterator<Item = Completion> {
    [(SPHERE, "sphere"), (CIRCLE, "circle")]
        .into_iter()
        .map(|(function, name)| {
            Completion::new(name, grap_runtime::call(function.into(), []))
                .with_detail("fidget library")
                .on_commit(crate::selection::pending_at(&[Step::Key(RADIUS)]))
        })
}

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    if request.scope != CompletionScope::Suggested {
        None
    } else if request.path.is_empty() {
        Some(match request.kind {
            CompletionKind::Value => vec![super::root_completion()],
            CompletionKind::Field => vec![
                Completion::new("fidget", FIDGET.into())
                    .with_aliases(["sdf"])
                    .with_detail("fidget library"),
            ],
        })
    } else {
        match (context(request)?, request.kind) {
            (Slot::Expression, CompletionKind::Value) => Some(
                shape_calls()
                    .chain(fields())
                    .chain(f32::completions(request.query))
                    .collect(),
            ),
            (Slot::Field, CompletionKind::Value) => Some(
                fields()
                    .into_iter()
                    .chain(f32::completions(request.query))
                    .collect(),
            ),
            (Slot::Field | Slot::Expression, CompletionKind::Field) => Some(
                labels(SHAPES)
                    .into_iter()
                    .chain(labels(&[(AXIS, "axis")]))
                    .collect(),
            ),
            (Slot::Parameters(marker), CompletionKind::Field) => Some(labels(parameters(marker)?)),
            (Slot::Parameters(marker), CompletionKind::Value) => Some(vec![
                Completion::new("parameters", Value::record([])).on_commit(
                    crate::selection::pending_at(
                        &parameters(marker)?
                            .first()
                            .map(|(id, _)| Step::Key(*id))
                            .into_iter()
                            .collect::<Vec<_>>(),
                    ),
                ),
            ]),
            (Slot::Number, CompletionKind::Value) => Some(f32::completions(request.query)),
            (Slot::Axis, CompletionKind::Value) => Some(labels(&[(X, "x"), (Y, "y"), (Z, "z")])),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_follows_cells_but_does_not_leak_into_arbitrary_descendants() {
        assert!(matches!(
            slot(&[Step::Key(FIDGET), Step::Follow(gid::Resolution::Document)]),
            Some(Slot::Expression)
        ));
        assert!(matches!(
            slot(&[
                Step::Key(UNION),
                Step::Follow(gid::Resolution::Document),
                Step::Key(LEFT)
            ]),
            Some(Slot::Field)
        ));
        assert!(matches!(
            slot(&[Step::Key(TRANSLATE), Step::Key(DELTA_X)]),
            Some(Slot::Number)
        ));
        assert!(slot(&[Step::Key(crate::grap::vocabulary::GRAP), Step::Key(LEFT)]).is_none());
        assert!(slot(&[Step::Key(FIDGET), Step::Key(crate::name::vocabulary::NAME)]).is_none());
        assert!(
            slot(&[
                Step::Key(SPHERE),
                Step::Key(RADIUS),
                Step::Key(f32::vocabulary::F32)
            ])
            .is_none()
        );
    }

    #[test]
    fn constructor_offers_use_the_fidget_data_language() {
        for offer in fields() {
            let value = offer.value.instantiate();
            let record = value.as_record().unwrap();
            assert!(!record.contains_key(&grap_runtime::vocabulary::FUNCTION));
            let marker = super::super::one_marker(record).unwrap();
            if marker == AXIS {
                assert!(super::super::tree(&value).is_some());
            } else {
                assert_eq!(record.get(&marker), Some(&Value::record([])));
                assert!(offer.on_commit.is_some());
            }
        }
    }

    #[test]
    fn radial_shapes_are_offered_as_calls_only_where_grap_evaluates() {
        let path = [Step::Key(FIDGET)];
        let lookup = |_: &[Step]| None;
        let request = CompletionRequest {
            path: &path,
            query: "",
            kind: CompletionKind::Value,
            scope: CompletionScope::Suggested,
            value_at: &lookup,
        };
        for shape in [SPHERE, CIRCLE] {
            assert!(offers(&request).unwrap().iter().any(|offer| {
                offer.value.instantiate() == grap_runtime::call(shape.into(), [])
            }));
            assert!(
                !offers(&CompletionRequest {
                    path: &[Step::Key(FIDGET), Step::Key(UNION), Step::Key(LEFT)],
                    ..request
                })
                .unwrap()
                .iter()
                .any(|offer| {
                    offer
                        .value
                        .instantiate()
                        .as_record()
                        .unwrap()
                        .contains_key(&grap_runtime::vocabulary::FUNCTION)
                })
            );
        }
    }
}
