use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::with_context;
use crate::display::widget;
use crate::{Editor, frame::Hovered};
pub fn paint(layout: &impl Recordable<Editor, Hovered>) -> (widget::Extent, puri::DrawList) {
    let Recorded::Widget(widget) = layout.record() else {
        panic!("expected a native widget");
    };
    let measured = with_context(&crate::display::test_support::NoProject, |context| {
        widget(context)
    });
    let extent = measured.extent;
    let fragment = widget::place(
        measured,
        puri::Placement::root(extent.rect_at(puri::Point::ZERO)),
    )
    .run(&Default::default());
    let mut canvas = puri::DrawList::new();
    for render in fragment.renders {
        render(&mut canvas, Default::default());
    }
    (extent, canvas)
}

pub fn point_update(
    layout: &impl Recordable<Editor, Hovered>,
    point: crate::display::PointEvent,
) -> crate::display::PointUpdate {
    use puri::handler::{PointerButton, PointerButtonEvent, PointerInfo, PointerType};
    let Recorded::Before { before, .. } = layout.record() else {
        panic!("expected a point-control wrapper");
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    world.model.selection = Some(crate::selection::Selection::edge(
        &crate::test_root(),
        vec![],
    ));
    let mut output = widget::Fragment::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output),
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    let mut event = PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: Default::default(),
    };
    event.state.position = (50.0, 50.0).into();
    assert!(
        output
            .handler
            .unwrap()
            .dispatch_pointer_down(&mut world, &event)
    );
    world.advance_gesture(&[puri::Point::new(point.x * 100.0, point.y * 100.0)]);
    crate::display::PointUpdate {
        value: world.model.doc.root.clone().expect("contact writes"),
        selection: world
            .model
            .selection
            .as_ref()
            .map(|s| s.payload().clone())
            .filter(|v| v != &crate::selection::payload::edge()),
    }
}

pub fn claim(layout: &impl Recordable<Editor, Hovered>) -> Option<puri::hover::Claim<Hovered>> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut output = widget::Fragment::default();
    let placement = puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0));
    place(
        &mut widget::HoverContext::new(
            widget::HoverInput {
                pointer: Some(placement.rect.center()),
                ..Default::default()
            },
            &mut output,
        ),
        placement,
    );
    output.claim.map(|(_, claim)| claim)
}

pub fn assert_delimiter(
    widget: &widget::Widget<crate::Editor, crate::frame::Hovered>,
    delim: puri::delim::Delim,
    side: puri::delim::Side,
) {
    use puri::{Affine, DrawCmd, DrawList, Placement, Point};
    let measured = with_context(&crate::display::test_support::NoProject, |context| {
        widget(context)
    });
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let fragment = widget::place(measured, placement).run(&Default::default());
    let mut canvas = DrawList::new();
    for render in fragment.renders {
        render(&mut canvas, Default::default());
    }
    let mut expected = DrawList::new();
    puri::delim::draw_stretched(
        delim,
        side,
        14.0,
        0.0,
        0.0,
        widget::style::editor(1.0).dim.brush,
        &mut expected,
        Affine::translate((
            if side == puri::delim::Side::Close {
                2.0
            } else {
                0.0
            },
            0.0,
        )),
    );
    let (
        [
            DrawCmd::Fill {
                shape: puri::Shape::Path(shape),
                transform,
                ..
            },
        ],
        [
            DrawCmd::Fill {
                shape: puri::Shape::Path(wanted),
                transform: wanted_transform,
                ..
            },
        ],
    ) = (&canvas.0[..], &expected.0[..])
    else {
        panic!("one delimiter outline")
    };
    assert_eq!(shape, wanted);
    assert_eq!(transform, wanted_transform);
}

pub fn hover(path: Vec<gid::Step>) -> Hovered {
    Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from(path)))
}

pub fn picked(layout: &impl Recordable<Editor, Hovered>) -> Option<gid::Value> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut output = widget::Fragment::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output),
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    world.model.selection = Some(crate::selection::pending_value(&crate::test_root(), vec![]));
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            modifiers: puri::handler::Modifiers::META | puri::handler::Modifiers::CONTROL,
            ..Default::default()
        },
    };
    output.handler?.dispatch_pointer_down_with(
        &mut world,
        &event,
        &mut widget::frame::DispatchContext::new(None, Some(hover(vec![]))),
    );
    world.model.doc.root.clone()
}

pub fn event_annotation(layout: &impl Recordable<Editor, Hovered>) -> Option<gid::Value> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut output = widget::Fragment::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output),
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    output.handler?.dispatch_key(
        &mut world,
        &puri::handler::KeyboardEvent {
            state: ui_events::keyboard::KeyState::Down,
            ..Default::default()
        },
    );
    world.model.workspace.document.annotations.at(&[]).cloned()
}
