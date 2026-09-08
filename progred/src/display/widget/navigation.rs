//! A projection declares a stop; placement supplies its settled rectangle.

use super::HoverPass;

use gid::Step;
use measured::Measured;
use puri::Rect;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub type Select<World> = Rc<dyn Fn(&mut World, Option<Direction>) -> bool>;

pub struct Landmark<World> {
    pub root: Option<super::view::Root>,
    pub path: Rc<[Step]>,
    pub rect: Rect,
    pub select: Select<World>,
}

impl<World> Clone for Landmark<World> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            rect: self.rect,
            select: self.select.clone(),
        }
    }
}

pub fn landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Rc<[Step]>,
    select: Select<World>,
) -> Measured<HoverPass<World, H>> {
    measured::around(child, move |placement, inner| {
        inner.place().map(move |mut output| {
            let select = output.landmark_select.take().unwrap_or(select);
            output.descends.push(Landmark {
                root: None,
                path,
                rect: placement.rect,
                select,
            });
            output
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::widget::{HoverContext, leaf};
    use measured::{Extent, Output, place};
    use puri::Placement;

    type World = Vec<(&'static str, Option<Direction>)>;
    type Frame<'a> = HoverContext<'a, World, ()>;

    fn select(name: &'static str) -> Select<Vec<(&'static str, Option<Direction>)>> {
        Rc::new(move |log, direction| {
            log.push((name, direction));
            true
        })
    }

    fn path() -> Rc<[Step]> {
        Rc::from([])
    }

    fn control(
        select: Option<Select<Vec<(&'static str, Option<Direction>)>>>,
    ) -> Measured<HoverPass<World, ()>> {
        leaf(
            Extent {
                width: 10.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |output: &mut Frame, _| output.on_arrival(select),
        )
    }

    #[test]
    fn only_the_nearest_landmark_consumes_a_controls_arrival_handler() {
        let child = landmark(control(Some(select("control"))), path(), select("child"));
        let child = landmark(child, path(), select("parent"));
        let child = crate::display::widget::before_hover(child, |_, output: &mut Frame| {
            output.on_arrival(Some(select("outside")));
        });
        let placement = Placement::root(Rect::new(15.0, 30.0, 25.0, 40.0));
        let output = place(child, placement).run(&Default::default());
        let mut log = vec![];
        for landmark in output.descends {
            assert_eq!(landmark.rect, placement.rect);
            (landmark.select)(&mut log, Some(Direction::Left));
        }
        output.landmark_select.unwrap()(&mut log, None);
        assert_eq!(
            log,
            vec![
                ("control", Some(Direction::Left)),
                ("parent", Some(Direction::Left)),
                ("outside", None)
            ]
        );
    }

    #[test]
    fn sibling_landmarks_do_not_share_arrival_handlers() {
        let first = landmark(control(Some(select("first"))), path(), select("unused"));
        let second = landmark(control(None), path(), select("second"));
        let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
        let output =
            place(measured::row(0.0, vec![first, second]), placement).run(&Default::default());
        let mut log = vec![];
        for landmark in output.descends {
            (landmark.select)(&mut log, None);
        }
        assert_eq!(log, vec![("first", None), ("second", None)]);
        assert!(output.landmark_select.is_none());
    }

    #[test]
    fn unplaced_subtrees_do_not_contribute_landmarks() {
        let child = landmark(control(Some(select("control"))), path(), select("unused"));
        let child = measured::around(child, |_, _| HoverPass::empty());
        let output =
            place(child, Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0))).run(&Default::default());
        assert!(output.descends.is_empty());
        assert!(output.landmark_select.is_none());
    }
}
