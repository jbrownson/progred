use super::*;
use crate::display::test_support::Landmarks;
use crate::display::widget::source::Secondary;
use crate::occurrence::{Step as Occurs, Trail};
use crate::test_support::key_down;
use gid::{CellId, Cells, Document, Step, Value};
use puri::handler::test_support::DispatchShorthands;

type HoverLog = Rc<std::cell::RefCell<Vec<(&'static str, f64)>>>;
type Probe = puri::hover::Probe<Hovered>;
const HOVER_VIEWPORT: Size = Size::new(500.0, 400.0);

#[test]
fn scrolling_a_view_that_no_longer_exists_declines() {
    use ui_events::pointer::PointerScrollEvent;
    let mut editor = crate::editor_of(Value::record([]));
    let gone = crate::display::widget::view::Root::pane(vec![Step::Key(gid::new_cell_id())]);
    let event = PointerScrollEvent {
        pointer: crate::display::test_support::mouse(),
        delta: ui_events::ScrollDelta::LineDelta(0.0, 1.0),
        state: Default::default(),
    };
    let outcome = editor.scroll_view(
        gone,
        &event,
        1.0,
        Size::new(100.0, 100.0),
        Vec2::new(0.0, 50.0),
    );
    assert!(!outcome.handled());
}

#[test]
fn the_document_view_reports_content_that_does_not_depend_on_its_height() {
    let mut editor = crate::editor_of(Value::list(
        (0..40).map(|n| crate::libraries::f64::value(n as f64)),
    ));
    editor.drawn_menu = false;
    let mut runner = EditorRunner::new(editor);
    let scale = 2.0;
    let contents = [100.0, 4_000.0].map(|height| {
        runner.refresh_frame(scale, Size::new(800.0, height));
        let region = runner
            .frame
            .dispatch
            .view_regions
            .iter()
            .find(|region| matches!(region.root.target(), Target::Document))
            .expect("document view");
        let content = region
            .content
            .expect("a scrolling view reports its content");
        // An embedding page sizing its frame to the content gets no scrolling.
        assert_eq!(region.maximum.y, (content.y - height / scale).max(0.0));
        content
    });
    assert!(contents[0].y > 100.0 / scale);
    assert_eq!(contents[0], contents[1]);
}

#[test]
fn source_hover_conjects_the_occurrence_in_its_own_view() {
    let (field, key) = (gid::new_cell_id(), gid::new_cell_id());
    let mut editor = crate::editor_of(Value::record([(
        field,
        crate::libraries::text::value("shared"),
    )]));
    let root = editor.model.workspace.document_root().clone();
    let source = vec![Step::Key(field)];
    let occurrence = Trail::from([
        Occurs::Key(key),
        Occurs::jump(crate::location::Location::root(source.clone())),
    ]);
    // What the projection draws a conjected occurrence sharing.
    let secondary = Secondary::Stored(crate::occurrence::of(&source).into());
    let mut landmarks = [crate::display::widget::navigation::Landmark::<Editor> {
        path: occurrence.clone(),
        root: Some(root.clone()),
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        select: Rc::new(|_, _| true),
        secondary: Some(secondary.clone()),
        selected: false,
        tab_stop: false,
    }];
    let hover = Some(Hovered::Tree(hover::Hover::Value(occurrence.clone())));
    let resolved = attribute(
        &editor.sources(),
        &landmarks,
        Some(&root),
        None,
        hover.clone(),
        false,
    );
    let expected = SourceTrace::Stored(Rc::from(source));
    assert_eq!(resolved.hovered_trace, Some(expected.clone()));
    assert_eq!(resolved.hovered_secondary, Some(secondary.clone()));
    // A held press must carry the owning view into successor attribution,
    // not just preserve the displayed hover path.
    let observed = Rc::new(std::cell::RefCell::new(None));
    let captured = observed.clone();
    let child = crate::display::widget::leaf(
        measured::Extent::default(),
        move |output: &mut crate::display::widget::HoverContext<'_, Editor, Hovered>, _| {
            output.after_hover(move |hover, _| {
                *captured.borrow_mut() =
                    Some((hover.hovered_trace.clone(), hover.hovered_secondary.clone()));
            });
        },
    );
    let layout = crate::display::widget::navigation::landmark(
        child,
        occurrence.clone(),
        Rc::new(|_, _| true),
        Some(secondary.clone()),
        false,
    );
    let frame = hover_frame(
        &editor,
        crate::display::widget::view::in_view(layout, root.clone()),
        None,
        Some((Some(&root), hover.as_ref().unwrap())),
        true,
    );
    assert_eq!(frame.dispatch.pointer_root, Some(root.clone()));
    assert_eq!(
        *observed.borrow(),
        Some((Some(expected.clone()), Some(secondary)))
    );
    // A computed occurrence must not acquire provenance even when its
    // spelling also names real document data.
    Rc::make_mut(&mut editor.model.doc).root = Some(Value::record([
        (field, crate::libraries::text::value("shared")),
        (key, crate::libraries::text::value("coincidence")),
    ]));
    let computed = Trail::from([Occurs::Key(key), Occurs::Computed]);
    landmarks[0].path = computed.clone();
    let resolved = attribute(
        &editor.sources(),
        &landmarks,
        Some(&root),
        None,
        Some(Hovered::Tree(hover::Hover::Value(computed))),
        false,
    );
    assert!(resolved.hovered_trace.is_none());
}

#[test]
fn while_picking_the_chosen_offer_marks_what_it_refers_to() {
    let editor = crate::editor_of(Value::record([]));
    let root = editor.model.workspace.document_root().clone();
    let (place, cell) = (Secondary::Cell(gid::new_cell_id()), gid::new_cell_id());
    // The pending slot, selected, shares its place.
    let landmarks = [crate::display::widget::navigation::Landmark::<Editor> {
        path: Trail::ROOT.clone(),
        root: Some(root.clone()),
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        select: Rc::new(|_, _| true),
        secondary: Some(place.clone()),
        selected: true,
        tab_stop: false,
    }];
    let entry = |source| crate::display::widget::offers::Entry {
        display: "offer".to_owned(),
        subscripts: Vec::new(),
        detail: None,
        matches: Vec::new(),
        face: crate::display::Face::Label,
        source,
        activate: Rc::new(|_: &mut Editor| {}),
    };
    let offers = |chosen| crate::display::widget::offers::Offers {
        entries: Rc::from([entry(None), entry(Some(cell))]),
        chosen,
    };
    let selected = |offers: Option<&crate::display::widget::offers::Offers<Editor>>| {
        attribute(
            &editor.sources(),
            &landmarks,
            Some(&root),
            offers,
            None,
            false,
        )
        .selected_secondary
    };
    assert_eq!(selected(Some(&offers(1))), Some(Secondary::Cell(cell)));
    assert_eq!(
        selected(Some(&offers(0))),
        Some(place.clone()),
        "an offer that refers to nothing leaves the selection's"
    );
    assert_eq!(selected(None), Some(place));
}

