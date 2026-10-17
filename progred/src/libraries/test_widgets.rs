use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::with_context;
use crate::display::widget;
use crate::display::{Env, ProjectionTarget, ProjectionTargets};
use crate::placed::{self, leaf};
use crate::view::{Layout, ProjectionInput};
use crate::{Editor, frame::Hovered};
use kurbo::Rect;
use measured::Extent;
use puri::Placement;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
};

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
pub fn paint(layout: &impl Recordable<Editor, Hovered>) -> (widget::Extent, puri::DrawList) {
    let Recorded::Widget(widget) = layout.record() else {
        panic!("expected a native widget");
    };
    let measured = with_context(&crate::display::test_support::NoProject, |context| {
        widget(context)
    });
    let extent = measured.extent;
    let fragment = crate::display::widget::frame::place(
        measured,
        puri::Placement::root(extent.rect_at(puri::Point::ZERO)),
        &Default::default(),
    );
    let mut canvas = puri::DrawList::new();
    for render in fragment.bind(Default::default()).renders {
        render(&mut canvas);
    }
    (extent, canvas)
}

pub fn point_update(
    layout: &impl Recordable<Editor, Hovered>,
    point: crate::display::PointEvent,
) -> crate::display::PointUpdate {
    use puri::handler::{PointerButton, PointerButtonEvent};
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
    let mut output = widget::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 100.0, 100.0)),
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
    let mut output = widget::HoverOutput::default();
    let placement = puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0));
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
    use puri::{Affine, DrawCmd, DrawList, Placement, Point};
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

/// A target that selects nothing and hovers the projected value, whatever
/// the steps below it.
pub fn unit_target(_: Vec<gid::Step>) -> ProjectionTarget<Hovered> {
    relative_target(Vec::new())
}

/// A target that selects nothing and hovers the steps it's given.
pub fn relative_target(steps: Vec<gid::Step>) -> ProjectionTarget<Hovered> {
    ProjectionTarget {
        select: std::rc::Rc::new(|_| false),
        select_with: std::rc::Rc::new(|_, _| false),
        hover: hover(steps),
    }
}

/// `value` projected writably at scale 1 through `env`, with no default
/// projection, selection, pending or state, and targets from [`unit_target`].
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
        targets: ProjectionTargets::new(&unit_target),
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

/// [`projection_input`], its targets from [`relative_target`].
pub fn relative_projection_input<'a, V>(env: &'a dyn Env, value: &'a V) -> ProjectionInput<'a, V> {
    ProjectionInput {
        targets: ProjectionTargets::new(&relative_target),
        ..projection_input(env, value)
    }
}

pub fn picked(layout: &impl Recordable<Editor, Hovered>) -> Option<gid::Value> {
    let Recorded::Before { before, .. } = layout.record() else {
        return None;
    };
    let place = with_context(&crate::display::test_support::NoProject, |context| {
        before(context)
    });
    let mut output = widget::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
        puri::Placement::root(puri::Rect::new(0.0, 0.0, 20.0, 20.0)),
    );
    let mut world = crate::test_editor(gid::Document {
        root: None,
        cells: gid::Cells::new(),
    });
    world.model.selection = Some(crate::selection::pending_value(&crate::test_root(), vec![]));
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: crate::display::test_support::mouse(),
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
    let mut output = widget::HoverOutput::default();
    place(
        &mut widget::HoverContext::new(Default::default(), &mut output, &mut Default::default()),
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
) -> crate::placed::HoverOutput<crate::Editor> {
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
        placed::in_view(node, crate::test_root()),
        placement,
        &Default::default(),
    );
    if covered {
        measured::Output::over(
            placed,
            crate::display::widget::frame::place(
                leaf(extent, |p, placement| p.occlude(placement)),
                placement,
                &Default::default(),
            ),
        )
    } else {
        placed
    }
}
