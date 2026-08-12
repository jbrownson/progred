//! Ordered partial projections above the total structural fallback.

use crate::display::{
    EditHandler, EditPresentation, Editor, Graphic, GraphicCommand, Language, LineEdit, TextRole,
};
use crate::layout::Extent;
use progred_graph::{CellId, Step, Value};
use puri::draw::Shape;
use vello::kurbo::{Circle, Point, Stroke};
use vello::peniko::{Brush, Color};

pub struct Projected<V> {
    pub view: V,
    pub editor: Option<EditPresentation>,
}

/// A place a projection can begin. Children retain the parent and
/// step rather than arriving pre-resolved, so absence is visible to
/// the projection just like every other graph state.
pub enum Location<'a> {
    Root(Option<&'a Value>),
    Child {
        parent: &'a Value,
        step: Step,
    },
}

impl Location<'_> {
    pub fn value<'a>(
        &'a self,
        resolve: impl Fn(CellId) -> Option<&'a Value>,
    ) -> Option<&'a Value> {
        match self {
            Self::Root(value) => *value,
            Self::Child { parent, step } => match step {
                Step::Key(label) => parent.as_record()?.get(label),
                Step::Element(position) => parent.as_list()?.get(position),
                Step::Follow => resolve(parent.as_cell()?),
            },
        }
    }

    pub fn field(&self) -> Option<CellId> {
        match self {
            Self::Child {
                step: Step::Key(label),
                ..
            } => Some(*label),
            _ => None,
        }
    }
}

type DomainProjection<D> =
    for<'a> fn(&mut D, &'a Value) -> Option<Projected<<D as Language>::View>>;

fn project<D: Language>(display: &mut D, value: &Value) -> Option<Projected<D::View>> {
    let projections: [DomainProjection<D>; 3] = [text, f64, circle];
    projections
        .into_iter()
        .find_map(|projection| projection(display, value))
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

pub fn whole_f64(value: &Value) -> Option<f64> {
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

#[derive(Clone, Copy)]
pub struct Projection<'a> {
    foreign: &'a grap::ForeignFunctions,
    domains: bool,
    evaluation: bool,
}

impl<'a> Projection<'a> {
    pub fn new(foreign: &'a grap::ForeignFunctions) -> Self {
        Self {
            foreign,
            domains: true,
            evaluation: true,
        }
    }

    pub fn raw(self) -> Self {
        Self {
            domains: false,
            evaluation: false,
            ..self
        }
    }

    pub fn without_evaluation(self) -> Self {
        Self {
            evaluation: false,
            ..self
        }
    }

    pub fn try_project<D: Language>(
        self,
        display: &mut D,
        value: &Value,
    ) -> Option<Projected<D::View>> {
        self.domains.then(|| project(display, value)).flatten()
    }

    pub fn try_evaluate(
        self,
        field: CellId,
        expression: &Value,
        resolve: impl Fn(CellId) -> Option<Value>,
    ) -> Option<Value> {
        (self.evaluation && field == crate::conventions::vocabulary::GRAP).then(|| {
            grap::evaluate(expression, resolve, self.foreign, grap::DEFAULT_FUEL).result
        })
    }
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
            Projection::new(&crate::conventions::foreign_functions()).try_project(
                &mut display,
                &number,
            )
            .map(|projected| projected.view),
            Some(View::Text("2.5".to_string()))
        );

        let enriched_number = Value::record(number.as_record().unwrap().clone().update(
            crate::test_values::label("created-at"),
            crate::test_values::text("now"),
        ));
        assert_eq!(grap_f64::read(&enriched_number), Some(2.5));
        assert!(Projection::new(&crate::conventions::foreign_functions()).try_project(
            &mut display,
            &enriched_number,
        )
        .is_none());

        let circle = grap_geometry::value(20.0);
        assert_eq!(
            Projection::new(&crate::conventions::foreign_functions()).try_project(
                &mut display,
                &circle,
            )
            .map(|projected| projected.view),
            Some(View::Circle(20.0))
        );
        let enriched_circle = Value::record(circle.as_record().unwrap().clone().update(
            crate::test_values::label("source"),
            crate::test_values::text("survey"),
        ));
        assert_eq!(grap_geometry::read(&enriched_circle), Some(20.0));
        assert!(Projection::new(&crate::conventions::foreign_functions()).try_project(
            &mut display,
            &enriched_circle,
        )
        .is_none());
    }

    #[test]
    fn a_child_location_looks_up_the_step_when_projected() {
        let child = crate::test_values::label("child");
        let value = Value::record([(child, grap_f64::value(2.5))]);
        let expected = grap_f64::value(2.5);
        assert_eq!(
            Location::Child {
                parent: &value,
                step: Step::Key(child),
            }
            .value(|_| None),
            Some(&expected),
        );
    }

    #[test]
    fn child_lookup_preserves_missing_fields_and_resolves_follows() {
        let missing = crate::test_values::label("missing");
        let record = Value::record([]);
        assert_eq!(
            Location::Child {
                parent: &record,
                step: Step::Key(missing),
            }
            .value(|_| None),
            None,
        );

        let cell = crate::test_values::label("cell");
        let link = Value::from(cell);
        let resolved = crate::test_values::text("resolved");
        assert_eq!(
            Location::Child {
                parent: &link,
                step: Step::Follow,
            }
            .value(|requested| (requested == cell).then_some(&resolved)),
            Some(&resolved),
        );
    }

    #[test]
    fn evaluation_is_an_ordinary_contextual_projection() {
        let foreign = crate::conventions::foreign_functions();
        let expression = grap::call(
            Value::from(grap_f64::vocabulary::ADD),
            [
                (grap_f64::vocabulary::LEFT, grap_f64::value(2.0)),
                (grap_f64::vocabulary::RIGHT, grap_f64::value(3.0)),
            ],
        );
        let projections = Projection::new(&foreign);
        assert_eq!(
            projections.try_evaluate(
                crate::conventions::vocabulary::GRAP,
                &expression,
                |_| None,
            ),
            Some(grap_f64::value(5.0))
        );
        assert_eq!(
            projections.without_evaluation().try_evaluate(
                crate::conventions::vocabulary::GRAP,
                &expression,
                |_| None,
            ),
            None
        );
        assert_eq!(
            projections.try_evaluate(crate::test_values::label("data"), &expression, |_| None),
            None
        );
        assert_eq!(
            projections.raw().try_evaluate(
                crate::conventions::vocabulary::GRAP,
                &expression,
                |_| None,
            ),
            None,
        );
        assert!(projections
            .raw()
            .try_project(&mut TestLanguage, &grap_f64::value(2.5))
            .is_none());
    }
}
