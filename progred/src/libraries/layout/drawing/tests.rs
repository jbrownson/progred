use super::*;
use crate::display::widget::recorded::test_support::Drawing;
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
            crate::libraries::color::tests::value(peniko::Color::BLACK),
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
                    super::super::test_support::rect(0.0, 0.0, 10.0, 10.0),
                ),
                (
                    vocabulary::PAINT,
                    crate::libraries::color::tests::value(peniko::Color::BLACK),
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
                crate::test_support::library(
                    id,
                    "drawing",
                    cells,
                    grap::ForeignFunctions::default(),
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

#[test]
fn a_nested_stored_drawing_links_its_fills_to_its_stored_program() {
    use crate::libraries::{layout::test_support as stored, workspace};
    let program = grap::call(
        vocabulary::FILL.into(),
        [
            (vocabulary::SHAPE, stored::circle(20.0, 20.0, 10.0)),
            (
                vocabulary::PAINT,
                crate::libraries::color::tests::value(peniko::Color::BLACK),
            ),
        ],
    );
    let picture = stored::row(
        4.0,
        [
            stored::text_leaf("dot", vocabulary::NAME_FACE),
            stored::program_drawing(40.0, 0.0, 40.0, 100, program.clone()),
        ],
    );
    // A pane draws the picture; the document, read raw with its panes
    // unfolded, draws the picture's source.
    let mut editor = crate::editor_of(Value::record([(
        workspace::vocabulary::PANES,
        Value::record([(workspace::vocabulary::LEFT, Value::list([picture]))]),
    )]));
    editor.model.workspace.document.projection = crate::workspace::Projection::Raw;
    crate::annotations::set_collapsed(
        &mut editor.model.workspace.document.annotations,
        &[Step::Key(workspace::vocabulary::PANES)],
        false,
        false,
    );
    let mut driver = crate::driver::Driver::new(editor, crate::driver::Driver::VIEWPORT);
    let pane = driver.element(
        &[
            Step::Key(workspace::vocabulary::PANES),
            Step::Key(workspace::vocabulary::LEFT),
        ],
        0,
        &[],
    );
    let stored_program = driver.element(
        &[
            pane.as_slice(),
            &[Step::Key(vocabulary::ROW), Step::Key(vocabulary::CHILDREN)],
        ]
        .concat(),
        1,
        &[
            Step::Key(vocabulary::DRAWING),
            Step::Key(vocabulary::PROGRAM),
        ],
    );
    assert_eq!(driver.value(&stored_program), program);
    let [(dot, _, _)] = driver.circles().try_into().expect("the drawing's one dot");
    driver.pick_at(dot);
    assert_eq!(driver.selection(), stored_program);
}
