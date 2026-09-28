//! Layout collects selectable stops; only the current selection's destinations survive.
use super::{HoverPass, frame::DispatchContext};
use gid::Step;
use measured::Measured;
use puri::Rect;
use puri::handler::{Event, EventOutcome, Handler};
use std::rc::Rc;

mod logical;
pub(crate) use logical::Construction;

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

pub fn nav_group(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    super::around(
        child,
        Rc::new(move |context| {
            let target = destination(Rc::from(context.path), context.inputs);
            let selected = context.inputs.selected(context.path);
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    let parent = pass.navigation.begin_container(target, selected);
                    inner.place_into(pass);
                    pass.navigation.end_container(parent);
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
