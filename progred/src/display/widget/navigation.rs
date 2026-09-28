//! Neighbors are connected during settled placement; only ordinary handlers survive.
use super::{HoverPass, frame::DispatchContext};
use gid::Step;
use measured::Measured;
use puri::Rect;
use puri::handler::{Event, EventOutcome, Handler};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub use puri::handler::NavigationDirection as Direction;
const DIRECTIONS: [Direction; 4] = [
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
];

// Pointer/source selection and Select All also use these direct selection helpers.
pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;

pub struct Target<C> {
    pub path: Rc<[Step]>,
    pub select: Select<C>,
}
impl<C> Clone for Target<C> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            select: self.select.clone(),
        }
    }
}

pub(crate) fn destination(
    path: Rc<[Step]>,
    cx: &crate::projection::Cx<'_>,
) -> Target<crate::Editor> {
    let root = cx.view.clone();
    let scope = cx.edits.clone();
    let occurrence = path.clone();
    Target {
        path,
        select: Rc::new(move |editor, _| {
            scope
                .open(crate::editing::Access::new(editor))
                .select(&root, &occurrence);
            true
        }),
    }
}

type Receive<T> = Box<dyn FnOnce(Option<T>)>;
type Provider<T> = Rc<dyn Fn(Receive<T>)>;
type Pending<T> = Rc<RefCell<[Vec<Receive<T>>; 4]>>;

struct Neighbors<T>([Provider<T>; 4]);
impl<T> Clone for Neighbors<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Clone + 'static> Neighbors<T> {
    fn root() -> Self {
        Self(std::array::from_fn(|_| {
            Rc::new(|receive: Receive<T>| receive(None)) as Provider<T>
        }))
    }
    fn request(&self, direction: Direction, receive: Receive<T>) {
        self.0[direction as usize](receive);
    }
    fn with(mut self, direction: Direction, target: T) -> Self {
        self.0[direction as usize] = Rc::new(move |receive| receive(Some(target.clone())));
        self
    }
}

struct Entries<T> {
    targets: [Option<T>; 4],
    flow: Option<Direction>,
}
impl<T: Clone> Entries<T> {
    fn empty() -> Self {
        Self {
            targets: std::array::from_fn(|_| None),
            flow: None,
        }
    }
    fn one(target: T) -> Self {
        Self {
            targets: std::array::from_fn(|_| Some(target.clone())),
            flow: None,
        }
    }
    fn get(&self, direction: Direction) -> Option<T> {
        self.targets[direction as usize].clone()
    }
}

struct Sequence<T> {
    outer: Neighbors<T>,
    forward: &'static [Direction],
    entries: Entries<T>,
    pending: [Vec<Receive<T>>; 4],
}
impl<T: Clone + 'static> Sequence<T> {
    fn new(outer: Neighbors<T>, forward: &'static [Direction]) -> Self {
        Self {
            outer,
            forward,
            entries: Entries {
                flow: forward.first().copied(),
                ..Entries::empty()
            },
            pending: std::array::from_fn(|_| vec![]),
        }
    }
    fn begin_child(&self) -> (Neighbors<T>, Pending<T>) {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Navigation);
        let mut neighbors = self.outer.clone();
        let pending: Pending<T> = Rc::new(RefCell::new(std::array::from_fn(|_| vec![])));
        for &forward in self.forward {
            if let Some(previous) = self.entries.get(forward.opposite()) {
                neighbors = neighbors.with(forward.opposite(), previous);
            }
            neighbors.0[forward as usize] = {
                let pending = pending.clone();
                Rc::new(move |receive| pending.borrow_mut()[forward as usize].push(receive))
            };
        }
        (neighbors, pending)
    }
    fn end_child(&mut self, entries: Entries<T>, own_pending: Pending<T>) {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Navigation);
        self.entries.flow = self.entries.flow.or(entries.flow);
        for &forward in self.forward {
            if let Some(next) = entries.get(forward) {
                for receive in self.pending[forward as usize].drain(..) {
                    receive(Some(next.clone()));
                }
            }
        }
        for direction in DIRECTIONS {
            self.pending[direction as usize]
                .extend(own_pending.borrow_mut()[direction as usize].drain(..));
            let slot = &mut self.entries.targets[direction as usize];
            if let Some(entry) = entries.get(direction)
                && (slot.is_none() || matches!(direction, Direction::Left | Direction::Up))
            {
                *slot = Some(entry);
            }
        }
    }
    fn finish(mut self) -> Entries<T> {
        for &forward in self.forward {
            for receive in self.pending[forward as usize].drain(..) {
                self.outer.request(forward, receive);
            }
        }
        assert!(self.pending.iter().all(Vec::is_empty));
        self.entries
    }
}

