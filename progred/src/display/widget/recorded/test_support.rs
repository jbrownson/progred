//! What a recording drew, for tests of the programs that draw.

use super::{Faces, Record, Recorded, draw};
use crate::display::widget::source::SourceTrace;
use crate::display::widget::style::Styles;
use crate::hover::Hover;
use kurbo::{Affine, Point};
use peniko::Brush;
use puri::draw::DrawList;

pub(crate) struct Drawing(Recorded);

impl Drawing {
    /// Runs `record` as a leaf would.
    pub(crate) fn of(
        record: &Record,
        host: &dyn grap::Host,
        input: Option<SourceTrace>,
        styles: &Styles,
    ) -> Self {
        Self(draw(record, host, input, Faces::new(styles)))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.commands.0.is_empty() && self.0.hits.is_empty()
    }

    pub(crate) fn hits(&self) -> usize {
        self.0.hits.len()
    }

    pub(crate) fn target_at(&self, point: Point) -> Option<Hover> {
        self.0.target_at(point, Affine::IDENTITY)
    }

    /// Whether highlighting `source` fills anything.
    pub(crate) fn highlights(&self, source: &SourceTrace) -> bool {
        let mut highlighted = DrawList::new();
        self.0.highlight(
            &mut highlighted,
            Affine::IDENTITY,
            source,
            &Brush::default(),
        );
        !highlighted.0.is_empty()
    }
}
