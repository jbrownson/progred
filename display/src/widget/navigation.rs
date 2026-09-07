//! A projection declares a stop; placement supplies its settled rectangle.

use gid::Step;
use measured::{Measured, Output};
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
    pub path: Rc<[Step]>,
    pub rect: Rect,
    pub select: Select<World>,
}

pub trait Navigation<World>: Output {
    /// A control can customize arrival at its nearest enclosing landmark.
    fn landmark_select(&mut self) -> &mut Option<Select<World>>;
    fn push_landmark(&mut self, landmark: Landmark<World>);
}

pub fn landmark<World: 'static, O: Navigation<World> + 'static>(
    child: Measured<O>,
    path: Rc<[Step]>,
    select: Select<World>,
) -> Measured<O> {
    measured::around_into(child, move |placement, inner, output: &mut O| {
        let outer_select = output.landmark_select().take();
        inner.place_into(output);
        let select = output.landmark_select().take().unwrap_or(select);
        *output.landmark_select() = outer_select;
        output.push_landmark(Landmark {
            path,
            rect: placement.rect,
            select,
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{Fragment, leaf};
    use measured::{Extent, place};
    use puri::Placement;

    type Frame = Fragment<Vec<(&'static str, Option<Direction>)>, ()>;

    fn select(name: &'static str) -> Select<Vec<(&'static str, Option<Direction>)>> {
        Rc::new(move |log, direction| {
            log.push((name, direction));
            true
        })
    }

    fn path() -> Rc<[Step]> {
        Rc::from([])
    }

    fn control(select: Option<Select<Vec<(&'static str, Option<Direction>)>>>) -> Measured<Frame> {
        leaf(
            Extent {
                width: 10.0,
                ascent: 8.0,
                descent: 2.0,
            },
            move |output: &mut Frame, _| output.landmark_select = select,
        )
    }

    #[test]
    fn only_the_nearest_landmark_consumes_a_controls_arrival_handler() {
        let child = landmark(control(Some(select("control"))), path(), select("child"));
        let child = landmark(child, path(), select("parent"));
        let child = measured::before_into(child, |_, output: &mut Frame| {
            output.landmark_select = Some(select("outside"));
        });
        let placement = Placement::root(Rect::new(15.0, 30.0, 25.0, 40.0));
        let output = place(child, placement);
        let mut log = vec![];
        for landmark in output.landmarks {
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
        let output = place(measured::row(0.0, vec![first, second]), placement);
        let mut log = vec![];
        for landmark in output.landmarks {
            (landmark.select)(&mut log, None);
        }
        assert_eq!(log, vec![("first", None), ("second", None)]);
        assert!(output.landmark_select.is_none());
    }

    #[test]
    fn unplaced_subtrees_do_not_contribute_landmarks() {
        let child = landmark(control(Some(select("control"))), path(), select("unused"));
        let child = measured::around(child, |_, _| Frame::empty());
        let output = place(child, Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!(output.landmarks.is_empty());
        assert!(output.landmark_select.is_none());
    }
}
