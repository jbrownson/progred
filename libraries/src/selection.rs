//! Selection effects in a caller-supplied document and view. GET reads
//! the payload at the current projection site; SET takes an explicit
//! PATH and VALUE. The host supplies the implementation while dispatching.

use crate::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xbd9a8ecaa53276087806022499c6a61e);

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub const PATH: CellId = CellId::from_u128(0xa1005961125b04b7285b8d5aa54a0566);
    pub const STAGE: CellId = CellId::from_u128(0x6a1fd3082b9c47e5f60d21a8c45e9b37);
    pub const EDGE: CellId = CellId::from_u128(0x2f74c8a1936e05bd4c17e2b98d60a5f4);
    pub const PENDING: CellId = CellId::from_u128(0x91d5e60b3a8f27c4058b39f6d2c471ea);
    pub const LABEL: CellId = CellId::from_u128(0x7be29f4680d1c5a3f2496e07b85d13c2);
    pub use crate::site::vocabulary::VALUE;
}

/// A Grap continuation which opens a value pending beneath its site.
pub fn pending_child(step: gid::Step) -> gid::Value {
    use gid::Value;
    use grap_runtime::{call, lambda};
    lambda(
        [],
        call(
            vocabulary::SET.into(),
            [
                (
                    vocabulary::PATH,
                    call(
                        crate::list::vocabulary::CONCAT.into(),
                        [
                            (
                                crate::number::vocabulary::LEFT,
                                call(crate::site::vocabulary::PATH.into(), []),
                            ),
                            (
                                crate::number::vocabulary::RIGHT,
                                crate::path::value(&[step]),
                            ),
                        ],
                    ),
                ),
                (
                    vocabulary::VALUE,
                    Value::record([(vocabulary::STAGE, vocabulary::PENDING.into())]),
                ),
            ],
        ),
    )
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "selection get"),
        (vocabulary::SET, "selection set"),
        (vocabulary::PATH, "path"),
        (vocabulary::STAGE, "stage"),
        (vocabulary::EDGE, "edge"),
        (vocabulary::PENDING, "pending"),
        (vocabulary::LABEL, "label"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        "selection",
        crate::Definitions::from_parts(cells, Default::default()),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::Value;

    #[test]
    fn the_vocabulary_alone_grants_no_selection_access() {
        let library = library::<(), ()>();
        assert_eq!(
            crate::test_evaluate(
                &grap_runtime::call(Value::from(vocabulary::GET), []),
                |cell| library.value(cell).cloned(),
                &library.functions(),
                10,
            )
            .result,
            grap_runtime::absent::with_detail(
                grap_runtime::absent::NOT_CALLABLE,
                grap_runtime::absent::VALUE,
                vocabulary::GET.into()
            )
        );
    }

    #[test]
    fn a_grap_selection_effect_can_run_with_a_different_interpreter() {
        use std::cell::RefCell;
        let site = vec![gid::Step::Element(
            gid::position::between(None, None).unwrap(),
        )];
        let child = gid::Step::Key(gid::new_cell_id());
        let writes = RefCell::new(Vec::new());
        let interpret = |function,
                         context: &mut grap_runtime::Context,
                         call,
                         environment: &grap_runtime::Environment| {
            if function == crate::site::vocabulary::PATH {
                Ok(crate::path::value(&site))
            } else {
                let path = context.field(call, vocabulary::PATH).unwrap();
                let path = context.eval(path, environment)?;
                let value = context.field(call, vocabulary::VALUE).unwrap();
                let value = context.eval(value, environment)?;
                Ok(context.effect(|| {
                    writes
                        .borrow_mut()
                        .push((crate::path::read(&path).unwrap(), value));
                    Value::record([])
                }))
            }
        };
        let functions = crate::list::library::<(), ()>().functions();
        let overlay = grap_runtime::ForeignOverlay::new(
            &[crate::site::vocabulary::PATH, vocabulary::SET],
            &interpret,
        );
        let result = grap_runtime::apply_scoped(
            &pending_child(child.clone()),
            [],
            |cell| {
                functions
                    .get(cell)
                    .cloned()
                    .map(|function| {
                        (
                            gid::Resolution::Document,
                            grap_runtime::Definition::ForeignFunction(function),
                        )
                    })
                    .into_iter()
                    .collect()
            },
            &overlay,
            100,
        );
        assert_eq!(result.result, Value::record([]));
        assert_eq!(
            writes.into_inner(),
            vec![(
                site.into_iter().chain([child]).collect(),
                Value::record([(vocabulary::STAGE, vocabulary::PENDING.into())])
            )],
        );
    }
}
