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

/// Plain arrival at an occurrence: select it, whatever the direction.
pub(crate) fn arrival(path: Rc<[Step]>, cx: &crate::projection::Cx<'_>) -> Select<crate::Editor> {
    let select = crate::projection::select_handler(path, cx);
    Rc::new(move |editor, _| select(editor))
}

/// A native control supplies its entry without introducing child routing.
pub fn target(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    super::before(
        child,
        Rc::new(|context| {
            let path: Rc<[Step]> = Rc::from(context.path);
            let selected = context.inputs.selected(context.path);
            Box::new(move |output, _| output.navigation_stop(path, selected))
        }),
    )
}

pub fn nav_group(
    child: crate::display::Layout<crate::Editor, crate::frame::Hovered>,
) -> crate::display::Layout<crate::Editor, crate::frame::Hovered> {
    super::around(
        child,
        Rc::new(move |context| {
            let path: Rc<[Step]> = Rc::from(context.path);
            let selected = context.inputs.selected(context.path);
            Box::new(move |child| {
                measured::around_into(child, move |_, inner, pass| {
                    pass.navigation.begin_container(path, selected);
                    inner.place_into(pass);
                    pass.navigation.end_container();
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
