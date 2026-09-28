//! Custom forms obey the same placed navigation policy as structural containers.
use super::*;
use crate::display::widget::navigation::Direction;
use crate::libraries::{control, f64, fidget};
use ::grap::vocabulary::{BODY, EVALUATE, EXPRESSION, FFI, FUNCTION, PARAMS, VALUE};

struct Fixture {
    world: World,
    context: BenchContext,
    before: Path,
    whole: Path,
    after: Path,
}

impl Fixture {
    fn new(value: Value, cells: Cells) -> Self {
        // Force the enclosing list vertical while allowing each form to choose
        // its own presentation at the test's width.
        let root = Value::list(
            [text::value("before"), value, text::value("after")]
                .into_iter()
                .chain((0..32).map(|_| text::value("padding"))),
        );
        let paths: Vec<_> = positions(&root)
            .into_iter()
            .map(|p| vec![Step::Element(p)])
            .collect();
        Self {
            context: BenchContext::new(),
            world: crate::test_editor(Document {
                root: Some(root),
                cells,
            }),
            before: paths[0].clone(),
            whole: paths[1].clone(),
            after: paths[2].clone(),
        }
    }

    fn child(&self, steps: impl IntoIterator<Item = Step>) -> Path {
        self.whole.iter().cloned().chain(steps).collect()
    }

    fn check(&mut self, width: f64, from: &Path, direction: Direction, to: &Path) {
        let document = self.world.model.doc.clone();
        self.world.model.selection = Some(make_selection(from.clone()));
        let (frame, _) = self.context.place(
            &document,
            self.world.model.selection.as_ref(),
            &Annotations::default(),
            width,
            None,
            None,
            None,
        );
        assert!(
            frame
                .handler
                .dispatch(
                    &mut self.world,
                    puri::handler::Event::Navigate(direction),
                    &mut Default::default()
                )
                .handled(),
            "{direction:?} from {from:?}"
        );
        assert_eq!(
            self.world.model.selection.as_ref().unwrap().path(),
            to,
            "{direction:?} from {from:?}"
        );
        assert!(Rc::ptr_eq(&document, &self.world.model.doc));
    }
}

#[test]
fn inline_custom_forms_keep_vertical_siblings_and_a_leading_whole_stop() {
    use Direction::*;
    let number = || f64::value(3.0);
    let cases = [
        (
            "arithmetic",
            ::grap::call(
                f64::vocabulary::SUM.into(),
                [
                    (f64::vocabulary::LEFT, number()),
                    (f64::vocabulary::RIGHT, number()),
                ],
            ),
            vec![Step::Key(f64::vocabulary::LEFT)],
        ),
        (
            "call",
            ::grap::call(
                f64::vocabulary::SIN.into(),
                [(f64::vocabulary::OPERAND, number())],
            ),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "lambda",
            name::record("test", [(PARAMS, Value::list([])), (BODY, number())]),
            vec![Step::Key(name::vocabulary::NAME)],
        ),
        (
            "unnamed lambda",
            ::grap::lambda([], number()),
            vec![Step::Key(PARAMS)],
        ),
        (
            "named value",
            name::record("test", [(VALUE, number())]),
            vec![Step::Key(name::vocabulary::NAME)],
        ),
        (
            "evaluate",
            Value::record([(EVALUATE, number())]),
            vec![Step::Key(EVALUATE)],
        ),
        (
            "quote",
            ::grap::call(control::vocabulary::QUOTE.into(), [(EXPRESSION, number())]),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "unquote",
            Value::record([(control::vocabulary::UNQUOTE, number())]),
            vec![Step::Key(control::vocabulary::UNQUOTE)],
        ),
        (
            "ffi",
            Value::record([(FFI, f64::vocabulary::SIN.into())]),
            vec![Step::Key(FFI)],
        ),
        (
            "do",
            ::grap::call(
                control::vocabulary::DO.into(),
                [(control::vocabulary::EXPRESSIONS, Value::list([]))],
            ),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "all",
            ::grap::call(
                control::vocabulary::ALL.into(),
                [(control::vocabulary::EXPRESSIONS, Value::list([]))],
            ),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "match",
            ::grap::call(
                control::vocabulary::MATCH.into(),
                [
                    (control::vocabulary::VALUE, number()),
                    (control::vocabulary::CASES, Value::list([])),
                ],
            ),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "let",
            ::grap::call(
                control::vocabulary::LET.into(),
                [
                    (control::vocabulary::BINDINGS, Value::list([])),
                    (EXPRESSION, number()),
                ],
            ),
            vec![Step::Key(FUNCTION)],
        ),
        (
            "where",
            ::grap::call(
                control::vocabulary::WHERE.into(),
                [
                    (control::vocabulary::BINDINGS, Value::list([])),
                    (EXPRESSION, number()),
                ],
            ),
            vec![Step::Key(EXPRESSION)],
        ),
        (
            "fidget infix",
            Value::record([(
                fidget::vocabulary::SUM,
                Value::record([
                    (fidget::vocabulary::LEFT, crate::libraries::f32::value(1.0)),
                    (fidget::vocabulary::RIGHT, crate::libraries::f32::value(2.0)),
                ]),
            )]),
            vec![
                Step::Key(fidget::vocabulary::SUM),
                Step::Key(fidget::vocabulary::LEFT),
            ],
        ),
    ];
    for (label, value, first) in cases {
        eprintln!("{label}");
        let mut f = Fixture::new(value, Cells::new());
        let first = f.child(first);
        for (from, direction, to) in [
            (f.before.clone(), Down, f.whole.clone()),
            (f.whole.clone(), Right, first.clone()),
            (first.clone(), Left, f.whole.clone()),
            (first.clone(), Down, f.after.clone()),
            (first, Up, f.before.clone()),
            (f.whole.clone(), Down, f.after.clone()),
            (f.after.clone(), Up, f.whole.clone()),
        ] {
            f.check(1500.0, &from, direction, &to);
        }
    }
}

