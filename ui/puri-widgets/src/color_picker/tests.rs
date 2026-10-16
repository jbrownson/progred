use super::*;

#[test]
fn picker_draws_gradients_and_markers_in_local_coordinates() {
    use puri::{Brush, DrawCmd, DrawList, Shape};
    for position in [0.0, 0.37, 1.0] {
        let color = Hsva {
            hue: position,
            saturation: position,
            value: position,
            alpha: position,
        };
        for scale in [1.0, 1.5, 2.0] {
            let transform = Affine::translate((31.0, 47.0)) * Affine::scale(scale);
            let mut canvas = DrawList::new();
            plane(color, &mut canvas, transform);
            assert!(matches!(
                canvas.0.as_slice(),
                [
                    DrawCmd::Fill {
                        brush: Brush::Gradient(_),
                        ..
                    },
                    DrawCmd::Fill {
                        brush: Brush::Gradient(_),
                        ..
                    },
                    DrawCmd::Stroke { .. },
                    DrawCmd::Stroke { .. }
                ]
            ));
            for command in &canvas.0 {
                match command {
                    DrawCmd::Fill {
                        shape: Shape::Rect(rect),
                        transform: actual,
                        ..
                    } => {
                        assert_eq!(*rect, Rect::new(0.0, 0.0, WIDTH, PLANE_HEIGHT));
                        assert_eq!(*actual, transform);
                    }
                    DrawCmd::Stroke {
                        shape: Shape::Circle(marker),
                        transform: actual,
                        ..
                    } => {
                        assert_eq!(
                            *marker,
                            Circle::new(
                                (
                                    5.0 + position * (WIDTH - 10.0),
                                    5.0 + (1.0 - position) * (PLANE_HEIGHT - 10.0)
                                ),
                                5.0
                            )
                        );
                        assert_eq!(*actual, transform);
                    }
                    _ => panic!("plane fills followed by its marker"),
                }
            }
            for draw in [hue, alpha] {
                let mut canvas = DrawList::new();
                draw(color, &mut canvas, transform);
                let (fills, markers) = canvas.0.split_at(canvas.0.len() - 2);
                assert!(matches!(
                    fills.last(),
                    Some(DrawCmd::Fill {
                        brush: Brush::Gradient(_),
                        ..
                    })
                ));
                for command in fills {
                    let DrawCmd::Fill {
                        shape: Shape::Rect(rect),
                        transform: actual,
                        ..
                    } = command
                    else {
                        panic!("rail fills");
                    };
                    assert!(
                        rect.x0 >= 0.0
                            && rect.y0 >= 0.0
                            && rect.x1 <= WIDTH
                            && rect.y1 <= RAIL_HEIGHT
                    );
                    assert_eq!(*actual, transform);
                }
                for command in markers {
                    let DrawCmd::Stroke {
                        shape: Shape::Line(line),
                        transform: actual,
                        ..
                    } = command
                    else {
                        panic!("rail markers");
                    };
                    let x = 1.5 + position * (WIDTH - 3.0);
                    assert_eq!(*line, Line::new((x, 0.0), (x, RAIL_HEIGHT)));
                    assert_eq!(*actual, transform);
                }
            }
        }
    }
}

#[test]
fn primary_colors_round_trip() {
    for color in [
        [0xff, 0x00, 0x00, 0xff],
        [0x00, 0xff, 0x00, 0x80],
        [0x00, 0x00, 0xff, 0x00],
        [0xb4, 0xe0, 0xfe, 0x99],
        [0x00, 0x00, 0x00, 0xff],
        [0xff, 0xff, 0xff, 0xff],
    ] {
        assert_eq!(Hsva::from_rgba8(color).to_rgba8(), color);
    }
}

#[test]
fn plane_and_rails_clamp_their_coordinates() {
    let color = Hsva::from_rgba8([0x80, 0x40, 0x20, 0x80]);
    assert_eq!(color.with_plane(2.0, -1.0).saturation, 1.0);
    assert_eq!(color.with_plane(2.0, -1.0).value, 1.0);
    assert_eq!(color.with_hue(-1.0).hue, 0.0);
    assert_eq!(color.with_alpha(2.0).alpha, 1.0);
}
