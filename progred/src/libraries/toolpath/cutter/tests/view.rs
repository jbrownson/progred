//! The tool's picture, clicked as a person would.

use super::*;
use crate::frame::Hovered;
use crate::hover::Hover;
use gid::{Cells, Document, Step, new_cell_id};
use kurbo::Point;
use std::rc::Rc;

#[test]
fn tool_profile_click_selects_its_stored_or_computed_occurrence() {
    let tool = Tool::ball(0.125, 0.22).unwrap().value();
    let owner = new_cell_id();
    let definition = new_cell_id();
    for evaluated in [false, true] {
        let value = if evaluated {
            Value::record([(::grap::vocabulary::EVALUATE, Value::Cell(definition))])
        } else {
            tool.clone()
        };
        let mut cells = Cells::new();
        cells.set_value(definition, tool.clone());
        let mut driver = crate::driver::Driver::new(
            crate::test_editor(Document {
                root: Some(Value::record([(owner, value)])),
                cells,
            }),
            crate::driver::Driver::VIEWPORT,
        );
        let selected = if evaluated {
            crate::test_examples::result_path(&[Step::Key(owner)])
        } else {
            crate::occurrence::of(&[Step::Key(owner)])
        };
        let profile = driver.rect(&selected);
        // Inside the 110 × 180 profile, clear of both its text and delimiters.
        let pointer = Point::new(profile.x0 + 55.0, profile.y0 + 90.0);
        assert_eq!(
            driver.hover_at(pointer),
            Some(Hovered::Tree(Hover::Value(Rc::from(selected.clone()))))
        );
        let document = driver.editor().model.doc.clone();
        driver.click_at(pointer);
        assert_eq!(driver.selection(), selected);
        assert!(
            Rc::ptr_eq(&driver.editor().model.doc, &document),
            "selection must not edit the tool"
        );
    }
}
