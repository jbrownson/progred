//! Layout collects selectable stops; only the current selection's destinations survive.
use super::{HoverPass, source::Secondary, view::Area};
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
    pub area: Option<Area>,
    pub path: Trail,
    pub rect: Rect,
    /// Its place in the view's traversal, before what's drawn inside it:
    /// the order Tab goes through.
    pub order: usize,
    pub select: Select<World>,
    /// What it shares with other projections of the same thing.
    pub secondary: Option<Secondary>,
    /// Whether it carries the primary highlight.
    pub selected: bool,
}

impl<World> Clone for Landmark<World> {
    fn clone(&self) -> Self {
        Self {
            area: self.area.clone(),
            path: self.path.clone(),
            rect: self.rect,
            order: self.order,
            select: self.select.clone(),
            secondary: self.secondary.clone(),
            selected: self.selected,
        }
    }
}

/// A place Tab visits: one to fill in, such as an empty hole or a missing
/// name, or one where something can be added, such as a list's insertion
/// point or the end of a record. Each is recorded by the widget drawn there,
/// apart from the landmarks: one where something's added stands for no
/// occurrence yet, so nothing finds it by a path.
pub struct TabStop<World> {
    pub area: Option<Area>,
    /// Its place in the view's traversal: the order Tab goes through.
    pub order: usize,
    pub rect: Rect,
    /// The occurrence it is, a place to fill in, which Tab goes on from
    /// when it's selected; none for a place where something's added.
    pub at: Option<Trail>,
    /// How arriving opens it.
    pub select: Select<World>,
}

impl<World> Clone for TabStop<World> {
    fn clone(&self) -> Self {
        Self {
            area: self.area.clone(),
            order: self.order,
            rect: self.rect,
            at: self.at.clone(),
            select: self.select.clone(),
        }
    }
}

/// `child` as a tab stop over its bounds, which arriving at through `select`
/// opens: the place to fill in `at` is, or with none, where something can be
/// added.
pub(crate) fn tab_stop<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    at: Option<Trail>,
    select: Select<World>,
) -> Measured<HoverPass<World, H>> {
    super::before_place(child, move |placement, output| {
        output.tab_stop(placement.rect, at, select);
    })
}

/// The landmark drawn at `path` in the area `area`.
pub(crate) fn landmark_at<'a, World, P: ?Sized>(
    landmarks: &'a [Landmark<World>],
    area: Option<&Area>,
    path: &P,
) -> Option<&'a Landmark<World>>
where
    Trail: PartialEq<P>,
{
    landmarks
        .iter()
        .find(|landmark| landmark.area.as_ref() == area && landmark.path == *path)
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
/// inside. A control drawn for it that names its own arrival, such as the
/// line of a definition yet to be written, is reached and revealed as a
/// landmark is.
pub(crate) fn part_landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Trail,
    selected: bool,
) -> Measured<HoverPass<World, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        let order = pass.navigation.place();
        pass.scope(
            |pass| inner.place_into(pass),
            move |mut output| {
                if let Some(arrival) = take_arrival(&mut output, &path) {
                    output.landmarks.push(Landmark {
                        area: None,
                        path,
                        rect: placement.rect,
                        order,
                        select: arrival,
                        secondary: None,
                        selected,
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
        let order = pass.navigation.place();
        pass.scope(
            |pass| {
                pass.navigation.begin_landmark(stop, selected);
                inner.place_into(pass);
                pass.navigation.end_landmark();
            },
            move |mut output| {
                let select = take_arrival(&mut output, &path).unwrap_or(select);
                output.landmarks.push(Landmark {
                    area: None,
                    path,
                    rect: placement.rect,
                    order,
                    select,
                    secondary,
                    selected,
                });
                output
            },
        );
    })
}

/// The arrival that what's drawn at `path` named for itself, taken from
/// `output`. One named at another occurrence is dropped.
fn take_arrival<World, H>(
    output: &mut super::frame::HoverOutput<World, H>,
    path: &Trail,
) -> Option<Select<World>> {
    match output.landmark_select.take() {
        Some((at, arrive)) if at == *path => Some(arrive),
        stray => {
            debug_assert!(stray.is_none(), "arrival override at another occurrence");
            None
        }
    }
}
