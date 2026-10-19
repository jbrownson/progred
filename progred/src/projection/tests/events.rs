use super::*;
use puri::handler::test_support::DispatchShorthands;

#[test]
fn native_annotation_handler_retains_the_projected_site() {
    let cell = new_cell_id();
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(vec![7u8]));
    let doc = Document {
        root: Some(cell.into()),
        cells,
    };
    let projection = Projection::new([crate::display::partial(|input| {
        input.value?.as_blob()?;
        Some(crate::display::widget::before(
            crate::display::test_support::text("scroll"),
            Rc::new(|context| {
                let root = context.toolkit.view().clone();
                let path = context.path().to_vec();
                crate::display::widget::scroll::scroll(
                    context.toolkit.styles().scale,
                    move |world: &mut crate::Editor, delta| {
                        crate::editing::Scope::default()
                            .open(world.session())
                            .annotate(&root, &path, f64::value(delta.y));
                        puri::handler::ScrollOutcome::handled_leaving(Default::default())
                    },
                )
            }),
        ))
    })]);
    let mut writes = crate::test_editor(doc);
    writes.stack.libraries = Libraries::default();
    let placed = framed(
        &mut writes,
        Framing {
            projection: Some(&projection),
            ..Framing::default()
        },
    );
    let center = placed
        .landmarks
        .landmark(&path)
        .expect("the cell's value is drawn")
        .rect
        .center();
    let event = PointerScrollEvent {
        pointer: crate::display::test_support::mouse(),
        state: PointerState {
            position: (center.x, center.y).into(),
            ..Default::default()
        },
        delta: ScrollDelta::LineDelta(0.0, 1.0),
    };
    let outcome = placed
        .resolve_for_dispatch()
        .dispatch_scroll(&mut writes, &event);
    assert!(outcome.handled());
    assert!(outcome.remaining.is_none());
    assert_eq!(
        writes.model.workspace.document.annotations.at(&path),
        Some(&f64::value(40.0))
    );
}
