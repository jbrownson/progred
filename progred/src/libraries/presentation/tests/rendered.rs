//! What a pane renders edits where it came from.

use crate::driver::Driver;
use crate::libraries::{editor::workspace, presentation, text};
use crate::location::Location;
use crate::occurrence;
use crate::selection::Stage;
use gid::{CellId, Cells, Document, Step, Value, new_cell_id};
use ui_events::keyboard::NamedKey;

/// A document whose left pane renders `render`, beside `program`.
fn rendered_in_a_pane(program: CellId, render: Value, cells: Cells) -> Driver {
    Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([
                (crate::libraries::grap::vocabulary::GRAP, program.into()),
                (
                    workspace::vocabulary::PANES,
                    Value::record([(
                        workspace::vocabulary::LEFT,
                        Value::list([Value::record([(presentation::vocabulary::RENDER, render)])]),
                    )]),
                ),
            ])),
            cells,
        }),
        Driver::VIEWPORT,
    )
}

/// The pane's result, and `rest` within it.
fn pane_result(driver: &Driver, rest: &[occurrence::Step]) -> occurrence::Path {
    let pane = driver.element(
        &[
            Step::Key(workspace::vocabulary::PANES),
            Step::Key(workspace::vocabulary::LEFT),
        ],
        0,
        &[],
    );
    [
        crate::test_examples::pane_root(&pane).as_slice(),
        &[
            occurrence::Step::Key(presentation::vocabulary::RESULT),
            occurrence::Step::Computed,
        ],
        rest,
    ]
    .concat()
}

#[test]
fn a_missing_cell_named_in_an_error_is_defined_from_the_error() {
    let program = new_cell_id();
    let mut driver = rendered_in_a_pane(program, program.into(), Cells::new());
    let hole = pane_result(
        &driver,
        &[
            occurrence::Step::Key(crate::libraries::absent::vocabulary::CELL),
            occurrence::Step::jump(Location::in_cell(
                program,
                gid::Resolution::Document,
                Vec::new(),
            )),
        ],
    );
    driver.click(&hole);
    assert_eq!(driver.selection(), hole);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert_eq!(
        driver.editor().model.selection.as_ref().unwrap().location(),
        Some(Location::in_cell(
            program,
            gid::Resolution::Document,
            Vec::new()
        ))
    );
    driver.type_text("\"drawn");
    driver.press(NamedKey::Enter);
    assert_eq!(
        driver.editor().model.doc.cells.value(program),
        Some(&text::value("drawn"))
    );
    assert!(driver.drawn(&pane_result(&driver, &[])));
    assert!(
        !driver.drawn(&hole),
        "the error gave way to the program's result"
    );
}

#[test]
fn a_missing_argument_is_given_from_the_error() {
    use crate::libraries::grap::numbers;
    let program = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(program, ::grap::call(numbers::f64::MULTIPLY.into(), []));
    let mut driver = rendered_in_a_pane(program, program.into(), cells);
    let argument =
        |label| Location::in_cell(program, gid::Resolution::Document, vec![Step::Key(label)]);
    let hole = |driver: &Driver, label| {
        pane_result(
            driver,
            &[
                occurrence::Step::Key(label),
                occurrence::Step::Key(crate::libraries::absent::vocabulary::CALL),
                occurrence::Step::jump(argument(label)),
            ],
        )
    };
    let left = hole(&driver, numbers::vocabulary::LEFT);
    let in_program = [
        Step::Key(crate::libraries::grap::vocabulary::GRAP),
        Step::Follow(gid::Resolution::Document),
        Step::Key(numbers::vocabulary::LEFT),
    ];
    // The error's hole and the program's are one place, drawn twice.
    assert!(driver.shared(&left).is_some());
    assert_eq!(driver.shared(&left), driver.shared(&in_program));
    assert_ne!(
        driver.shared(&left),
        driver.shared(&hole(&driver, numbers::vocabulary::RIGHT))
    );
    driver.click(&left);
    assert_eq!(
        driver.editor().model.selection.as_ref().unwrap().location(),
        Some(argument(numbers::vocabulary::LEFT))
    );
    driver.type_text("2");
    driver.press(NamedKey::Enter);
    let call = driver.editor().model.doc.cells.value(program).unwrap();
    let arguments = call.as_record().unwrap();
    assert!(
        arguments.contains_key(&numbers::vocabulary::LEFT),
        "{call:?}"
    );
    assert!(!arguments.contains_key(&numbers::vocabulary::RIGHT));
    assert!(
        !driver.drawn(&left),
        "the left argument is no longer missing"
    );
    assert!(driver.drawn(&hole(&driver, numbers::vocabulary::RIGHT)));
}

