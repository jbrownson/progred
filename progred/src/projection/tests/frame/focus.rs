use super::*;
use kurbo::Size;

const VIEWPORT: Size = Size::new(900.0, 600.0);

fn render(runner: &mut crate::EditorRunner) -> DrawList {
    let mut list = DrawList::default();
    puri::frame::render(runner.prepare_paint(1.0, VIEWPORT).renders, &mut list);
    list
}

fn selection_marks(commands: &[DrawCmd], alpha: f32) -> usize {
    let color = Brush::from(
        crate::styles::Theme::Light
            .palette()
            .accent
            .with_alpha(alpha),
    );
    commands
        .iter()
        .map(|command| match command {
            DrawCmd::Fill { brush, .. } if *brush == color => 1,
            DrawCmd::Clip { children, .. } => selection_marks(children, alpha),
            _ => 0,
        })
        .sum()
}

#[test]
fn changing_palette_repaints_without_losing_edits_selection_or_history() {
    let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
        root: Some(text::value("hello")),
        cells: Cells::new(),
    }));
    runner.editor.model.selection = Some(Selection::edge(&crate::test_root(), vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key: Key::Character("!".into()),
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        VIEWPORT,
    ));
    let document = runner.editor.model.doc.clone();
    fn has_ink(commands: &[DrawCmd], color: Color) -> bool {
        commands.iter().any(|command| match command {
            DrawCmd::GlyphRun(run) => run.brush == Brush::from(color),
            DrawCmd::Clip { children, .. } => has_ink(children, color),
            _ => false,
        })
    }
    for theme in [crate::styles::Theme::Dark, crate::styles::Theme::Light] {
        runner.editor.palette = theme.palette();
        runner.refresh_frame(1.0, VIEWPORT);
        assert!(has_ink(&render(&mut runner).0, theme.palette().literal));
        assert!(Rc::ptr_eq(&document, &runner.editor.model.doc));
        assert_eq!(
            runner
                .editor
                .model
                .selection
                .as_ref()
                .unwrap()
                .edit()
                .unwrap()
                .text(),
            "hello!"
        );
    }
    assert!(runner.editor.model.step_history(true));
    assert_eq!(
        text::read(runner.editor.model.doc.root.as_ref().unwrap()),
        Some("hello")
    );
}

#[test]
fn focus_loss_removes_primary_and_secondary_selection_marks() {
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, text::value("shared"));
    let root = Value::list([Value::from(cell), Value::from(cell)]);
    let position = root.as_list().unwrap().keys().next().unwrap().clone();
    let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
        root: Some(root),
        cells,
    }));
    runner.editor.model.selection = Some(Selection::edge(
        &crate::test_root(),
        vec![Step::Element(position)],
    ));
    runner.refresh_frame(1.0, VIEWPORT);
    let active = render(&mut runner);
    assert!(selection_marks(&active.0, 0.22) > 0);
    assert!(selection_marks(&active.0, 0.10) > 0);
    assert!(runner.focus_changed(false, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
    let inactive = render(&mut runner);
    assert_eq!(selection_marks(&inactive.0, 0.22), 0);
    assert_eq!(selection_marks(&inactive.0, 0.10), 0);
    assert!(runner.focus_changed(true, 1.0, VIEWPORT));
    let refocused = render(&mut runner);
    assert_eq!(selection_marks(&refocused.0, 0.22), 0);
    assert_eq!(selection_marks(&refocused.0, 0.10), 0);
}

#[test]
fn focus_loss_discards_invalid_atom_draft_without_changing_the_document() {
    let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
        root: Some(f64::value(7.0)),
        cells: Cells::new(),
    }));
    runner.editor.model.selection = Some(Selection::edge(&crate::test_root(), vec![]));
    runner.refresh_frame(1.0, VIEWPORT);
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key: Key::Character("x".into()),
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        VIEWPORT
    ));
    assert_eq!(
        runner
            .editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .text(),
        "7x"
    );
    let document = runner.editor.model.doc.clone();
    assert!(runner.focus_changed(false, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
    assert!(Rc::ptr_eq(&document, &runner.editor.model.doc));
    assert!(runner.focus_changed(true, 1.0, VIEWPORT));
    assert!(runner.editor.model.selection.is_none());
}

#[test]
fn focus_loss_clears_completion_for_values_and_labels() {
    for labels in [false, true] {
        let mut runner = crate::EditorRunner::new(crate::test_editor(Document {
            root: labels.then(|| Value::record([])),
            cells: Cells::new(),
        }));
        runner.editor.model.selection = Some(if labels {
            pending_edge(&crate::test_root(), &runner.editor.sources(), vec![]).unwrap()
        } else {
            pending_value(&crate::test_root(), vec![])
        });
        runner.refresh_frame(1.0, VIEWPORT);
        assert!(runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Character("hello".into()),
                state: KeyState::Down,
                ..Default::default()
            },
            1.0,
            VIEWPORT
        ));
        assert!(
            editing_frame(&mut runner.editor, false)
                .completion
                .is_some()
        );
        let document = runner.editor.model.doc.clone();
        assert!(runner.focus_changed(false, 1.0, VIEWPORT));
        assert!(runner.editor.model.selection.is_none());
        assert!(
            editing_frame(&mut runner.editor, false)
                .completion
                .is_none()
        );
        assert!(runner.focus_changed(true, 1.0, VIEWPORT));
        assert!(runner.editor.model.selection.is_none());
        assert!(
            editing_frame(&mut runner.editor, false)
                .completion
                .is_none()
        );
        assert!(Rc::ptr_eq(&document, &runner.editor.model.doc));
    }
}
