use super::*;
use parley::LayoutContext;
use parley::style::GenericFamily;
use puri::text::{TextCache, TextCtx, TextStyle, line_layout};

/// Asserts that every family shapes every text with real glyphs.
fn shapes_all(mut fonts: FontContext, texts: &[&str]) {
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
        for text in texts {
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

#[test]
fn bundled_fonts_shape_grap_symbols_without_system_fallback() {
    shapes_all(
        bundled_font_context(),
        &["→", "input + 2 → 5", "λ x → x", "…"],
    );
}

/// Tests lay out text with the bundled fonts, so they match on every
/// machine; this covers the native app's system fonts.
#[test]
fn system_fonts_shape_text() {
    shapes_all(font_context(), &["Progred", "input + 2"]);
}
