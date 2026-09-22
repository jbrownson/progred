use peniko::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub paper: Color,
    pub panel: Color,
    pub chrome: Color,
    pub ink: Color,
    pub name: Color,
    pub literal: Color,
    pub label: Color,
    pub muted: Color,
    pub delimiter: Color,
    pub border: Color,
    pub library_ground: Color,
    pub accent: Color,
    pub disabled: Color,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl std::str::FromStr for Theme {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err("theme must be 'light' or 'dark'"),
        }
    }
}

impl Theme {
    pub fn palette(self) -> Palette {
        let rgb = Color::from_rgb8;
        match self {
            Self::Light => Palette {
                paper: rgb(255, 255, 255),
                panel: rgb(255, 255, 255),
                chrome: rgb(243, 246, 252),
                ink: rgb(33, 43, 65),
                name: rgb(20, 91, 219),
                literal: rgb(195, 65, 0),
                label: rgb(123, 63, 199),
                muted: rgb(102, 115, 139),
                delimiter: rgb(141, 155, 179),
                border: rgb(199, 209, 226),
                library_ground: Color::from_rgba8(20, 91, 219, 9),
                accent: rgb(0, 122, 255),
                disabled: rgb(135, 144, 163),
            },
            Self::Dark => Palette {
                paper: rgb(21, 27, 44),
                panel: rgb(32, 41, 64),
                chrome: rgb(26, 34, 55),
                ink: rgb(230, 237, 252),
                name: rgb(98, 186, 255),
                literal: rgb(255, 173, 102),
                label: rgb(197, 154, 255),
                muted: rgb(155, 170, 198),
                delimiter: rgb(113, 131, 163),
                border: rgb(65, 81, 113),
                library_ground: Color::from_rgba8(140, 180, 255, 13),
                accent: rgb(120, 176, 255),
                disabled: rgb(117, 133, 162),
            },
        }
    }
}
