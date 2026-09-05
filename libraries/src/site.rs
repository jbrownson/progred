//! Per-site access to the editor's annotation trie. GET and SET never
//! take a path: the host supplies them while dispatching an event
//! at a projection site. PATH exposes that site's document path as data.
//! The library contributes only their vocabulary.

use crate::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xab8d8d4b75ef3526d258d2689f95daba);

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0xbba7ddab82fa7d703c90b250785e3273);
    pub const SET: CellId = CellId::from_u128(0xd9329d07bcdc791919f5252846fa152c);
    /// SET stores this (evaluated).
    pub const VALUE: CellId = CellId::from_u128(0x544d3b52ea73cd263c435ecddfe5e8bf);
    pub const PATH: CellId = CellId::from_u128(0x803e2b0d0c621eb9688569d0879c3b23);

    pub const FOLD: CellId = CellId::from_u128(0x3fa8d15e60b7c2941d8ea05b47f2c6d3);
    pub const FOLDED: CellId = CellId::from_u128(0x84c07f3b9ad2561e02c6b4d81f7a39e5);
    pub const EXPANDED: CellId = CellId::from_u128(0x1d5b0c47e8f6a923d7405c9128b3fae6);
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "get"),
        (vocabulary::SET, "set"),
        (vocabulary::PATH, "site path"),
        (vocabulary::VALUE, "value"),
        (vocabulary::FOLD, "fold"),
        (vocabulary::FOLDED, "folded"),
        (vocabulary::EXPANDED, "expanded"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        "site",
        crate::Definitions::from_parts(cells, Default::default()),
        vec![],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::Value;
    use grap_runtime as grap;

    #[test]
    fn without_an_overlay_they_are_not_callable() {
        let library = library::<(), ()>();
        assert_eq!(
            crate::test_evaluate(
                &grap::call(Value::from(vocabulary::GET), []),
                |cell| library.value(cell).cloned(),
                &library.functions(),
                10,
            )
            .result,
            grap::absent::with_detail(
                grap::absent::NOT_CALLABLE,
                grap::absent::VALUE,
                vocabulary::GET.into()
            )
        );
    }
}
