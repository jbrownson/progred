//! Frame-local navigation data, composed explicitly by projections.
use super::HoverPass;
use gid::Step;
use measured::Measured;
use puri::Rect;
use std::rc::Rc;

pub mod graph;
pub use graph::{Graph, Navigation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Up, Self::Down];

    pub fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

// Direct selection for pointer/source selection and Select All, not arrow routing.
pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Entry {
    #[default]
    Value,
    Line,
}

#[derive(Clone)]
pub struct Stop {
    pub path: Rc<[Step]>,
    pub entry: Entry,
    pub(crate) scope: crate::editing::Scope,
}

pub struct ViewNavigation<N = Navigation> {
    pub root: Option<super::view::Root>,
    pub navigation: N,
}

pub(super) fn resolve_views(views: Vec<ViewNavigation>) -> Vec<ViewNavigation<Graph>> {
    let mut grouped: Vec<(Option<super::view::Root>, Vec<Navigation>)> = Vec::new();
    for view in views {
        if let Some((_, parts)) = grouped.iter_mut().find(|(root, _)| *root == view.root) {
            parts.push(view.navigation);
        } else {
            grouped.push((view.root, vec![view.navigation]));
        }
    }
    grouped
        .into_iter()
        .map(|(root, parts)| {
            let navigation = Navigation::join(parts).resolve();
            for issue in navigation.issues() {
                eprintln!("Invalid navigation description: {issue}");
            }
            ViewNavigation { root, navigation }
        })
        .collect()
}

/// Make this projected value a stop, without changing any child routing.
pub fn target<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
) -> crate::display::Layout<W, H> {
    super::before(
        child,
        Rc::new(|context| {
            let stop = Stop {
                path: Rc::from(context.path),
                entry: Entry::Value,
                scope: context.inputs.edits.clone(),
            };
            Box::new(move |output, _| output.navigation(Navigation::stop(stop)))
        }),
    )
}

pub fn horizontal(children: Vec<Navigation>) -> Navigation {
    Navigation::sequence(children, Direction::Right)
}

pub fn vertical(children: Vec<Navigation>) -> Navigation {
    Navigation::sequence(children, Direction::Down)
}

/// Compose only the navigation contributed by this explicitly wrapped subtree.
pub fn scope<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
    compose: fn(Vec<Navigation>) -> Navigation,
) -> crate::display::Layout<W, H> {
    super::around(
        child,
        Rc::new(move |_| {
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    pass.scope(
                        |pass| inner.place_into(pass),
                        move |mut output| {
                            let children = std::mem::take(&mut output.navigation);
                            output.navigation.push(ViewNavigation {
                                root: None,
                                navigation: compose(
                                    children.into_iter().map(|child| child.navigation).collect(),
                                ),
                            });
                            output
                        },
                    );
                })
            })
        }),
    )
}

/// Incoming navigation selects the group before entering its contents.
/// Ordinary occurrence bookkeeping adds no links.
pub fn nav_group(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    super::around(
        child,
        Rc::new(|context| {
            let stop = Stop {
                path: Rc::from(context.path),
                entry: Entry::Value,
                scope: context.inputs.edits.clone(),
            };
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    pass.scope(
                        |pass| inner.place_into(pass),
                        move |mut output| {
                            let children = std::mem::take(&mut output.navigation);
                            let nav = Navigation::join(
                                children.into_iter().map(|n| n.navigation).collect(),
                            );
                            output.navigation.push(ViewNavigation {
                                root: None,
                                navigation: Navigation::group(stop, nav),
                            });
                            output
                        },
                    );
                })
            })
        }),
    )
}

pub struct Landmark<World> {
    pub root: Option<super::view::Root>,
    pub path: Rc<[Step]>,
    pub rect: Rect,
    pub select: Select<World>,
    pub(crate) scope: crate::editing::Scope,
}

impl<World> Clone for Landmark<World> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            rect: self.rect,
            select: self.select.clone(),
            scope: self.scope.clone(),
        }
    }
}

