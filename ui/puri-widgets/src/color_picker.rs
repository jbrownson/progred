use puri::{
    Affine, Brush, Circle, Color, ColorStop, Command, Drawing, Gradient, Line, Rect, Shape, Stroke,
};

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

fn drawing(height: f64, commands: Vec<Command<Brush>>) -> Drawing<Brush> {
    Drawing {
        width: WIDTH,
        ascent: height,
        descent: 0.0,
        commands,
    }
}

fn marker(center: (f64, f64), radius: f64) -> Vec<Command<Brush>> {
    let shape = Shape::Circle(Circle::new(center, radius));
    [(2.5, Color::BLACK), (1.25, Color::WHITE)]
        .map(|(width, color)| Command::Stroke {
            shape: shape.clone(),
            style: Stroke::new(width),
            paint: Brush::from(color),
            transform: Affine::IDENTITY,
        })
        .into()
}

pub fn plane(color: Hsva) -> Drawing<Brush> {
    let rect = Shape::Rect(Rect::new(0.0, 0.0, WIDTH, PLANE_HEIGHT));
    let mut commands = vec![
        Command::Fill {
            shape: rect.clone(),
            paint: Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0))
                .with_stops(
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
                )
                .into(),
            transform: Affine::IDENTITY,
        },
        Command::Fill {
            shape: rect,
            paint: Gradient::new_linear((0.0, 0.0), (0.0, PLANE_HEIGHT))
                .with_stops(
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
                )
                .into(),
            transform: Affine::IDENTITY,
        },
    ];
    commands.extend(marker(
        (
            5.0 + color.saturation * (WIDTH - 10.0),
            5.0 + (1.0 - color.value) * (PLANE_HEIGHT - 10.0),
        ),
        5.0,
    ));
    drawing(PLANE_HEIGHT, commands)
}

pub fn hue(color: Hsva) -> Drawing<Brush> {
    let colors: [u32; 7] = [
        0xff0000ff, 0xffff00ff, 0x00ff00ff, 0x00ffffff, 0x0000ffff, 0xff00ffff, 0xff0000ff,
    ];
    let stops = colors
        .into_iter()
        .enumerate()
        .map(|(index, rgba)| ColorStop {
            offset: index as f32 / 6.0,
            color: Color::from_rgba8(
                (rgba >> 24) as u8,
                (rgba >> 16) as u8,
                (rgba >> 8) as u8,
                rgba as u8,
            )
            .into(),
        })
        .collect::<Vec<_>>();
    let mut commands = vec![Command::Fill {
        shape: Shape::Rect(Rect::new(0.0, 0.0, WIDTH, RAIL_HEIGHT)),
        paint: Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0))
            .with_stops(stops.as_slice())
            .into(),
        transform: Affine::IDENTITY,
    }];
    let x = 1.5 + color.hue * (WIDTH - 3.0);
    for (width, brush) in [
        (3.0, Brush::from(Color::BLACK)),
        (1.5, Brush::from(Color::WHITE)),
    ] {
        commands.push(Command::Stroke {
            shape: Shape::Line(Line::new((x, 0.0), (x, RAIL_HEIGHT))),
            style: Stroke::new(width),
            paint: brush,
            transform: Affine::IDENTITY,
        });
    }
    drawing(RAIL_HEIGHT, commands)
}

pub fn alpha(color: Hsva) -> Drawing<Brush> {
    let tile = RAIL_HEIGHT / 2.0;
    let mut commands = (0..2)
        .flat_map(|row| (0..(WIDTH / tile).ceil() as usize).map(move |column| (row, column)))
        .filter(|(row, column)| (row + column) % 2 == 0)
        .map(|(row, column)| Command::Fill {
            shape: Shape::Rect(Rect::new(
                column as f64 * tile,
                row as f64 * tile,
                ((column as f64 + 1.0) * tile).min(WIDTH),
                (row as f64 + 1.0) * tile,
            )),
            paint: Brush::from(Color::from_rgba8(0xc8, 0xc8, 0xc8, 0xff)),
            transform: Affine::IDENTITY,
        })
        .collect::<Vec<_>>();
    commands.push(Command::Fill {
        shape: Shape::Rect(Rect::new(0.0, 0.0, WIDTH, RAIL_HEIGHT)),
        paint: Gradient::new_linear((0.0, 0.0), (WIDTH, 0.0))
            .with_stops(
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
            )
            .into(),
        transform: Affine::IDENTITY,
    });
    let x = 1.5 + color.alpha * (WIDTH - 3.0);
    for (width, brush) in [
        (3.0, Brush::from(Color::BLACK)),
        (1.5, Brush::from(Color::WHITE)),
    ] {
        commands.push(Command::Stroke {
            shape: Shape::Line(Line::new((x, 0.0), (x, RAIL_HEIGHT))),
            style: Stroke::new(width),
            paint: brush,
            transform: Affine::IDENTITY,
        });
    }
    drawing(RAIL_HEIGHT, commands)
}

#[cfg(test)]
mod tests {
    use super::*;

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
