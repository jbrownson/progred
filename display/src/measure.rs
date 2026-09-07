//! Box composition. Leaf programs are opaque to this interpreter.

use crate::{Layout, widget};
use measured::choices::{ChoiceBuild, ChoiceLayout};
use widget::{Context, Fragment};

impl<W: 'static, H: 'static> Layout<W, H> {
    pub fn measure(
        self,
        context: &mut Context<'_, '_, W, H>,
        build: &mut ChoiceBuild<Fragment<W, H>>,
    ) -> ChoiceLayout<Fragment<W, H>> {
        let scale = context.styles.scale;
        match self {
            Self::Leaf(leaf) => ChoiceLayout::fixed(widget::debug_geometry(
                widget::drawing::prepare(context, leaf),
            )),
            Self::Widget(widget) => ChoiceLayout::fixed(widget::debug_geometry(widget(context))),
            Self::Program(program) => program(context, build),
            Self::Before { child, before } => {
                let before = before(context);
                ChoiceLayout::map(child.measure(context, build), 0.0, move |child| {
                    measured::before_into(child, move |placement, output| before(output, placement))
                })
            }
            Self::After { child, after } => {
                let after = after(context);
                ChoiceLayout::map(child.measure(context, build), 0.0, move |child| {
                    measured::after_into(child, move |placement, output| after(output, placement))
                })
            }
            Self::Row {
                alignment,
                gap,
                children,
            } => ChoiceLayout::aligned_row(
                alignment,
                gap * scale,
                children
                    .into_iter()
                    .map(|child| child.measure(context, build))
                    .collect(),
            ),
            Self::Col {
                baseline,
                gap,
                children,
            } => ChoiceLayout::col(
                baseline,
                gap * scale,
                children
                    .into_iter()
                    .map(|child| child.measure(context, build))
                    .collect(),
            ),
            Self::Overlay { children } => ChoiceLayout::overlay(
                children
                    .into_iter()
                    .map(|child| child.measure(context, build))
                    .collect(),
            ),
            Self::Pad {
                left,
                top,
                right,
                bottom,
                child,
            } => ChoiceLayout::pad(
                (left * scale, top * scale, right * scale, bottom * scale).into(),
                child.measure(context, build),
            ),
            Self::Surround { left, child, right } => {
                let left = left(context);
                let right = right(context);
                ChoiceLayout::map(
                    child.measure(context, build),
                    left.maximum_width + right.maximum_width,
                    move |child| {
                        let extent = child.extent;
                        measured::row(
                            0.0,
                            vec![
                                widget::debug_geometry((left.measure)(extent)),
                                child,
                                widget::debug_geometry((right.measure)(extent)),
                            ],
                        )
                    },
                )
            }
            Self::Floating {
                base,
                content,
                position,
            } => {
                let base = base.measure(context, build);
                let content = content.measure(context, build);
                ChoiceLayout::attach(base, content, move |base, content| {
                    widget::container::floating(base, content, move |placement, extent| {
                        position(scale, placement, extent)
                    })
                })
            }
            Self::Shared { id, child } => {
                build.shared(id, |build| child.as_ref().clone().measure(context, build))
            }
            Self::Alternatives(options) => {
                let options = options
                    .into_iter()
                    .map(|option| option.measure(context, build))
                    .collect();
                build.alternatives(options)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{NoProject, with_context};
    use measured::choices::resolve_choices;
    use measured::{Extent, place};
    use puri::{DrawCmd, DrawList, Placement, Rect};
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn shared_program_prepares_once_per_frame_and_places_only_the_chosen_use() {
        let preparations = Rc::new(Cell::new(0));
        let placements = Rc::new(Cell::new(0));
        let (prepared, placed) = (preparations.clone(), placements.clone());
        let child = crate::shared(Layout::Program(Rc::new(move |_, _| {
            prepared.set(prepared.get() + 1);
            let placed = placed.clone();
            ChoiceLayout::fixed(widget::leaf(
                Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                move |_: &mut Fragment<(), ()>, _| placed.set(placed.get() + 1),
            ))
        })));
        let layout = crate::alternatives([crate::pad(100.0, child.clone()), child]);
        for (frame, width, expected_width) in [(1, 120.0, 110.0), (2, 20.0, 10.0)] {
            let mut build = ChoiceBuild::default();
            let prepared = with_context(&NoProject, |context| {
                layout.clone().measure(context, &mut build)
            });
            assert_eq!(preparations.get(), frame);
            assert_eq!(placements.get(), frame - 1);
            let measured = resolve_choices(build.finish(prepared), width, false);
            assert_eq!(measured.extent.width, expected_width);
            place(measured, Placement::root(Rect::new(0.0, 0.0, width, 10.0)));
            assert_eq!(placements.get(), frame);
        }
    }

    #[test]
    fn native_leaf_debug_ink_uses_its_full_settled_rectangle() {
        let rect = Rect::new(20.0, 30.0, 33.0, 47.0);
        for debug in [false, true] {
            let layout = Layout::Widget(Rc::new(|_| {
                widget::leaf(
                    Extent {
                        width: 13.0,
                        ascent: 0.0,
                        descent: 17.0,
                    },
                    |_: &mut Fragment<(), ()>, _| {},
                )
            }));
            let mut build = ChoiceBuild::default();
            let prepared = with_context(&NoProject, |context| layout.measure(context, &mut build));
            let measured = resolve_choices(build.finish(prepared), 13.0, false);
            let output = place(measured, Placement::root(rect));
            let mut canvas = DrawList::new();
            Fragment::<(), ()>::paint(
                output.renders,
                &mut canvas,
                widget::Ink {
                    debug_geometry: debug,
                    ..Default::default()
                },
            );
            if debug {
                assert!(
                    matches!(&canvas.0[..],[DrawCmd::Stroke {shape:puri::Shape::Rect(drawn),..}] if *drawn==rect)
                );
            } else {
                assert!(canvas.0.is_empty());
            }
        }
    }
}