/// Caller-owned construction capability. Not part of the installed frame.
pub(crate) struct Construction<C, H> {
    sequence: Sequence<Target<C>>,
    handlers: Rc<RefCell<Option<Handler<C, DispatchContext<C, H>>>>>,
    requested: Option<Rc<[Step]>>,
    answered: Rc<Cell<usize>>,
}
impl<C: 'static, H: 'static> Default for Construction<C, H> {
    fn default() -> Self {
        Self {
            sequence: Sequence::new(Neighbors::root(), &[]),
            handlers: Rc::new(RefCell::new(None)),
            requested: None,
            answered: Rc::new(Cell::new(0)),
        }
    }
}
impl<C: 'static, H: 'static> Construction<C, H> {
    fn request(&mut self, from: &Target<C>, neighbors: &Neighbors<Target<C>>) {
        if let Some(path) = &self.requested {
            // A facet and its enclosing whole-value wrapper may name the same
            // occurrence. The inner contribution already owns its navigation.
            assert_eq!(
                path, &from.path,
                "multiple selected navigation occurrences in one view"
            );
        } else {
            self.requested = Some(from.path.clone());
            for direction in DIRECTIONS {
                let handlers = self.handlers.clone();
                let answered = self.answered.clone();
                let from = from.path.clone();
                neighbors.request(
                    direction,
                    Box::new(move |target| {
                        answered.set(answered.get() + 1);
                        if let Some(target) = target.filter(|target| target.path != from) {
                            handlers.borrow_mut().get_or_insert_with(Handler::new).on(
                                move |world, event, _| match event {
                                    Event::Navigate(actual) if actual == direction => {
                                        EventOutcome::from_handled(
                                            event,
                                            (target.select)(world, Some(direction)),
                                        )
                                    }
                                    _ => EventOutcome::decline(event),
                                },
                            );
                        }
                    }),
                );
            }
        }
    }
    pub fn target(&mut self, target: Target<C>, selected: bool) {
        let (neighbors, pending) = self.sequence.begin_child();
        if selected {
            self.request(&target, &neighbors);
        }
        self.sequence.end_child(Entries::one(target), pending);
    }
    pub fn finish(self) -> Option<Handler<C, DispatchContext<C, H>>> {
        self.sequence.finish();
        assert_eq!(
            self.answered.get(),
            if self.requested.is_some() { 4 } else { 0 },
            "unanswered navigation request"
        );
        Rc::try_unwrap(self.handlers)
            .ok()
            .expect("navigation provider escaped construction")
            .into_inner()
    }
}

/// A native control supplies its entry without introducing child routing.
pub fn target(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    super::before(
        child,
        Rc::new(|context| {
            let target = destination(Rc::from(context.path), context.inputs);
            let selected = context.inputs.selected(context.path);
            Box::new(move |output, _| output.navigation_target(target, selected))
        }),
    )
}

pub fn horizontal<C: 'static, H: 'static>(
    child: crate::display::Layout<C, H>,
) -> crate::display::Layout<C, H> {
    with_navigation(child, Direction::Right)
}
pub fn vertical<C: 'static, H: 'static>(
    child: crate::display::Layout<C, H>,
) -> crate::display::Layout<C, H> {
    with_navigation(child, Direction::Down)
}

