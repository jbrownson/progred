use super::*;

#[test]
fn new_color_is_only_offered_where_its_picker_can_be_presented() {
    use crate::display::{CompletionKind, CompletionRequest, CompletionScope};
    for raw in [false, true] {
        for kind in [CompletionKind::Value, CompletionKind::Field] {
            let request = CompletionRequest {
                raw,
                kind,
                query: "color",
                scope: CompletionScope::Everything,
                path: &[],
                value_at: &|_| None,
                resolve: &|_| None,
                cells: &Vec::new,
                name: &|_| None,
                libraries: None,
            };
            assert_eq!(
                completions(&request).is_some(),
                !raw && kind == CompletionKind::Value
            );
        }
    }
}
use crate::display::recording::{Recordable, Recorded};

use crate::display::RowAlignment;
use crate::display::test_support::{NoEval, ProjectionCall, inspect};
use crate::libraries::test_widgets::projection_input;
use gid::{Cells, new_cell_id};

#[test]
fn swatch_keeps_its_metrics_fill_and_inset_border() {
    let color = Color::from_rgba8(0xb4, 0xe0, 0xfe, 0x99);
    let (extent, drawing) = crate::libraries::test_widgets::paint(&swatch(color));
    assert_eq!(
        extent,
        widget::Extent {
            width: 15.0,
            ascent: 11.0,
            descent: 4.0
        }
    );
    let [
        puri::draw::DrawCmd::Fill {
            shape: puri::draw::Shape::RoundedRect(fill),
            brush,
            ..
        },
        puri::draw::DrawCmd::Stroke {
            shape: puri::draw::Shape::RoundedRect(border),
            style,
            ..
        },
    ] = drawing.0.as_slice()
    else {
        panic!("fill followed by border");
    };
    assert_eq!(fill, border);
    assert_eq!(
        *fill,
        RoundedRect::from_rect(Rect::new(0.5, 0.5, 14.5, 14.5), 2.5)
    );
    assert_eq!(*brush, color.into());
    assert_eq!(style.width, 1.0);
}

#[test]
fn rgb_and_rgba_are_open_library_data() {
    let color = Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff);
    let extra = new_cell_id();
    let enriched = Value::record(
        value(color)
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"metadata".to_vec())),
    );

    assert_eq!(
        value(color).as_record().unwrap().get(&vocabulary::RGB),
        Some(&Value::from(vec![0xb4, 0xe0, 0xfe])),
    );
    assert_eq!(read(&enriched), Some(color));
    assert_eq!(
        value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0x99))
            .as_record()
            .unwrap()
            .get(&vocabulary::RGBA),
        Some(&Value::from(vec![0xb4, 0xe0, 0xfe, 0x99])),
    );
}

#[test]
fn spelling_round_trips_through_the_line_conversion() {
    let extra = new_cell_id();
    let current = Value::record(
        value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff))
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"metadata".to_vec())),
    );
    let update = |input: &str| edit(input, Some(&current));

    let updated = update("ebb4cc99").unwrap();
    assert_eq!(
        read(&updated),
        Some(Color::from_rgba8(0xeb, 0xb4, 0xcc, 0x99)),
    );
    assert_eq!(
        updated.as_record().unwrap().get(&extra),
        Some(&Value::from(b"metadata".to_vec())),
    );
    assert!(!updated.as_record().unwrap().contains_key(&vocabulary::RGB));
    assert_eq!(
        updated.as_record().unwrap().get(&vocabulary::RGBA),
        Some(&Value::from(vec![0xeb, 0xb4, 0xcc, 0x99])),
    );
    assert_eq!(
        update("b4e").as_ref().and_then(read),
        Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0xff)),
    );
    assert_eq!(
        update("b4e8").as_ref().and_then(read),
        Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0x88)),
    );
    assert_eq!(update("rebeccapurple"), None);
}

