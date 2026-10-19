use super::*;
use gid::{Resolution, new_cell_id};
use grap::SourceOrigin;
use kurbo::Circle;

/// The calls behind a fill made by a call from `origin`, which names its
/// source.
fn calls_at(origin: SourceOrigin) -> SourceCalls {
    let sink = new_cell_id();
    let trace = Rc::new(std::cell::RefCell::new(None));
    let captured = trace.clone();
    let function = grap::ForeignFunction::new(move |context, _, _| {
        *captured.borrow_mut() = context.call_trace();
        Ok(gid::Value::record([]).into())
    });
    let host = crate::test_support::TestHost(move |cell| {
        (cell == sink).then(|| {
            (
                Resolution::Document,
                grap::Definition::foreign(gid::Value::record([]), function.clone()),
            )
        })
    });
    grap::evaluate_runtime_at(&grap::call(sink.into(), []).into(), Some(origin), &host, 10);
    SourceCalls::new(trace.take().expect("the call has its origin"), None)
}

#[test]
fn recorded_hits_use_paint_order_and_the_current_placement() {
    let back = calls_at(SourceOrigin::Stored(vec![Step::Key(new_cell_id())]));
    let front = calls_at(SourceOrigin::Stored(vec![Step::Key(new_cell_id())]));
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![
            Hit::new(
                puri::draw::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                Affine::IDENTITY,
                back,
            ),
            Hit::new(
                puri::draw::Shape::Circle(Circle::new((5.0, 5.0), 4.0)),
                Affine::IDENTITY,
                front.clone(),
            ),
        ],
    };

    assert_eq!(
        drawing.target_at(Point::new(25.0, 35.0), Affine::translate((20.0, 30.0)),),
        Some(Hover::Calls(front)),
    );
}

/// A drawing with one fill, made by a call at `path` in `cell`'s definition.
fn drawn_in(cell: gid::CellId, path: Vec<Step>) -> Recorded {
    Recorded {
        commands: DrawList::new(),
        hits: vec![Hit::new(
            puri::draw::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
            Affine::IDENTITY,
            calls_at(SourceOrigin::Cell {
                cell,
                resolution: Resolution::Document,
                path,
            }),
        )],
    }
}

/// How many fills highlighting `source` in `drawing` paints.
fn highlighted(drawing: &Recorded, source: &SourceTrace) -> usize {
    let mut highlighted = DrawList::new();
    drawing.highlight(
        &mut highlighted,
        Affine::IDENTITY,
        source,
        &Brush::from(peniko::Color::WHITE),
    );
    highlighted.0.len()
}

#[test]
fn a_source_descendant_does_not_highlight_its_operation() {
    let cell = new_cell_id();
    let call = new_cell_id();
    let argument = new_cell_id();
    let hovered = SourceTrace::InCell {
        cell,
        resolution: Resolution::Document,
        path: Rc::from([Step::Key(call), Step::Key(argument)]),
    };
    assert_eq!(
        highlighted(&drawn_in(cell, vec![Step::Key(call)]), &hovered),
        0
    );
}

#[test]
fn a_calls_own_source_highlights_its_operation() {
    let cell = new_cell_id();
    let call = new_cell_id();
    let source = SourceTrace::InCell {
        cell,
        resolution: Resolution::Document,
        path: Rc::from([Step::Key(call)]),
    };
    assert_eq!(
        highlighted(&drawn_in(cell, vec![Step::Key(call)]), &source),
        1
    );
}
