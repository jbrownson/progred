//! Layout collects selectable stops; only the current selection's destinations survive.
use super::{HoverPass, source::Secondary, view::Root};
use crate::occurrence::Trail;
use kurbo::Rect;
use measured::Measured;
use puri::handler::NavigationDirection;
use std::rc::Rc;

mod geometry;
mod logical;
pub(crate) use geometry::{Geometry, advance};
pub(crate) use logical::Collector;

// Pointer/source selection and Select All also use these direct selection helpers.
pub type Select<World> = Rc<dyn Fn(&mut World, Option<NavigationDirection>) -> bool>;

/// Select `site`, as clicking it does.
pub(crate) fn selecting(site: crate::editing::Site) -> crate::display::Action {
    Rc::new(move |session| {
        site.open(session).select();
        true
    })
}

/// Plain arrival at an occurrence: select it, whatever the direction.
pub(crate) fn arrival<W: crate::display::Host>(site: crate::editing::Site) -> Select<W> {
    let select = selecting(site);
    Rc::new(move |world: &mut W, _| select(&mut world.session()))
}

pub struct Landmark<World> {
    pub root: Option<Root>,
    pub path: Trail,
    pub rect: Rect,
    pub select: Select<World>,
    /// What it shares with other projections of the same thing.
    pub secondary: Option<Secondary>,
    /// Whether it carries the primary highlight.
    pub selected: bool,
    /// Whether Tab visits it: a place to fill in, as the widget drawn there
    /// marked it.
    pub tab_stop: bool,
}

impl<World> Clone for Landmark<World> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            path: self.path.clone(),
            rect: self.rect,
            select: self.select.clone(),
            secondary: self.secondary.clone(),
            selected: self.selected,
            tab_stop: self.tab_stop,
        }
    }
}

/// The landmark drawn at `path` in the view `root`.
pub(crate) fn landmark_at<'a, World, P: ?Sized>(
    landmarks: &'a [Landmark<World>],
    root: Option<&Root>,
    path: &P,
) -> Option<&'a Landmark<World>>
where
    Trail: PartialEq<P>,
{
    landmarks
        .iter()
        .find(|landmark| landmark.root.as_ref() == root && landmark.path == *path)
}

/// What the selected occurrence shares with the projections that carry the
/// secondary mark, in whichever view they are drawn.
pub fn selected_secondary<World>(landmarks: &[Landmark<World>]) -> Option<Secondary> {
    landmarks
        .iter()
        .filter(|landmark| landmark.selected)
        .find_map(|landmark| landmark.secondary.clone())
}

/// `child` drawn as part of the occurrence around it, as [`landmark`] but
/// with no stop of its own, so arrows pass from what's around it to what's
/// inside. A control drawn for it, such as the line of a definition yet to
/// be written, still declares its stop there, and then it's reached and
/// revealed as a landmark is.
pub(crate) fn part_landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Trail,
    select: Select<World>,
    selected: bool,
) -> Measured<HoverPass<World, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.scope(
            |pass| inner.place_into(pass),
            move |mut output| {
                let arrival = match output.landmark_select.take() {
                    Some((at, arrive)) if at == path => Some(arrive),
                    stray => {
                        debug_assert!(stray.is_none(), "arrival override at another occurrence");
                        None
                    }
                };
                let tab_stop = output.tab_stop.take().is_some_and(|at| at == path);
                if arrival.is_some() || tab_stop {
                    output.landmarks.push(Landmark {
                        root: None,
                        path,
                        rect: placement.rect,
                        select: arrival.unwrap_or(select),
                        secondary: None,
                        selected,
                        tab_stop,
                    });
                }
                output
            },
        );
    })
}

/// `child` registered as the landmark of the occurrence at `path`: what
/// keyboard navigation reaches and reveals, over its full bounds. It claims
/// no clicks: pointer selection belongs to the targets the view registers
/// (heads, delimiters, rows), so a click on structural whitespace (gutters,
/// gaps between rows, the dead space inside a box) falls through to the
/// background's deselect. Arriving selects through `select` unless the child
/// names its own arrival. Every occurrence drawn is an arrow stop: where a
/// control drawn for it declares its stop, there, and otherwise before what's
/// drawn inside it.
pub(crate) fn landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Trail,
    select: Select<World>,
    secondary: Option<Secondary>,
    selected: bool,
) -> Measured<HoverPass<World, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        let stop = path.clone();
        pass.scope(
            |pass| {
                pass.navigation.begin_landmark(stop, selected);
                inner.place_into(pass);
                pass.navigation.end_landmark();
            },
            move |mut output| {
                let select = match output.landmark_select.take() {
                    Some((at, arrive)) if at == path => arrive,
                    stray => {
                        debug_assert!(stray.is_none(), "arrival override at another occurrence");
                        select
                    }
                };
                let tab_stop = match output.tab_stop.take() {
                    Some(at) => {
                        debug_assert!(at == path, "tab stop at another occurrence");
                        at == path
                    }
                    None => false,
                };
                output.landmarks.push(Landmark {
                    root: None,
                    path,
                    rect: placement.rect,
                    select,
                    secondary,
                    selected,
                    tab_stop,
                });
                output
            },
        );
    })
}
