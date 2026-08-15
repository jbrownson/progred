//! Named faces the walk picks from. The editor fills them in.

use puri::edit::{EditStyle, LineEditPresentation};
use puri::text::TextStyle;

pub struct Styles {
    pub label: TextStyle,
    pub name: TextStyle,
    pub string: TextStyle,
    pub dim: TextStyle,
    pub id: TextStyle,
    pub edit: EditStyle,
    pub scale: f64,
}

impl Styles {
    pub fn line_presentation(&self, prefix: &str, suffix: &str) -> LineEditPresentation {
        LineEditPresentation::new(self.string.size, self.string.brush.clone())
            .with_affixes(prefix, suffix)
    }
}