fn hover_runner(log: &HoverLog, change_target: bool) -> EditorRunner {
    use crate::display::{Layout, partial, widget};
    use crate::libraries::f64;
    use puri::handler::EventOutcome;

    let mut editor = crate::editor_of(f64::value(0.0));
    editor.pointer = Some(Point::new(25.0, 25.0));
    editor.stack.projection = projection::Projection::new([partial({
        let log = log.clone();
        move |input| {
            let value = f64::read(input.value?)?;
            log.borrow_mut().push(("project", value));
            let log = log.clone();
            Some(Layout::widget(Rc::new(move |_| {
                let log = log.clone();
                widget::leaf(
                    measured::Extent {
                        width: 100.0,
                        ascent: 40.0,
                        descent: 0.0,
                    },
                    move |output, placement| {
                        let target = Hovered::Tree(if (value as u64).is_multiple_of(2) {
                            hover::Hover::Value(Trail::root())
                        } else {
                            hover::Hover::Toggle(Trail::root())
                        });
                        output.claim(Probe::exact(placement, target));
                        output.after_hover(move |_, effects| {
                            effects.handler().on({
                                let log = log.clone();
                                move |editor: &mut Editor, event, input| {
                                    if matches!(event, Event::HoverChanged) {
                                        log.borrow_mut().push((
                                            if input.hovered().is_some() {
                                                "hover"
                                            } else {
                                                "leave"
                                            },
                                            value,
                                        ));
                                        if input.hovered().is_some() {
                                            if change_target {
                                                Rc::make_mut(&mut editor.model.doc).root =
                                                    Some(f64::value(value + 1.0));
                                            }
                                            return EventOutcome::accept();
                                        }
                                    }
                                    EventOutcome::decline(event)
                                }
                            });
                            effects
                                .renders
                                .push(Box::new(move |_| log.borrow_mut().push(("paint", value))));
                        });
                    },
                )
            })))
        }
    })]);
    EditorRunner::new(editor)
}

fn paint_hover_frame(runner: &mut EditorRunner) -> bool {
    puri::frame::render(
        runner.prepare_paint(1.0, HOVER_VIEWPORT).renders,
        &mut puri::draw::DrawList::default(),
    );
    runner.frame_presented()
}

fn move_hover_pointer(runner: &mut EditorRunner, point: Point) {
    use ui_events::pointer::{PointerEvent, PointerState, PointerUpdate};
    assert!(runner.pointer_event(
        &PointerEvent::Move(PointerUpdate {
            pointer: crate::display::test_support::mouse(),
            current: PointerState {
                position: (point.x, point.y).into(),
                ..Default::default()
            },
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }),
        1.0,
        HOVER_VIEWPORT,
    ));
    assert!(runner.flush_pending_continuous());
}

#[test]
fn pointer_hover_reacts_against_installed_geometry_before_one_successor_build() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, false);
    runner.editor.pointer = None;
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    assert!(!paint_hover_frame(&mut runner));
    log.take();

    move_hover_pointer(&mut runner, Point::new(25.0, 25.0));
    assert_eq!(log.take(), [("hover", 0.0), ("project", 0.0)]);
    assert!(!paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 0.0)]);

    move_hover_pointer(&mut runner, Point::new(26.0, 25.0));
    assert_eq!(log.take(), [("project", 0.0)]);
    assert!(!paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 0.0)]);

    move_hover_pointer(&mut runner, Point::new(450.0, 300.0));
    assert_eq!(log.take(), [("leave", 0.0), ("project", 0.0)]);
    assert!(!paint_hover_frame(&mut runner));
}

#[test]
fn pointer_hover_reactions_that_change_geometry_wait_for_paint_before_reacting_again() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, true);
    runner.editor.pointer = None;
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    assert!(!paint_hover_frame(&mut runner));
    log.take();

    move_hover_pointer(&mut runner, Point::new(25.0, 25.0));
    assert_eq!(log.take(), [("hover", 0.0), ("project", 1.0)]);
    assert_eq!(
        runner.frame.hover,
        Some(Hovered::Tree(hover::Hover::Toggle(Trail::root())))
    );
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 1.0)]);
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(
        log.take(),
        [
            ("project", 1.0),
            ("hover", 1.0),
            ("project", 2.0),
            ("paint", 2.0)
        ]
    );
}

#[test]
fn hover_reactions_continue_across_painted_frames_without_recursion_or_a_limit() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, true);
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    assert_eq!(
        log.take(),
        [("project", 0.0), ("hover", 0.0), ("project", 1.0)]
    );
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 1.0)]);
    for value in 1..20 {
        assert!(paint_hover_frame(&mut runner));
        let value = value as f64;
        assert_eq!(
            log.take(),
            [
                ("project", value),
                ("hover", value),
                ("project", value + 1.0),
                ("paint", value + 1.0),
            ]
        );
    }
    // An ordinary input can interrupt the oscillation between paintings.
    runner.update_frame(1.0, HOVER_VIEWPORT, |editor, _, _| {
        editor.pointer = None;
        true
    });
    assert_eq!(log.take(), [("project", 20.0), ("leave", 20.0)]);
    assert!(!paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 20.0)]);
}

#[test]
fn accepting_hover_without_changing_it_only_builds_one_successor() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, false);
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    assert_eq!(
        log.take(),
        [("project", 0.0), ("hover", 0.0), ("project", 0.0)]
    );
    assert!(!paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 0.0)]);
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    assert_eq!(log.take(), [("project", 0.0)]);
    assert!(!paint_hover_frame(&mut runner));
}

#[test]
fn preparing_or_replacing_unpainted_frames_does_not_release_hover_feedback() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, true);
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    log.borrow_mut().clear();
    drop(runner.prepare_paint(1.0, HOVER_VIEWPORT));
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    drop(runner.prepare_paint(1.0, HOVER_VIEWPORT));
    assert_eq!(log.take(), [("project", 1.0)]);
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("project", 1.0), ("paint", 1.0)]);
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(
        log.take(),
        [
            ("project", 1.0),
            ("hover", 1.0),
            ("project", 2.0),
            ("paint", 2.0)
        ]
    );
}

#[test]
fn new_input_supersedes_an_unpainted_target_without_replaying_stale_handlers() {
    let log = HoverLog::default();
    let mut runner = hover_runner(&log, true);
    runner.refresh_frame(1.0, HOVER_VIEWPORT);
    log.borrow_mut().clear();
    runner.update_frame(1.0, HOVER_VIEWPORT, |editor, _, _| {
        editor.pointer = None;
        true
    });
    assert_eq!(log.take(), [("project", 1.0)]);
    assert!(paint_hover_frame(&mut runner));
    assert_eq!(log.take(), [("paint", 1.0)]);
    assert!(!paint_hover_frame(&mut runner));
    assert_eq!(
        log.take(),
        [("project", 1.0), ("leave", 1.0), ("paint", 1.0)]
    );
}

