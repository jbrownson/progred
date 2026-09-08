use progred_display::recording::{Recordable, Recorded};
// Inspect the description captured by a real placed text handler, not a layout opcode.
use progred_display::{LineEdit, widget};
use puri::text::{FontContext, LayoutContext, TextCache, TextCtx};
use std::{cell::RefCell, rc::Rc};

fn with_context<Hover: Default, R>(
    edit: widget::Edit<()>,
    run: impl FnOnce(&mut widget::Context<'_, '_, (), Hover>) -> R,
) -> R {
    with_interpreter(
        edit,
        Rc::new(|_, _, _| panic!("unexpected Grap interpretation")),
        run,
    )
}

fn with_interpreter<Hover: Default, R>(
    edit: widget::Edit<()>,
    interpret: widget::EventInterpreter<()>,
    run: impl FnOnce(&mut widget::Context<'_, '_, (), Hover>) -> R,
) -> R {
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    run(&mut widget::Context {
        project: &progred_display::test_support::NoProject,
        completion: &|_, _, _| panic!("unexpected completion control"),
        drawing: &|_, _, _| panic!("unexpected drawing control"),
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        styles: &widget::style::editor(1.0),
        event_interpreter: &|| interpret.clone(),
        annotate: &|| panic!("unexpected annotation request"),
        start_gesture: &|| panic!("unexpected gesture startup request"),
        value_edit: &|| panic!("unexpected value edit request"),
        drag_threshold: 3.0,
        command: |_| false,
        site: &|| widget::Site {
            writable: true,
            selected: true,
            editing: None,
            spelling: None,
            initial_text: &|spelling| puri::LineEditState::new(spelling).with_cursor_at_end(),
            target: Hover::default(),
            value: None,
            select: Rc::new(|_| true),
            edit: edit.clone(),
        },
        pick: Rc::new(|_, _| false),
        picking: |_| false,
        same_target: |_, _| false,
        primary_edit: |_| true,
    })
}

pub fn line<Hover: Default + 'static>(layout: &impl Recordable<(), Hover>) -> Option<LineEdit> {
    let Recorded::Widget(widget) = layout.record() else {
        return None;
    };
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let measured = with_context(
        Rc::new(move |_, description, _| {
            output.replace(Some(description.clone()));
            true
        }),
        |context| widget(context),
    );
    let placement = puri::Placement::root(measured.extent.rect_at(puri::Point::ZERO));
    let mut placed = widget::place(measured, placement).run(&Default::default());
    placed
        .handler
        .take()?
        .dispatch_key(&mut (), &puri::handler::KeyboardEvent::default());
    captured.take()
}

