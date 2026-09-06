use super::{f32, node, parameters, vocabulary::*};
use gid::{CellId, Step, Value};
use progred_display::{Completion, CompletionKind, CompletionRequest, CompletionScope};

const SHAPES: &[CellId] = &[
    UNION,
    DIFFERENCE,
    INTERSECTION,
    TRANSLATE,
    SUM,
    SUBTRACT,
    MULTIPLY,
    DIVIDE,
    MIN,
    MAX,
    NEGATE,
    ABS,
    SQRT,
    SQUARE,
    SIN,
    COS,
];

enum Slot {
    Expression,
    Field,
    Parameters(Vec<CellId>),
    Number,
    Axis,
}

fn argument(parameters: &[CellId], field: CellId) -> Option<Slot> {
    parameters.contains(&field).then_some(())?;
    match field {
        LEFT | RIGHT | OPERAND | FIELD => Some(Slot::Field),
        RADIUS | DELTA_X | DELTA_Y | DELTA_Z => Some(Slot::Number),
        _ => None,
    }
}

fn called_shape(value: &Value) -> Option<CellId> {
    let function = value
        .as_record()?
        .get(&grap_runtime::vocabulary::FUNCTION)?
        .as_cell()?;
    (matches!(function, SPHERE | CIRCLE) || SHAPES.contains(&function)).then_some(function)
}

fn call_parameters(request: &CompletionRequest<'_>, function: CellId) -> Option<Vec<CellId>> {
    if (request.resolve)(function)?.native {
        parameters(function).map(<[CellId]>::to_vec)
    } else {
        crate::grap::function_parameters(&function.into(), request.resolve)
    }
}

fn context(request: &CompletionRequest<'_>) -> Option<Slot> {
    match request.kind {
        CompletionKind::Field => request
            .value()
            .and_then(called_shape)
            .and_then(|function| call_parameters(request, function))
            .map(Slot::Parameters),
        CompletionKind::Value => request
            .path
            .iter()
            .enumerate()
            .rev()
            .find(|(_, step)| !matches!(step, Step::Follow(_)))
            .and_then(|(index, step)| match step {
                Step::Key(field) => (request.value_at)(&request.path[..index])
                    .and_then(called_shape)
                    .and_then(|function| call_parameters(request, function))
                    .and_then(|parameters| argument(&parameters, *field))
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
        Step::Key(marker) if SHAPES.contains(marker) => {
            Some(Slot::Parameters(parameters(*marker)?.to_vec()))
        }
        Step::Key(field) => match steps.next()? {
            Step::Key(marker) if SHAPES.contains(marker) => argument(parameters(*marker)?, *field),
            _ => None,
        },
        Step::Element(_) if matches!(steps.next(), Some(Step::Key(FIDGET))) => {
            Some(Slot::Expression)
        }
        _ => None,
    }
}

fn labels(fields: &[CellId]) -> Vec<Completion> {
    crate::completion::labels(fields.iter().copied())
        .into_iter()
        .map(|offer| offer.with_detail(super::ID))
        .collect()
}

fn fields() -> Vec<Completion> {
    SHAPES
        .iter()
        .map(|marker| {
            let path = std::iter::once(Step::Key(*marker))
                .chain(
                    parameters(*marker)
                        .and_then(|fields| fields.first())
                        .map(|field| Step::Key(*field)),
                )
                .collect::<Vec<_>>();
            Completion::new(*marker, node(*marker, Value::record([])))
                .with_detail(super::ID)
                .on_commit(crate::selection::pending_at(&path))
        })
        .chain([X, Y, Z].map(|axis| {
            crate::completion::select(Completion::new(axis, node(AXIS, axis.into())))
                .with_detail(super::ID)
        }))
        .collect()
}

fn shape_calls<'a>(request: &'a CompletionRequest<'_>) -> impl Iterator<Item = Completion> + 'a {
    [SPHERE, CIRCLE].into_iter().map(|function| {
        crate::grap::call_completion(function.into(), function, request.resolve)
            .with_detail(super::ID)
    })
}

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    if request.scope != CompletionScope::Suggested {
        None
    } else if request.path.is_empty() {
        Some(match request.kind {
            CompletionKind::Value => vec![super::root_completion()],
            CompletionKind::Field => vec![
                crate::completion::label(FIDGET)
                    .with_aliases(["sdf"])
                    .with_detail(super::ID),
            ],
        })
    } else {
        match (context(request)?, request.kind) {
            (Slot::Expression, CompletionKind::Value) => Some(
                shape_calls(request)
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
            (Slot::Field | Slot::Expression, CompletionKind::Field) => {
                Some(labels(SHAPES).into_iter().chain(labels(&[AXIS])).collect())
            }
            (Slot::Parameters(parameters), CompletionKind::Field) => Some(labels(&parameters)),
            (Slot::Parameters(parameters), CompletionKind::Value) => Some(vec![
                Completion::new(grap_runtime::vocabulary::PARAMS, Value::record([])).on_commit(
                    crate::selection::pending_at(
                        &parameters
                            .first()
                            .map(|id| Step::Key(*id))
                            .into_iter()
                            .collect::<Vec<_>>(),
                    ),
                ),
            ]),
            (Slot::Number, CompletionKind::Value) => Some(f32::completions(request.query)),
            (Slot::Axis, CompletionKind::Value) => Some(
                [X, Y, Z]
                    .into_iter()
                    .map(|axis| {
                        crate::completion::select(Completion::new(axis, axis.into()))
                            .with_detail(super::ID)
                    })
                    .collect(),
            ),
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
            resolve: &|_| None,
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