#[test]
fn hover_notifications_include_view_ownership_and_are_reset_with_the_document() {
    let mut runner = EditorRunner::new(crate::test_editor(Document::default()));
    let roots = Rc::new(std::cell::RefCell::new(Vec::new()));
    runner.frame.dispatch.handler_mut().on({
        let roots = roots.clone();
        move |_, event, input| {
            if matches!(event, Event::HoverChanged) {
                roots.borrow_mut().push(input.root.clone());
            }
            puri::handler::EventOutcome::decline(event)
        }
    });
    runner.frame.hover = Some(Hovered::Blocked);
    let first = Root::document();
    let second = Root::pane(vec![Step::Key(gid::new_cell_id())]);
    runner.frame.dispatch.pointer_root = Some(first.clone());
    runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
    runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
    runner.frame.dispatch.pointer_root = Some(second.clone());
    runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
    assert_eq!(roots.take(), [Some(first), Some(second)]);
    runner.frame.hover_awaits_paint = true;
    runner.adopt_model(Document::default(), None, Default::default());
    assert!(runner.frame.notified_hover.is_none());
    assert!(!runner.frame.hover_awaits_paint);
    assert!(!runner.frame_presented());
}

fn scrolling_runner(projected: &Rc<std::cell::Cell<usize>>) -> EditorRunner {
    let mut editor = crate::editor_of(Value::list(
        (0..40).map(|n| crate::libraries::f64::value(n as f64)),
    ));
    let projected = projected.clone();
    editor.stack.projection =
        projection::Projection::new([crate::display::partial(move |input| {
            crate::libraries::f64::read(input.value?)?;
            projected.set(projected.get() + 1);
            Some(crate::display::Layout::widget(Rc::new(|_| {
                crate::display::widget::leaf(
                    measured::Extent {
                        width: 100.0,
                        ascent: 25.0,
                        descent: 5.0,
                    },
                    |_, _| {},
                )
            })))
        })]);
    EditorRunner::new(editor)
}

#[test]
fn selection_reveal_uses_installed_geometry_before_one_successor_build() {
    let projected = Rc::new(std::cell::Cell::new(0));
    let mut runner = scrolling_runner(&projected);
    let viewport = Size::new(500.0, 400.0);
    runner.refresh_frame(1.0, viewport);
    let per_frame = projected.replace(0);
    let path = vec![Step::Element(
        runner
            .editor
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .last()
            .unwrap()
            .clone(),
    )];
    let target = runner.frame.dispatch.landmarks.landmark(&path).unwrap();
    assert!(target.rect.y0 > viewport.height);
    let target = target.clone();
    assert!(runner.update_frame(1.0, viewport, |editor, dispatch, _| {
        dispatch.geometry(1.0).arrive(editor, &target, None)
    }));
    assert_eq!(projected.replace(0), per_frame);
    assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
    let target = runner.frame.dispatch.landmarks.landmark(&path).unwrap();
    assert!((0.0..viewport.height).contains(&target.rect.y0));

    // Scrolling away from an unchanged selection must not reveal it again.
    runner.update_frame(1.0, viewport, |editor, dispatch, _| {
        dispatch
            .handler
            .dispatch_scroll(
                editor,
                &ui_events::pointer::PointerScrollEvent {
                    pointer: crate::display::test_support::mouse(),
                    state: ui_events::pointer::PointerState {
                        position: (200.0, 200.0).into(),
                        ..Default::default()
                    },
                    delta: ui_events::ScrollDelta::PixelDelta((0.0, 2_000.0).into()),
                },
            )
            .handled()
    });
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    assert_eq!(projected.get(), per_frame);
}

#[test]
fn keyboard_reveal_shows_the_whole_partly_visible_destination() {
    use ui_events::keyboard::{Modifiers, NamedKey};
    let mut runner = scrolling_runner(&Default::default());
    let viewport = Size::new(500.0, 400.0);
    runner.refresh_frame(1.0, viewport);
    let paths: Vec<_> = runner
        .editor
        .model
        .doc
        .root
        .as_ref()
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .rev()
        .take(2)
        .map(|position| vec![Occurs::Element(position.clone())])
        .collect();
    let [path, previous] = paths.as_slice() else {
        panic!("expected two list items");
    };
    let root = runner.editor.model.workspace.document_root().clone();
    let region = runner
        .frame
        .dispatch
        .view_regions
        .iter()
        .find(|region| region.root == root)
        .unwrap()
        .rect;
    let target = runner.frame.dispatch.landmarks.landmark(path).unwrap();
    runner.editor.model.workspace.document.scroll.y = target.rect.center().y - region.y1;
    crate::editing::Edit::new(runner.editor.session()).select(previous);
    runner.refresh_frame(1.0, viewport);
    let target = runner.frame.dispatch.landmarks.landmark(path).unwrap();
    assert!(target.rect.y0 < region.y1 && target.rect.y1 > region.y1);

    assert!(runner.keyboard_event(
        &key_down(NamedKey::ArrowDown, Modifiers::empty()),
        1.0,
        viewport,
    ));
    assert_eq!(runner.editor.model.selection.as_ref().unwrap().path(), path);
    let target = runner.frame.dispatch.landmarks.landmark(path).unwrap();
    assert!(target.rect.y0 >= region.y0 && target.rect.y1 <= region.y1);
}

#[test]
fn refreshes_and_unrelated_inputs_do_not_reveal_an_existing_selection() {
    let projected = Rc::new(std::cell::Cell::new(0));
    let mut runner = scrolling_runner(&projected);
    let viewport = Size::new(500.0, 400.0);
    runner.refresh_frame(1.0, viewport);
    let path = runner
        .frame
        .dispatch
        .landmarks
        .iter()
        .find(|target| target.rect.y0 > viewport.height)
        .unwrap()
        .path
        .to_vec();
    runner.editor.model.selection = Some(crate::selection::Selection::edge(path));

    runner.refresh_frame(1.0, viewport);
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    runner.update_frame(1.0, viewport, |_, _, _| true);
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
}

#[test]
fn selection_changes_do_not_implicitly_request_reveal() {
    let projected = Rc::new(std::cell::Cell::new(0));
    let mut runner = scrolling_runner(&projected);
    let viewport = Size::new(500.0, 400.0);
    runner.refresh_frame(1.0, viewport);
    let path = runner
        .frame
        .dispatch
        .landmarks
        .iter()
        .find(|target| target.rect.y0 > viewport.height)
        .unwrap()
        .path
        .to_vec();
    runner.update_frame(1.0, viewport, |editor, _, _| {
        crate::editing::Edit::new(editor.session()).select(&path);
        true
    });
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);

    runner.update_frame(1.0, viewport, |editor, _, _| {
        editor.model.selection = Some(crate::selection::pending_value(path));
        true
    });
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
}

fn select_offscreen(runner: &mut EditorRunner, viewport: Size) -> crate::occurrence::Path {
    let target = runner
        .frame
        .dispatch
        .landmarks
        .iter()
        .find(|target| target.rect.y0 > viewport.height)
        .unwrap();
    let path = target.path.to_vec();
    crate::editing::Edit::new(runner.editor.session()).select(&path);
    path
}

