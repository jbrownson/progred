use super::*;
use crate::draw::{DrawCmd, DrawList};
use kurbo::Rect;
use peniko::Color;

#[test]
fn cached_line_layouts_share_the_shaped_result() {
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    let mut ctx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let style = TextStyle {
        size: 16.0,
        brush: Color::WHITE.into(),
        weight: None,
        family: GenericFamily::SystemUi,
    };

    let first = line_layout(&mut ctx, "shared", &style);
    let second = line_layout(&mut ctx, "shared", &style);

    assert!(Rc::ptr_eq(&first, &second));
}

#[test]
fn subscript_metrics_preserve_the_alignment_baseline_and_cover_the_shaped_line() {
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    for scale in [1.0, 1.5, 2.0] {
        let mut ctx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale,
            cache: &mut cache,
        };
        let style = TextStyle {
            size: 13.0,
            brush: Color::WHITE.into(),
            weight: None,
            family: GenericFamily::SystemUi,
        };
        let normal = scripted_text(&mut ctx, "f32", &style, Script::Normal);
        let subscript = scripted_text(&mut ctx, "f32", &style, Script::Subscript);
        let small = text(
            &mut ctx,
            "f32",
            &TextStyle {
                size: 13.0 * 0.70,
                ..style
            },
        );
        assert!(Rc::ptr_eq(&subscript.layout, &small.layout));
        let metrics = subscript.metrics();
        assert!(metrics.width < normal.metrics().width);
        let offset = f64::from(13.0 * 0.23 * scale);
        assert!((metrics.ascent + offset - small.metrics().ascent).abs() < 1e-6);
        assert!((metrics.descent - offset - small.metrics().descent).abs() < 1e-6);

        let rect = Rect::new(
            0.0,
            100.0 - metrics.ascent,
            metrics.width,
            100.0 + metrics.descent,
        );
        assert!((rect.height() - subscript.layout.height() as f64).abs() < 1e-6);
        let mut recording = DrawList::new();
        subscript.place(&mut recording, Placement::root(rect));
        let run = recording
            .0
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::GlyphRun(run) => Some(run),
                _ => None,
            })
            .unwrap();
        let glyph_baseline =
            run.transform.translation().y + f64::from(run.glyphs.first().unwrap().y);
        assert!((glyph_baseline - (100.0 + offset)).abs() < 1e-6);
    }
}

#[test]
fn text_metrics_support_a_shared_baseline() {
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    let mut ctx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let big = TextStyle {
        size: 28.0,
        brush: Color::WHITE.into(),
        weight: None,
        family: GenericFamily::SystemUi,
    };
    let small = TextStyle {
        size: 12.0,
        brush: Color::WHITE.into(),
        weight: None,
        family: GenericFamily::SystemUi,
    };
    let big = text(&mut ctx, "big", &big);
    let small = text(&mut ctx, "small", &small);

    let mut recording = DrawList::new();
    let big_rect = Rect::new(
        0.0,
        100.0 - big.metrics().ascent,
        big.metrics().width,
        100.0 + big.metrics().descent,
    );
    let small_rect = Rect::new(
        big.metrics().width + 4.0,
        100.0 - small.metrics().ascent,
        big.metrics().width + 4.0 + small.metrics().width,
        100.0 + small.metrics().descent,
    );
    big.place(&mut recording, Placement::root(big_rect));
    small.place(&mut recording, Placement::root(small_rect));

    let baselines: Vec<f64> = recording
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::GlyphRun(run) => {
                Some(run.transform.translation().y + run.glyphs.first()?.y as f64)
            }
            _ => None,
        })
        .collect();
    assert!(baselines.len() >= 2);
    assert!(
        baselines.iter().all(|y| (y - 100.0).abs() < 1.0),
        "baselines: {baselines:?}"
    );
}