#[test]
fn projection_is_a_swatch_and_editable_spelling() {
    let color = value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff));
    let layout = display(&projection_input(&NoEval, &(&color).into())).expect("color projection");
    let Recorded::Row {
        alignment,
        children,
        ..
    } = layout.record()
    else {
        panic!("color projection is one row")
    };

    assert!(matches!(alignment, RowAlignment::Center));
    assert!(matches!(
        children[0],
        Recorded::Before { ref child, .. }
            if matches!(child.as_ref(), Recorded::Before { child, .. }
                if matches!(child.as_ref(), Recorded::Widget(_)))
    ));
    assert!(matches!(
        hex_editor(encoded(&color).unwrap()),
        line
            if line.text == "b4e0fe"
                && line.prefix == "#"
                && line.suffix.is_empty()
                && line.family == TextFamily::Monospace
    ));
}

#[test]
fn a_read_only_color_selects_without_a_picker_or_popup() {
    let color = value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff));
    let selection = picker_selection(0.1);
    let layout = display(&ProjectionInput {
        writable: false,
        selection: Some(&selection),
        ..projection_input(&NoEval, &(&color).into())
    })
    .expect("read-only color projection");
    let Recorded::Row { children, .. } = layout.record() else {
        panic!("color projection is one row")
    };

    // The hover claim around a plain selecting activation around the
    // swatch; no popup wraps it.
    assert!(matches!(
        children[0],
        Recorded::Before { ref child, .. }
            if matches!(
                child.as_ref(),
                Recorded::Before { child, .. } if matches!(child.as_ref(), Recorded::Widget(_))
            )
    ));
}

#[test]
fn a_named_color_projects_its_editable_name_and_hex() {
    let color = name::record(
        "rebeccapurple",
        [(vocabulary::RGB, Value::from(vec![0x66, 0x33, 0x99]))],
    );
    let layout =
        display(&projection_input(&NoEval, &(&color).into())).expect("named color projection");
    let Recorded::Row { children, .. } = layout.record() else {
        panic!("named color projection is one row")
    };

    assert!(matches!(&inspect(&(children[1])),
        ProjectionCall::Descend {
            steps,
            ..
        } if *steps == [Step::Key(name::vocabulary::NAME)]
    ));
    assert!(matches!(
        hex_editor(encoded(&color).unwrap()),
        line if line.text == "663399"
    ));
}

#[test]
fn picker_mode_floats_point_controls_and_preserves_open_metadata() {
    let extra = new_cell_id();
    let color = Value::record(
        value(Color::from_rgba8(0x66, 0x33, 0x99, 0xff))
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(vec![7])),
    );
    let selection = picker_selection(0.1);
    // Its extra field would follow the color in a record of its own.
    let plain = value(Color::from_rgba8(0x66, 0x33, 0x99, 0xff));
    let layout = display(&ProjectionInput {
        selection: Some(&selection),
        ..projection_input(&NoEval, &(&plain).into())
    })
    .expect("selected color projection");
    let Recorded::Row { children, .. } = layout.record() else {
        panic!("color projection is one row")
    };
    let Recorded::Floating { .. } = &children[0] else {
        panic!("picker mode floats from the swatch")
    };
    let Recorded::Col { children, .. } = picker(&color, encoded(&color).unwrap(), 0.1).record()
    else {
        panic!("picker controls are stacked")
    };
    let updated =
        crate::libraries::test_widgets::point_update(&children[0], PointEvent { x: 1.0, y: 0.0 });

    assert_eq!(
        updated.value.as_record().unwrap().get(&extra),
        Some(&Value::from(vec![7]))
    );
    assert!(matches!(encoded(&updated.value), Some(Encoded::Rgb(_))));
    assert!(updated.selection.is_none());

    let updated =
        crate::libraries::test_widgets::point_update(&children[1], PointEvent { x: 0.25, y: 0.0 });
    assert_eq!(picker_hue(updated.selection.as_ref()), Some(0.25));
}

#[test]
fn closing_the_picker_removes_only_its_selection_state() {
    let other = new_cell_id();
    let mut selection = picker_selection(0.25).as_record().unwrap().clone();
    selection.insert(other, Value::from(vec![7]));
    let closed = without_picker(Some(&Value::Record(selection)));

    assert_eq!(picker_hue(Some(&closed)), None);
    assert_eq!(
        closed.as_record().unwrap().get(&other),
        Some(&Value::from(vec![7]))
    );
}

