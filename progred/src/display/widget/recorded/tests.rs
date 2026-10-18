use super::*;
use gid::{Resolution, new_cell_id};
use kurbo::Circle;

#[test]
fn recorded_hits_use_paint_order_and_the_current_placement() {
    let back = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
    let front = SourceTrace::Stored(Rc::from([Step::Key(new_cell_id())]));
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
        Some(Hover::Source(front)),
    );
}

#[test]
fn a_source_descendant_does_not_highlight_its_operation() {
    let cell = new_cell_id();
    let call = new_cell_id();
    let argument = new_cell_id();
    let source = SourceTrace::InCell {
        cell,
        resolution: Resolution::Document,
        path: Rc::from([Step::Key(call)]),
    };
    let hovered = SourceTrace::InCell {
        cell,
        resolution: Resolution::Document,
        path: Rc::from([Step::Key(call), Step::Key(argument)]),
    };
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![Hit::new(
            puri::draw::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
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
        resolution: Resolution::Document,
        path: Rc::from([Step::Key(call)]),
    };
    let drawing = Recorded {
        commands: DrawList::new(),
        hits: vec![Hit::new(
            puri::draw::Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
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