#[test]
fn keyboard_navigation_deletion_and_pending_cancellation_explicitly_reveal() {
    use ui_events::keyboard::{Modifiers, NamedKey};
    for key in [NamedKey::ArrowDown, NamedKey::Delete, NamedKey::Backspace] {
        let projected = Rc::new(std::cell::Cell::new(0));
        let mut runner = scrolling_runner(&projected);
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        let path = select_offscreen(&mut runner, viewport);
        if key == NamedKey::Backspace {
            runner.editor.model.selection = Some(crate::selection::pending_value(path.clone()));
        }
        // Include the selected widget's installed delete handler, not only
        // the shell's structural fallback.
        runner.refresh_frame(1.0, viewport);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        projected.set(0);
        assert!(runner.keyboard_event(&key_down(key, Modifiers::empty()), 1.0, viewport));
        let selection = runner.editor.model.selection.as_ref().unwrap();
        if key == NamedKey::Delete {
            // Deleting keeps the place selected, as a hole.
            assert_eq!(selection.path(), path);
            assert_eq!(
                selection.stage(&runner.editor.sources()),
                crate::selection::Stage::Pending
            );
        } else {
            assert_ne!(selection.path(), path);
        }
        assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
        assert_eq!(
            projected.get(),
            if key == NamedKey::Delete { 39 } else { 40 }
        );
    }
}

#[test]
fn omitted_menu_keeps_history_but_not_menu_commands() {
    use ui_events::keyboard::{Key, Modifiers, NamedKey};
    let doc = crate::gid_text::parse(include_str!("../../fixtures/values.gid"))
        .unwrap()
        .0;
    let mut runner = crate::EditorRunner::new(crate::test_editor(doc.clone()));
    runner.editor.command_modifier = puri::keyboard::CommandModifier::Control;
    let viewport = Size::new(620.0, 304.0);
    runner
        .editor
        .model
        .history
        .record(runner.editor.model.snapshot());
    Rc::make_mut(&mut runner.editor.model.doc).root = Some(gid::Value::list([]));
    runner.refresh_frame(1.0, viewport);
    assert!(runner.keyboard_event(
        &key_down(Key::Character("z".into()), Modifiers::CONTROL),
        1.0,
        viewport
    ));
    assert_eq!(runner.editor.model.doc.root, doc.root);
    for event in [
        key_down(Key::Character("n".into()), Modifiers::CONTROL),
        key_down(Key::Character("r".into()), Modifiers::CONTROL),
        key_down(Key::Character("d".into()), Modifiers::CONTROL),
        key_down(Key::Character("p".into()), Modifiers::CONTROL),
        key_down(Key::Character("1".into()), Modifiers::CONTROL),
        key_down(Key::Named(NamedKey::F10), Modifiers::empty()),
    ] {
        assert!(!runner.keyboard_event(&event, 1.0, viewport));
    }
    assert!(runner.keyboard_event(
        &key_down(
            Key::Character("z".into()),
            Modifiers::CONTROL | Modifiers::SHIFT
        ),
        1.0,
        viewport
    ));
    assert_eq!(runner.editor.model.doc.root, Some(gid::Value::list([])));
}

#[test]
fn raw_shortcut_is_contributed_only_by_the_menu() {
    use puri::keyboard::CommandModifier;
    use ui_events::keyboard::{Key, Modifiers};
    for (modifier, modifiers) in [
        (CommandModifier::Meta, Modifiers::META),
        (CommandModifier::Control, Modifiers::CONTROL),
    ] {
        for menu in [false, true] {
            let mut editor = crate::editor_of(Value::list([]));
            editor.drawn_menu = menu;
            editor.command_modifier = modifier;
            let mut runner = crate::EditorRunner::new(editor);
            let viewport = Size::new(620.0, 304.0);
            runner.refresh_frame(1.0, viewport);
            let raw = |runner: &crate::EditorRunner| {
                runner.editor.menu_toggles().preset == Some(crate::stack::Preset::Raw)
            };
            assert!(!raw(&runner));
            assert_eq!(
                runner.keyboard_event(
                    &key_down(Key::Character("r".into()), modifiers),
                    1.0,
                    viewport
                ),
                menu
            );
            assert_eq!(raw(&runner), menu);
            // Again, it goes back to every library.
            runner.keyboard_event(
                &key_down(Key::Character("r".into()), modifiers),
                1.0,
                viewport,
            );
            assert_eq!(
                runner.editor.menu_toggles().preset,
                Some(crate::stack::Preset::All)
            );
        }
    }
}

#[test]
fn raw_switches_every_library_off() {
    let stack = crate::stack::load();
    let document = Document::default();
    let view = |shown| workspace::View {
        root: crate::test_root(),
        shown,
        annotations: Default::default(),
        scroll: Vec2::ZERO,
    };
    let starters = |view: &workspace::View| {
        let provider = completions(&stack, view);
        crate::test_support::ask_over(
            &crate::test_support::src(&document, &stack.libraries),
            crate::display::CompletionKind::Value,
            crate::display::CompletionScope::Start,
            "",
            &[],
            |request| (**provider)(request),
        )
        .unwrap_or_default()
        .len()
    };
    let standard = view(workspace::Shown::ALL);
    assert!(names(&stack, &standard));
    assert!(starters(&standard) > 0);
    let raw = view(workspace::Shown::RAW);
    assert!(!names(&stack, &raw));
    assert_eq!(starters(&raw), 0);
}

#[test]
fn native_and_drawn_history_commands_reveal_even_the_same_selection() {
    use ui_events::keyboard::{Key, Modifiers};
    for drawn in [false, true] {
        let mut runner = scrolling_runner(&Default::default());
        runner.editor.drawn_menu = drawn;
        runner.editor.command_modifier = puri::keyboard::CommandModifier::Control;
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        let path = select_offscreen(&mut runner, viewport);
        runner
            .editor
            .model
            .history
            .record(runner.editor.model.snapshot());
        // History restores the same selection. It still explicitly reveals;
        // selection equality is not involved in the command's policy.
        if drawn {
            assert!(runner.keyboard_event(
                &key_down(Key::Character("z".into()), Modifiers::CONTROL),
                1.0,
                viewport
            ));
        } else {
            runner.update_frame(1.0, viewport, |editor, dispatch, _| {
                editor.run_doc_command(crate::command::DocCommand::Undo, dispatch.geometry(1.0));
                true
            });
        }
        assert_eq!(runner.editor.model.selection.as_ref().unwrap().path(), path);
        assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
    }
}

#[test]
fn a_new_target_missing_from_the_previous_frame_does_not_trigger_a_retry() {
    let projected = Rc::new(std::cell::Cell::new(0));
    let mut runner = scrolling_runner(&projected);
    let viewport = Size::new(500.0, 400.0);
    runner.refresh_frame(1.0, viewport);
    projected.set(0);
    let content = runner
        .editor
        .model
        .doc
        .root
        .as_ref()
        .unwrap()
        .as_list()
        .unwrap();
    let position = gid::position::between(content.keys().last(), None).unwrap();
    let path = vec![Step::Element(position.clone())];
    let content = Value::List(content.update(position, crate::libraries::f64::value(40.0)));
    assert!(!runner.frame.dispatch.landmarks.landmark(&path).is_some());
    runner.update_frame(1.0, viewport, |editor, dispatch, _| {
        Rc::make_mut(&mut editor.model.doc).root = Some(content);
        editor.model.selection = Some(crate::selection::Selection::edge(crate::occurrence::of(
            &path,
        )));
        dispatch.geometry(1.0).reveal_selection(&mut editor.model);
        true
    });
    assert_eq!(projected.replace(0), 41);
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    let target = runner.frame.dispatch.landmarks.landmark(&path).unwrap();
    assert!(target.rect.y0 > viewport.height);

    runner.refresh_frame(1.0, viewport);
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    assert_eq!(projected.replace(0), 41);
    runner.update_frame(1.0, viewport, |_, _, _| true);
    assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    assert_eq!(projected.get(), 41);
}

