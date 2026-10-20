//! Layout collects selectable stops; only the current selection's destinations survive.
use super::{HoverPass, source::Secondary, view::Root};
use gid::Step;
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

/// A native control supplies its entry without introducing child routing.
pub fn stop<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
) -> crate::display::Layout<W, H> {
    super::before(
        child,
        Rc::new(|context| {
            let path: Rc<[Step]> = Rc::from(context.path());
            let selected = context.toolkit.selected();
            Box::new(move |output, _| output.navigation_stop(path, selected))
        }),
    )
}

/// The occurrence `child` draws in place of, as drawing nothing of its own:
/// it has no landmark, so selection and keyboard navigation pass straight to
/// what `child` contains.
pub fn transparent<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
) -> crate::display::Layout<W, H> {
    super::before(
        child,
        Rc::new(|context| {
            let path: Rc<[Step]> = Rc::from(context.path());
            Box::new(move |output, _| output.transparent(path))
        }),
    )
}

pub fn group<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
) -> crate::display::Layout<W, H> {
    super::around(
        child,
        Rc::new(move |context| {
            let path: Rc<[Step]> = Rc::from(context.path());
            let selected = context.toolkit.selected();
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
    pub root: Option<Root>,
    pub path: Rc<[Step]>,
    pub rect: Rect,
    pub select: Select<World>,
    pub(crate) scope: crate::editing::Scope,
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
            scope: self.scope.clone(),
            secondary: self.secondary.clone(),
            selected: self.selected,
            tab_stop: self.tab_stop,
        }
    }
}

/// The landmark drawn at `path` in the view `root`.
pub(crate) fn landmark_at<'a, World>(
    landmarks: &'a [Landmark<World>],
    root: Option<&Root>,
    path: &[Step],
) -> Option<&'a Landmark<World>> {
    landmarks
        .iter()
        .find(|landmark| landmark.root.as_ref() == root && landmark.path.as_ref() == path)
}

/// What the selected occurrence shares with the projections that carry the
/// secondary mark, in whichever view they are drawn.
pub fn selected_secondary<World>(landmarks: &[Landmark<World>]) -> Option<Secondary> {
    landmarks
        .iter()
        .filter(|landmark| landmark.selected)
        .find_map(|landmark| landmark.secondary.clone())
}

/// `child` registered as the landmark of the occurrence at `path`: what
/// keyboard navigation reaches and reveals, over its full bounds. It claims
/// no clicks: pointer selection belongs to the targets the view registers
/// (heads, delimiters, rows), so a click on structural whitespace (gutters,
/// gaps between rows, the dead space inside a box) falls through to the
/// background's deselect. Arriving selects through `select` unless the child
/// names its own arrival.
pub(crate) fn landmark<World: 'static, H: 'static>(
    child: Measured<HoverPass<World, H>>,
    path: Rc<[Step]>,
    select: Select<World>,
    scope: crate::editing::Scope,
    secondary: Option<Secondary>,
    selected: bool,
) -> Measured<HoverPass<World, H>> {
    measured::around_into(child, move |placement, inner, pass| {
        pass.scope(
            |pass| inner.place_into(pass),
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
                let transparent = output.transparent.take().is_some_and(|at| {
                    debug_assert!(at == path, "transparency at another occurrence");
                    at == path
                });
                if !transparent {
                    output.landmarks.push(Landmark {
                        root: None,
                        path,
                        rect: placement.rect,
                        select,
                        scope,
                        secondary,
                        selected,
                        tab_stop,
                    });
                }
                output
            },
        );
    })
}
