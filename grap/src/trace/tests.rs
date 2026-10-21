use super::*;
use crate::{ForeignFunction, call, lambda, vocabulary};
use gid::{Value, new_cell_id};

/// A host where `fail` answers an absent for `reason`, and nothing else is
/// defined.
fn failing(fail: CellId, reason: CellId) -> impl Host {
    crate::TestHost(move |cell| {
        (cell == fail)
            .then(|| {
                (
                    Resolution::Document,
                    Definition::foreign(
                        Value::record([]),
                        ForeignFunction::from_value(move |_, _, _| {
                            Ok(crate::absent::value(reason))
                        }),
                    ),
                )
            })
            .into_iter()
            .collect()
    })
}

#[test]
fn answering_finds_the_calls_the_answer_was_made_under() {
    let (fail, reason) = (new_cell_id(), new_cell_id());
    let host = failing(fail, reason);
    // A lambda calling `fail`, called: the absent comes from the inner call.
    let expression = call(lambda([], call(fail.into(), [])), []);
    let inner = SourceOrigin::Input(vec![
        gid::Step::Key(vocabulary::FUNCTION),
        gid::Step::Key(vocabulary::BODY),
    ]);
    let answering = Answering::new(inner.clone(), crate::absent::value(reason));
    let result = crate::evaluate(&expression, &Observed::new(&host, &answering), 100);
    assert_eq!(result.result.to_value(), crate::absent::value(reason));
    assert_eq!(
        answering
            .found()
            .expect("the inner call answered it")
            .origins()
            .cloned()
            .collect::<Vec<_>>(),
        vec![inner, SourceOrigin::Input(vec![])]
    );
}

#[test]
fn answering_finds_nothing_a_call_there_did_not_answer() {
    let (fail, reason) = (new_cell_id(), new_cell_id());
    let host = failing(fail, reason);
    let expression = call(fail.into(), []);
    for answering in [
        Answering::new(
            SourceOrigin::Input(vec![]),
            crate::absent::value(new_cell_id()),
        ),
        Answering::new(
            SourceOrigin::Input(vec![gid::Step::Key(new_cell_id())]),
            crate::absent::value(reason),
        ),
    ] {
        crate::evaluate(&expression, &Observed::new(&host, &answering), 100);
        assert!(answering.found().is_none());
    }
}
