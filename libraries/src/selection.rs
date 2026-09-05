//! Access to the selection at the current projection site. The host
//! overlays these functions only while dispatching an event there;
//! Grap never receives the site's document path.

use crate::{Library, name};

pub const ID: gid::CellId = gid::CellId::from_u128(0xbd9a8ecaa53276087806022499c6a61e);

pub mod vocabulary {
    use gid::CellId;

    pub const GET: CellId = CellId::from_u128(0x67de640ac4e9859c87668564bc008ea0);
    pub const SET: CellId = CellId::from_u128(0xa12f30681dc2a8465da7673a7e2da9a8);
    pub use crate::site::vocabulary::VALUE;
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = gid::Cells::new();
    for (cell, spelling) in [
        (vocabulary::GET, "selection get"),
        (vocabulary::SET, "selection set"),
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
}
