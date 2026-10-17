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

use crate::display::test_support::{ProjectionCall, inspect};
use crate::display::{Env, RowAlignment};
use gid::new_cell_id;

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
        puri::DrawCmd::Fill {
            shape: puri::Shape::RoundedRect(fill),
            brush,
            ..
        },
        puri::DrawCmd::Stroke {
            shape: puri::Shape::RoundedRect(border),
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
fn spelling_round_trips_through_the_update_function() {
    let extra = new_cell_id();
    let current = Value::record(
        value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff))
            .as_record()
            .unwrap()
            .clone()
            .update(extra, Value::from(b"metadata".to_vec())),
    );
    let update = |input: &str| {
        crate::libraries::test_evaluate(
            &::grap::call(
                ::grap::ffi(vocabulary::UPDATE),
                [
                    (line_edit::vocabulary::CURRENT, current.clone()),
                    (line_edit::vocabulary::INPUT, text::value(input)),
                ],
            ),
            |_| None,
            &functions(),
            100,
        )
        .result
    };

    let updated = update("ebb4cc99");
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
        read(&update("b4e")),
        Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0xff)),
    );
    assert_eq!(
        read(&update("b4e8")),
        Some(Color::from_rgba8(0xbb, 0x44, 0xee, 0x88)),
    );
    assert!(crate::libraries::absent::is_absent(&update(
        "rebeccapurple"
    )));
}

#[test]
fn projection_is_a_swatch_and_editable_spelling() {
    let color = value(Color::from_rgba8(0xb4, 0xe0, 0xfe, 0xff));
    let target = |_| crate::display::ProjectionTarget {
        select: std::rc::Rc::new(|_| false),
        select_with: std::rc::Rc::new(|_, _| false),
        hover: crate::libraries::test_widgets::hover(vec![]),
    };
    let layout = display(&ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &NoEval,
        value: Some(&(&color).into()),
        scale_factor: 1.0,
        writable: true,
        selection: None,
        pending: None,
        state: None,
        targets: crate::display::ProjectionTargets::new(&target),
    })
    .expect("color projection");
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
    let target = |_| crate::display::ProjectionTarget {
        select: std::rc::Rc::new(|_| false),
        select_with: std::rc::Rc::new(|_, _| false),
        hover: crate::libraries::test_widgets::hover(vec![]),
    };
    let layout = display(&ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &NoEval,
        value: Some(&(&color).into()),
        scale_factor: 1.0,
        writable: false,
        selection: Some(&selection),
        pending: None,
        state: None,
        targets: crate::display::ProjectionTargets::new(&target),
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
    let target = |_| crate::display::ProjectionTarget {
        select: std::rc::Rc::new(|_| false),
        select_with: std::rc::Rc::new(|_, _| false),
        hover: crate::libraries::test_widgets::hover(vec![]),
    };
    let layout = display(&ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &NoEval,
        value: Some(&(&color).into()),
        scale_factor: 1.0,
        writable: true,
        selection: None,
        pending: None,
        state: None,
        targets: crate::display::ProjectionTargets::new(&target),
    })
    .expect("named color projection");
    let Recorded::Row { children, .. } = layout.record() else {
        panic!("named color projection is one row")
    };

    assert!(matches!(&inspect(&(children[1])),
        ProjectionCall::Descend {
            step: Step::Key(field),
            ..
        } if *field == name::vocabulary::NAME
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
    let target = |_| crate::display::ProjectionTarget {
        select: std::rc::Rc::new(|_| false),
        select_with: std::rc::Rc::new(|_, _| false),
        hover: crate::libraries::test_widgets::hover(vec![]),
    };
    // Its extra field would follow the color in a record of its own.
    let plain = value(Color::from_rgba8(0x66, 0x33, 0x99, 0xff));
    let layout = display(&ProjectionInput {
        default_projection: crate::display::runtime_partial(|_| None),
        env: &NoEval,
        value: Some(&(&plain).into()),
        scale_factor: 1.0,
        writable: true,
        selection: Some(&selection),
        pending: None,
        state: None,
        targets: crate::display::ProjectionTargets::new(&target),
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

struct NoEval;

impl Env for NoEval {
    fn apply_scoped(
        &self,
        _: &gid::Value,
        _: &[(gid::CellId, gid::Value)],
        _scope: Option<&::grap::ForeignOverlay<'_>>,
    ) -> ::grap::Evaluation<gid::Value> {
        panic!("unexpected projection application")
    }

    fn evaluate(&self, _: &Value) -> Value {
        panic!("color projection does not evaluate while projecting")
    }
}
