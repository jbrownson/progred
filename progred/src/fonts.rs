//! Font collections: the system's natively, bundled fonts in browsers.

use parley::FontContext;

pub(crate) fn font_context() -> FontContext {
    #[cfg(not(target_arch = "wasm32"))]
    return FontContext::new();
    #[cfg(target_arch = "wasm32")]
    bundled_font_context()
}

#[cfg(any(target_arch = "wasm32", test))]
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
mod font_tests {
    use super::*;
    use parley::LayoutContext;
    use puri::text::{GenericFamily, TextCache, TextCtx, TextStyle, line_layout};

    #[test]
    fn bundled_fonts_shape_grap_symbols_without_system_fallback() {
        let mut fonts = bundled_font_context();
        let mut layouts = LayoutContext::new();
        let mut cache = TextCache::default();
        let mut ctx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        for family in [
            GenericFamily::SystemUi,
            GenericFamily::SansSerif,
            GenericFamily::Monospace,
        ] {
            for text in ["→", "input + 2 → 5", "λ x → x", "…"] {
                let layout = line_layout(
                    &mut ctx,
                    text,
                    &TextStyle {
                        size: 16.0,
                        brush: peniko::color::palette::css::BLACK.into(),
                        weight: None,
                        family,
                    },
                );
                assert!(
                    layout.width() > 0.0,
                    "empty layout for {text:?} in {family:?}"
                );
                assert!(
                    layout.lines().all(|line| line.runs().all(|run| run
                        .clusters()
                        .all(|cluster| cluster.glyphs().all(|glyph| glyph.id != 0)))),
                    "missing glyph in {text:?} in {family:?}"
                );
            }
        }
    }
}
