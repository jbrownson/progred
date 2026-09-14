use super::*;
use crate::display::test_support::{ResolveForDispatch, with_context};
use crate::display::widget::project::Project;
use crate::libraries::control::vocabulary as c;
use measured::choices::{ChoiceBuild, resolve_choices};
use puri::Placement;

const A: CellId = CellId::from_u128(1);
const B: CellId = CellId::from_u128(2);

struct Host(crate::libraries::Libraries);
impl display::Env for Host {
    fn apply_scoped(
        &self,
        function: &Value,
        args: &[(CellId, Value)],
        scope: Option<&ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation {
        match scope {
            Some(scope) => {
                ::grap::apply_scoped(function, args.iter().cloned(), &self.0, scope, 10_000)
            }
            None => ::grap::apply(function, args.iter().cloned(), &self.0, 10_000),
        }
    }
    fn evaluate(&self, _: &Value) -> (Value, usize) {
        unreachable!()
    }
}

#[derive(Default)]
struct Output(RefCell<Option<Value>>);
impl Project<crate::Editor, crate::frame::Hovered> for Output {
    fn descend(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        _: gid::Step,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        unreachable!()
    }
    fn at(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        _: Vec<gid::Step>,
        _: Value,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
        _: Option<display::Partial<crate::Editor, crate::frame::Hovered>>,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        unreachable!()
    }
    fn transient(
        &self,
        _: &mut puri::text::TextCtx,
        _: &mut ChoiceBuild<widget::HoverPass<crate::Editor, crate::frame::Hovered>>,
        value: Value,
        _: usize,
    ) -> ChoiceLayout<widget::HoverPass<crate::Editor, crate::frame::Hovered>> {
        let height = f64::read(value.as_record().unwrap().get(&HEIGHT).unwrap()).unwrap();
        self.0.replace(Some(value));
        ChoiceLayout::fixed(widget::leaf(
            Extent {
                width: 200.0,
                ascent: height,
                descent: 0.0,
            },
            |_, _| {},
        ))
    }
}

fn quote(value: Value) -> Value {
    ::grap::call(c::QUOTE.into(), [(::grap::vocabulary::EXPRESSION, value)])
}
fn splice(value: Value) -> Value {
    Value::record([(c::UNQUOTE, value)])
}
fn slider(key: CellId, initial: f64, max: f64) -> Value {
    ::grap::call(
        SLIDER.into(),
        [
            (KEY, quote(key.into())),
            (INITIAL, f64::value(initial)),
            (MAXIMUM, f64::value(max)),
        ],
    )
}

#[test]
fn controls_compose_and_the_view_receives_values_and_remaining_height() {
    let host = Host(crate::stack::load().libraries);
    let controls = ::grap::lambda(
        [],
        quote(Value::list([
            splice(slider(A, 0.25, 1.0)),
            splice(slider(B, 5.0, 10.0)),
        ])),
    );
    let view = ::grap::lambda(
        [VALUE, WIDTH, HEIGHT, PARAMETERS],
        quote(Value::record([
            (HEIGHT, splice(HEIGHT.into())),
            (PARAMETERS, splice(PARAMETERS.into())),
        ])),
    );
    let call = ::grap::call(
        WITH_CONTROLS.into(),
        [
            (CONTROLS, controls),
            (VIEW, view),
            (VALUE, Value::record([])),
            (WIDTH, f64::value(200.0)),
            (HEIGHT, f64::value(300.0)),
        ],
    );
    let declaration = ::grap::evaluate(&call, &host.0, 10_000).result;
    let target = |_| unreachable!();
    let input = ProjectionInput {
        env: &host,
        value: Some(&declaration),
        default_projection: display::partial(|_| None),
        scale_factor: 1.0,
        writable: false,
        selection: None,
        pending: None,
        state: None,
        targets: display::ProjectionTargets::new(&target),
    };
    let output = Output::default();
    let mut build = ChoiceBuild::default();
    // The test's ordinary projection interpreter records the viewport arguments.
    let graph = with_context(&output, |context| {
        let libraries = &host.0;
        let inputs = crate::projection::Cx {
            sources: crate::sources::Sources {
                doc: context.inputs.sources.doc,
                libraries,
            },
            ..context.inputs.clone()
        };
        let mut context = widget::Context {
            inputs: &inputs,
            text: context.text,
            project: context.project,
            path: context.path,
            value: context.value,
        };
        display(&input).unwrap().measure(&mut context, &mut build)
    });
    let measured = resolve_choices(build.finish(graph), 200.0, false);
    assert_eq!(measured.extent.height(), 300.0);
    let result = output.0.borrow();
    let result = result.as_ref().unwrap().as_record().unwrap();
    assert!(f64::read(result.get(&HEIGHT).unwrap()).unwrap() < 250.0);
    let values: Vec<_> = result
        .get(&PARAMETERS)
        .unwrap()
        .as_list()
        .unwrap()
        .values()
        .map(|v| f64::read(v).unwrap())
        .collect();
    assert_eq!(values, [0.25, 5.0]);
}

#[test]
fn slider_dispatch_updates_only_its_own_view_state_and_keeps_camera_fields() {
    let mut editor = crate::test_editor(gid::Document {
        root: None,
        cells: Cells::new(),
    });
    let original = editor.model.doc.clone();
    let root = editor.model.workspace.document_root().clone();
    let camera = crate::libraries::fidget::vocabulary::CAMERA;
    crate::editing::annotate(
        &mut editor,
        &root,
        &[],
        Value::record([(camera, Value::record([]))]),
    );
    let control = slider_widget(
        A,
        "First".into(),
        Slider::new(0.0, 1.0, 0.2).unwrap(),
        180.0,
    );
    let measured = with_context(&Output::default(), |context| control(context));
    let height = measured.extent.height();
    let placed = widget::frame::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 200.0, height)),
        &Default::default(),
    );
    let event = puri::handler::PointerButtonEvent {
        button: Some(puri::handler::PointerButton::Primary),
        pointer: puri::handler::PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: puri::handler::PointerType::Mouse,
        },
        state: puri::handler::PointerState {
            position: (100.0, height - 18.0).into(),
            ..Default::default()
        },
    };
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut editor, &event)
    );
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), A),
        Some(0.5)
    );
    editor.advance_gesture(&[Point::new(-100.0, -100.0), Point::new(1000.0, -100.0)]);
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), A),
        Some(1.0)
    );
    assert!(
        editor
            .model
            .workspace
            .document
            .annotations
            .at(&[])
            .unwrap()
            .as_record()
            .unwrap()
            .contains_key(&camera)
    );
    assert_eq!(
        read_state(editor.model.workspace.document.annotations.at(&[]), B),
        None
    );
    assert!(Rc::ptr_eq(&editor.model.doc, &original));
    editor.finish_gesture();
}
