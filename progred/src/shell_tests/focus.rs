//! The window's focus and appearance, as the shell takes them in.

use crate::driver::Driver;
use crate::libraries::{f64, text};
use crate::selection::Stage;
use gid::{Cells, Document, Value, new_cell_id};
use kurbo::Size;
use peniko::{Brush, Color};
use puri::draw::DrawCmd;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

const VIEWPORT: Size = Size::new(900.0, 600.0);

fn driver(root: Option<Value>, cells: Cells) -> Driver {
    Driver::new(crate::test_editor(Document { root, cells }), VIEWPORT)
}

fn selection_marks(commands: &[DrawCmd], alpha: f32) -> usize {
    let color = Brush::from(
        crate::styles::Theme::Light
            .palette()
            .accent
            .with_alpha(alpha),
    );
    crate::display::test_support::painted(commands)
        .filter(|command| matches!(command, DrawCmd::Fill { brush, .. } if *brush == color))
        .count()
}

#[test]
fn changing_palette_repaints_without_losing_edits_selection_or_history() {
    let mut driver = driver(Some(text::value("hello")), Cells::new());
    driver.select(&[]);
    driver.type_text("!");
    let document = driver.editor().model.doc.clone();
    fn has_ink(commands: &[DrawCmd], color: Color) -> bool {
        crate::display::test_support::painted(commands).any(
            |command| matches!(command, DrawCmd::GlyphRun(run) if run.brush == Brush::from(color)),
        )
    }
    for theme in [crate::styles::Theme::Dark, crate::styles::Theme::Light] {
        assert!(driver.palette(theme.palette()));
        assert!(!driver.palette(theme.palette()));
        assert!(has_ink(&driver.painted().0, theme.palette().literal));
        assert!(Rc::ptr_eq(&document, &driver.editor().model.doc));
        assert_eq!(driver.line(), Some("hello!"));
    }
    assert!(driver.keystroke(Key::Character("z".into()), crate::modifiers::command()));
    assert_eq!(text::read(&driver.value(&[])), Some("hello"));
}

#[cfg(target_os = "macos")]
#[test]
fn native_appearance_follows_system_until_overridden_and_can_follow_again() {
    use winit::window::Theme::{Dark, Light};
    let mut driver = driver(Some(text::value("hello")), Cells::new());
    let document = driver.editor().model.doc.clone();
    for (preference, observed, expected) in [
        (None, Some(Dark), crate::styles::Theme::Dark),
        (None, Some(Light), crate::styles::Theme::Light),
        (Some(Dark), Some(Light), crate::styles::Theme::Dark),
        (Some(Light), Some(Dark), crate::styles::Theme::Light),
        (None, Some(Dark), crate::styles::Theme::Dark),
        (None, None, crate::styles::Theme::Light),
    ] {
        driver.palette(crate::macos_window::palette(preference, observed));
        assert_eq!(driver.editor().palette, expected.palette());
        assert!(Rc::ptr_eq(&document, &driver.editor().model.doc));
    }
}

#[test]
fn focus_loss_removes_primary_and_secondary_selection_marks() {
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, text::value("shared"));
    let mut driver = driver(Some(Value::list([cell.into(), cell.into()])), cells);
    driver.select(&driver.element(&[], 0, &[]));
    assert!(selection_marks(&driver.painted().0, 0.22) > 0);
    assert!(selection_marks(&driver.painted().0, 0.10) > 0);
    assert!(driver.focus(false));
    assert!(driver.editor().model.selection.is_none());
    assert_eq!(selection_marks(&driver.painted().0, 0.22), 0);
    assert_eq!(selection_marks(&driver.painted().0, 0.10), 0);
    assert!(driver.focus(true));
    assert_eq!(selection_marks(&driver.painted().0, 0.22), 0);
    assert_eq!(selection_marks(&driver.painted().0, 0.10), 0);
}

#[test]
fn focus_loss_discards_invalid_atom_draft_without_changing_the_document() {
    let mut driver = driver(Some(f64::value(7.0)), Cells::new());
    driver.select(&[]);
    driver.type_text("x");
    assert_eq!(driver.line(), Some("7x"));
    let document = driver.editor().model.doc.clone();
    assert!(driver.focus(false));
    assert!(driver.editor().model.selection.is_none());
    assert!(Rc::ptr_eq(&document, &driver.editor().model.doc));
    assert!(driver.focus(true));
    assert!(driver.editor().model.selection.is_none());
}

#[test]
fn focus_loss_clears_completion_for_values_and_labels() {
    for labels in [false, true] {
        let mut driver = driver(labels.then(|| Value::record([])), Cells::new());
        driver.select(&[]);
        if labels {
            // Enter on a record starts a new field's label.
            driver.press(NamedKey::Enter);
        }
        assert_eq!(
            driver.stage(),
            Some(if labels { Stage::Label } else { Stage::Pending })
        );
        driver.type_text("hello");
        assert!(driver.picker_entries().is_some());
        let document = driver.editor().model.doc.clone();
        assert!(driver.focus(false));
        assert!(driver.editor().model.selection.is_none());
        assert!(driver.picker_entries().is_none());
        assert!(driver.focus(true));
        assert!(driver.editor().model.selection.is_none());
        assert!(driver.picker_entries().is_none());
        assert!(Rc::ptr_eq(&document, &driver.editor().model.doc));
    }
}
