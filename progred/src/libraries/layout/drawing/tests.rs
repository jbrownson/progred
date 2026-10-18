use super::*;
use crate::display::widget::recorded::Drawing;
use crate::display::widget::source::SourceTrace;
use crate::display::widget::style;
use crate::library::Libraries;
use crate::location::Location;
use crate::sources::Sources;
use gid::{Cells, Resolution, new_cell_id};
use kurbo::Point;

#[test]
fn declining_after_drawing_discards_the_whole_recording() {
    use crate::libraries::{control, f64};
    use vocabulary as draw;
    let sequence = |values| {
        grap::call(
            control::vocabulary::DO.into(),
            [(control::vocabulary::EXPRESSIONS, Value::list(values))],
        )
    };
    let point = |function: CellId, x, y| {
        grap::call(
            function.into(),
            [(draw::X, f64::value(x)), (draw::Y, f64::value(y))],
        )
    };
    let fill = grap::call(
        draw::FILL.into(),
        [(
            draw::PAINT,
            crate::libraries::color::value(peniko::Color::BLACK),
        )],
    );
    let function = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        function,
        grap::lambda(
            [],
            sequence(vec![
                grap::call(draw::PATH.into(), []),
                point(draw::MOVE_TO, 100.0, 100.0),
                point(draw::LINE_TO, 110.0, 110.0),
                fill.clone(),
                ::grap::absent::decline(),
            ]),
        ),
    );
    let doc = gid::Document { root: None, cells };
    let mut stack = crate::stack::load();
    let library = new_cell_id();
    let mut fallback = Cells::new();
    fallback.set_value(function, grap::lambda([], fill));
    stack.libraries.insert(
        library,
        crate::library::Definitions::from_parts(fallback, Default::default()),
    );
    let drawing = Drawing::of(
        &record(
            sequence(vec![
                grap::call(draw::PATH.into(), []),
                point(draw::MOVE_TO, 0.0, 0.0),
                point(draw::LINE_TO, 10.0, 10.0),
                grap::call(function.into(), []),
            ])
            .into(),
            200,
        ),
        &Sources {
            doc: &doc,
            libraries: &stack.libraries,
        },
        Some(SourceTrace::Stored(Rc::from([]))),
        &style::editor(style::Theme::Light.palette(), 1.0),
    );
    assert!(drawing.is_empty());
}

#[test]
fn recorded_hits_keep_the_executing_library_definition() {
    let function = new_cell_id();
    let library_ids = [new_cell_id(), new_cell_id()];
    let doc = gid::Document {
        root: Some(Value::from(function)),
        cells: Cells::new(),
    };
    let definition = grap::lambda(
        [],
        grap::call(
            Value::from(vocabulary::FILL),
            [
                (
                    vocabulary::SHAPE,
                    crate::libraries::layout::rect(0.0, 0.0, 10.0, 10.0),
                ),
                (
                    vocabulary::PAINT,
                    crate::libraries::color::value(peniko::Color::BLACK),
                ),
            ],
        ),
    );
    for order in [library_ids, [library_ids[1], library_ids[0]]] {
        let libraries = Libraries::from_contributions(order.map(|id| {
            let mut cells = Cells::new();
            cells.set_value(function, definition.clone());
            (
                id,
                crate::libraries::name::named_library::<crate::Editor>(
                    id,
                    "drawing",
                    crate::library::Definitions::from_parts(
                        cells,
                        grap::ForeignFunctions::default(),
                    ),
                ),
            )
        }))
        .0;
        let sources = Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let drawing = Drawing::of(
            &record(grap::call(Value::from(function), []).into(), 100),
            &sources,
            Some(SourceTrace::Stored(Rc::from([]))),
            &style::editor(style::Theme::Light.palette(), 1.0),
        );
        assert_eq!(drawing.hits(), 1);
        let selected = SourceTrace::from_location(
            &sources,
            &Location::root(vec![
                Step::Follow(Resolution::Library(order[0])),
                Step::Key(grap::vocabulary::BODY),
            ]),
        );
        let Some(hover) = drawing.target_at(Point::new(5.0, 5.0)) else {
            panic!("drawing should have a source");
        };
        assert_eq!(
            crate::display::widget::source_link::hover_source::<crate::Editor>(
                &sources,
                &[],
                &hover
            ),
            Some(selected)
        );
        let other = SourceTrace::from_location(
            &sources,
            &Location::root(vec![
                Step::Follow(Resolution::Library(order[1])),
                Step::Key(grap::vocabulary::BODY),
            ]),
        );
        assert!(!drawing.highlights(&other));
    }
}
