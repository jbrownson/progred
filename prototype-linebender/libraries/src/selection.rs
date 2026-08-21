//! Access to the selection at the current projection site. The host
//! overlays these functions only while dispatching an event there;
//! Grap never receives the site's document path.

use crate::{Library, absent, name};
use gid::Value;
use grap_runtime::{ForeignFunction, ForeignFunctions};

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub use crate::site::vocabulary::VALUE;
}

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
            ForeignFunction::new(move |context, call, environment| {
                let Some(value) = context.field(call, vocabulary::VALUE) else {
                    return Ok(context.missing_argument(vocabulary::VALUE));
                };
                let value = context.eval(value, environment)?;
                set((!absent::is_absent(&value)).then_some(value.clone()));
                Ok(value)
            }),
        )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "selection get"),
        (vocabulary::SET, "selection set"),
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
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn the_overlay_reads_and_replaces_only_the_current_payload() {
        let payload = Rc::new(RefCell::new(None));
        let functions = at(
            {
                let payload = payload.clone();
                move || payload.borrow().clone()
            },
            {
                let payload = payload.clone();
                move |next| *payload.borrow_mut() = next
            },
        );
        let next = Value::record([]);
        let result = grap_runtime::evaluate(
            &grap_runtime::call(
                Value::from(vocabulary::SET),
                [(vocabulary::VALUE, next.clone())],
            ),
            |_| None,
            &functions,
            10,
        );
        assert!(result.diagnostics.is_empty());
        assert_eq!(&*payload.borrow(), &Some(next));
    }

    #[test]
    fn the_vocabulary_alone_grants_no_selection_access() {
        let library = library::<(), ()>();
        assert_eq!(
            grap_runtime::evaluate(
                &grap_runtime::call(Value::from(vocabulary::GET), []),
                |cell| library.cells.value(cell).cloned(),
                &library.functions,
                10,
            )
            .result,
            Value::from(grap_runtime::absent::NOT_CALLABLE)
        );
    }
}
