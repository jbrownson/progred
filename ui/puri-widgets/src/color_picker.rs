use puri::draw::CanvasSink;
use puri::{Affine, Canvas, Circle, Color, ColorStop, Gradient, Line, Rect, Stroke};

pub const WIDTH: f64 = 192.0;
pub const PLANE_HEIGHT: f64 = 128.0;
pub const RAIL_HEIGHT: f64 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub hue: f64,
    pub saturation: f64,
    pub value: f64,
    pub alpha: f64,
}

impl Hsva {
    pub fn from_rgba8([red, green, blue, alpha]: [u8; 4]) -> Self {
        let [red, green, blue] = [red, green, blue].map(|channel| f64::from(channel) / 255.0);
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let range = maximum - minimum;
        let hue = if range == 0.0 {
            0.0
        } else if maximum == red {
            ((green - blue) / range).rem_euclid(6.0) / 6.0
        } else if maximum == green {
            ((blue - red) / range + 2.0) / 6.0
        } else {
            ((red - green) / range + 4.0) / 6.0
        };
        Self {
            hue,
            saturation: if maximum == 0.0 { 0.0 } else { range / maximum },
            value: maximum,
            alpha: f64::from(alpha) / 255.0,
        }
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        let hue = self.hue.rem_euclid(1.0) * 6.0;
        let chroma = self.value * self.saturation;
        let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
        let (red, green, blue) = match hue as u8 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let minimum = self.value - chroma;
        [red + minimum, green + minimum, blue + minimum, self.alpha]
            .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    pub fn with_plane(self, x: f64, y: f64) -> Self {
        Self {
            saturation: x.clamp(0.0, 1.0),
            value: 1.0 - y.clamp(0.0, 1.0),
            ..self
        }
    }

    pub fn with_hue(self, x: f64) -> Self {
        Self {
            hue: x.clamp(0.0, 1.0),
            ..self
        }
    }

    pub fn with_alpha(self, x: f64) -> Self {
        Self {
            alpha: x.clamp(0.0, 1.0),
            ..self
        }
    }

    pub fn opaque_hue(self) -> Color {
        let [red, green, blue, _] = Self {
            saturation: 1.0,
            value: 1.0,
            alpha: 1.0,
            ..self
        }
        .to_rgba8();
        Color::from_rgba8(red, green, blue, 0xff)
    }

    pub fn opaque_color(self) -> Color {
        let [red, green, blue, _] = self.to_rgba8();
        Color::from_rgba8(red, green, blue, 0xff)
    }
}

pub fn plane(color: Hsva, canvas: &mut dyn CanvasSink, transform: Affine) {
    let rect = Rect::new(0.0, 0.0, WIDTH, PLANE_HEIGHT);
    canvas.fill(
        rect,
        Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0)).with_stops(
            [
                ColorStop {
                    offset: 0.0,
                    color: Color::WHITE.into(),
                },
                ColorStop {
                    offset: 1.0,
                    color: color.opaque_hue().into(),
                },
            ]
            .as_slice(),
        ),
        transform,
    );
    canvas.fill(
        rect,
        Gradient::new_linear((0.0, 0.0), (0.0, PLANE_HEIGHT)).with_stops(
            [
                ColorStop {
                    offset: 0.0,
                    color: Color::TRANSPARENT.into(),
                },
                ColorStop {
                    offset: 1.0,
                    color: Color::BLACK.into(),
                },
            ]
            .as_slice(),
        ),
        transform,
    );
    let marker = Circle::new(
        (
            5.0 + color.saturation * (WIDTH - 10.0),
            5.0 + (1.0 - color.value) * (PLANE_HEIGHT - 10.0),
        ),
        5.0,
    );
    for (width, brush) in [(2.5, Color::BLACK), (1.25, Color::WHITE)] {
        canvas.stroke(marker, Stroke::new(width), brush, transform);
    }
}

pub fn hue(color: Hsva, canvas: &mut dyn CanvasSink, transform: Affine) {
    let colors: [u32; 7] = [
        0xff0000ff, 0xffff00ff, 0x00ff00ff, 0x00ffffff, 0x0000ffff, 0xff00ffff, 0xff0000ff,
    ];
    let stops = std::array::from_fn::<_, 7, _>(|index| {
        let rgba = colors[index];
        ColorStop {
            offset: index as f32 / 6.0,
            color: Color::from_rgba8(
                (rgba >> 24) as u8,
                (rgba >> 16) as u8,
                (rgba >> 8) as u8,
                rgba as u8,
            )
            .into(),
        }
    });
    canvas.fill(
        Rect::new(0.0, 0.0, WIDTH, RAIL_HEIGHT),
        Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0)).with_stops(stops.as_slice()),
        transform,
    );
    rail_marker(color.hue, canvas, transform);
}

pub fn alpha(color: Hsva, canvas: &mut dyn CanvasSink, transform: Affine) {
    let tile = RAIL_HEIGHT / 2.0;
    for (row, column) in (0..2)
        .flat_map(|row| (0..(WIDTH / tile).ceil() as usize).map(move |column| (row, column)))
        .filter(|(row, column)| (row + column) % 2 == 0)
    {
        canvas.fill(
            Rect::new(
                column as f64 * tile,
                row as f64 * tile,
                ((column as f64 + 1.0) * tile).min(WIDTH),
                (row as f64 + 1.0) * tile,
            ),
            Color::from_rgba8(0xc8, 0xc8, 0xc8, 0xff),
            transform,
        );
    }
    canvas.fill(
        Rect::new(0.0, 0.0, WIDTH, RAIL_HEIGHT),
        Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0)).with_stops(
            [
                ColorStop {
                    offset: 0.0,
                    color: Color::TRANSPARENT.into(),
                },
                ColorStop {
                    offset: 1.0,
                    color: color.opaque_color().into(),
                },
            ]
            .as_slice(),
        ),
        transform,
    );
    rail_marker(color.alpha, canvas, transform);
}

fn rail_marker(position: f64, canvas: &mut dyn CanvasSink, transform: Affine) {
    let x = 1.5 + position * (WIDTH - 3.0);
    for (width, brush) in [(3.0, Color::BLACK), (1.5, Color::WHITE)] {
        canvas.stroke(
            Line::new((x, 0.0), (x, RAIL_HEIGHT)),
            Stroke::new(width),
            brush,
            transform,
        );
    }
}

#[cfg(test)]
mod tests {
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
}