#[test]
fn call_function_and_arguments_are_one_form_in_both_presentations() {
    use Direction::*;
    for (width, forward, backward) in [(1500.0, Right, Left), (90.0, Down, Up)] {
        let mut f = Fixture::new(
            ::grap::call(
                f64::vocabulary::SIN.into(),
                [(f64::vocabulary::OPERAND, f64::value(3.0))],
            ),
            Cells::new(),
        );
        let function = f.child([Step::Key(FUNCTION)]);
        let argument = f.child([Step::Key(f64::vocabulary::OPERAND)]);
        for (from, direction, to) in [
            (f.whole.clone(), forward, function.clone()),
            (function.clone(), forward, argument.clone()),
            (argument.clone(), backward, function.clone()),
            (function, backward, f.whole.clone()),
            (argument, forward, f.after.clone()),
        ] {
            f.check(width, &from, direction, &to);
        }
    }
}

#[test]
fn parameter_declarations_keep_cell_and_name_stops_inside_the_parameter_list() {
    use Direction::*;
    let parameter = gid::new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(parameter, name::record("parameter", []));
    let params = Value::list([parameter.into()]);
    let position = positions(&params)[0].clone();
    let lambda = name::record("lambda", [(PARAMS, params), (BODY, parameter.into())]);
    let mut f = Fixture::new(lambda, cells);
    let params = f.child([Step::Key(PARAMS)]);
    let cell = f.child([Step::Key(PARAMS), Step::Element(position.clone())]);
    let name = f.child([
        Step::Key(PARAMS),
        Step::Element(position),
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME),
    ]);
    let body = f.child([Step::Key(BODY)]);
    for (from, direction, to) in [
        (params.clone(), Right, cell.clone()),
        (cell.clone(), Right, name.clone()),
        (name.clone(), Left, cell.clone()),
        (cell, Left, params),
        (name, Right, body.clone()),
        (body, Right, f.after.clone()),
    ] {
        f.check(1500.0, &from, direction, &to);
    }
}

#[test]
fn multiline_cell_adds_an_entry_line_before_its_wrapped_function() {
    use Direction::*;
    let cell = gid::new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record(
            "function",
            [(PARAMS, Value::list([])), (BODY, f64::value(3.0))],
        ),
    );
    let mut f = Fixture::new(cell.into(), cells);
    let body = f.child([Step::Follow(gid::Resolution::Document)]);
    let name = f.child([
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME),
    ]);
    for (from, direction, to) in [
        (f.whole.clone(), Right, body.clone()),
        (f.whole.clone(), Down, body.clone()),
        (f.whole.clone(), Up, f.before.clone()),
        (body.clone(), Left, f.whole.clone()),
        (body, Down, name),
    ] {
        f.check(90.0, &from, direction, &to);
    }
}

