//! Exercise the drawn menus through the installed frame's real hit-testing
//! and event handlers, without creating a window.
use super::*;
use crate::driver::Driver;
use crate::frame::Hovered;
use gid::{Cells, Document};
use kurbo::{Point, Size};
use ui_events::keyboard::{KeyState, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::PointerEvent;

// Tall enough for the whole Projections menu.
const VIEWPORT: Size = Size::new(640.0, 720.0);

fn driver(doc: Document) -> Driver {
    driver_in(doc, VIEWPORT)
}

fn driver_in(doc: Document, viewport: Size) -> Driver {
    let mut editor = crate::test_editor(doc);
    editor.drawn_menu = true;
    editor.command_modifier = puri::keyboard::CommandModifier::Control;
    Driver::new(editor, viewport)
}

fn empty() -> Driver {
    driver(Document::default())
}

/// The menus `driver`'s editor draws.
fn menus(driver: &Driver) -> Vec<Menu> {
    let stack = &driver.runner.editor.stack;
    definition(
        stack.switchable().map(|(library, _)| library),
        crate::libraries::name::library_names(&stack.libraries).map(|(library, _)| library),
    )
}

fn menu_at(driver: &Driver, point: Point) -> Option<Hover> {
    match driver.hover_at(point) {
        Some(Hovered::Menu(hover)) => Some(hover),
        _ => None,
    }
}

fn point_for(driver: &Driver, hover: Hover) -> Point {
    driver.locate(Hovered::Menu(hover))
}

fn key(driver: &mut Driver, key: Key, modifiers: Modifiers) {
    let named = format!("{key:?}");
    assert!(driver.keystroke(key, modifiers), "{named} did nothing");
}

#[test]
fn pointer_switches_open_menus_but_does_not_open_closed_ones() {
    let mut driver = empty();
    let file = point_for(&driver, Hover::Heading(0));
    let examples = point_for(&driver, Hover::Heading(1));
    driver.move_to(file);
    assert_eq!(driver.runner.editor.menu.open(), None);
    driver.click_at(file);
    assert_eq!(driver.runner.editor.menu.open(), Some(0));
    driver.move_to(examples);
    assert_eq!(driver.runner.editor.menu.open(), Some(1));
    let item = point_for(
        &driver,
        Hover::Item(Command::App(AppCommand::Example(Example::Fidget))),
    );
    driver.move_to(item);
    assert_eq!(driver.runner.editor.menu.cursor(), Some(1));
    key(
        &mut driver,
        Key::Named(NamedKey::ArrowDown),
        Modifiers::empty(),
    );
    assert_eq!(driver.runner.editor.menu.cursor(), Some(2));
    driver.move_to(item + (1.0, 0.0));
    assert_eq!(driver.runner.editor.menu.cursor(), Some(1));
    driver.click_at(examples);
    assert_eq!(driver.runner.editor.menu.open(), None);
}

#[test]
fn tapping_alt_enters_and_leaves_the_bar_unless_something_comes_between() {
    let mut driver = empty();
    let alt = |driver: &mut Driver, state| {
        let scale = driver.runner.editor.zoom;
        driver.runner.keyboard_event(
            &KeyboardEvent {
                key: Key::Named(NamedKey::Alt),
                state,
                modifiers: Modifiers::ALT,
                ..Default::default()
            },
            scale,
            VIEWPORT,
        )
    };
    alt(&mut driver, KeyState::Down);
    assert!(alt(&mut driver, KeyState::Up));
    assert_eq!(driver.runner.editor.menu.open(), Some(0));
    key(
        &mut driver,
        Key::Named(NamedKey::ArrowRight),
        Modifiers::empty(),
    );
    assert_eq!(driver.runner.editor.menu.open(), Some(1));
    alt(&mut driver, KeyState::Down);
    assert!(alt(&mut driver, KeyState::Up));
    assert_eq!(driver.runner.editor.menu.open(), None);
    // A click, a window moved by Alt+drag, or a trip to another window.
    for between in ["click", "move", "focus"] {
        alt(&mut driver, KeyState::Down);
        let scale = driver.runner.editor.zoom;
        match between {
            "click" => driver.click_at(Point::new(320.0, 300.0)),
            "move" => {
                driver.runner.window_moved(scale, VIEWPORT);
            }
            _ => {
                driver.runner.focus_changed(false, scale, VIEWPORT);
                driver.runner.focus_changed(true, scale, VIEWPORT);
            }
        }
        assert!(!alt(&mut driver, KeyState::Up), "{between}");
        assert_eq!(driver.runner.editor.menu.open(), None, "{between}");
    }
}

#[test]
fn tapping_alt_enters_the_bar_when_the_page_reports_focus_before_each_key() {
    let mut driver = empty();
    let scale = driver.runner.editor.zoom;
    for (state, modifiers) in [
        (KeyState::Down, Modifiers::ALT),
        (KeyState::Up, Modifiers::empty()),
    ] {
        driver.runner.focus_changed(true, scale, VIEWPORT);
        driver.runner.forwarded_key(
            &KeyboardEvent {
                key: Key::Named(NamedKey::Alt),
                state,
                modifiers,
                ..Default::default()
            },
            scale,
            VIEWPORT,
        );
    }
    assert_eq!(driver.runner.editor.menu.open(), Some(0));
}

#[test]
fn disabled_items_and_outside_clicks_do_not_reach_the_document() {
    let mut driver = empty();
    let edit = point_for(&driver, Hover::Heading(2));
    driver.click_at(edit);
    let undo = point_for(&driver, Hover::Item(Command::Doc(DocCommand::Undo)));
    driver.click_at(undo);
    assert_eq!(driver.runner.editor.menu.open(), Some(2));
    assert_eq!(driver.runner.editor.menu.cursor(), None);
    assert!(driver.runner.editor.model.selection.is_none());
    driver.click_at(Point::new(600.0, 300.0));
    assert_eq!(driver.runner.editor.menu.open(), None);
    assert!(driver.runner.editor.model.selection.is_none());
}

#[test]
fn open_menu_accepts_shortcuts_and_pointer_cancellation() {
    let mut driver = empty();
    key(&mut driver, Key::Named(NamedKey::F10), Modifiers::empty());
    assert_eq!(driver.runner.editor.menu.open(), Some(0));
    assert_ne!(
        driver.runner.editor.menu_toggles().preset,
        Some(crate::stack::Preset::Raw)
    );
    key(&mut driver, Key::Character("r".into()), Modifiers::CONTROL);
    assert!(driver.runner.editor.menu_toggles().preset == Some(crate::stack::Preset::Raw));
    assert_eq!(driver.runner.editor.menu.open(), None);
    key(&mut driver, Key::Named(NamedKey::F10), Modifiers::empty());
    driver.runner.pointer_event(
        &PointerEvent::Cancel(crate::display::test_support::mouse()),
        1.0,
        VIEWPORT,
    );
    assert_eq!(driver.runner.editor.menu.open(), None);
}

#[test]
fn clicking_a_command_executes_it_and_closes_the_menu() {
    let mut driver = empty();
    let projections = point_for(&driver, Hover::Heading(4));
    driver.click_at(projections);
    let raw = point_for(
        &driver,
        Hover::Item(Command::Doc(DocCommand::Preset(crate::stack::Preset::Raw))),
    );
    driver.click_at(raw);
    assert!(driver.runner.editor.menu_toggles().preset == Some(crate::stack::Preset::Raw));
    assert_eq!(driver.runner.editor.menu.open(), None);
    key(&mut driver, Key::Named(NamedKey::F10), Modifiers::empty());
    key(&mut driver, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(driver.runner.editor.menu.open(), None);
}

#[test]
fn bar_hit_height_tracks_display_scale() {
    // Room for the largest scale, so the whole bar is found at each.
    let mut driver = driver_in(Document::default(), VIEWPORT * 2.0);
    for scale in [1.0, 1.5, 2.0] {
        // The window keeps its logical size as the display's scale changes.
        driver.runner.refresh_frame(scale, VIEWPORT * scale);
        for index in 0..menus(&driver).len() {
            let point = point_for(&driver, Hover::Heading(index));
            for y in [0.5, bar_height(scale) - 0.5] {
                assert_eq!(
                    menu_at(&driver, Point::new(point.x, y)),
                    Some(Hover::Heading(index))
                );
            }
        }
    }
}

#[test]
fn headings_fill_the_bar_and_popup_rows_share_their_hit_width() {
    let mut driver = empty();
    let examples = point_for(&driver, Hover::Heading(1));
    for y in [0.5, bar_height(1.0) - 0.5] {
        assert_eq!(
            menu_at(&driver, Point::new(examples.x, y)),
            Some(Hover::Heading(1))
        );
    }
    driver.click_at(examples);
    let mut width = None;
    for command in commands(&menus(&driver)[1..2]) {
        let hover = Hover::Item(command);
        let point = point_for(&driver, hover);
        let columns: Vec<_> = (0..VIEWPORT.width as usize)
            .filter(|x| menu_at(&driver, Point::new(*x as f64, point.y)) == Some(hover))
            .collect();
        assert!(!columns.is_empty());
        let extent = (*columns.first().unwrap(), *columns.last().unwrap());
        assert_eq!(*width.get_or_insert(extent), extent);
    }
}

#[test]
fn a_popup_near_the_right_edge_stays_inside_the_viewport() {
    let mut driver = empty();
    driver.runner.editor.menu.toggle(4);
    driver
        .runner
        .refresh_frame(1.0, Size::new(350.0, VIEWPORT.height));
    let raw = Hover::Item(Command::Doc(DocCommand::Preset(crate::stack::Preset::Raw)));
    let point = point_for(&driver, raw);
    assert_eq!(menu_at(&driver, Point::new(348.0, point.y)), Some(raw));
    assert_eq!(menu_at(&driver, Point::new(350.0, point.y)), None);
}

#[test]
fn zoom_shortcuts_step_the_window_zoom_and_stop_at_its_ends() {
    let mut driver = empty();
    let character = |key: &str| Key::Character(key.into());
    key(&mut driver, character("="), Modifiers::CONTROL);
    assert_eq!(driver.runner.editor.zoom, 1.1);
    key(
        &mut driver,
        character("+"),
        Modifiers::CONTROL | Modifiers::SHIFT,
    );
    assert_eq!(driver.runner.editor.zoom, 1.25);
    key(&mut driver, character("-"), Modifiers::CONTROL);
    assert_eq!(driver.runner.editor.zoom, 1.1);
    key(&mut driver, character("0"), Modifiers::CONTROL);
    assert_eq!(driver.runner.editor.zoom, 1.0);
    let zoom_out = Command::Doc(DocCommand::ZoomOut);
    while driver.runner.editor.menu_availability().enabled(zoom_out) {
        key(&mut driver, character("-"), Modifiers::CONTROL);
    }
    assert_eq!(driver.runner.editor.zoom, 0.5);
}

#[test]
fn a_projections_entry_switches_one_library_off_in_the_selected_area() {
    use crate::libraries::color;
    let mut driver = driver(Document {
        root: Some(color::tests::value(peniko::Color::from_rgb8(
            0x50, 0x96, 0xdc,
        ))),
        cells: Cells::new(),
    });
    let command = Command::Doc(DocCommand::Projection(color::ID));
    let heading = menus(&driver)
        .iter()
        .position(|menu| menu.label == "Projections")
        .unwrap();
    // A color draws as a swatch and its hex; only with the color library's
    // projection off is it the stored record, whose `rgb` field is drawn.
    let rgb = [gid::Step::Key(color::vocabulary::RGB)];
    let stored = |driver: &Driver| driver.drawn(&rgb);
    assert!(driver.runner.editor.menu_toggles().checked(command));
    assert!(!stored(&driver));
    for on in [false, true] {
        let projections = point_for(&driver, Hover::Heading(heading));
        driver.click_at(projections);
        let entry = point_for(&driver, Hover::Item(command));
        driver.click_at(entry);
        driver.runner.refresh_frame(1.0, VIEWPORT);
        assert_eq!(driver.runner.editor.menu_toggles().checked(command), on);
        assert_eq!(stored(&driver), !on);
    }
}

#[test]
fn the_document_libraries_entry_switches_the_documents_own_projections() {
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../../examples/libraries.gid")).unwrap();
    let mut driver = driver(doc);
    let command = Command::Doc(DocCommand::Projection(crate::libraries::declared::ID));
    let heading = menus(&driver)
        .iter()
        .position(|menu| menu.label == "Projections")
        .unwrap();
    // The document's fraction library stacks a fraction's parts; the stored
    // record lists them on one line.
    let stacked = |driver: &Driver| {
        let part = |key: &str| {
            driver
                .runner
                .frame
                .dispatch
                .landmarks
                .iter()
                .find(|landmark| {
                    landmark
                        .path
                        .starts_with(&[crate::occurrence::Step::Key(names["fractions"])])
                        && landmark.path.last() == Some(&crate::occurrence::Step::Key(names[key]))
                })
                .unwrap()
                .rect
        };
        part("over").y1 <= part("under").y0
    };
    assert!(stacked(&driver));
    for on in [false, true] {
        let projections = point_for(&driver, Hover::Heading(heading));
        driver.click_at(projections);
        let entry = point_for(&driver, Hover::Item(command));
        driver.click_at(entry);
        driver.runner.refresh_frame(1.0, VIEWPORT);
        assert_eq!(driver.runner.editor.menu_toggles().checked(command), on);
        assert_eq!(stacked(&driver), on);
    }
}

#[test]
fn the_name_entry_switches_labels_to_identities() {
    use crate::libraries::{f64, layout, name};
    let mut driver = driver(Document {
        root: Some(gid::Value::record([(
            layout::vocabulary::WIDTH,
            f64::value(2.0),
        )])),
        cells: Cells::new(),
    });
    let command = Command::Doc(DocCommand::Projection(name::ID));
    let heading = menus(&driver)
        .iter()
        .position(|menu| menu.label == "Projections")
        .unwrap();
    let glyphs = |driver: &Driver| {
        crate::display::test_support::painted(&driver.painted().0)
            .map(|command| match command {
                puri::draw::DrawCmd::GlyphRun(run) => run.glyphs.len(),
                _ => 0,
            })
            .sum::<usize>()
    };
    let named = glyphs(&driver);
    let projections = point_for(&driver, Hover::Heading(heading));
    driver.click_at(projections);
    let entry = point_for(&driver, Hover::Item(command));
    driver.click_at(entry);
    driver.runner.refresh_frame(1.0, VIEWPORT);
    driver.present();
    assert!(!driver.runner.editor.menu_toggles().checked(command));
    // The field's label and the number's representation tag both give way
    // to their identities.
    let lengthened = [
        (layout::vocabulary::WIDTH, "width"),
        (f64::vocabulary::F64, "f64"),
    ]
    .map(|(cell, name)| crate::identity::short_id(cell).chars().count() - name.len())
    .iter()
    .sum::<usize>();
    assert_eq!(glyphs(&driver) - named, lengthened);
}

#[test]
fn the_version_copies_from_the_bar_and_the_help_menu() {
    use puri::edit::TextClipboard;
    let mut driver = empty();
    driver.click_at(point_for(&driver, Hover::Stage));
    assert_eq!(
        driver.runner.editor.text_clipboard.get_text(),
        Some(crate::editor::stage())
    );
    assert_eq!(driver.runner.editor.menu.open(), None);

    driver.runner.editor.text_clipboard = Default::default();
    let help = menus(&driver).len() - 1;
    driver.click_at(point_for(&driver, Hover::Heading(help)));
    driver.click_at(point_for(
        &driver,
        Hover::Item(Command::Doc(DocCommand::CopyVersion)),
    ));
    assert_eq!(
        driver.runner.editor.text_clipboard.get_text(),
        Some(crate::editor::stage())
    );
}