#[test]
fn source_hover_is_immediate_from_code_and_explicit_from_drawing() {
    let code = Hovered::Tree(hover::Hover::Value(Trail::root()));
    let drawing = Hovered::Tree(hover::Hover::Source(SourceTrace::Stored(Rc::from([]))));

    assert!(source_hover_visible(Some(&code), false));
    assert!(!source_hover_visible(Some(&drawing), false));
    assert!(source_hover_visible(Some(&drawing), true));
}

#[test]
fn a_drawing_source_selects_its_definition_among_equal_cell_paths() {
    let cell = CellId::from_u128(1);
    let call = CellId::from_u128(2);
    let library_id = CellId::from_u128(3);
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(call, Value::from(vec![1]))]));
    let doc = Document {
        root: Some(Value::Cell(cell)),
        cells,
    };
    let libraries = crate::library::Libraries::from_contributions([(
        library_id,
        crate::test_support::library(
            library_id,
            "source",
            doc.cells.clone(),
            grap::ForeignFunctions::default(),
        ),
    )])
    .0;
    let sources = sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let root = Root::document();
    let targets = [
        (gid::Resolution::Document, Rect::new(10.0, 20.0, 30.0, 40.0)),
        (
            gid::Resolution::Library(library_id),
            Rect::new(10.0, 50.0, 30.0, 70.0),
        ),
    ];
    let landmarks =
        targets.map(
            |(resolution, rect)| crate::display::widget::navigation::Landmark::<
                Option<gid::Resolution>,
            > {
                root: Some(root.clone()),
                path: Trail::from([Occurs::Follow(resolution), Occurs::Key(call)]),
                rect,
                select: Rc::new(move |selected, _| {
                    *selected = Some(resolution);
                    true
                }),
                secondary: None,
                selected: false,
                tab_stop: false,
            },
        );
    for (resolution, rect) in targets {
        let trace = SourceTrace::InCell {
            cell,
            resolution,
            path: Rc::from([Step::Key(call)]),
        };
        let landmark =
            crate::display::widget::source_link::source_landmark(&sources, &landmarks, &trace)
                .unwrap();
        assert_eq!(landmark.root, Some(root.clone()));
        assert_eq!(landmark.rect, rect);
        let mut selected = None;
        assert!((landmark.select)(&mut selected, None));
        assert_eq!(selected, Some(resolution));
    }
}

#[test]
fn pane_presentations_apply_only_at_entry_and_raw_keeps_the_declaration() {
    use crate::libraries::presentation;
    use crate::library::Definitions;
    use std::cell::{Cell, RefCell};

    let projector = CellId::from_u128(1);
    let linked = CellId::from_u128(2);
    let library = CellId::from_u128(3);
    let nested_projector = CellId::from_u128(4);
    let alias = CellId::from_u128(5);
    let source = Value::record([
        (
            presentation::vocabulary::VALUE,
            Value::from(b"source".to_vec()),
        ),
        (
            presentation::vocabulary::PROJECTION,
            Value::from(nested_projector),
        ),
    ]);
    let declaration = Value::record([
        (presentation::vocabulary::VALUE, source.clone()),
        (presentation::vocabulary::PROJECTION, Value::from(projector)),
    ]);
    let mut cells = Cells::new();
    cells.set_value(linked, declaration.clone());
    cells.set_value(alias, Value::from(linked));
    let mut library_cells = Cells::new();
    library_cells.set_value(linked, declaration.clone());
    let mut model = Model::new(Document {
        root: Some(Value::record([])),
        cells,
    });
    for value in [
        declaration,
        Value::from(alias),
        Value::list([source.clone(), Value::from(linked)]),
        Value::record([(presentation::vocabulary::VALUE, source.clone())]),
    ] {
        Rc::make_mut(&mut model.doc).root = Some(
            workspace::append(
                model.doc.root.as_ref().unwrap(),
                workspace::Side::Left,
                value,
            )
            .unwrap()
            .0,
        );
    }
    model
        .workspace
        .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
    // The document view shows the declarations as data only without the
    // editor library, which otherwise draws the workspace's main value.
    model.workspace.document.shown = workspace::Shown::AllBut(vec![crate::libraries::editor::ID]);
    crate::annotations::set_folded(
        &mut model.workspace.document.annotations,
        &[Occurs::Key(workspace::vocabulary::PANES)],
        false,
        false,
    );
    let calls = Rc::new(Cell::new(0));
    let result = Rc::new(RefCell::new(source.clone()));
    let mut stack = stack::load();
    stack.libraries.insert(
        library,
        Definitions::from_parts(
            library_cells,
            grap::ForeignFunctions::default().register(
                projector,
                grap::ForeignFunction::from_value({
                    let calls = calls.clone();
                    let result = result.clone();
                    move |context, call, environment| {
                        let value = context
                            .field(call, presentation::vocabulary::VALUE)
                            .unwrap();
                        assert_eq!(context.eval_to_value(value, environment)?, source);
                        calls.set(calls.get() + 1);
                        Ok(result.borrow().clone())
                    }
                }),
            ).register(
                nested_projector,
                grap::ForeignFunction::from_value(|_, _, _| {
                    panic!("nested declarations are ordinary data, including results and absent fallbacks")
                }),
            ),
        ),
    );
    let styles = style::editor(style::Theme::Light.palette(), 1.0);
    let mut fonts = crate::display::test_support::Fonts::new();
    let mut tcx = fonts.text(1.0);
    let size = Size::new(1200.0, 1000.0);
    let computations = crate::computations::Computations::default();
    let mut place = |model: &Model| {
        crate::display::widget::frame::place(
            project_workspace(
                &workspace_description(model, &stack, &computations, 1.0),
                &styles,
                &mut tcx,
                size,
            ),
            Placement::root(Rect::from_origin_size(Point::ZERO, size)),
            &Default::default(),
        )
    };
    let document = model.workspace.document_root();
    let sources: Vec<_> = [
        (0, vec![]),
        (
            1,
            vec![
                Step::Follow(gid::Resolution::Document),
                Step::Follow(gid::Resolution::Document),
            ],
        ),
    ]
    .into_iter()
    .map(|(index, steps)| {
        let pane = &model.workspace.left.panes[index];
        let Target::Pane { path } = pane.view.root.target() else {
            panic!("pane path")
        };
        // The document view draws the declaration where it's stored, and the
        // pane draws it under the declaration, after a pane step.
        let mut stored = path.clone();
        stored.extend(steps.iter().cloned());
        let drawn = crate::occurrence::below(&crate::test_examples::pane_root(path), &steps);
        (pane.view.root.clone(), stored, drawn)
    })
    .collect();
    let shown = place(&model);
    assert_eq!(
        calls.replace(0),
        2,
        "only the inline declaration and the selected cell definition at pane entry apply"
    );
    for (pane, stored, drawn) in &sources {
        for field in [
            presentation::vocabulary::VALUE,
            presentation::vocabulary::PROJECTION,
        ] {
            let stored =
                crate::occurrence::below(&crate::occurrence::of(stored), &[Step::Key(field)]);
            let drawn = crate::occurrence::below(drawn, &[Step::Key(field)]);
            assert!(
                shown
                    .landmarks
                    .iter()
                    .any(|target| target.root.as_ref() == Some(document) && target.path == stored)
            );
            assert!(
                !shown
                    .landmarks
                    .iter()
                    .any(|target| target.root.as_ref() == Some(pane) && target.path == drawn)
            );
        }
    }
    for pane in &mut model.workspace.left.panes {
        pane.view.shown = workspace::Shown::RAW;
    }
    let raw = place(&model);
    assert_eq!(calls.replace(0), 0);
    for (pane, _, drawn) in &sources {
        for field in [
            presentation::vocabulary::VALUE,
            presentation::vocabulary::PROJECTION,
        ] {
            let drawn = crate::occurrence::below(drawn, &[Step::Key(field)]);
            assert!(
                raw.landmarks
                    .iter()
                    .any(|target| target.root.as_ref() == Some(pane) && target.path == drawn)
            );
        }
    }
    for pane in &mut model.workspace.left.panes {
        pane.view.shown = workspace::Shown::ALL;
    }
    *result.borrow_mut() = ::grap::absent::value(projector);
    let absent = place(&model);
    assert_eq!(calls.get(), 2);
    for (pane, _, drawn) in &sources {
        let source_path =
            crate::occurrence::below(drawn, &[Step::Key(presentation::vocabulary::VALUE)]);
        assert!(
            absent
                .landmarks
                .iter()
                .any(|target| target.root.as_ref() == Some(pane) && target.path == source_path)
        );
    }
}

