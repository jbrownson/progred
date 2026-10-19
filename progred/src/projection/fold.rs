//! The form a value takes while its fold is closed, in Raw and the other
//! projections alike: its brackets around an ellipsis, which opens it.

use super::Cx;
use super::Layout;
use crate::display::{Delim, dim, on_activate, on_hover, selectable_bracket};
use crate::frame::Hovered;
use crate::hover::Hover;
use gid::Step;
use std::rc::Rc;

/// The editor-owned folded form shared by raw and custom projections.
/// Active structural editors keep their containing value open.
pub(super) fn collapsed_layout(
    cx: &Cx,
    path: &[Step],
    value: &grap::RuntimeValue,
    default: bool,
) -> Option<Layout> {
    match value.as_cell() {
        Some(cell) => cx.sources.resolve(cell).and_then(|definition| {
            let followed: Vec<Step> = path
                .iter()
                .cloned()
                .chain([Step::Follow(definition.resolution)])
                .collect();
            (cx.pending_child_of(&followed).is_none() && cx.pending_edge_under(&followed).is_none())
                .then_some(Delim::Paren)
        }),
        None if value.list_len().is_some_and(|len| len != 0)
            && cx.pending_child_of(path).is_none() =>
        {
            Some(Delim::Bracket)
        }
        None if value.record_len().is_some_and(|len| len != 0)
            && cx.pending_child_of(path).is_none()
            && cx.pending_edge_under(path).is_none() =>
        {
            Some(Delim::Brace)
        }
        None => None,
    }
    .map(|delim| {
        crate::display::selectable_value(
            selectable_bracket(delim, toggle(dim("…"), path, cx, default)),
            &cx.site(Rc::from(path)),
            value,
        )
    })
}

/// Opens the fold, selecting the occurrence first as a click on its
/// brackets would, so Space closes it again.
fn toggle(child: Layout, path: &[Step], cx: &Cx, default: bool) -> Layout {
    let target: Rc<[Step]> = Rc::from(path);
    let site = cx.site(target.clone());
    let hover = Hovered::Tree(Hover::Toggle(target));
    let toggled = on_activate(
        crate::display::hover_highlight(child, hover.clone()),
        hover.clone(),
        Rc::new(move |session| {
            let mut edit = site.open(session);
            edit.select();
            edit.fold(default, None);
            true
        }),
    );
    on_hover(toggled, hover)
}
