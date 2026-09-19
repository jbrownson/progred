//! Exercise the drawn menus through the installed frame's real hit-testing
//! and event handlers, without creating a window.
use super::*;
use crate::{EditorRunner, frame::Hovered};
use gid::{Cells, Document};
use kurbo::{Point, Size};
use puri::hover::Claim;
use ui_events::keyboard::{KeyState, Modifiers, NamedKey};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerEvent, PointerInfo, PointerState, PointerType,
    PointerUpdate,
};

const VIEWPORT: Size = Size::new(640.0, 480.0);

fn runner() -> EditorRunner {
    let mut editor = crate::test_editor(Document {
        root: None,
        cells: Cells::new(),
    });
    editor.drawn_menu = true;
    let mut runner = EditorRunner::new(editor);
    runner.refresh_frame(1.0, VIEWPORT);
    runner
}

fn hover_at(runner: &EditorRunner, point: Point) -> Option<Hover> {
    match runner
        .frame
        .dispatch
        .hover_geometry
        .probe(Some(point), None, 0.0)
    {
        Some((_, Claim::Direct(Hovered::Menu(hover)))) => Some(hover),
        _ => None,
    }
}

fn point_for(runner: &EditorRunner, hover: Hover) -> Point {
    for y in (2..VIEWPORT.height as usize).step_by(4) {
        for x in (2..VIEWPORT.width as usize).step_by(4) {
            let point = Point::new(x as f64, y as f64);
            if hover_at(runner, point) == Some(hover) {
                return point;
            }
        }
    }
    panic!("no hit region for {hover:?}");
}

fn pointer() -> PointerInfo {
    PointerInfo {
        pointer_id: None,
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    }
}

fn move_to(runner: &mut EditorRunner, point: Point) {
    runner.pointer_event(
        &PointerEvent::Move(PointerUpdate {
            pointer: pointer(),
            current: PointerState {
                position: (point.x, point.y).into(),
                ..Default::default()
            },
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }),
        1.0,
        VIEWPORT,
    );
    runner.flush_pending_continuous();
}

fn click(runner: &mut EditorRunner, point: Point) {
    move_to(runner, point);
    let event = PointerButtonEvent {
        pointer: pointer(),
        state: PointerState {
            position: (point.x, point.y).into(),
            ..Default::default()
        },
        button: Some(PointerButton::Primary),
    };
    runner.pointer_event(&PointerEvent::Down(event.clone()), 1.0, VIEWPORT);
    runner.pointer_event(&PointerEvent::Up(event), 1.0, VIEWPORT);
}

fn key(runner: &mut EditorRunner, key: Key, modifiers: Modifiers) {
    assert!(runner.keyboard_event(
        &KeyboardEvent {
            key,
            modifiers,
            state: KeyState::Down,
            ..Default::default()
        },
        1.0,
        VIEWPORT
    ));
}

