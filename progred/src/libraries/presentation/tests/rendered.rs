//! What a pane renders edits where it came from.

use crate::driver::Driver;
use crate::libraries::{editor::workspace, presentation, text};
use crate::location::Location;
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
fn pane_result(driver: &Driver, rest: &[Step]) -> Vec<Step> {
    driver.element(
        &[
            Step::Key(workspace::vocabulary::PANES),
            Step::Key(workspace::vocabulary::LEFT),
        ],
        0,
        &[
            [Step::Key(presentation::vocabulary::RESULT)].as_slice(),
            rest,
        ]
        .concat(),
    )
}

#[test]
fn a_missing_cell_named_in_an_error_is_defined_from_the_error() {
    let program = new_cell_id();
    let mut driver = rendered_in_a_pane(program, program.into(), Cells::new());
    let hole = pane_result(
        &driver,
        &[
            Step::Key(crate::libraries::absent::vocabulary::CELL),
            Step::Follow(gid::Resolution::Document),
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
fn a_quoted_reference_in_a_result_edits_its_cell() {
    let (program, quoted) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(quoted, text::value("before"));
    let render = ::grap::call(
        crate::libraries::grap::control::vocabulary::QUOTE.into(),
        [(::grap::vocabulary::EXPRESSION, quoted.into())],
    );
    let mut driver = rendered_in_a_pane(program, render, cells);
    let definition = pane_result(&driver, &[Step::Follow(gid::Resolution::Document)]);
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
