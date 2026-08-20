//! Apply a Grap callable at a document path with GET/SET closed over
//! that place. The path stays in Rust.

use crate::annotations::Annotations;
use crate::sources::Sources;
use crate::App;
use gid::{Path, Value};
use progred_libraries::site;
use std::cell::RefCell;
use std::rc::Rc;

pub fn apply(app: &mut App, path: Path, function: Value) -> grap::Evaluation {
    let mut annotations = std::mem::take(&mut app.model.annotations);
    let evaluation = apply_at(
        &path,
        &mut annotations,
        &function,
        &Sources {
            doc: &app.model.doc,
            library: &app.stack.library,
        },
        &app.stack.foreign,
    );
    app.model.annotations = annotations;
    evaluation
}

pub fn apply_at(
    path: &[gid::Step],
    annotations: &mut Annotations,
    function: &Value,
    sources: &Sources<'_>,
    foreign: &grap::ForeignFunctions,
) -> grap::Evaluation {
    let store = Rc::new(RefCell::new(std::mem::take(annotations)));
    let evaluation = {
        let site = site::at(
            {
                let store = store.clone();
                let path = path.to_vec();
                move || store.borrow().at(&path).cloned()
            },
            {
                let store = store.clone();
                let path = path.to_vec();
                move |value| store.borrow_mut().set(&path, value)
            },
        );
        grap::apply(
            function,
            [],
            |cell| sources.value(cell).cloned(),
            &foreign.clone().merge(site),
            grap::DEFAULT_FUEL,
        )
    };
    *annotations = match Rc::try_unwrap(store) {
        Ok(store) => store.into_inner(),
        Err(_) => panic!("site overlay dropped"),
    };
    evaluation
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations;
    use gid::{Document, Step};
    use progred_libraries::control;

    fn quote(value: Value) -> Value {
        grap::call(
            Value::from(control::vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, value)],
        )
    }

    #[test]
    fn a_click_writes_the_closed_over_path_and_not_another() {
        let stack = crate::stack::load::<()>();
        let doc = Document {
            root: None,
            cells: gid::Cells::new(),
        };
        let sources = Sources {
            doc: &doc,
            library: &stack.library,
        };
        let mut annotations = Annotations::default();
        let here = vec![Step::Follow];
        let elsewhere = Vec::new();
        let function = grap::lambda(
            [],
            grap::call(
                Value::from(site::vocabulary::SET),
                [(
                    site::vocabulary::VALUE,
                    quote(Value::record([(
                        site::vocabulary::FOLD,
                        Value::from(site::vocabulary::FOLDED),
                    )])),
                )],
            ),
        );
        let evaluation = apply_at(
            &here,
            &mut annotations,
            &function,
            &sources,
            &stack.foreign,
        );
        assert!(evaluation.diagnostics.is_empty());
        assert_eq!(
            annotations.field(&here, annotations::FOLD),
            Some(&Value::from(annotations::FOLDED))
        );
        assert!(annotations.at(&elsewhere).is_none());
    }
}