#[test]
fn pointer_switches_open_menus_but_does_not_open_closed_ones() {
    let mut runner = runner();
    let file = point_for(&runner, Hover::Heading(0));
    let examples = point_for(&runner, Hover::Heading(1));
    move_to(&mut runner, file);
    assert_eq!(runner.editor.menu.open(), None);
    click(&mut runner, file);
    assert_eq!(runner.editor.menu.open(), Some(0));
    move_to(&mut runner, examples);
    assert_eq!(runner.editor.menu.open(), Some(1));
    let item = point_for(
        &runner,
        Hover::Item(Command::App(AppCommand::Example(Example::IopTree))),
    );
    move_to(&mut runner, item);
    assert_eq!(runner.editor.menu.cursor(), Some(2));
    key(
        &mut runner,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(runner.editor.menu.cursor(), Some(3));
    move_to(&mut runner, item + (1.0, 0.0));
    assert_eq!(runner.editor.menu.cursor(), Some(2));
    click(&mut runner, examples);
    assert_eq!(runner.editor.menu.open(), None);
}

#[test]
fn disabled_items_and_outside_clicks_do_not_reach_the_document() {
    let mut runner = runner();
    let edit = point_for(&runner, Hover::Heading(2));
    click(&mut runner, edit);
    let undo = point_for(&runner, Hover::Item(Command::Doc(DocCommand::Undo)));
    click(&mut runner, undo);
    assert_eq!(runner.editor.menu.open(), Some(2));
    assert_eq!(runner.editor.menu.cursor(), None);
    assert!(runner.editor.model.selection.is_none());
    click(&mut runner, Point::new(600.0, 300.0));
    assert_eq!(runner.editor.menu.open(), None);
    assert!(runner.editor.model.selection.is_none());
}

#[test]
fn open_menu_accepts_shortcuts_and_pointer_cancellation() {
    let mut runner = runner();
    key(&mut runner, Key::Named(NamedKey::F10), Modifiers::empty());
    assert_eq!(runner.editor.menu.open(), Some(0));
    assert!(!runner.editor.menu_toggles().raw);
    key(&mut runner, Key::Character("r".into()), Modifiers::CONTROL);
    assert!(runner.editor.menu_toggles().raw);
    assert_eq!(runner.editor.menu.open(), None);
    key(&mut runner, Key::Named(NamedKey::F10), Modifiers::empty());
    runner.pointer_event(&PointerEvent::Cancel(pointer()), 1.0, VIEWPORT);
    assert_eq!(runner.editor.menu.open(), None);
}

#[test]
fn clicking_a_command_executes_it_and_closes_the_menu() {
    let mut runner = runner();
    let view = point_for(&runner, Hover::Heading(3));
    click(&mut runner, view);
    let raw = point_for(&runner, Hover::Item(Command::Doc(DocCommand::Raw)));
    click(&mut runner, raw);
    assert!(runner.editor.menu_toggles().raw);
    assert_eq!(runner.editor.menu.open(), None);
    key(&mut runner, Key::Named(NamedKey::F10), Modifiers::empty());
    key(&mut runner, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(runner.editor.menu.open(), None);
}

#[test]
fn bar_hit_height_tracks_display_scale() {
    let mut runner = runner();
    for scale in [1.0, 1.5, 2.0] {
        runner.refresh_frame(scale, VIEWPORT);
        for index in 0..definition().len() {
            let point = point_for(&runner, Hover::Heading(index));
            for y in [0.5, bar_height(scale) - 0.5] {
                assert_eq!(
                    hover_at(&runner, Point::new(point.x, y)),
                    Some(Hover::Heading(index))
                );
            }
        }
    }
}

#[test]
fn headings_fill_the_bar_and_popup_rows_share_their_hit_width() {
    let mut runner = runner();
    let examples = point_for(&runner, Hover::Heading(1));
    for y in [0.5, bar_height(1.0) - 0.5] {
        assert_eq!(
            hover_at(&runner, Point::new(examples.x, y)),
            Some(Hover::Heading(1))
        );
    }
    click(&mut runner, examples);
    let mut width = None;
    for command in commands(&definition()[1..2]) {
        let hover = Hover::Item(command);
        let point = point_for(&runner, hover);
        let columns: Vec<_> = (0..VIEWPORT.width as usize)
            .filter(|x| hover_at(&runner, Point::new(*x as f64, point.y)) == Some(hover))
            .collect();
        assert!(!columns.is_empty());
        let extent = (*columns.first().unwrap(), *columns.last().unwrap());
        assert_eq!(*width.get_or_insert(extent), extent);
    }
}

#[test]
fn a_popup_near_the_right_edge_stays_inside_the_viewport() {
    let mut runner = runner();
    runner.editor.menu.toggle(3);
    runner.refresh_frame(1.0, Size::new(350.0, VIEWPORT.height));
    let raw = Hover::Item(Command::Doc(DocCommand::Raw));
    let point = point_for(&runner, raw);
    assert_eq!(hover_at(&runner, Point::new(348.0, point.y)), Some(raw));
    assert_eq!(hover_at(&runner, Point::new(350.0, point.y)), None);
}