#[test]
fn viewport_panes_receive_their_size_without_margins_or_document_scrolling() {
    use crate::libraries::{f64, layout, presentation};
    use crate::library::Definitions;
    use std::cell::RefCell;
    let function = gid::new_cell_id();
    let source = Value::from(b"source".to_vec());
    let declaration = Value::record([
        (presentation::vocabulary::VALUE, source.clone()),
        (
            presentation::vocabulary::VIEWPORT,
            grap::lambda(
                [
                    presentation::vocabulary::VALUE,
                    layout::vocabulary::WIDTH,
                    layout::vocabulary::HEIGHT,
                ],
                grap::call(
                    function.into(),
                    [
                        presentation::vocabulary::VALUE,
                        layout::vocabulary::WIDTH,
                        layout::vocabulary::HEIGHT,
                    ]
                    .map(|label| (label, label.into())),
                ),
            ),
        ),
    ]);
    let mut model = Model::new(Document {
        root: Some(
            workspace::append(
                &Value::record([]),
                workspace::Side::Left,
                declaration.clone(),
            )
            .unwrap()
            .0,
        ),
        cells: Cells::new(),
    });
    model
        .workspace
        .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
    model.workspace.left.panes[0].view.scroll = Vec2::new(75.0, 150.0);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut stack = stack::load();
    stack.libraries.insert(
        gid::new_cell_id(),
        Definitions::from_parts(
            Cells::new(),
            grap::ForeignFunctions::default().register(
                function,
                grap::ForeignFunction::from_value({
                    let calls = calls.clone();
                    move |context, call, environment| {
                        let value = context
                            .field(call, presentation::vocabulary::VALUE)
                            .unwrap();
                        assert_eq!(context.eval_to_value(value, environment)?, source);
                        let width = context.field(call, layout::vocabulary::WIDTH).unwrap();
                        let width = f64::read(&context.eval_to_value(width, environment)?).unwrap();
                        let height = context.field(call, layout::vocabulary::HEIGHT).unwrap();
                        let height =
                            f64::read(&context.eval_to_value(height, environment)?).unwrap();
                        calls.borrow_mut().push(Size::new(width, height));
                        Ok(layout::test_support::selectable_node(
                            layout::test_support::drawing(width, 0.0, height, []),
                        ))
                    }
                }),
            ),
        ),
    );
    let pane = &model.workspace.left.panes[0].view;
    let Target::Pane { path } = pane.root.target() else {
        panic!("pane")
    };
    let path = crate::test_examples::pane_root(path);
    for (size, scale) in [
        (Size::new(300.0, 500.0), 1.0),
        (Size::new(700.0, 400.0), 2.0),
    ] {
        let styles = style::editor(style::Theme::Light.palette(), scale);
        let mut fonts = crate::display::test_support::Fonts::new();
        let mut tcx = fonts.text(scale);
        let computations = crate::computations::Computations::default();
        let mut place = |view, pointer| {
            crate::display::widget::frame::place(
                project_workspace_view(
                    &workspace_description(&model, &stack, &computations, scale),
                    &styles,
                    &mut tcx,
                    view,
                    size,
                ),
                Placement::root(Rect::from_origin_size(Point::new(30.0, 40.0), size)),
                &crate::display::widget::HoverInput {
                    pointer,
                    ..Default::default()
                },
            )
        };
        let placed = place(pane, Some(Point::new(30.5, 40.5)));
        assert_eq!(calls.borrow_mut().pop(), Some(size / scale));
        let rect = Rect::from_origin_size(Point::new(30.0, 40.0), size);
        assert_eq!(placed.view_regions[0].rect, rect);
        assert_eq!(placed.view_regions[0].maximum, Vec2::ZERO);
        assert!(
            placed
                .landmarks
                .iter()
                .any(|node| node.path == path && node.rect == rect)
        );
        assert!(matches!(
            placed.claim.map(|(_, claim)| claim),
            Some(Claim::Direct(_))
        ));
        assert!(place(pane, Some(Point::new(29.5, 40.5))).claim.is_none());
        assert_eq!(
            model.workspace.left.panes[0].view.scroll,
            Vec2::new(75.0, 150.0)
        );

        let raw = workspace::View {
            root: pane.root.clone(),
            shown: workspace::Shown::RAW,
            annotations: Default::default(),
            scroll: Vec2::ZERO,
        };
        calls.borrow_mut().clear();
        let placed = place(&raw, None);
        assert!(calls.borrow().is_empty(), "Raw never invokes the viewport");
        let field_path =
            crate::occurrence::below(&path, &[Step::Key(presentation::vocabulary::VIEWPORT)]);
        assert!(placed.landmarks.landmark(&field_path).is_some());
        place(&model.workspace.document, None);
        assert!(
            calls.borrow().is_empty(),
            "the document leaves declarations editable"
        );
    }
}

