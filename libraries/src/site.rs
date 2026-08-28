//! Per-site access to the editor's annotation trie. GET and SET never
//! take a path: the host merges [`at`] when it has a place (a click,
//! a projection). They are not global functions.

use crate::{Library, absent, name};
use gid::Value;
use grap_runtime::{ForeignFunction, ForeignFunctions};

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0xbba7ddab82fa7d703c90b250785e3273);
    pub const SET: CellId = CellId::from_u128(0xd9329d07bcdc791919f5252846fa152c);
    /// SET stores this (evaluated).
    pub const VALUE: CellId = CellId::from_u128(0x544d3b52ea73cd263c435ecddfe5e8bf);

    pub const FOLD: CellId = CellId::from_u128(0x3fa8d15e60b7c2941d8ea05b47f2c6d3);
    pub const FOLDED: CellId = CellId::from_u128(0x84c07f3b9ad2561e02c6b4d81f7a39e5);
    pub const EXPANDED: CellId = CellId::from_u128(0x1d5b0c47e8f6a923d7405c9128b3fae6);
}

/// Overlay GET/SET closed over one place. `get` is the current value
/// (None means absent); `set` writes (None clears). The editor
/// supplies these; Grap never sees the path.
pub fn at(
    get: impl Fn() -> Option<Value> + 'static,
    set: impl Fn(Option<Value>) + 'static,
) -> ForeignFunctions {
    let get = std::rc::Rc::new(get);
    let set = std::rc::Rc::new(set);
    ForeignFunctions::default()
        .register(
            vocabulary::GET,
            ForeignFunction::new({
                let get = get.clone();
                move |_, _, _| Ok(get().unwrap_or_else(absent::value))
            }),
        )
        .register(
            vocabulary::SET,
            ForeignFunction::new({
                let set = set;
                move |context, call, environment| {
                    let Some(value) = context.field(call, vocabulary::VALUE) else {
                        return Ok(context.missing_argument(vocabulary::VALUE));
                    };
                    let value = context.eval(value, environment)?;
                    set((!absent::is_absent(&value)).then_some(value.clone()));
                    Ok(value)
                }
            }),
        )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "get"),
        (vocabulary::SET, "set"),
        (vocabulary::VALUE, "value"),
        (vocabulary::FOLD, "fold"),
        (vocabulary::FOLDED, "folded"),
        (vocabulary::EXPANDED, "expanded"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library {
        cells,
        ..Library::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control;
    use grap_runtime as grap;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn store() -> (Rc<RefCell<Option<Value>>>, ForeignFunctions) {
        let value = Rc::new(RefCell::new(None));
        let foreign = at(
            {
                let value = value.clone();
                move || value.borrow().clone()
            },
            {
                let value = value.clone();
                move |next| *value.borrow_mut() = next
            },
        );
        (value, foreign)
    }

    fn quote(value: Value) -> Value {
        grap::call(
            Value::from(control::vocabulary::QUOTE),
            [(grap::vocabulary::EXPRESSION, value)],
        )
    }

    #[test]
    fn get_and_set_the_closed_over_value() {
        let (stored, site) = store();
        let foreign = control::functions().merge(site);
        assert!(absent::is_absent(
            &grap::evaluate(
                &grap::call(Value::from(vocabulary::GET), []),
                |_| None,
                &foreign,
                10,
            )
            .result
        ));

        let written = Value::record([(vocabulary::FOLD, Value::from(vocabulary::FOLDED))]);
        let set = grap::evaluate(
            &grap::call(
                Value::from(vocabulary::SET),
                [(vocabulary::VALUE, quote(written.clone()))],
            ),
            |_| None,
            &foreign,
            30,
        );
        assert!(set.diagnostics.is_empty());
        assert_eq!(&*stored.borrow(), &Some(written.clone()));
        assert_eq!(
            grap::evaluate(
                &grap::call(Value::from(vocabulary::GET), []),
                |_| None,
                &foreign,
                10,
            )
            .result,
            written
        );

        let cleared = grap::evaluate(
            &grap::call(
                Value::from(vocabulary::SET),
                [(vocabulary::VALUE, absent::value())],
            ),
            |_| None,
            &foreign,
            10,
        );
        assert!(absent::is_absent(&cleared.result));
        assert!(stored.borrow().is_none());
    }

    #[test]
    fn without_an_overlay_they_are_not_callable() {
        let library = library::<(), ()>();
        assert_eq!(
            grap::evaluate(
                &grap::call(Value::from(vocabulary::GET), []),
                |cell| library.cells.value(cell).cloned(),
                &library.functions,
                10,
            )
            .result,
            grap::absent::value(grap::absent::NOT_CALLABLE)
        );
    }
}