#[test]
fn a_read_only_swatch_selects_its_color() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let color = value(Color::from_rgb8(0x50, 0x96, 0xdc));
    let mut driver = crate::driver::Driver::new(
        crate::test_editor(gid::Document {
            root: Some(Value::record([(::grap::vocabulary::EVALUATE, color)])),
            cells: gid::Cells::new(),
        }),
        kurbo::Size::new(900.0, 600.0),
    );
    // The evaluated result is computed, so it's read-only.
    let result = vec![gid::Step::Key(RESULT)];
    let rect = driver.rect(&result);
    driver.click_at(kurbo::Point::new(rect.x0 + 7.0, rect.center().y));
    assert_eq!(driver.selection(), result);
    let selection = driver.editor().model.selection.as_ref().unwrap();
    assert!(
        selection
            .payload()
            .as_record()
            .is_none_or(|fields| !fields.contains_key(&vocabulary::PICKER)),
        "no picker opens on a read-only color"
    );
}

#[test]
fn new_color_completion_opens_without_editing_and_commits_at_the_missing_location() {
    use crate::driver::Driver;
    use crate::selection::Stage;
    use kurbo::Point;
    use puri::draw::{DrawCmd, Shape};
    use ui_events::keyboard::{Key, NamedKey};
    let function = new_cell_id();
    let argument = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(function, ::grap::lambda([argument], argument.into()));
    let root = ::grap::call(function.into(), []);
    let mut driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(root.clone()),
            cells,
        }),
        kurbo::Size::new(900.0, 600.0),
    );
    // A Grap argument's hole offers the libraries' values, a new color among
    // them.
    let path = vec![Step::Key(argument)];
    let open = |driver: &mut Driver| {
        driver.select(&path);
        driver.type_text("new color");
        driver.choose("new color");
    };
    let document = driver.editor().model.doc.clone();
    let unchanged = |driver: &Driver| {
        std::rc::Rc::ptr_eq(&document, &driver.editor().model.doc)
            && !driver.editor().model.history.can_undo()
    };
    open(&mut driver);
    assert!(unchanged(&driver));
    assert_eq!(driver.selection(), path);
    assert!(
        driver.picker_entries().is_none(),
        "picker replaces completion"
    );
    driver.select(&[]);
    assert!(unchanged(&driver), "leaving does not insert the seed");
    open(&mut driver);
    driver.press(NamedKey::Escape);
    assert!(unchanged(&driver));
    assert!(
        driver.picker_entries().is_some(),
        "Escape returns to the offers"
    );

    // Drag across the color plane the picker draws.
    driver.select(&[]);
    open(&mut driver);
    let plane = crate::display::test_support::painted(&driver.painted().0)
        .find_map(|command| match command {
            DrawCmd::Fill {
                shape: Shape::Rect(rect),
                brush: peniko::Brush::Gradient(_),
                transform,
                ..
            } if rect.width() == puri_widgets::color_picker::WIDTH
                && rect.height() == puri_widgets::color_picker::PLANE_HEIGHT =>
            {
                Some(transform.transform_rect_bbox(*rect))
            }
            _ => None,
        })
        .expect("visible color plane");
    driver.drag(
        plane.center(),
        &[Point::new(
            plane.x0 + plane.width() * 0.75,
            plane.center().y,
        )],
    );
    assert!(read(&driver.value(&path)).is_some());
    assert_eq!(driver.selection(), path);
    assert_eq!(driver.stage(), Some(Stage::Edge));
    assert!(driver.picker_entries().is_none());
    assert!(
        driver.keystroke(Key::Character("z".into()), crate::modifiers::command()),
        "one undo for the whole drag"
    );
    assert_eq!(driver.editor().model.doc.root, Some(root));
    assert!(!driver.editor().model.history.can_undo());
}