pub(crate) fn landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Rc<[Step]>,
    select: Select<World>,
    scope: crate::editing::Scope,
) -> Measured<HoverPass<World, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.scope(
            |pass| inner.place_into(pass),
            move |mut output| {
                let select = output.landmark_select.take().unwrap_or(select);
                output.descends.push(Landmark {
                    root: None,
                    path,
                    rect: placement.rect,
                    select,
                    scope,
                });
                output
            },
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph::{Issue, Link};
    fn stop() -> (gid::Path, Navigation) {
        let path = vec![Step::Key(gid::new_cell_id())];
        (
            path.clone(),
            Navigation::stop(Stop {
                path: Rc::from(path),
                entry: Entry::Value,
                scope: Default::default(),
            }),
        )
    }

    #[test]
    fn nested_sequences_build_links_without_an_editor_or_callbacks() {
        let (a, first) = stop();
        let (b, second) = stop();
        let (c, third) = stop();
        let (d, fourth) = stop();
        let nav = vertical(vec![
            horizontal(vec![first, second]),
            horizontal(vec![third, fourth]),
        ])
        .resolve();
        assert!(nav.issues().is_empty());
        for (from, direction, to) in [
            (&a, Direction::Right, &b),
            (&b, Direction::Left, &a),
            (&a, Direction::Down, &c),
            (&b, Direction::Down, &c),
            (&c, Direction::Up, &b),
            (&d, Direction::Left, &c),
        ] {
            assert_eq!(nav.destination(from, direction).unwrap().path.as_ref(), to);
        }
        assert!(nav.destination(&a, Direction::Left).is_none());
        assert!(nav.destination(&d, Direction::Down).is_none());
    }

    #[test]
    fn irregular_links_are_directed_and_require_existing_stops() {
        let (a, first) = stop();
        let (b, second) = stop();
        let declarations = Navigation::new(
            vec![],
            vec![
                Link::new(Rc::from(a.clone()), Direction::Up, Rc::from(b.clone())),
                Link::new(Rc::from(a.clone()), Direction::Left, Rc::from([])),
            ],
            Default::default(),
        );
        let nav = Navigation::join(vec![
            declarations,
            horizontal(vec![first, Navigation::default(), second]),
        ])
        .resolve();
        assert_eq!(
            nav.destination(&a, Direction::Up).unwrap().path.as_ref(),
            &b
        );
        assert!(nav.destination(&b, Direction::Down).is_none());
        assert!(matches!(nav.issues(), [Issue::MissingStop(_)]));
        assert!(nav.destination(&a, Direction::Left).is_none());
        assert_eq!(
            nav.destination(&a, Direction::Right).unwrap().path.as_ref(),
            &b
        );
    }

    fn leaf(nav: Navigation) -> crate::display::Layout<(), ()> {
        crate::display::Layout::widget(Rc::new(move |_| {
            let nav = nav.clone();
            super::super::leaf(
                measured::Extent {
                    width: 10.0,
                    ascent: 8.0,
                    descent: 2.0,
                },
                move |output, _| output.navigation(nav),
            )
        }))
    }

    fn place(
        layout: crate::display::Layout<(), ()>,
        width: f64,
    ) -> super::super::HoverOutput<(), ()> {
        use crate::display::test_support::{NoProject, with_context};
        use measured::choices::{ChoiceBuild, resolve_choices};
        let mut build = ChoiceBuild::default();
        let prepared = with_context(&NoProject, |context| layout.measure(context, &mut build));
        let measured = resolve_choices(build.finish(prepared), width, false);
        let placement = puri::Placement::root(measured.extent.rect_at(puri::Point::ZERO));
        super::super::frame::place(measured, placement, &Default::default())
    }

    #[test]
    fn layout_and_landmarks_do_not_silently_choose_navigation() {
        let (a, first) = stop();
        let (b, second) = stop();
        for layout in [
            crate::display::row(0.0, [leaf(first.clone()), leaf(second.clone())]),
            crate::display::col(0, 0.0, [leaf(first.clone()), leaf(second.clone())]),
        ] {
            let output = place(layout, 100.0);
            assert_eq!(output.navigation.len(), 2);
            for child in output.navigation {
                let nav = child.navigation.resolve();
                assert!(nav.destination(&a, Direction::Right).is_none());
                assert!(nav.destination(&b, Direction::Up).is_none());
            }
        }
        let measured: Measured<HoverPass<(), ()>> = landmark(
            super::super::leaf(measured::Extent::default(), |_, _| {}),
            Rc::from(a),
            Rc::new(|_: &mut (), _| panic!("not an arrow action")),
            Default::default(),
        );
        let output = super::super::frame::place(
            measured,
            puri::Placement::root(Rect::ZERO),
            &Default::default(),
        );
        assert_eq!(output.descends.len(), 1);
        assert!(output.navigation.is_empty());
    }

    #[test]
    fn explicit_routes_follow_only_the_placed_alternative() {
        let (a, first) = stop();
        let (b, second) = stop();
        let layout = crate::display::alternatives([
            scope(
                crate::display::row(0.0, [leaf(first.clone()), leaf(second.clone())]),
                horizontal,
            ),
            scope(
                crate::display::col(0, 0.0, [leaf(first), leaf(second)]),
                vertical,
            ),
        ]);
        for (width, along, across) in [
            (100.0, Direction::Right, Direction::Down),
            (10.0, Direction::Down, Direction::Right),
        ] {
            let output = place(layout.clone(), width);
            assert_eq!(output.navigation.len(), 1);
            let nav = output.navigation[0].navigation.clone().resolve();
            assert!(nav.issues().is_empty());
            assert_eq!(nav.destination(&a, along).unwrap().path.as_ref(), &b);
            assert!(nav.destination(&a, across).is_none());
        }
        // Identical horizontal geometry can deliberately carry vertical routing.
        let (a, first) = stop();
        let (b, second) = stop();
        let output = place(
            scope(
                crate::display::row(0.0, [leaf(first), leaf(second)]),
                vertical,
            ),
            100.0,
        );
        let nav = output.navigation[0].navigation.clone().resolve();
        assert_eq!(
            nav.destination(&a, Direction::Down).unwrap().path.as_ref(),
            &b
        );
        assert!(nav.destination(&a, Direction::Right).is_none());
    }

    #[test]
    fn conflicting_links_are_rejected_independently_of_declaration_order() {
        let (a, first) = stop();
        let (b, second) = stop();
        let (c, third) = stop();
        let mut links = vec![
            Link::new(Rc::from(a.clone()), Direction::Right, Rc::from(b.clone())),
            Link::new(Rc::from(a.clone()), Direction::Right, Rc::from(c.clone())),
        ];
        links.extend([
            Link::new(Rc::from(b.clone()), Direction::Down, Rc::from(c.clone())),
            Link::new(Rc::from(c.clone()), Direction::Up, Rc::from(b.clone())),
        ]);
        // Identical declarations are harmless; conflicting ones never overwrite.
        links.push(links[0].clone());
        for reversed in [false, true] {
            if reversed {
                links.reverse();
            }
            let nav = Navigation::join(vec![
                Navigation::new(vec![], links.clone(), Default::default()),
                first.clone(),
                second.clone(),
                third.clone(),
            ])
            .resolve();
            assert!(matches!(nav.issues(), [Issue::ConflictingLink(_)]));
            assert!(nav.destination(&a, Direction::Right).is_none());
            assert_eq!(
                nav.destination(&b, Direction::Down).unwrap().path.as_ref(),
                &c
            );
            assert_eq!(
                nav.destination(&c, Direction::Up).unwrap().path.as_ref(),
                &b
            );
        }
    }

    #[test]
    fn only_declared_boundary_exits_connect_to_siblings() {
        let (a, first) = stop();
        let (b, second) = stop();
        let interior = Navigation::new(
            vec![Stop {
                path: Rc::from(a.clone()),
                entry: Entry::Value,
                scope: Default::default(),
            }],
            vec![],
            Default::default(),
        );
        let nav = horizontal(vec![interior, second]).resolve();
        assert!(nav.issues().is_empty());
        assert!(nav.destination(&a, Direction::Right).is_none());
        assert!(nav.destination(&b, Direction::Left).is_none());
        let group_path: Rc<[Step]> = Rc::from([]);
        let nav = Navigation::group(
            Stop {
                path: group_path.clone(),
                entry: Entry::Value,
                scope: Default::default(),
            },
            first,
        )
        .resolve();
        assert!(nav.issues().is_empty());
        for direction in [Direction::Right, Direction::Down] {
            assert_eq!(
                nav.destination(&group_path, direction)
                    .unwrap()
                    .path
                    .as_ref(),
                &a
            );
            assert_eq!(
                nav.destination(&a, direction.opposite()).unwrap().path,
                group_path
            );
        }
    }

    #[test]
    fn resolution_combines_parts_within_a_view_but_not_between_views() {
        let (a, first) = stop();
        let (b, second) = stop();
        let forward = horizontal(vec![first.clone(), second.clone()]);
        let backward = horizontal(vec![second, first]);
        let root = super::super::view::Root::document();
        let other = super::super::view::Root::document();
        let views = resolve_views(vec![
            ViewNavigation {
                root: Some(root.clone()),
                navigation: forward.clone(),
            },
            ViewNavigation {
                root: Some(other),
                navigation: backward,
            },
            ViewNavigation {
                root: Some(root),
                navigation: forward,
            },
        ]);
        assert_eq!(views.len(), 2);
        assert!(views.iter().all(|v| v.navigation.issues().is_empty()));
        assert_eq!(
            views[0]
                .navigation
                .destination(&a, Direction::Right)
                .unwrap()
                .path
                .as_ref(),
            &b
        );
        assert!(
            views[1]
                .navigation
                .destination(&a, Direction::Right)
                .is_none()
        );
        assert_eq!(
            views[1]
                .navigation
                .destination(&b, Direction::Right)
                .unwrap()
                .path
                .as_ref(),
            &a
        );
    }
}