pub fn with_navigation<C: 'static, H: 'static>(
    child: crate::display::Layout<C, H>,
    direction: Direction,
) -> crate::display::Layout<C, H> {
    sequence(
        child,
        match direction {
            Direction::Right => &[Direction::Right],
            // Primary flow stays vertical; unclaimed horizontal movement wraps rows.
            Direction::Down => &[Direction::Down, Direction::Right],
            _ => panic!("sequences progress right or down"),
        },
    )
}

fn sequence<C: 'static, H: 'static>(
    child: crate::display::Layout<C, H>,
    directions: &'static [Direction],
) -> crate::display::Layout<C, H> {
    super::around(
        child,
        Rc::new(move |_| {
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    compose(pass, directions, None, false, |pass| inner.place_into(pass));
                })
            })
        }),
    )
}

pub fn nav_group(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    // The selected presentation supplies the flow; a leaf defaults to horizontal.
    super::around(
        child,
        Rc::new(move |context| {
            let target = destination(Rc::from(context.path), context.inputs);
            let selected = context.inputs.selected(context.path);
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    compose(pass, &[], Some(target), selected, |pass| {
                        inner.place_into(pass)
                    });
                })
            })
        }),
    )
}

fn compose<C: 'static, H: 'static>(
    pass: &mut HoverPass<C, H>,
    directions: &'static [Direction],
    whole: Option<Target<C>>,
    selected: bool,
    content: impl FnOnce(&mut HoverPass<C, H>),
) {
    let (neighbors, pending) = pass.navigation.sequence.begin_child();
    // The child's chosen alternative declares its flow during placement. Only
    // backward boundary answers need to wait for that declaration.
    let boundary: Option<Pending<Target<C>>> = (whole.is_some() && !selected)
        .then(|| Rc::new(RefCell::new(std::array::from_fn(|_| vec![]))));
    let mut child_neighbors = neighbors.clone();
    if let Some(boundary) = &boundary {
        for direction in [Direction::Left, Direction::Up] {
            let boundary = boundary.clone();
            child_neighbors.0[direction as usize] = Rc::new(move |receive| {
                boundary.borrow_mut()[direction as usize].push(receive);
            });
        }
    }
    let parent = std::mem::replace(
        &mut pass.navigation.sequence,
        Sequence::new(child_neighbors, directions),
    );
    content(pass);
    let children = std::mem::replace(&mut pass.navigation.sequence, parent).finish();
    let entries = if let Some(target) = whole {
        let forward = children.flow.unwrap_or(Direction::Right);
        let backward = forward.opposite();
        if let Some(boundary) = boundary {
            for direction in [Direction::Left, Direction::Up] {
                for receive in boundary.borrow_mut()[direction as usize].drain(..) {
                    if direction == backward {
                        receive(Some(target.clone()));
                    } else {
                        neighbors.request(direction, receive);
                    }
                }
            }
        }
        if selected {
            let mut arrival = neighbors;
            if let Some(entry) = children.get(forward) {
                arrival = arrival.with(forward, entry);
            }
            pass.navigation.request(&target, &arrival);
        }
        let mut entries = Entries::one(target);
        entries.flow = Some(forward);
        if let Some(entry) = children.get(backward) {
            entries.targets[backward as usize] = Some(entry);
        }
        entries
    } else {
        children
    };
    pass.navigation.sequence.end_child(entries, pending);
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

    fn emit<T: Clone + 'static>(
        sequence: &mut Sequence<T>,
        body: impl FnOnce(Neighbors<T>) -> Entries<T>,
    ) {
        let (neighbors, pending) = sequence.begin_child();
        let entries = body(neighbors);
        sequence.end_child(entries, pending);
    }

    #[test]
    fn callbacks_forward_past_empty_children_and_nested_boundaries() {
        let answers = Rc::new(RefCell::new(vec![]));
        let mut outer = Sequence::new(Neighbors::root(), &[Direction::Right]);
        emit(&mut outer, |_| Entries::one("before"));
        emit(&mut outer, |neighbors| {
            let mut inner = Sequence::new(neighbors, &[Direction::Right]);
            emit(&mut inner, |neighbors| {
                for direction in DIRECTIONS {
                    let answers = answers.clone();
                    neighbors.request(
                        direction,
                        Box::new(move |target| answers.borrow_mut().push((direction, target))),
                    );
                }
                Entries::one("selected")
            });
            emit(&mut inner, |_| Entries::empty());
            inner.finish()
        });
        assert!(
            answers
                .borrow()
                .contains(&(Direction::Left, Some("before")))
        );
        assert!(!answers.borrow().iter().any(|(d, _)| *d == Direction::Right));
        emit(&mut outer, |_| Entries::empty());
        emit(&mut outer, |_| Entries::one("after"));
        outer.finish();
        assert_eq!(answers.borrow().len(), 4);
        assert!(
            answers
                .borrow()
                .contains(&(Direction::Right, Some("after")))
        );
        assert!(answers.borrow().contains(&(Direction::Up, None)));
        assert!(answers.borrow().contains(&(Direction::Down, None)));
        assert_eq!(Rc::strong_count(&answers), 1);
    }

    fn target_value(id: usize, path: Rc<[Step]>) -> Target<Vec<usize>> {
        Target {
            path,
            select: Rc::new(move |world, _| {
                world.push(id);
                true
            }),
        }
    }

    #[test]
    fn reading_order_uses_each_neighbors_directional_entry() {
        let answers = Rc::new(RefCell::new(vec![]));
        let mut sequence = Sequence::new(Neighbors::root(), &[Direction::Right, Direction::Down]);
        emit(&mut sequence, |neighbors| {
            for direction in [Direction::Right, Direction::Down] {
                let answers = answers.clone();
                neighbors.request(
                    direction,
                    Box::new(move |target| {
                        answers.borrow_mut().push((direction, target));
                    }),
                );
            }
            Entries::one("selected")
        });
        emit(&mut sequence, |_| Entries::empty());
        emit(&mut sequence, |_| Entries {
            targets: [
                Some("last leaf"),
                Some("first leaf"),
                Some("whole"),
                Some("whole"),
            ],
            flow: Some(Direction::Right),
        });
        sequence.finish();
        assert_eq!(
            *answers.borrow(),
            [
                (Direction::Right, Some("first leaf")),
                (Direction::Down, Some("whole")),
            ]
        );
        assert_eq!(Rc::strong_count(&answers), 1);
    }

    #[test]
    fn root_and_inactive_navigation_add_no_handlers() {
        for selected in [false, true] {
            let mut construction = Construction::<Vec<usize>, ()>::default();
            construction.target(target_value(0, Rc::from([])), selected);
            assert!(construction.finish().is_none());
        }
    }

    #[test]
    fn views_with_identical_paths_keep_independent_neighbors() {
        let paths: [Rc<[Step]>; 2] =
            std::array::from_fn(|_| Rc::from([Step::Key(gid::new_cell_id())]));
        let mut pass = HoverPass::<Vec<usize>, ()>::new(&Default::default());
        for view in 0..2 {
            pass.in_view(super::super::view::Root::document(), |pass| {
                compose(pass, &[Direction::Right], None, false, |pass| {
                    for (index, path) in paths.iter().enumerate() {
                        pass.visit(|output| {
                            output.navigation_target(
                                target_value(view * 10 + index, path.clone()),
                                view == 0 && index == 0,
                            )
                        });
                    }
                });
            });
        }
        let frame = pass.finish().bind(Default::default());
        let mut visits = vec![];
        assert!(
            frame
                .handler
                .unwrap()
                .dispatch(
                    &mut visits,
                    Event::Navigate(Direction::Right),
                    &mut Default::default()
                )
                .handled()
        );
        assert_eq!(visits, [1]);
    }
}
