use super::*;
use crate::display::widget::view::{Area, ViewId};
use std::rc::Rc;

#[test]
fn reveal_moves_a_partly_visible_target_fully_into_view() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (180.0, 220.0), (30.0, 200.0), 1.0),
        152.0
    );
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (10.0, 50.0), (30.0, 200.0), 1.0),
        88.0
    );
}

#[test]
fn reveal_leaves_fully_visible_targets_alone_even_inside_the_margin() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (30.0, 200.0), (30.0, 200.0), 1.0),
        120.0
    );
}

#[test]
fn reveal_aligns_an_oversized_target_at_its_leading_edge() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (80.0, 500.0), (30.0, 200.0), 1.0),
        158.0
    );
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (42.0, 462.0), (30.0, 200.0), 1.0),
        120.0
    );
}

#[test]
fn a_target_starting_beyond_the_viewport_is_revealed() {
    assert_eq!(
        reveal_axis(120.0, 1_000.0, (220.0, 260.0), (30.0, 200.0), 1.0),
        192.0
    );
}

#[test]
fn reveal_clamps_an_offset_left_stale_by_a_resize() {
    assert_eq!(
        reveal_axis(300.0, 100.0, (80.0, 120.0), (30.0, 200.0), 1.0),
        100.0
    );
}

#[test]
fn reveal_uses_the_requested_view_and_converts_pixels_to_scroll_units() {
    let mut editor = crate::test_editor(gid::Document::default());
    let root = editor.model.workspace.document_area();
    let other = Area::part(ViewId::Document, vec![gid::Step::Key(gid::new_cell_id())]);
    let landmarks: [Landmark<()>; 2] = [
        Landmark {
            area: Some(other.clone()),
            path: crate::occurrence::Trail::root(ViewId::Document),
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            select: Rc::new(|_, _| false),
            secondary: None,
            selected: false,
            tab_stop: false,
        },
        Landmark {
            area: Some(root.clone()),
            path: crate::occurrence::Trail::root(ViewId::Document),
            rect: Rect::new(0.0, 1_000.0, 20.0, 1_060.0),
            select: Rc::new(|_, _| false),
            secondary: None,
            selected: false,
            tab_stop: false,
        },
    ];
    let regions = [ViewRegion {
        area: root.clone(),
        rect: Rect::new(0.0, 0.0, 400.0, 200.0),
        maximum: Vec2::new(0.0, 1_000.0),
        content: None,
    }];
    let geometry = Geometry {
        landmarks: &landmarks,
        view_regions: &regions,
        scale: 2.0,
    };
    geometry.reveal_path(&mut editor.model, &other, &[]);
    assert_eq!(editor.model.workspace.document.scroll, Vec2::ZERO);
    geometry.reveal_path(&mut editor.model, &root, &[]);
    assert_eq!(
        editor.model.workspace.document.scroll,
        Vec2::new(0.0, 442.0)
    );
}
