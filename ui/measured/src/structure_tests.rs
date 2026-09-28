use super::*;
use crate::choices::{ChoiceBuild, ChoiceLayout, resolve_choices};

#[derive(Default)]
struct Trace {
    events: Vec<&'static str>,
    deferred: Vec<PlaceInner<Self>>,
}

impl ObserveLayout for Trace {
    fn layout(&mut self, composition: Composition, children: impl FnOnce(&mut Self)) {
        self.events.push(match composition {
            Composition::Row(_) => "row",
            Composition::Column { .. } => "column",
            Composition::Overlay => "overlay",
        });
        children(self);
        self.events.push("end");
    }

    fn layout_child(&mut self, child: impl FnOnce(&mut Self)) {
        self.events.push("child");
        child(self);
        self.events.push("end child");
    }
}

fn leaf(label: &'static str) -> Measured<Trace> {
    leaf_into(
        Extent {
            width: 10.0,
            ascent: 8.0,
            descent: 2.0,
        },
        move |_, out: &mut Trace| out.events.push(label),
    )
}

fn placement() -> Placement {
    Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0))
}

#[test]
fn chosen_structure_and_direct_measured_structure_have_the_same_traversal() {
    for available in [10.0, 100.0] {
        let mut build = ChoiceBuild::default();
        let children = || {
            vec![
                ChoiceLayout::fixed(leaf("a")),
                ChoiceLayout::fixed(leaf("b")),
            ]
        };
        let layout = build.alternatives(vec![
            ChoiceLayout::aligned_row(RowAlignment::Baseline, 0.0, children()),
            ChoiceLayout::col(1, 0.0, children()),
        ]);
        let chosen = resolve_choices(build.finish(layout), available, false);
        let direct = if available < 20.0 {
            col(1, 0.0, vec![leaf("a"), leaf("b")])
        } else {
            row(0.0, vec![leaf("a"), leaf("b")])
        };
        let mut chosen_trace = Trace::default();
        let mut direct_trace = Trace::default();
        place_into(chosen, placement(), &mut chosen_trace);
        place_into(direct, placement(), &mut direct_trace);
        assert_eq!(chosen_trace.events, direct_trace.events);
        assert_eq!(
            &chosen_trace.events[1..],
            ["child", "a", "end child", "child", "b", "end child", "end"]
        );
    }
}

#[test]
fn omitted_and_deferred_subtrees_are_observed_only_when_placed() {
    let omitted = around_into(col(0, 0.0, vec![leaf("not placed")]), |_, _inner, _out| {});
    let deferred = around_into(
        col(0, 0.0, vec![leaf("later")]),
        |_, inner, out: &mut Trace| out.deferred.push(inner),
    );
    let layout = row(0.0, vec![omitted, leaf("now"), deferred]);
    let mut out = Trace::default();
    place_into(layout, placement(), &mut out);
    assert_eq!(
        out.events,
        [
            "row",
            "child",
            "end child",
            "child",
            "now",
            "end child",
            "child",
            "end child",
            "end"
        ]
    );
    out.deferred.pop().unwrap().place_into(&mut out);
    assert_eq!(
        &out.events[9..],
        ["column", "child", "later", "end child", "end"]
    );
}
