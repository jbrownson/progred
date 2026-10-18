use super::*;

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
    let libraries = Libraries::default();
    let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0);
    let projection = Projection::new([crate::display::partial(|input| {
        input.value?.as_blob()?;
        Some(crate::display::widget::before(
            crate::display::text("scroll"),
            Rc::new(|context| {
                let root = context.toolkit.view().clone();
                let path = context.path().to_vec();
                crate::display::widget::scroll::scroll(
                    context.toolkit.styles().scale,
                    move |world: &mut crate::Editor, delta| {
                        crate::editing::annotate(
                            &mut world.session(),
                            &root,
                            &path,
                            f64::value(delta.y),
                        );
                        puri::handler::ScrollOutcome::with_remainder(Default::default())
                    },
                )
            }),
        ))
    })]);
    let mut fonts = crate::fonts::bundled_font_context();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let computations = crate::computations::Computations::from_sources(Sources {
        doc: &doc,
        libraries: &libraries,
    });
    let measured = project(
        ProjectDescription {
            focused: true,
            computations: &computations,
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &doc,
                libraries: &libraries,
            },
            root: doc.cells.value(cell),
            root_path: &path,
            selection: None,
            source_selection: None,
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 500.0,
            projection: Some(&projection),
        },
        &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
    );
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let placed = crate::display::widget::frame::place(measured, placement, &Default::default());
    let mut writes = crate::test_editor(doc.clone());
    let event = PointerScrollEvent {
        pointer: crate::display::test_support::mouse(),
        state: PointerState {
            position: (placement.rect.center().x, placement.rect.center().y).into(),
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
