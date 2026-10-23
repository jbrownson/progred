use super::*;
use crate::display::test_support::{NoProject, with_context};
use crate::display::widget::HoverContext;
use kurbo::Rect;
use measured::Extent;
use measured::choices::resolve_choices;
use std::{cell::Cell, rc::Rc};
use uig::Placement;

#[test]
fn shared_program_prepares_once_per_frame_and_places_only_the_chosen_use() {
    let preparations = Rc::new(Cell::new(0));
    let placements = Rc::new(Cell::new(0));
    let (prepared, placed) = (preparations.clone(), placements.clone());
    let child = crate::display::shared(Layout::program(Rc::new(move |_, _| {
        prepared.set(prepared.get() + 1);
        let placed = placed.clone();
        ChoiceLayout::fixed(widget::leaf(
            Extent {
                width: 10.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |_: &mut HoverContext<'_, (), ()>, _| placed.set(placed.get() + 1),
        ))
    })));
    let layout =
        crate::display::alternatives([crate::display::indent(100.0, child.clone()), child]);
    for (frame, width, expected_width) in [(1, 120.0, 110.0), (2, 20.0, 10.0)] {
        let mut build = ChoiceBuild::default();
        let prepared = with_context(&NoProject, |context| {
            layout.clone().measure(context, &mut build)
        });
        assert_eq!(preparations.get(), frame);
        assert_eq!(placements.get(), frame - 1);
        let measured = resolve_choices(build.finish(prepared), width);
        assert_eq!(measured.extent.width, expected_width);
        crate::display::widget::frame::place(
            measured,
            Placement::root(Rect::new(0.0, 0.0, width, 10.0)),
            &Default::default(),
        );
        assert_eq!(placements.get(), frame);
    }
}