pub fn point_update(
    layout: &impl Recordable<(), ()>,
    point: progred_display::PointEvent,
) -> progred_display::PointUpdate {
    use progred_display::widget::gesture::{BeginEdit, ValueEdit};
    use puri::handler::{PointerButton, PointerButtonEvent, PointerInfo, PointerType};
    let Recorded::Before { before, .. } = layout.record() else {
        panic!("expected a point-control wrapper");
    };
    let value = Rc::new(RefCell::new(None));
    let selection = Rc::new(RefCell::new(None));
    let write = value.clone();
    let payload = selection.clone();
    let edit: BeginEdit<()> = Rc::new(move || {
        let write = write.clone();
        let payload = payload.clone();
        ValueEdit {
            select: Rc::new(|_| panic!("point controls do not change the selected site")),
            write: Box::new(move |_, value| {
                write.replace(Some(value));
                true
            }),
            selection: Rc::new(move |_, value| {
                payload.replace(Some(value));
            }),
        }
    });
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    let place = before(&mut widget::Context {
        project: &progred_display::test_support::NoProject,
        completion: &|_, _, _| panic!("unexpected completion control"),
        drawing: &|_, _, _| panic!("unexpected drawing control"),
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        styles: &widget::style::editor(1.0),
        site: &|| panic!("point control does not request text editing"),
        event_interpreter: &|| panic!("native point control does not interpret Grap"),
        annotate: &|| panic!("point control does not change annotations"),
        value_edit: &|| Some(edit.clone()),
        start_gesture: &|| {
            Rc::new(move |world, mut gesture, samples| {
                gesture.advance(world, samples);
                gesture.advance(world, &[puri::Point::new(point.x * 100.0, point.y * 100.0)]);
            })
        },
        drag_threshold: 3.0,
        command: |_| false,
        pick: Rc::new(|_, _| false),
        picking: |_| false,
        same_target: |_, _| false,
        primary_edit: |_| true,
    });
    let mut output = widget::Fragment::default();
    let mut fragment = widget::HoverContext::new(Default::default(), &mut output);
    place(
        &mut fragment,
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
    event.state.position.x = 50.0;
    event.state.position.y = 50.0;
    assert!(output.handler.unwrap().dispatch_pointer_down_with(
        &mut (),
        &event,
        &mut Default::default()
    ));
    progred_display::PointUpdate {
        value: value.take().expect("initial contact writes"),
        selection: selection.take(),
    }
}

pub fn picked(layout: &impl Recordable<(), ()>) -> Option<gid::Value> {
    use puri::handler::{PointerButton, PointerButtonEvent, PointerInfo, PointerType};
    use puri::{Placement, Rect};
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let picked = Rc::new(RefCell::new(None));
    let capture = picked.clone();
    let place = with_context(Rc::new(|_, _, _| false), |context| {
        context.pick = Rc::new(move |_, value| {
            capture.replace(Some(value));
            true
        });
        context.picking = |_| true;
        context.same_target = |_, _| true;
        before(context)
    });
    let mut output = widget::Fragment::default();
    let mut fragment = widget::HoverContext::new(Default::default(), &mut output);
    place(
        &mut fragment,
        Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    output.handler?.dispatch_pointer_down_with(
        &mut (),
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: Default::default(),
        },
        &mut widget::frame::DispatchContext::new(None, Some(())),
    );
    picked.take()
}

pub fn claim<Hover: Default + Clone + PartialEq + 'static>(
    layout: &impl Recordable<(), Hover>,
) -> Option<puri::hover::Claim<Hover>> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(Rc::new(|_, _, _| false), |context| before(context));
    let mut output = widget::Fragment::default();
    let mut fragment = widget::HoverContext::new(Default::default(), &mut output);
    let placement = puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0));
    fragment.input.pointer = Some(placement.rect.center());
    place(&mut fragment, placement);
    output.claim.map(|(_, claim)| claim)
}

pub fn event_handler(layout: &impl Recordable<(), ()>) -> Option<gid::Value> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let captured = Rc::new(RefCell::new(None));
    let capture = captured.clone();
    let interpret: widget::EventInterpreter<()> = Rc::new(move |_, function, _| {
        capture.replace(Some(function.clone()));
        true
    });
    let place = with_interpreter(Rc::new(|_, _, _| false), interpret, |context| {
        before(context)
    });
    let mut output = widget::Fragment::default();
    let mut fragment = widget::HoverContext::new(Default::default(), &mut output);
    place(
        &mut fragment,
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    output
        .handler?
        .dispatch_key(&mut (), &puri::handler::KeyboardEvent::default());
    captured.take()
}

pub fn assert_delimiter<Hover: Default + 'static>(
    widget: &widget::Widget<(), Hover>,
    delim: puri::delim::Delim,
    side: puri::delim::Side,
) {
    use puri::{Affine, DrawCmd, DrawList, Placement, Point};
    let measured = with_context(Rc::new(|_, _, _| false), |context| widget(context));
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let fragment = widget::place(measured, placement).run(&Default::default());
    let mut canvas = DrawList::new();
    for render in fragment.renders {
        render(&mut canvas, Default::default());
    }
    let mut expected = DrawList::new();
    puri::draw::draw(
        puri::delim::stretched(
            delim,
            side,
            14.0,
            0.0,
            0.0,
            widget::style::editor(1.0).dim.brush,
        ),
        &mut expected,
        Affine::translate((
            if side == puri::delim::Side::Close {
                2.0
            } else {
                0.0
            },
            0.0,
        )),
        Clone::clone,
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
