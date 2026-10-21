use crate::display::Env;
use crate::display::recording::{Recordable, Recorded};
use crate::display::test_support::with_context;
use crate::display::test_support::{place_decoration, primary_press};
use crate::display::widget;
use crate::display::widget::placed::leaf;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;
use crate::{Editor, frame::Hovered};
use kurbo::Point;
use kurbo::Rect;
use measured::Extent;
use puri::handler::test_support::DispatchShorthands;
use ui_events::keyboard::Modifiers;
use uig::Placement;

/// Exercise an offer's actual handler, including its selection effects.
pub fn activate_completion(offer: &crate::display::Completion) -> Editor {
    let mut world = pending_root();
    (offer.activate)(&mut world.session());
    world
}

/// An empty document's editor, its missing root a pending hole.
fn pending_root() -> Editor {
    let mut world = crate::test_editor(gid::Document::default());
    world.model.selection = Some(crate::selection::pending_value(vec![]));
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
    let output = place_decoration(
        layout,
        Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0)),
        Default::default(),
    )
    .expect("a point-control wrapper");
    let mut world = crate::test_editor(gid::Document::default());
    world.model.selection = Some(crate::selection::Selection::edge(vec![]));
    assert!(output.handler.unwrap().dispatch_pointer_down(
        &mut world,
        &primary_press(Point::new(50.0, 50.0), Modifiers::empty())
    ));
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
    let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
    place_decoration(
        layout,
        placement,
        widget::HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        },
    )?
    .claim
    .map(|(_, claim)| claim)
}

pub fn assert_delimiter(
    widget: &widget::Widget<crate::Editor, crate::frame::Hovered>,
    delim: puri::delim::Delim,
    side: puri::delim::Side,
) {
    use kurbo::Affine;
    use puri::draw::{DrawCmd, DrawList};
    let (_, canvas) = paint(&Layout::widget(widget.clone()));
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

/// What hovering the value drawn at the document path `path` shows.
pub fn hover(path: Vec<gid::Step>) -> Hovered {
    Hovered::Tree(crate::hover::Hover::Value(
        crate::occurrence::of(&path).into(),
    ))
}

/// A projection test's occurrence: the root of the test view, so a site
/// below it hovers and selects the steps it's given.
pub fn test_site() -> crate::editing::Site {
    crate::editing::Site::new(crate::occurrence::Trail::root())
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
    let output = place_decoration(
        layout,
        Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
        Default::default(),
    )?;
    let mut world = pending_root();
    output.handler?.dispatch_pointer_down_with(
        &mut world,
        &primary_press(Point::ZERO, crate::test_support::command()),
        &mut widget::frame::DispatchContext::new(None, Some(hover(vec![]))),
    );
    world.model.doc.root.clone()
}

pub fn event_annotation(layout: &impl Recordable<Editor, Hovered>) -> Option<gid::Value> {
    let output = place_decoration(
        layout,
        Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
        Default::default(),
    )?;
    let mut world = crate::test_editor(gid::Document::default());
    output.handler?.dispatch_key(
        &mut world,
        &ui_events::keyboard::KeyboardEvent {
            state: ui_events::keyboard::KeyState::Down,
            ..Default::default()
        },
    );
    world.model.workspace.document.annotations.at(&[]).cloned()
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
            let path: &[crate::occurrence::Step] = if readonly {
                &[crate::occurrence::Step::Computed]
            } else {
                &[]
            };
            crate::projection::test_support::at_path(cx, text, project, path, None, |context| {
                before(context)
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
