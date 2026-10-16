//! Font collections: the system's natively, bundled fonts in browsers.

use parley::FontContext;

pub(crate) fn font_context() -> FontContext {
    #[cfg(not(target_arch = "wasm32"))]
    return FontContext::new();
    #[cfg(target_arch = "wasm32")]
    bundled_font_context()
}

#[cfg(any(target_arch = "wasm32", test, feature = "control"))]
pub(crate) fn bundled_font_context() -> FontContext {
    use parley::fontique::{Blob, Collection, CollectionOptions};
    use parley::style::GenericFamily;

    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        }),
        source_cache: Default::default(),
    };
    let sans = fonts
        .collection
        .register_fonts(
            Blob::from(include_bytes!("../assets/NotoSans-Regular.ttf").to_vec()),
            None,
        )
        .into_iter()
        .map(|(family, _)| family)
        .collect::<Vec<_>>();
    let mono = fonts
        .collection
        .register_fonts(
            Blob::from(include_bytes!("../assets/NotoSansMono-Regular.ttf").to_vec()),
            None,
        )
        .into_iter()
        .map(|(family, _)| family)
        .collect::<Vec<_>>();
    for family in [GenericFamily::SystemUi, GenericFamily::SansSerif] {
        fonts
            .collection
            .set_generic_families(family, sans.iter().chain(&mono).copied());
    }
    fonts
        .collection
        .set_generic_families(GenericFamily::Monospace, mono.into_iter());
    fonts
}

#[cfg(test)]
mod font_tests;
