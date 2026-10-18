use super::*;
use peniko::Color;

fn sample() -> DrawList {
    let mut list = DrawList::new();
    list.fill(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Color::WHITE,
        Affine::IDENTITY,
    );
    list.clip(Rect::new(1.0, 1.0, 9.0, 9.0), Affine::IDENTITY, |list| {
        list.stroke(
            Line::new((0.0, 0.0), (10.0, 10.0)),
            Stroke::new(2.0),
            Color::BLACK,
            Affine::translate((5.0, 0.0)),
        );
    });
    list
}

#[test]
fn recording_keeps_call_order_and_nesting() {
    let list = sample();
    assert!(matches!(
        &list.0[..],
        [
            DrawCmd::Fill {
                shape: Shape::Rect(_),
                ..
            },
            DrawCmd::Clip { children, .. },
        ] if matches!(
            &children[..],
            [DrawCmd::Stroke {
                shape: Shape::Line(_),
                ..
            }]
        )
    ));
}

#[test]
fn recording_keeps_geometry_exactly() {
    let DrawCmd::Fill {
        shape: Shape::Rect(rect),
        transform,
        ..
    } = &sample().0[0]
    else {
        panic!("expected a rect fill");
    };
    assert_eq!(*rect, Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(*transform, Affine::IDENTITY);
}

#[test]
fn replay_reproduces_the_recording() {
    let original = sample();
    let mut replayed = DrawList::new();
    replay(&original, &mut replayed);
    assert_eq!(format!("{original:?}"), format!("{replayed:?}"));
}

#[test]
fn replay_at_places_every_recorded_command() {
    let outer = Affine::translate((20.0, 30.0));
    let mut replayed = DrawList::new();
    replay_at(&sample(), &mut replayed, outer);
    assert!(matches!(
        &replayed.0[..],
        [
            DrawCmd::Fill { transform, .. },
            DrawCmd::Clip {
                transform: clip_transform,
                children,
                ..
            },
        ] if *transform == outer
            && *clip_transform == outer
            && matches!(
                &children[..],
                [DrawCmd::Stroke { transform, .. }]
                    if *transform == outer * Affine::translate((5.0, 0.0))
            )
    ));
}

#[test]
fn images_record_and_replay_with_placement() {
    let image = ImageData {
        data: vec![0, 0, 0, 255].into(),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: 1,
        height: 1,
    };
    let mut original = DrawList::new();
    original.draw_image(image.clone(), Affine::scale(2.0));
    let mut replayed = DrawList::new();
    replay_at(&original, &mut replayed, Affine::translate((20.0, 30.0)));

    assert!(matches!(
        replayed.0.as_slice(),
        [DrawCmd::Image {
            image: replayed_image,
            transform,
        }] if replayed_image == &image
            && *transform == Affine::translate((20.0, 30.0)) * Affine::scale(2.0)
    ));
}

#[test]
fn an_initial_drawing_interprets_through_the_canvas_language() {
    let drawing = Drawing {
        width: 10.0,
        ascent: 8.0,
        descent: 2.0,
        commands: vec![
            Command::Fill {
                shape: Shape::Circle(Circle::new((5.0, 5.0), 4.0)),
                paint: Color::WHITE,
                transform: Affine::IDENTITY,
            },
            Command::Clip {
                shape: Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                transform: Affine::IDENTITY,
                children: vec![Command::Stroke {
                    shape: Shape::Line(Line::new((0.0, 0.0), (10.0, 10.0))),
                    style: Stroke::new(2.0),
                    paint: Color::BLACK,
                    transform: Affine::IDENTITY,
                }],
            },
        ],
    };
    let mut recorded = DrawList::new();
    draw(
        drawing.map_paint(Brush::from),
        &mut recorded,
        Affine::translate((20.0, 30.0)),
    );
    assert!(matches!(
        &recorded.0[..],
        [
            DrawCmd::Fill {
                shape: Shape::Circle(_),
                transform,
                ..
            },
            DrawCmd::Clip { children, .. },
        ] if *transform == Affine::translate((20.0, 30.0))
            && matches!(children.as_slice(), [DrawCmd::Stroke { shape: Shape::Line(_), .. }])
    ));
}

#[test]
fn straight_rgba_swaps_bgra_and_divides_out_premultiplied_alpha() {
    let image = |format, alpha_type| ImageData {
        data: vec![16, 32, 64, 128, 0, 0, 0, 0].into(),
        format,
        alpha_type,
        width: 2,
        height: 1,
    };
    let straight = image(ImageFormat::Rgba8, ImageAlphaType::Alpha);
    assert!(matches!(
        straight_rgba(&straight),
        Some(Cow::Borrowed([16, 32, 64, 128, 0, 0, 0, 0]))
    ));
    let converted =
        |format, alpha_type| straight_rgba(&image(format, alpha_type)).map(Cow::into_owned);
    assert_eq!(
        converted(ImageFormat::Bgra8, ImageAlphaType::Alpha),
        Some(vec![64, 32, 16, 128, 0, 0, 0, 0])
    );
    assert_eq!(
        converted(ImageFormat::Rgba8, ImageAlphaType::AlphaPremultiplied),
        Some(vec![32, 64, 128, 128, 0, 0, 0, 0])
    );
    assert_eq!(
        converted(ImageFormat::Bgra8, ImageAlphaType::AlphaPremultiplied),
        Some(vec![128, 64, 32, 128, 0, 0, 0, 0])
    );
}
