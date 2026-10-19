use crate::display::Env;
use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::with_context;
use crate::display::widget;
use crate::display::widget::placed::leaf;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;
use crate::{Editor, frame::Hovered};
use kurbo::Rect;
use measured::Extent;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
};
use uig::Placement;

/// Exercise an offer's actual handler, including its selection effects.
pub fn activate_completion(offer: &crate::display::Completion) -> Editor {
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    world.model.selection = Some(crate::selection::pending_value(&crate::test_root(), vec![]));
    (offer.activate)(&mut world.session());
    world
}

impl crate::display::Completion {
    pub(crate) fn test_value(&self) -> gid::Value {
        activate_completion(self)
            .model
            .doc
            .root
            .clone()
            .expect("offer inserts a value")
    }
}
pub fn paint(layout: &impl Recordable<Editor, Hovered>) -> (widget::Extent, puri::draw::DrawList) {
    let Recorded::Widget(widget) = layout.record() else {
        panic!("expected a native widget");
    };
    let measured = with_context(&crate::display::test_support::NoProject, |context| {
        widget(context)
    });
    let extent = measured.extent;
    let fragment = crate::display::widget::frame::place(
        measured,
        uig::Placement::root(extent.rect_at(kurbo::Point::ZERO)),
        &Default::default(),
    );
    let mut canvas = puri::draw::DrawList::new();
    for render in fragment.bind(Default::default()).renders {
        render(&mut canvas);
    }
    (extent, canvas)
}

pub fn point_update(
    layout: &impl Recordable<Editor, Hovered>,
    point: crate::display::PointEvent,
) -> crate::display::PointUpdate {
    use ui_events::pointer::{PointerButton, PointerButtonEvent};
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
    let mut output = widget::frame::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
        uig::Placement::root(kurbo::Rect::new(0.0, 0.0, 100.0, 100.0)),
    );
    let mut event = PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: crate::display::test_support::mouse(),
        state: Default::default(),
    };
    event.state.position = (50.0, 50.0).into();
    assert!(
        output
            .handler
            .unwrap()
            .dispatch_pointer_down(&mut world, &event)
    );
    world.advance_gesture(&[kurbo::Point::new(point.x * 100.0, point.y * 100.0)]);
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
    let mut output = widget::frame::HoverOutput::default();
    let placement = uig::Placement::root(kurbo::Rect::new(0.0, 0.0, 20.0, 20.0));
    place(
        &mut widget::HoverContext::new(
            widget::HoverInput {
                pointer: Some(placement.rect.center()),
                ..Default::default()
            },
            &mut output,
            &mut Default::default(),
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
    use kurbo::{Affine, Point};
    use puri::draw::{DrawCmd, DrawList};
    use uig::Placement;
    let measured = with_context(&crate::display::test_support::NoProject, |context| {
        widget(context)
    });
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let fragment = crate::display::widget::frame::place(measured, placement, &Default::default());
    let mut canvas = DrawList::new();
    for render in fragment.bind(Default::default()).renders {
        render(&mut canvas);
    }
    let mut expected = DrawList::new();
    puri::delim::draw_stretched(
        delim,
        side,
        14.0,
        0.0,
        0.0,
        widget::style::editor(widget::style::Theme::Light.palette(), 1.0)
            .dim
            .brush,
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
                shape: puri::draw::Shape::Path(shape),
                transform,
                ..
            },
        ],
        [
            DrawCmd::Fill {
                shape: puri::draw::Shape::Path(wanted),
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

/// A projection test's occurrence: the root of the test view, read with no
/// scope, so a site below it hovers and selects the steps it's given.
pub fn test_site() -> crate::editing::Site {
    crate::editing::Site::new(
        crate::test_root(),
        std::rc::Rc::from([]),
        Default::default(),
    )
}

/// `value` projected writably at scale 1 through `env`, with no default
/// projection, selection, pending or state, at [`test_site`].
pub fn projection_input<'a, V>(env: &'a dyn Env, value: &'a V) -> ProjectionInput<'a, V> {
    ProjectionInput {
        env,
        default_projection: crate::display::runtime_partial(|_| None),
        value: Some(value),
        scale_factor: 1.0,
        writable: true,
        selection: None,
        pending: None,
        state: None,
        occurrence: &test_site,
    }
}

/// `projection` of `input`'s value read as a runtime value, as the projection
/// runtime hands values to projections.
pub fn project_runtime(
    projection: impl FnOnce(&ProjectionInput<'_, grap::RuntimeValue>) -> Option<Layout>,
    input: &ProjectionInput<'_>,
) -> Option<Layout> {
    let value = input.value.map(grap::RuntimeValue::from);
    projection(&input.with_value(value.as_ref()))
}

pub fn picked(layout: &impl Recordable<Editor, Hovered>) -> Option<gid::Value> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut output = widget::frame::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
        uig::Placement::root(kurbo::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    world.model.selection = Some(crate::selection::pending_value(&crate::test_root(), vec![]));
    let event = ui_events::pointer::PointerButtonEvent {
        button: Some(ui_events::pointer::PointerButton::Primary),
        pointer: crate::display::test_support::mouse(),
        state: ui_events::pointer::PointerState {
            modifiers: ui_events::keyboard::Modifiers::META
                | ui_events::keyboard::Modifiers::CONTROL,
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
    let mut output = widget::frame::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
        uig::Placement::root(kurbo::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    output.handler?.dispatch_key(
        &mut world,
        &ui_events::keyboard::KeyboardEvent {
            state: ui_events::keyboard::KeyState::Down,
            ..Default::default()
        },
    );
    world.model.workspace.document.annotations.at(&[]).cloned()
}

pub fn target() -> Hovered {
    crate::libraries::test_widgets::hover(vec![])
}

pub fn press(x: f64, pick: bool) -> PointerButtonEvent {
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState {
            position: (x, 5.0).into(),
            modifiers: if pick {
                Modifiers::META | Modifiers::CONTROL
            } else {
                Modifiers::empty()
            },
            ..Default::default()
        },
    }
}

pub fn gesture_place(
    layout: Layout,
    readonly: bool,
) -> crate::display::widget::HoverCallback<crate::Editor, Hovered> {
    let crate::display::recording::Recorded::Before { before, .. } =
        crate::display::recording::record(&layout)
    else {
        panic!("widget wrapper");
    };
    crate::display::test_support::with_empty_view(
        &crate::display::test_support::NoProject,
        |cx, text, project| {
            let mut cx = cx.clone();
            if readonly {
                cx.edits = cx.edits.detached(vec![]);
            }
            before(&mut crate::display::widget::Context {
                toolkit: &crate::projection::Occurrence { cx: &cx, path: &[] },
                project,
                value: None,
                text,
            })
        },
    )
}

pub fn drag_frame(
    place: crate::display::widget::HoverCallback<crate::Editor, Hovered>,
    covered: bool,
) -> crate::projection::test_support::HoverOutput<crate::Editor> {
    let extent = Extent {
        width: 20.0,
        ascent: 0.0,
        descent: 20.0,
    };
    let node =
        crate::display::widget::before_place(leaf(extent, |_, _| {}), move |placement, output| {
            place(output, placement)
        });
    let placement = Placement::new(
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Rect::new(0.0, 0.0, 10.0, 20.0),
    );
    let placed = crate::display::widget::frame::place(
        crate::display::widget::view::in_view(node, crate::test_root()),
        placement,
        &Default::default(),
    );
    if covered {
        placed.over(crate::display::widget::frame::place(
            leaf(extent, |p, placement| p.occlude(placement)),
            placement,
            &Default::default(),
        ))
    } else {
        placed
    }
}
