use super::*;
use puri::draw::Canvas;
use vello::{
    kurbo::Rect,
    peniko::{Color, ImageAlphaType, ImageFormat},
};

fn image() -> ImageData {
    ImageData {
        data: vec![255; 4].into(),
        width: 1,
        height: 1,
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
    }
}

#[test]
fn images_split_only_nonempty_vector_runs() {
    let mut canvas = SplitCanvas::default();
    canvas.image(image(), Affine::IDENTITY);
    canvas.image(image(), Affine::IDENTITY);
    canvas.fill(
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        Affine::IDENTITY,
    );
    canvas.image(image(), Affine::IDENTITY);
    assert!(matches!(
        canvas.finish().as_slice(),
        [
            Layer::Image(..),
            Layer::Image(..),
            Layer::Vector(_),
            Layer::Image(..)
        ]
    ));
}

#[test]
fn vector_only_clips_remain_one_vector_run() {
    let mut canvas = SplitCanvas::default();
    canvas.clip(Rect::new(0.0, 0.0, 4.0, 4.0), Affine::IDENTITY, |canvas| {
        canvas.fill(
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
    });
    canvas.fill(
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Color::BLACK,
        Affine::IDENTITY,
    );
    assert!(matches!(canvas.finish().as_slice(), [Layer::Vector(_)]));
}

#[test]
fn mixed_clips_preserve_the_scope_and_paint_order() {
    let mut canvas = SplitCanvas::default();
    canvas.clip(Rect::new(0.0, 0.0, 4.0, 4.0), Affine::IDENTITY, |canvas| {
        canvas.fill(
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
        canvas.image(image(), Affine::IDENTITY);
    });
    let layers = canvas.finish();
    assert!(matches!(layers.as_slice(), [Layer::Clip(_, _, children)]
        if matches!(children.as_slice(), [Layer::Vector(_), Layer::Image(..)])));
}

#[test]
fn empty_clip_emits_nothing() {
    let mut canvas = SplitCanvas::default();
    canvas.clip(Rect::ZERO, Affine::IDENTITY, |_| {});
    assert!(canvas.finish().is_empty());
}
