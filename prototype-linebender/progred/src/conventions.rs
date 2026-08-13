//! Bootstrap library pack: merged well-known libraries, the `grap`
//! field convention, and compact projections of the constructs this
//! pack covers. Each function checks its own preconditions.

use crate::display::{
    EditHandler, EditPresentation, Editor, Graphic, GraphicCommand, Language, LineEdit, TextRole,
};
use crate::layout::Extent;
use crate::projection::{self, Projected};
use crate::sources::Sources;
use progred_graph::{CellId, Cells, Value};
use puri::draw::Shape;
use vello::kurbo::{Circle, Point, Stroke};
use vello::peniko::{Brush, Color};

pub mod vocabulary {
    use progred_graph::CellId;

    /// Field projection: its value is evaluated by Grap and the result
    /// is recursively projected in a derived, read-only context. This
    /// is not a Grap evaluator form.
    pub const GRAP: CellId = CellId::from_u128(0xac807d20d964e141d44c1b2eb98e5ca9);
}

pub fn library() -> Cells {
    let mut cells = progred_name::library();
    cells.merge(progred_isa::library());
    cells.merge(grap::library());
    cells.merge(grap_absent::library());
    cells.merge(grap_control::library());
    cells.merge(grap_f64::library());
    cells.merge(grap_geometry::library());
    cells.set_value(vocabulary::GRAP, progred_name::record("grap", []));
    cells
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    let mut foreign = grap::ForeignFunctions::new();
    grap_control::install(&mut foreign).expect("control foreign functions are distinct");
    grap_f64::install(&mut foreign).expect("f64 foreign functions are distinct");
    grap_geometry::install(&mut foreign).expect("geometry foreign functions are distinct");
    foreign
}

pub fn name<'a>(sources: &'a Sources, cell: CellId) -> Option<&'a str> {
    sources.value(cell).and_then(progred_name::read)
}

/// Raw shows the uninterpreted value and therefore uses the short id.
pub fn display_name<'a>(sources: &'a Sources, raw: bool, cell: CellId) -> Option<&'a str> {
    (!raw).then(|| name(sources, cell)).flatten()
}

pub fn compact<D: Language>(
    display: &mut D,
    value: &Value,
) -> Option<Projected<D::View>> {
    projection::try_partials([text, f64, circle], display, value)
}

pub fn grap(
    foreign: &grap::ForeignFunctions,
    field: CellId,
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
) -> Option<Value> {
    (field == vocabulary::GRAP).then(|| {
        grap::evaluate(expression, resolve, foreign, grap::DEFAULT_FUEL).result
    })
}

pub fn editor(value: &Value) -> Option<Editor> {
    text_editor(value).or_else(|| f64_editor(value))
}

pub fn editable(value: &Value) -> bool {
    whole_text(value).is_some() || whole_f64(value).is_some()
}

pub fn whole_text(value: &Value) -> Option<&str> {
    let text = progred_text::read(value)?;
    value
        .as_record()?
        .keys()
        .all(|label| *label == progred_text::vocabulary::UTF8)
        .then_some(text)
}

fn whole_f64(value: &Value) -> Option<f64> {
    let number = grap_f64::read(value)?;
    value
        .as_record()?
        .keys()
        .all(|label| *label == grap_f64::vocabulary::F64)
        .then_some(number)
}

fn whole_circle(value: &Value) -> Option<f64> {
    let radius = grap_geometry::read(value)?;
    let fields = value.as_record()?;
    let circle = fields
        .get(&grap_geometry::vocabulary::CIRCLE)?
        .as_record()?;
    let radius_value = circle.get(&grap_geometry::vocabulary::RADIUS)?;
    (fields
        .keys()
        .all(|label| *label == grap_geometry::vocabulary::CIRCLE)
        && circle
            .keys()
            .all(|label| *label == grap_geometry::vocabulary::RADIUS)
        && radius_value
            .as_record()?
            .keys()
            .all(|label| *label == grap_f64::vocabulary::F64))
    .then_some(radius)
}

fn text_value(_: &Value, text: &str) -> Option<Value> {
    Some(progred_text::value(text))
}

fn f64_value(_: &Value, text: &str) -> Option<Value> {
    text.parse::<f64>().ok().map(grap_f64::value)
}

fn text_editor(value: &Value) -> Option<Editor> {
    whole_text(value).map(|text| Editor {
        text: text.to_string(),
        handler: EditHandler::new(text_value),
        presentation: EditPresentation::new(TextRole::String).with_affixes("\"", "\""),
    })
}