#[test]
fn an_errors_call_is_drawn_where_the_call_is() {
    use crate::libraries::grap::numbers;
    let program = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        program,
        ::grap::call(
            numbers::f64::MULTIPLY.into(),
            [(numbers::vocabulary::LEFT, text::value("three"))],
        ),
    );
    let driver = rendered_in_a_pane(program, program.into(), cells);
    // The call reads as itself, its parts the program's own.
    let in_error = |key| {
        pane_result(
            &driver,
            &[
                occurrence::Step::Key(crate::libraries::absent::vocabulary::CALL),
                occurrence::Step::jump(Location::in_cell(
                    program,
                    gid::Resolution::Document,
                    Vec::new(),
                )),
                occurrence::Step::Key(key),
            ],
        )
    };
    for key in [::grap::vocabulary::FUNCTION, numbers::vocabulary::LEFT] {
        assert!(driver.drawn(&in_error(key)));
        assert_eq!(
            driver.shared(&in_error(key)),
            driver.shared(&[
                Step::Key(crate::libraries::grap::vocabulary::GRAP),
                Step::Follow(gid::Resolution::Document),
                Step::Key(key),
            ])
        );
    }
}

#[test]
fn an_errors_callers_open_to_the_calls_it_was_made_under() {
    use crate::libraries::absent::vocabulary::CALLERS;
    use crate::libraries::grap::numbers;
    let (program, area, width) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(
        area,
        ::grap::lambda(
            [width],
            ::grap::call(
                numbers::f64::MULTIPLY.into(),
                [(numbers::vocabulary::LEFT, width.into())],
            ),
        ),
    );
    cells.set_value(program, ::grap::call(area.into(), []));
    let mut driver = rendered_in_a_pane(program, program.into(), cells);
    let error = pane_result(&driver, &[]);
    let callers = pane_result(
        &driver,
        &[occurrence::Step::Key(CALLERS), occurrence::Step::Computed],
    );
    assert!(!driver.drawn(&callers), "they start shut, and nothing runs");
    driver.click_at(
        driver.locate(crate::frame::Hovered::Tree(crate::hover::Hover::Toggle(
            occurrence::Trail::of(&error),
        ))),
    );
    assert!(driver.drawn(&callers));
    // The `*` in `area` failed; its one caller is the program's call of
    // `area`, drawn where it is.
    let program_call = occurrence::Step::jump(Location::in_cell(
        program,
        gid::Resolution::Document,
        Vec::new(),
    ));
    let drawn: Vec<_> = driver
        .drawn_under(&callers)
        .into_iter()
        .map(|(path, _)| path.to_vec())
        .filter(|path| path.last() == Some(&program_call))
        .collect();
    assert_eq!(drawn.len(), 1, "{:?}", driver.drawn_under(&callers));
}

#[test]
fn a_quoted_reference_in_a_result_edits_its_cell() {
    let (program, quoted) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(quoted, text::value("before"));
    let render = ::grap::call(
        crate::libraries::grap::control::vocabulary::QUOTE.into(),
        [(::grap::vocabulary::EXPRESSION, quoted.into())],
    );
    let mut driver = rendered_in_a_pane(program, render, cells);
    let definition = pane_result(
        &driver,
        &[occurrence::Step::jump(Location::in_cell(
            quoted,
            gid::Resolution::Document,
            Vec::new(),
        ))],
    );
    driver.double_click(&definition);
    driver.type_text("after");
    assert_eq!(
        driver.editor().model.doc.cells.value(quoted),
        Some(&text::value("after"))
    );
    assert_eq!(
        driver.editor().model.doc.cells.value(program),
        None,
        "nothing else was written"
    );
}