#[test]
fn nested_control_forms_preserve_each_whole_stop_and_leave_to_the_outer_sibling() {
    use Direction::*;
    use control::vocabulary as c;
    let binding = Value::record([
        (c::BIND, text::value("pattern")),
        (c::VALUE, f64::value(1.0)),
    ]);
    let bindings = Value::list([binding]);
    let position = positions(&bindings)[0].clone();
    let mut f = Fixture::new(
        ::grap::call(
            c::LET.into(),
            [(c::BINDINGS, bindings), (EXPRESSION, f64::value(2.0))],
        ),
        Cells::new(),
    );
    let list = f.child([Step::Key(c::BINDINGS)]);
    let binding = f.child([Step::Key(c::BINDINGS), Step::Element(position.clone())]);
    let pattern = f.child([
        Step::Key(c::BINDINGS),
        Step::Element(position.clone()),
        Step::Key(c::BIND),
    ]);
    let value = f.child([
        Step::Key(c::BINDINGS),
        Step::Element(position),
        Step::Key(c::VALUE),
    ]);
    let body = f.child([Step::Key(EXPRESSION)]);
    for (from, direction, to) in [
        (list, Right, binding.clone()),
        (binding.clone(), Right, pattern.clone()),
        (pattern.clone(), Left, binding),
        (pattern.clone(), Right, value.clone()),
        (value.clone(), Left, pattern.clone()),
        (value, Right, body.clone()),
        (body, Right, f.after.clone()),
        (pattern, Down, f.after.clone()),
    ] {
        f.check(1500.0, &from, direction, &to);
    }
}

#[test]
fn named_color_name_and_spelling_form_one_horizontal_row() {
    use Direction::*;
    let color = crate::libraries::color::value(puri::Color::new([0.7, 0.3, 0.1, 1.0]));
    let color = Value::record(
        color
            .as_record()
            .unwrap()
            .update(name::vocabulary::NAME, text::value("copper")),
    );
    let mut f = Fixture::new(color, Cells::new());
    let name = f.child([Step::Key(name::vocabulary::NAME)]);
    for (from, direction, to) in [
        (name.clone(), Right, f.whole.clone()),
        (f.whole.clone(), Left, name.clone()),
        (name.clone(), Down, f.after.clone()),
        (name, Up, f.before.clone()),
        (f.whole.clone(), Right, f.after.clone()),
    ] {
        f.check(1500.0, &from, direction, &to);
    }
}

#[test]
fn tutorial_slots_use_vertical_flow_including_missing_slots() {
    use Direction::*;
    let first = gid::new_cell_id();
    let missing = gid::new_cell_id();
    let last = gid::new_cell_id();
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (first, text::value("first")),
            (last, f64::value(3.0)),
        ])),
        cells: Cells::new(),
    });
    let projection = crate::web_embed::tutorial_slots(
        Some(&format!("{first},{missing},{last}")),
        world.stack.projection.clone(),
    )
    .unwrap();
    for (from, direction, to) in [
        (vec![], Down, vec![Step::Key(first)]),
        (vec![Step::Key(first)], Down, vec![Step::Key(missing)]),
        (vec![Step::Key(missing)], Down, vec![Step::Key(last)]),
        (vec![Step::Key(last)], Up, vec![Step::Key(missing)]),
        (vec![Step::Key(first)], Up, vec![]),
        (vec![Step::Key(first)], Right, vec![Step::Key(missing)]),
    ] {
        world.model.selection = Some(make_selection(from.clone()));
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        assert!(
            frame
                .resolve_for_dispatch()
                .dispatch(
                    &mut world,
                    puri::handler::Event::Navigate(direction),
                    &mut Default::default()
                )
                .handled(),
            "{direction:?} from {from:?}"
        );
        assert_eq!(world.model.selection.as_ref().unwrap().path(), to);
    }
}