#[test]
fn workspace_columns_are_editor_geometry_with_independent_view_regions() {
    let cell = CellId::from_u128(1);
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(b"pane".to_vec()));
    let mut model = Model::new(Document {
        root: Some(Value::record([])),
        cells,
    });
    let document = model.workspace.document_root().clone();
    for _ in 0..2 {
        Rc::make_mut(&mut model.doc).root = Some(
            workspace::append(
                model.doc.root.as_ref().unwrap(),
                workspace::Side::Left,
                Value::from(cell),
            )
            .unwrap()
            .0,
        );
    }
    model
        .workspace
        .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
    let upper = model.workspace.left.panes[0].view.root.clone();
    let lower = model.workspace.left.panes[1].view.root.clone();
    let stack = crate::stack::load();
    let styles = style::editor(style::Theme::Light.palette(), 1.0);
    let mut fonts = crate::display::test_support::Fonts::new();
    let mut tcx = fonts.text(1.0);
    let size = Size::new(801.0, 600.0);
    let placed = crate::display::widget::frame::place(
        project_workspace(
            &workspace_description(
                &model,
                &stack,
                &crate::computations::Computations::default(),
                1.0,
            ),
            &styles,
            &mut tcx,
            size,
        ),
        Placement::root(Rect::from_origin_size(Point::ZERO, size)),
        &Default::default(),
    );

    assert_eq!(placed.view_regions.len(), 3);
    let upper_region = placed
        .view_regions
        .iter()
        .find(|region| region.root == upper)
        .expect("upper pane");
    let lower_region = placed
        .view_regions
        .iter()
        .find(|region| region.root == lower)
        .expect("lower pane");
    let document_region = placed
        .view_regions
        .iter()
        .find(|region| region.root == document)
        .expect("document view");
    assert_eq!(
        (document_region.rect.y0, document_region.rect.y1),
        (0.0, 600.0)
    );
    assert_eq!(upper_region.rect.x0, lower_region.rect.x0);
    assert_eq!(upper_region.rect.x1, lower_region.rect.x1);
    assert_eq!(lower_region.rect.y0 - upper_region.rect.y1, 1.0);
    assert!(
        placed
            .landmarks
            .iter()
            .all(|landmark| landmark.root.is_some())
    );
    let Target::Pane { path } = upper.target() else {
        panic!("pane root")
    };
    let path = crate::test_examples::pane_root(path);
    assert!(placed.landmarks.iter().any(|landmark| {
        landmark.root.as_ref() == Some(&upper) && landmark.path == path.as_slice()
    }));
}

#[test]
fn hover_prefers_direct_claims_and_uses_extensions_only_to_retain() {
    let editor = crate::test_editor(gid::Document::default());
    let target = |index| Hovered::Tree(hover::Hover::Entry(index));
    let viewport = Rect::new(-100.0, -100.0, 100.0, 100.0);
    for (prior, pointer, pressed, covered, expected) in [
        (None, Some(5.0), false, false, Some(target(0))),
        (Some(target(0)), Some(12.0), false, false, Some(target(0))),
        (Some(target(1)), Some(12.0), false, false, Some(target(1))),
        (Some(target(1)), Some(8.0), false, false, Some(target(0))),
        (Some(target(0)), Some(16.0), false, false, Some(target(1))),
        (None, Some(12.0), false, false, None),
        (Some(target(0)), None, false, false, None),
        (Some(target(0)), Some(40.0), true, false, Some(target(0))),
        (None, Some(25.0), false, true, Some(Hovered::Blocked)),
    ] {
        let layout = crate::display::widget::leaf(
            measured::Extent::default(),
            move |output: &mut crate::display::widget::HoverContext<'_, Editor, Hovered>, _| {
                output.claim(Probe::retaining(
                    Placement::new(Rect::new(0.0, 0.0, 10.0, 10.0), viewport),
                    target(0),
                ));
                output.claim(Probe::retaining(
                    Placement::new(Rect::new(14.0, 0.0, 24.0, 10.0), viewport),
                    target(1),
                ));
                if covered {
                    output.claim(Probe::occludes(Placement::new(
                        Rect::new(0.0, 0.0, 40.0, 40.0),
                        viewport,
                    )));
                }
            },
        );
        assert_eq!(
            hover_frame(
                &editor,
                layout,
                pointer.map(|x| Point::new(x, 5.0)),
                prior.as_ref().map(|hover| (None, hover)),
                pressed
            )
            .hover,
            expected
        );
    }
}

#[test]
fn exact_hover_claims_do_not_retain_outside_their_hit_geometry() {
    let editor = crate::test_editor(gid::Document::default());
    let target = Hovered::Divider(workspace::Divider::Columns(workspace::Side::Left));
    for (x, pressed, expected) in [
        (5.0, false, Some(target.clone())),
        (11.0, false, None),
        (11.0, true, Some(target.clone())),
    ] {
        let claimed = target.clone();
        let layout = crate::display::widget::leaf(
            measured::Extent::default(),
            move |output: &mut crate::display::widget::HoverContext<'_, Editor, Hovered>, _| {
                output.claim(Probe::exact(
                    Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                    claimed,
                ));
            },
        );
        assert_eq!(
            hover_frame(
                &editor,
                layout,
                Some(Point::new(x, 5.0)),
                Some((None, &target)),
                pressed
            )
            .hover,
            expected
        );
    }
}

#[test]
fn a_hovered_target_shows_the_cursor_it_asked_for() {
    let mut runner = EditorRunner::new(crate::test_editor(gid::Document::default()));
    let target = Hovered::Divider(workspace::Divider::Columns(workspace::Side::Left));
    for (x, expected) in [
        (5.0, winit::window::CursorIcon::ColResize),
        (50.0, winit::window::CursorIcon::Default),
    ] {
        let claimed = target.clone();
        let layout = crate::display::widget::leaf(
            measured::Extent::default(),
            move |output: &mut crate::display::widget::HoverContext<'_, Editor, Hovered>, _| {
                output.claim(Probe::exact(
                    Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                    claimed.clone(),
                ));
                output.cursor(claimed, crate::display::widget::frame::Cursor::ColumnResize);
            },
        );
        let frame = hover_frame(
            &runner.editor,
            layout,
            Some(Point::new(x, 5.0)),
            None,
            false,
        );
        runner.frame.hover = frame.hover;
        runner.frame.dispatch = frame.dispatch;
        assert_eq!(runner.cursor(), expected);
    }
}

/// A frame's description over `model` with no menu bar, for projecting its
/// workspace.
fn workspace_description<'a>(
    model: &'a Model,
    stack: &'a crate::stack::Stack,
    computations: &'a crate::computations::Computations,
    scale: f64,
) -> FrameDescription<'a> {
    FrameDescription {
        palette: style::Theme::Light.palette(),
        command_modifier: crate::modifiers::native(),
        computations,
        focused: true,
        drawn_menu: false,
        model,
        stack,
        menu: menu::State::default(),
        availability: crate::command::Availability {
            save: false,
            undo: false,
            redo: false,
            open_pane: false,
            close_pane: false,
            show_panes: false,
            move_up: false,
            move_down: false,
            move_left: false,
            move_right: false,
            actual_size: false,
            zoom_in: false,
            zoom_out: false,
        },
        toggles: Default::default(),
        scale,
        viewport: Size::ZERO,
    }
}