fn f64_editor(value: &Value) -> Option<Editor> {
    whole_f64(value).map(|number| Editor {
        text: number.to_string(),
        handler: EditHandler::new(f64_value),
        presentation: EditPresentation::new(TextRole::Number),
    })
}

fn project_editor<D: Language>(display: &mut D, editor: Editor) -> Projected<D::View> {
    let presentation = editor.presentation.clone();
    let view = display.line_edit(LineEdit {
        editor,
        placeholder: None,
    });
    Projected {
        view,
        editor: Some(presentation),
    }
}

fn text<D: Language>(display: &mut D, value: &Value) -> Option<Projected<D::View>> {
    let editor = text_editor(value)?;
    Some(project_editor(display, editor))
}

fn f64<D: Language>(display: &mut D, value: &Value) -> Option<Projected<D::View>> {
    let editor = f64_editor(value)?;
    Some(project_editor(display, editor))
}

fn circle<D: Language>(display: &mut D, value: &Value) -> Option<Projected<D::View>> {
    whole_circle(value).map(|radius| {
        let padding = 4.0;
        let half = radius + padding;
        let shape = Shape::Circle(Circle::new(Point::new(half, half), radius));
        Projected {
            view: display.graphic(Graphic {
                extent: Extent {
                    width: 2.0 * half,
                    ascent: half,
                    descent: half,
                },
                commands: vec![
                    GraphicCommand::Fill {
                        shape: shape.clone(),
                        brush: Brush::from(Color::new([0.0, 0.48, 1.0, 0.10])),
                    },
                    GraphicCommand::Stroke {
                        shape,
                        style: Stroke::new(1.5),
                        brush: Brush::from(Color::new([0.0, 0.36, 0.78, 0.9])),
                    },
                ],
            }),
            editor: None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display;

    #[derive(Debug, PartialEq)]
    enum View {
        Text(String),
        Circle(f64),
        Row,
        Col,
    }

    #[derive(Default)]
    struct TestLanguage;

    impl display::Language for TestLanguage {
        type View = View;

        fn text(&mut self, text: &str, _: TextRole) -> Self::View {
            View::Text(text.to_string())
        }

        fn line_edit(&mut self, edit: display::LineEdit) -> Self::View {
            View::Text(format!(
                "{}{}{}",
                edit.editor.presentation.prefix,
                edit.editor.text,
                edit.editor.presentation.suffix
            ))
        }

        fn graphic(&mut self, graphic: display::Graphic) -> Self::View {
            View::Circle(graphic.extent.width / 2.0 - 4.0)
        }

        fn row(&mut self, _: f64, _: Vec<Self::View>) -> Self::View {
            View::Row
        }

        fn col(&mut self, _: usize, _: f64, _: Vec<Self::View>) -> Self::View {
            View::Col
        }
    }

    #[test]
    fn domain_projections_are_partial_and_open_recognition_stays_visible() {
        let mut display = TestLanguage;
        let number = grap_f64::value(2.5);
        assert_eq!(
            compact(&mut display, &number).map(|projected| projected.view),
            Some(View::Text("2.5".to_string()))
        );

        let enriched_number = Value::record(number.as_record().unwrap().clone().update(
            crate::test_values::label("created-at"),
            crate::test_values::text("now"),
        ));
        assert_eq!(grap_f64::read(&enriched_number), Some(2.5));
        assert!(compact(&mut display, &enriched_number).is_none());

        let circle = grap_geometry::value(20.0);
        assert_eq!(
            compact(&mut display, &circle).map(|projected| projected.view),
            Some(View::Circle(20.0))
        );
        let enriched_circle = Value::record(circle.as_record().unwrap().clone().update(
            crate::test_values::label("source"),
            crate::test_values::text("survey"),
        ));
        assert_eq!(grap_geometry::read(&enriched_circle), Some(20.0));
        assert!(compact(&mut display, &enriched_circle).is_none());
    }

    #[test]
    fn grap_evaluates_only_its_own_field() {
        let foreign = foreign_functions();
        let expression = grap::call(
            Value::from(grap_f64::vocabulary::ADD),
            [
                (grap_f64::vocabulary::LEFT, grap_f64::value(2.0)),
                (grap_f64::vocabulary::RIGHT, grap_f64::value(3.0)),
            ],
        );
        assert_eq!(
            grap(&foreign, vocabulary::GRAP, &expression, |_| None),
            Some(grap_f64::value(5.0))
        );
        assert_eq!(
            grap(&foreign, crate::test_values::label("data"), &expression, |_| None),
            None
        );
    }
}
