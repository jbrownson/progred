use super::*;
use crate::libraries::Libraries;
use crate::location::Location;
use gid::{Cells, Resolution, new_cell_id};

#[test]
fn declining_after_drawing_discards_the_whole_recording() {
    use crate::libraries::{control, f64};
    use layout_data::vocabulary as draw;
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
                absent::decline(),
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
        crate::libraries::Definitions::from_parts(fallback, Default::default()),
    );
    let drawing = record_program(
        sequence(vec![
            grap::call(draw::PATH.into(), []),
            point(draw::MOVE_TO, 0.0, 0.0),
            point(draw::LINE_TO, 10.0, 10.0),
            grap::call(function.into(), []),
        ]),
        &Sources {
            doc: &doc,
            libraries: &stack.libraries,
        },
        &Faces::new(&crate::styles::editor(
            crate::styles::Theme::Light.palette(),
            1.0,
        )),
        Some(&SourceTrace::Stored(Rc::from([]))),
        200,
    );
    assert!(drawing.commands.0.is_empty());
    assert!(drawing.hits.is_empty());
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
            Value::from(layout_data::vocabulary::FILL),
            [
                (
                    layout_data::vocabulary::SHAPE,
                    layout_data::rect(0.0, 0.0, 10.0, 10.0),
                ),
                (
                    layout_data::vocabulary::PAINT,
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
                crate::libraries::Library::<(), ()>::named(
                    id,
                    "drawing",
                    crate::libraries::Definitions::from_parts(
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
        let drawing = record_program(
            grap::call(Value::from(function), []),
            &sources,
            &Faces::new(&crate::styles::editor(
                crate::styles::Theme::Light.palette(),
                1.0,
            )),
            Some(&SourceTrace::Stored(Rc::from([]))),
            100,
        );
        assert_eq!(drawing.hits.len(), 1);
        let selected = SourceTrace::from_location(
            &sources,
            &Location::root(vec![
                Step::Follow(Resolution::Library(order[0])),
                Step::Key(grap::vocabulary::BODY),
            ]),
        );
        let Some(Hovered::Tree(hover)) = drawing.target_at(Point::new(5.0, 5.0), Affine::IDENTITY)
        else {
            panic!("drawing should have a source");
        };
        assert_eq!(
            super::super::source_link::hover_source::<crate::Editor>(&sources, &[], &hover),
            Some(selected)
        );
        let other = SourceTrace::from_location(
            &sources,
            &Location::root(vec![
                Step::Follow(Resolution::Library(order[1])),
                Step::Key(grap::vocabulary::BODY),
            ]),
        );
        let mut highlight = DrawList::new();
        drawing.highlight(
            &mut highlight,
            Affine::IDENTITY,
            &other,
            &Brush::from(peniko::Color::WHITE),
        );
        assert!(highlight.0.is_empty());
    }
}

#[test]
fn recorded_hits_use_paint_order_and_the_current_placement() {
    let back = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
    let front = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![
            Hit::new(
                puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                Affine::IDENTITY,
                back,
            ),
            Hit::new(
                puri::Shape::Circle(Circle::new((5.0, 5.0), 4.0)),
                Affine::IDENTITY,
                front.clone(),
            ),
        ],
    };

    assert_eq!(
        drawing.target_at(Point::new(25.0, 35.0), Affine::translate((20.0, 30.0)),),
        Some(Hovered::Tree(Hover::Source(front))),
    );
}

#[test]
fn a_source_descendant_does_not_highlight_its_operation() {
    let cell = new_cell_id();
    let call = new_cell_id();
    let argument = new_cell_id();
    let source = SourceTrace::InCell {
        cell,
        source: Resolution::Document,
        path: Rc::from([Step::Key(call)]),
    };
    let hovered = SourceTrace::InCell {
        cell,
        source: Resolution::Document,
        path: Rc::from([Step::Key(call), Step::Key(argument)]),
    };
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![Hit::new(
            puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
            Affine::IDENTITY,
            source,
        )],
    };
    let mut highlighted = DrawList::new();

    drawing.highlight(
        &mut highlighted,
        Affine::IDENTITY,
        &hovered,
        &Brush::from(peniko::Color::WHITE),
    );

    assert!(highlighted.0.is_empty());
}

#[test]
fn an_exact_source_highlights_its_operation() {
    let cell = new_cell_id();
    let call = new_cell_id();
    let source = SourceTrace::InCell {
        cell,
        source: Resolution::Document,
        path: Rc::from([Step::Key(call)]),
    };
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![Hit::new(
            puri::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
            Affine::IDENTITY,
            source.clone(),
        )],
    };
    let mut highlighted = DrawList::new();

    drawing.highlight(
        &mut highlighted,
        Affine::IDENTITY,
        &source,
        &Brush::from(peniko::Color::WHITE),
    );

    assert_eq!(highlighted.0.len(), 1);
}