fn hover_frame(
    editor: &Editor,
    layout: measured::Measured<HoverPass<Editor>>,
    position: Option<Point>,
    previous: Option<(Option<&Root>, &Hovered)>,
    pressed: bool,
) -> Frame {
    compute_hover(
        layout,
        &FrameDescription {
            palette: editor.palette,
            command_modifier: crate::modifiers::native(),
            computations: &editor.computations,
            focused: editor.focused,
            drawn_menu: false,
            model: &editor.model,
            stack: &editor.stack,
            menu: menu::State::default(),
            availability: editor.menu_availability(),
            toggles: editor.menu_toggles(),
            scale: 1.0,
            viewport: Size::new(100.0, 100.0),
        },
        PointerInput {
            position,
            previous,
            pressed,
            link_sources: false,
        },
    )
}

#[test]
fn building_a_frame_leaves_installed_hover_and_dispatch_unchanged() {
    let mut runner = EditorRunner::new(crate::test_editor(gid::Document::default()));
    let prior = Hovered::Tree(hover::Hover::Entry(7));
    runner.frame.hover = Some(prior.clone());
    let prior_root = Root::document();
    runner.frame.dispatch.pointer_root = Some(prior_root.clone());
    let frame =
        runner
            .editor
            .build_frame(1.0, Size::new(300.0, 200.0), runner.frame.hover_location());
    assert_eq!(runner.frame.hover, Some(prior));
    assert_eq!(runner.frame.dispatch.pointer_root, Some(prior_root));
    assert!(runner.frame.dispatch.landmarks.is_empty());
    assert_eq!(frame.hover, None);
    let pointer_root = frame.dispatch.pointer_root.clone();
    let landmarks = frame.dispatch.landmarks.clone();
    let paint = runner.install_frame(frame, 1.0, Size::new(300.0, 200.0));
    assert_eq!(runner.frame.hover, None);
    assert_eq!(runner.frame.dispatch.pointer_root, pointer_root);
    assert!(Rc::ptr_eq(&runner.frame.dispatch.landmarks, &landmarks));
    drop(paint);
}

#[test]
fn pressed_hover_keeps_its_owner_and_skips_probes_until_release() {
    let editor = crate::test_editor(gid::Document::default());
    let root = editor.model.workspace.document_root().clone();
    let prior_root = Root::pane(vec![Step::Key(gid::new_cell_id())]);
    let prior = Hovered::Tree(hover::Hover::Entry(7));
    for pressed in [true, false] {
        let calls = Rc::new(std::cell::Cell::new(0));
        let counted = calls.clone();
        let layout = crate::display::widget::view::in_view(
            crate::display::widget::leaf(
                measured::Extent::default(),
                move |output: &mut crate::display::widget::HoverContext<'_, Editor, Hovered>, _| {
                    output.claim(Probe::dynamic(
                        Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                        move |_| {
                            counted.set(counted.get() + 1);
                            Some(Hovered::Blocked)
                        },
                    ));
                },
            ),
            root.clone(),
        );
        let frame = hover_frame(
            &editor,
            layout,
            Some(Point::new(5.0, 5.0)),
            Some((Some(&prior_root), &prior)),
            pressed,
        );
        assert_eq!(calls.get(), usize::from(!pressed));
        assert_eq!(
            frame.hover,
            Some(if pressed {
                prior.clone()
            } else {
                Hovered::Blocked
            })
        );
        assert_eq!(
            frame.dispatch.pointer_root,
            Some(if pressed {
                prior_root.clone()
            } else {
                root.clone()
            })
        );
        let mut runner = EditorRunner::new(crate::test_editor(gid::Document::default()));
        runner.editor.pointer = Some(Point::new(6.0, 5.0));
        runner.editor.pressed = pressed;
        drop(runner.install_frame(frame, 1.0, Size::new(100.0, 100.0)));
        calls.set(0);
        runner.probe_pointer(1.0);
        assert_eq!(calls.get(), usize::from(!pressed));
        assert_eq!(
            runner.frame.hover,
            Some(if pressed {
                prior.clone()
            } else {
                Hovered::Blocked
            })
        );
        assert_eq!(
            runner.frame.dispatch.pointer_root,
            Some(if pressed {
                prior_root.clone()
            } else {
                root.clone()
            })
        );
        // Releasing allows the current geometry to choose a new owner.
        runner.editor.pressed = false;
        runner.probe_pointer(1.0);
        assert_eq!(runner.frame.hover, Some(Hovered::Blocked));
        assert_eq!(runner.frame.dispatch.pointer_root, Some(root.clone()));
    }
}

#[test]
fn a_selected_or_hovered_reference_marks_its_cell_in_every_view() {
    use crate::libraries::{absent, grap::vocabulary::GRAP, presentation};
    use puri::draw::{DrawCmd, Shape};
    let program = gid::new_cell_id();
    let doc = Document {
        root: Some(Value::record([
            (GRAP, program.into()),
            (
                workspace::vocabulary::PANES,
                Value::record([(
                    workspace::vocabulary::LEFT,
                    Value::list([presentation::render(program.into())]),
                )]),
            ),
        ])),
        cells: Cells::new(),
    };
    let mut driver =
        crate::driver::Driver::new(crate::test_editor(doc), crate::driver::Driver::VIEWPORT);
    let drawn = |driver: &crate::driver::Driver, key| {
        driver
            .drawn_under::<Step>(&[])
            .into_iter()
            .find(|(path, _)| path.last() == Some(&Occurs::Key(key)))
            .expect("drawn")
    };
    // The document's program field, and the pane's result naming it missing.
    let (field, missing) = (
        drawn(&driver, GRAP),
        drawn(&driver, absent::vocabulary::CELL),
    );
    assert!(
        missing
            .0
            .starts_with(&[Occurs::Key(workspace::vocabulary::PANES)])
    );
    let marked = |driver: &crate::driver::Driver, alpha, other: Rect| {
        let brush = Brush::from(style::Theme::Light.palette().accent.with_alpha(alpha));
        let outline = style::highlight_outline(1.0, other);
        crate::display::test_support::painted(&driver.painted().0).any(|command| {
            matches!(command, DrawCmd::Fill {
                shape: Shape::RoundedRect(rect),
                brush: fill,
                ..
            } if *fill == brush && *rect == outline)
        })
    };
    for (selected, other) in [(&field, &missing), (&missing, &field)] {
        driver.select(&selected.0.to_vec());
        assert!(
            marked(&driver, 0.10, other.1),
            "selecting {:?} marks {:?}",
            selected.0,
            other.0,
        );
    }
    driver.press(ui_events::keyboard::NamedKey::Escape);
    assert!(driver.editor().model.selection.is_none());
    for (hovered, other) in [(&field, &missing), (&missing, &field)] {
        // On the reference's opening parenthesis, not the missing value inside.
        driver.move_to(Point::new(hovered.1.x0 + 2.0, hovered.1.center().y));
        assert!(
            marked(&driver, 0.05, other.1),
            "hovering {:?} marks {:?}",
            hovered.0,
            other.0,
        );
    }
}
